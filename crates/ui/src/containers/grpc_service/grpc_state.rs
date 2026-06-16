//! GrpcState: state + logic layer for a gRPC service view.
//!
//! Holds the editable gRPC config (endpoint, proto, selected method, request
//! body, metadata), compiles the proto to discover methods, and performs unary
//! calls via [`grpc_client`]. Per design choice A, gRPC does not use the CRDT
//! draft/undo machinery — saving writes the `GrpcConfig` directly.

use super::grpc_client::{self, call_stream, call_unary, reflect, MethodInfo, ProtoDescriptors};
use super::grpc_response_panel::GrpcResponsePanel;
use gpui::*;
use gpui_tokio::Tokio;
use models::{GrpcConfig, Header, ServiceType};
use std::path::PathBuf;
use std::sync::Arc;

/// Where the service definition comes from.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DefinitionSource {
    /// Compiled from the proto editor / imported `.proto` files.
    #[default]
    Proto,
    /// Discovered from the server via gRPC reflection.
    Reflection,
}

pub struct GrpcState {
    pub service: models::Service,
    service_state: Entity<crate::state::ServiceState>,

    // Editable config snapshot.
    endpoint: String,
    proto_content: String,
    selected_service: String,
    selected_method: String,
    request_body: String,
    metadata: Vec<Header>,

    /// Methods discovered from the last successful proto compile.
    methods: Vec<MethodInfo>,
    /// Compiled descriptors, shared into the async call task.
    descriptors: Option<Arc<ProtoDescriptors>>,
    /// Last proto compile / reflection error, shown in the definition panel.
    proto_error: Option<String>,

    /// Where the current definition comes from (proto editor vs reflection).
    source: DefinitionSource,
    /// Extra import roots for resolving `import` statements (proto libraries).
    include_dirs: Vec<PathBuf>,
    /// Whether a reflection fetch is currently in flight.
    reflecting: bool,

    /// Saved config to compare against for the modified flag.
    saved_config: GrpcConfig,

    /// True only during a blocking unary call (shows the loading overlay).
    pub is_loading: bool,
    /// True while a streaming call (server/client/bidi) is open. Non-blocking:
    /// the user sees appended messages and can finish/cancel.
    stream_active: bool,
    /// The in-flight request task; dropping it cancels the request.
    pending_request: Option<Task<()>>,
    /// For client/bidi streaming: the open request stream's sender. Present only
    /// while a client-streaming or bidi call is accepting more messages.
    request_tx: Option<futures::channel::mpsc::Sender<Vec<u8>>>,

    /// Response view to write results into (owned by `GrpcResponsePanel`).
    response_panel: Option<WeakEntity<GrpcResponsePanel>>,
}

impl GrpcState {
    pub fn new(
        service: models::Service,
        service_state: Entity<crate::state::ServiceState>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        let config = match &service.service_type {
            ServiceType::Grpc(c) => c.clone(),
            _ => GrpcConfig::default(),
        };

        let mut state = Self {
            endpoint: config.service_url.clone(),
            proto_content: config.proto_content.clone().unwrap_or_default(),
            selected_service: config.service.clone(),
            selected_method: config.method.clone(),
            request_body: String::new(),
            metadata: config.metadata.clone(),
            methods: Vec::new(),
            descriptors: None,
            proto_error: None,
            source: DefinitionSource::Proto,
            include_dirs: Vec::new(),
            reflecting: false,
            saved_config: config,
            is_loading: false,
            stream_active: false,
            pending_request: None,
            request_tx: None,
            response_panel: None,
            service,
            service_state,
        };
        // Compile any persisted proto so the method list is available immediately.
        if !state.proto_content.is_empty() {
            state.compile_proto();
        }
        state
    }

    // === Accessors ===

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn proto_content(&self) -> &str {
        &self.proto_content
    }
    pub fn request_body(&self) -> &str {
        &self.request_body
    }
    pub fn metadata(&self) -> &[Header] {
        &self.metadata
    }
    pub fn methods(&self) -> &[MethodInfo] {
        &self.methods
    }
    pub fn proto_error(&self) -> Option<&str> {
        self.proto_error.as_deref()
    }
    pub fn selected_service(&self) -> &str {
        &self.selected_service
    }
    pub fn selected_method(&self) -> &str {
        &self.selected_method
    }

    /// Index of the currently selected method within `methods`, if any.
    pub fn selected_method_index(&self) -> Option<usize> {
        self.methods
            .iter()
            .position(|m| m.service == self.selected_service && m.method == self.selected_method)
    }

    // === Mutators ===

    pub fn set_endpoint(&mut self, endpoint: String, cx: &mut Context<Self>) {
        self.endpoint = endpoint;
        cx.notify();
    }

    pub fn set_request_body(&mut self, body: String, cx: &mut Context<Self>) {
        self.request_body = body;
        cx.notify();
    }

    pub fn set_metadata(&mut self, metadata: Vec<Header>, cx: &mut Context<Self>) {
        self.metadata = metadata;
        cx.notify();
    }

    pub fn set_proto_content(&mut self, proto: String, cx: &mut Context<Self>) {
        self.proto_content = proto;
        self.source = DefinitionSource::Proto;
        self.compile_proto();
        cx.notify();
    }

    pub fn source(&self) -> DefinitionSource {
        self.source
    }

    pub fn set_source(&mut self, source: DefinitionSource, cx: &mut Context<Self>) {
        self.source = source;
        cx.notify();
    }

    pub fn is_reflecting(&self) -> bool {
        self.reflecting
    }

    pub fn include_dirs(&self) -> &[PathBuf] {
        &self.include_dirs
    }

    /// Imports a `.proto` file from disk, replacing the editor content.
    pub fn import_proto_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.proto_content = content;
                self.source = DefinitionSource::Proto;
                self.compile_proto();
            }
            Err(e) => {
                self.proto_error = Some(format!("Failed to read {}: {e}", path.display()));
            }
        }
        cx.notify();
    }

    /// Adds an import root (proto library directory) and recompiles.
    pub fn add_include_dir(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        if !self.include_dirs.contains(&dir) {
            self.include_dirs.push(dir);
            self.compile_proto();
        }
        cx.notify();
    }

    pub fn remove_include_dir(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.include_dirs.len() {
            self.include_dirs.remove(index);
            self.compile_proto();
        }
        cx.notify();
    }

    /// Fetches the service definition from the server via gRPC reflection,
    /// populating `methods`/`descriptors` on success.
    pub fn refresh_from_reflection(&mut self, cx: &mut Context<Self>) {
        if self.endpoint.trim().is_empty() {
            self.proto_error = Some("Endpoint is empty.".to_string());
            cx.notify();
            return;
        }
        self.source = DefinitionSource::Reflection;
        self.reflecting = true;
        self.proto_error = None;
        cx.notify();

        let endpoint = self.endpoint.clone();
        let task = Tokio::spawn(cx, async move { reflect(&endpoint).await });

        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.reflecting = false;
                match result {
                    Ok(Ok((descriptors, methods))) => {
                        this.methods = methods;
                        this.descriptors = Some(Arc::new(descriptors));
                        this.proto_error = None;
                        if this.selected_method_index().is_none() {
                            if let Some(first) = this.methods.first() {
                                this.selected_service = first.service.clone();
                                this.selected_method = first.method.clone();
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        this.methods.clear();
                        this.descriptors = None;
                        this.proto_error = Some(format!("Reflection failed: {e}"));
                    }
                    Err(e) => {
                        this.proto_error = Some(format!("Reflection task failed: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn select_method(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(m) = self.methods.get(index) {
            self.selected_service = m.service.clone();
            self.selected_method = m.method.clone();
            cx.notify();
        }
    }

    pub fn set_response_panel(&mut self, panel: WeakEntity<GrpcResponsePanel>) {
        self.response_panel = Some(panel);
    }

    fn response_panel(&self) -> Option<Entity<GrpcResponsePanel>> {
        self.response_panel.as_ref().and_then(|p| p.upgrade())
    }

    // === Proto compilation ===

    /// Recompiles `proto_content`, refreshing `methods`/`descriptors`/`proto_error`.
    fn compile_proto(&mut self) {
        if self.proto_content.trim().is_empty() {
            self.methods.clear();
            self.descriptors = None;
            self.proto_error = None;
            return;
        }
        match ProtoDescriptors::compile(&self.proto_content, &self.include_dirs) {
            Ok(desc) => {
                self.methods = desc.methods();
                self.descriptors = Some(Arc::new(desc));
                self.proto_error = None;
                // Keep selection valid: default to the first method if none set.
                if self.selected_method_index().is_none() {
                    if let Some(first) = self.methods.first() {
                        self.selected_service = first.service.clone();
                        self.selected_method = first.method.clone();
                    }
                }
            }
            Err(e) => {
                self.methods.clear();
                self.descriptors = None;
                self.proto_error = Some(e.to_string());
            }
        }
    }

    // === Save (design A: write GrpcConfig directly, no draft) ===

    fn current_config(&self) -> GrpcConfig {
        let streaming = self
            .methods
            .iter()
            .find(|m| m.service == self.selected_service && m.method == self.selected_method)
            .map(|m| match (m.client_streaming, m.server_streaming) {
                (false, false) => models::StreamingType::Unary,
                (true, false) => models::StreamingType::ClientStream,
                (false, true) => models::StreamingType::ServerStream,
                (true, true) => models::StreamingType::Bidirectional,
            })
            .unwrap_or_default();

        GrpcConfig {
            service_url: self.endpoint.clone(),
            package: String::new(),
            service: self.selected_service.clone(),
            method: self.selected_method.clone(),
            request_type: String::new(),
            response_type: String::new(),
            streaming,
            metadata: self.metadata.clone(),
            proto_content: if self.proto_content.is_empty() {
                None
            } else {
                Some(self.proto_content.clone())
            },
        }
    }

    pub fn is_modified(&self, _cx: &App) -> bool {
        let c = self.current_config();
        let s = &self.saved_config;
        c.service_url != s.service_url
            || c.service != s.service
            || c.method != s.method
            || c.proto_content != s.proto_content
            || c.metadata != s.metadata
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        let config = self.current_config();
        let service_id = self.service.id;
        let service_type = ServiceType::Grpc(config.clone());
        self.service_state.update(cx, |s, cx| {
            if s.is_unsaved(service_id) {
                s.commit_unsaved_service_typed(service_id, service_type, cx);
            } else {
                s.update_service_config_typed(service_id, service_type, cx);
            }
        });
        self.saved_config = config;
        cx.notify();
    }

    // === Send ===

    /// Streaming flags of the currently selected method, if known.
    fn selected_streaming(&self) -> (bool, bool) {
        self.methods
            .iter()
            .find(|m| m.service == self.selected_service && m.method == self.selected_method)
            .map(|m| (m.client_streaming, m.server_streaming))
            .unwrap_or((false, false))
    }

    /// Whether a streaming call (server/client/bidi) is currently open.
    pub fn is_streaming(&self) -> bool {
        self.stream_active
    }

    /// Whether the open stream still accepts more request messages (client/bidi).
    pub fn can_send_more(&self) -> bool {
        self.request_tx.is_some()
    }

    /// Validates prerequisites and returns (descriptors, endpoint, service,
    /// method, metadata) for a send, or writes an error to the panel and returns
    /// `None`.
    fn prepare_send(
        &self,
        panel: &Entity<GrpcResponsePanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(
        Arc<ProtoDescriptors>,
        String,
        String,
        String,
        Vec<(String, String)>,
    )> {
        let Some(descriptors) = self.descriptors.clone() else {
            panel.update(cx, |p, cx| {
                p.set_error("No compiled proto. Paste a valid .proto first.", window, cx)
            });
            return None;
        };
        if self.selected_method.is_empty() {
            panel.update(cx, |p, cx| p.set_error("No method selected.", window, cx));
            return None;
        }
        if self.endpoint.trim().is_empty() {
            panel.update(cx, |p, cx| p.set_error("Endpoint is empty.", window, cx));
            return None;
        }
        let metadata = self
            .metadata
            .iter()
            .filter(|h| !h.key.trim().is_empty())
            .map(|h| (h.key.to_string(), h.value.to_string()))
            .collect();
        Some((
            descriptors,
            self.endpoint.clone(),
            self.selected_service.clone(),
            self.selected_method.clone(),
            metadata,
        ))
    }

    fn current_request_json(&self) -> String {
        if self.request_body.trim().is_empty() {
            "{}".to_string()
        } else {
            self.request_body.clone()
        }
    }

    pub fn send_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = self.response_panel() else {
            return;
        };
        let (client_streaming, server_streaming) = self.selected_streaming();
        if client_streaming || server_streaming {
            self.start_stream(panel, client_streaming, window, cx);
        } else {
            self.send_unary(panel, window, cx);
        }
    }

    fn send_unary(
        &mut self,
        panel: Entity<GrpcResponsePanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((descriptors, endpoint, service, method, metadata)) =
            self.prepare_send(&panel, window, cx)
        else {
            return;
        };
        let request_json = self.current_request_json();

        self.is_loading = true;
        panel.update(cx, |p, cx| p.clear(window, cx));
        cx.notify();

        let task = Tokio::spawn(cx, async move {
            call_unary(
                &descriptors,
                &endpoint,
                &service,
                &method,
                &metadata,
                &request_json,
            )
            .await
        });

        let panel = panel.downgrade();
        let pending = cx.spawn_in(window, async move |this: WeakEntity<Self>, cx| {
            let result = task.await;
            this.update_in(cx, |this, _, cx| {
                this.is_loading = false;
                this.pending_request = None;
                cx.notify();
            })
            .ok();
            match result {
                Ok(Ok(json)) => {
                    panel
                        .update_in(cx, |p, window, cx| p.set_response(&json, window, cx))
                        .ok();
                }
                Ok(Err(e)) => {
                    panel
                        .update_in(cx, |p, window, cx| {
                            p.set_error(format!("Error: {e}"), window, cx)
                        })
                        .ok();
                }
                Err(join_err) => {
                    panel
                        .update_in(cx, |p, window, cx| {
                            p.set_error(format!("Request task failed: {join_err}"), window, cx)
                        })
                        .ok();
                }
            }
        });
        self.pending_request = Some(pending);
    }

    /// Starts a streaming call (server / client / bidi). The first request
    /// message is the current request body. For client/bidi the request stream
    /// stays open (`request_tx` retained) so the user can send more messages and
    /// finish; for server-only streaming it is closed immediately.
    fn start_stream(
        &mut self,
        panel: Entity<GrpcResponsePanel>,
        client_streaming: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((descriptors, endpoint, service, method, metadata)) =
            self.prepare_send(&panel, window, cx)
        else {
            return;
        };

        // Encode the first request message up-front so JSON errors surface now.
        let first =
            match descriptors.encode_message(&service, &method, &self.current_request_json()) {
                Ok(bytes) => bytes,
                Err(e) => {
                    panel.update(cx, |p, cx| p.set_error(format!("Error: {e}"), window, cx));
                    return;
                }
            };

        // Streaming is non-blocking: mark the stream active (no loading overlay)
        // so the user sees messages append and can finish/cancel.
        self.stream_active = true;
        panel.update(cx, |p, cx| p.clear(window, cx));

        let (mut req_tx, req_rx) = futures::channel::mpsc::channel::<Vec<u8>>(16);
        let (resp_tx, mut resp_rx) = futures::channel::mpsc::channel::<grpc_client::StreamItem>(16);

        // Seed the first message; capacity reserved so this can't block.
        let _ = req_tx.try_send(first);
        if client_streaming {
            // Keep the sender open for further messages + finish.
            self.request_tx = Some(req_tx);
        } else {
            // Server-only streaming: half-close immediately.
            drop(req_tx);
        }

        let call = Tokio::spawn(cx, async move {
            call_stream(
                descriptors,
                endpoint,
                service,
                method,
                metadata,
                req_rx,
                resp_tx,
            )
            .await
        });

        let panel = panel.downgrade();
        let pending = cx.spawn_in(window, async move |this: WeakEntity<Self>, cx| {
            use futures::StreamExt;
            // Append each response message as it arrives.
            while let Some(item) = resp_rx.next().await {
                let panel = panel.clone();
                match item {
                    Ok(json) => {
                        panel
                            .update_in(cx, |p, window, cx| p.append(&json, window, cx))
                            .ok();
                    }
                    Err(e) => {
                        panel
                            .update_in(cx, |p, window, cx| {
                                p.append(&format!("[error] {e}"), window, cx)
                            })
                            .ok();
                    }
                }
            }
            // Stream finished (server closed or call ended). Surface setup errors.
            if let Ok(Err(e)) = call.await {
                panel
                    .update_in(cx, |p, window, cx| {
                        p.append(&format!("[error] {e}"), window, cx)
                    })
                    .ok();
            }
            this.update_in(cx, |this, _, cx| {
                this.stream_active = false;
                this.pending_request = None;
                this.request_tx = None;
                cx.notify();
            })
            .ok();
        });
        self.pending_request = Some(pending);
        cx.notify();
    }

    /// Sends an additional request message on an open client/bidi stream.
    pub fn send_stream_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(descriptors) = self.descriptors.clone() else {
            return;
        };
        let service = self.selected_service.clone();
        let method = self.selected_method.clone();
        let json = self.current_request_json();
        let bytes = match descriptors.encode_message(&service, &method, &json) {
            Ok(b) => b,
            Err(e) => {
                if let Some(panel) = self.response_panel() {
                    panel.update(cx, |p, cx| p.append(&format!("[error] {e}"), window, cx));
                }
                return;
            }
        };
        if let Some(tx) = self.request_tx.as_mut() {
            // Drop the message if the buffer is full rather than blocking the UI.
            let _ = tx.try_send(bytes);
        }
        cx.notify();
    }

    /// Half-closes an open client/bidi stream: no more request messages, but the
    /// server may keep responding until it closes its side.
    pub fn finish_stream(&mut self, cx: &mut Context<Self>) {
        // Dropping the sender signals end-of-request-stream to the server.
        self.request_tx = None;
        cx.notify();
    }

    /// Cancels an in-flight request/stream, if any, and resets all run state.
    pub fn cancel_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_tx = None;
        // Dropping the task aborts the underlying call (unary or streaming).
        let had_task = self.pending_request.take().is_some();
        if !self.is_loading && !self.stream_active && !had_task {
            return;
        }
        self.is_loading = false;
        self.stream_active = false;
        if let Some(panel) = self.response_panel() {
            panel.update(cx, |p, cx| p.append("[cancelled]", window, cx));
        }
        cx.notify();
    }
}
