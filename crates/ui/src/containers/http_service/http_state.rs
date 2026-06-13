//! HttpState: state + logic layer for an HTTP service view.
//!
//! Owns the request UI sub-panels, response inputs, draft/undo state, and all
//! request logic (send/save/url resolution). The view layer (`HttpService`,
//! `RequestPanel`, and the leaf panels) reads this state and drives it via
//! `http_state.update(...)` — no per-component event types are needed.

use super::param_utils::{build_url_with_query_params, merge_query_params, substitute_path_params};
use super::request_body::{BodyType, RequestBody};
use super::request_headers::RequestHeaders;
use super::request_path_params::RequestPathParams;
use super::request_query_params::RequestQueryParams;
use super::response_panel::ResponsePanel;
use super::url_utils;
use futures::StreamExt;
use gpui::*;
use gpui_component::input::{InputState, TabSize};
use gpui_tokio::Tokio;
use i18n::t;
use models::{Header, HttpConfig, HttpDraftConfig, HttpMethod, QueryParameter, SearchParameter};
use smallvec::{smallvec, SmallVec};
use workspace::ServiceDraft;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestTab {
    Path,
    #[default]
    Query,
    Body,
    Headers,
}

pub struct HttpState {
    pub service: models::Service,
    folder_id: Option<uuid::Uuid>,
    service_state: Entity<crate::state::ServiceState>,
    /// Editing draft (undo/redo) for this service.
    draft: ServiceDraft,

    // Request UI sub-panels
    pub url: Entity<InputState>,
    pub body_panel: Entity<RequestBody>,
    pub headers_panel: Entity<RequestHeaders>,
    pub query_params_panel: Entity<RequestQueryParams>,
    pub path_params_panel: Entity<RequestPathParams>,

    // Response views
    pub response: Entity<InputState>,
    pub response_headers: Entity<InputState>,
    pub request_info: Entity<InputState>,
    pub response_panel: Entity<ResponsePanel>,

    // UI flags
    pub active_tab: RequestTab,
    pub is_loading: bool,
    pub use_base_url: bool,
    /// Current HTTP method (mirrors the method selector's selection).
    method: HttpMethod,

    _subscriptions: SmallVec<[Subscription; 2]>,
}

impl HttpState {
    pub fn new(
        service: models::Service,
        service_state: Entity<crate::state::ServiceState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let folder_id = service.folder_id;
        let config = match &service.service_type {
            models::ServiceType::Http(config) => config.clone(),
            _ => HttpConfig::default(),
        };
        let use_base_url = config.use_base_url;

        let url = cx.new(|cx| InputState::new(window, cx).default_value(&config.path));
        let response = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .searchable(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .line_number(true)
        });
        let response_headers = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("text")
                .line_number(true)
        });
        let request_info = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("text")
                .line_number(true)
        });
        let response_panel = cx.new(|_cx| {
            ResponsePanel::new(
                response.clone(),
                response_headers.clone(),
                request_info.clone(),
            )
        });

        // Folder query params for inheritance.
        let folder_query_params = folder_id
            .map(|folder_id| {
                service_state
                    .read(cx)
                    .folders
                    .iter()
                    .find(|f| f.id == folder_id)
                    .map(|folder| {
                        folder
                            .common_search_params
                            .iter()
                            .map(|p| QueryParameter::new(p.key.clone(), p.value.clone()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let http_state = cx.entity();
        let body_panel = cx.new(|cx| RequestBody::new(http_state.clone(), window, cx));
        let headers_panel = cx.new(|cx| RequestHeaders::new(Vec::new(), window, cx));
        let query_params_panel = cx.new(|cx| {
            RequestQueryParams::new(config.query_params.clone(), folder_query_params, window, cx)
        });
        let path_params_panel =
            cx.new(|cx| RequestPathParams::new(config.path_params.clone(), window, cx));

        let active_tab = if config.path_params.is_empty() {
            RequestTab::Query
        } else {
            RequestTab::Path
        };

        // React to URL edits: refresh path params + update draft.
        let url_subscription = cx.subscribe_in(
            &url,
            window,
            |this, url_entity, _event: &gpui_component::input::InputEvent, window, cx| {
                let url_value = url_entity.read(cx).value().to_string();
                this.path_params_panel.update(cx, |panel, cx| {
                    panel.update_from_url(&url_value, window, cx);
                });
                this.update_draft_from_ui(cx);
            },
        );

        // Initialize the editing draft for this service.
        let draft_store = register::GlobalStorage::global(cx).draft.clone();
        let draft = ServiceDraft::new(draft_store, &service);

        Self {
            service,
            folder_id,
            service_state,
            draft,
            url,
            body_panel,
            headers_panel,
            query_params_panel,
            path_params_panel,
            response,
            response_headers,
            request_info,
            response_panel,
            active_tab,
            is_loading: false,
            use_base_url,
            method: config.method,
            _subscriptions: smallvec![url_subscription],
        }
    }

    // === Sub-panel accessors ===

    pub fn method(&self) -> HttpMethod {
        self.method
    }

    /// Updates the current method (called by the method selector) and refreshes the draft.
    pub fn set_method(&mut self, method: HttpMethod, cx: &mut Context<Self>) {
        self.method = method;
        self.update_draft_from_ui(cx);
    }

    pub fn headers(&self, cx: &App) -> Vec<Header> {
        self.headers_panel.read(cx).headers(cx)
    }

    pub fn body(&self, cx: &App) -> Option<String> {
        let body_panel = self.body_panel.read(cx);
        if body_panel.body_type() == BodyType::None {
            None
        } else {
            Some(body_panel.body(cx))
        }
    }

    pub fn query_params(&self, cx: &App) -> Vec<QueryParameter> {
        self.query_params_panel.read(cx).params(cx)
    }

    pub fn path_params(&self, cx: &App) -> Vec<models::PathParameter> {
        self.path_params_panel.read(cx).params(cx)
    }

    pub fn set_active_tab(&mut self, tab: RequestTab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        cx.notify();
    }

    pub fn add_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.headers_panel
            .update(cx, |panel, cx| panel.add_header(window, cx));
        self.update_draft_from_ui(cx);
    }

    pub fn add_query_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query_params_panel
            .update(cx, |panel, cx| panel.add_param(window, cx));
        self.update_draft_from_ui(cx);
    }

    /// Called by sub-panels (method/body) when their value changes.
    pub fn on_input_changed(&mut self, cx: &mut Context<Self>) {
        self.update_draft_from_ui(cx);
    }

    /// Handles a body-type change: sync the Content-Type header, then draft.
    pub fn on_body_type_changed(
        &mut self,
        body_type: BodyType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update_content_type_header(body_type, window, cx);
        self.update_draft_from_ui(cx);
    }

    // === Draft state ===

    fn collect_current_draft_config(&self, cx: &Context<Self>) -> HttpDraftConfig {
        let path = self.url.read(cx).value().to_string().into();
        let method = self.method();
        let headers = self.headers(cx);
        let query_params = self.query_params(cx);
        let path_params = self.path_params(cx);
        let body = self
            .body(cx)
            .map(|b| models::HttpBody::from_content(models::BodyType::Text, &b));

        HttpDraftConfig {
            method,
            path,
            headers,
            query_params,
            path_params,
            body,
            use_base_url: self.use_base_url,
        }
    }

    fn update_draft_from_ui(&mut self, cx: &mut Context<Self>) {
        let config = self.collect_current_draft_config(cx);
        if let Err(e) = self.draft.update(&config) {
            log::error!("Failed to update draft for {}: {}", self.service.id, e);
        }
        cx.notify();
    }

    pub fn is_modified(&self, _cx: &App) -> bool {
        self.draft.is_dirty()
    }

    /// Saves the current draft to the service.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        let config = match self.draft.save_to_service() {
            Ok(config) => config,
            Err(e) => {
                log::error!("Failed to save draft for {}: {}", self.service.id, e);
                return;
            }
        };
        let service_id = self.service.id;
        self.service_state.update(cx, |s, cx| {
            s.update_service_config(service_id, config.to_http_config(), cx);
        });
        cx.notify();
    }

    pub fn toggle_use_base_url(&mut self, cx: &mut Context<Self>) {
        self.use_base_url = !self.use_base_url;
        self.update_draft_from_ui(cx);
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(draft_config) = self.draft.undo() {
            self.apply_draft_to_ui(&draft_config, cx);
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(draft_config) = self.draft.redo() {
            self.apply_draft_to_ui(&draft_config, cx);
        }
    }

    pub fn can_undo(&self, _cx: &App) -> bool {
        self.draft.can_undo()
    }

    pub fn can_redo(&self, _cx: &App) -> bool {
        self.draft.can_redo()
    }

    fn apply_draft_to_ui(&mut self, draft: &HttpDraftConfig, cx: &mut Context<Self>) {
        // Only base-url toggle is reflected eagerly; other fields render from
        // the stored draft on the next render.
        self.use_base_url = draft.use_base_url;
        cx.notify();
    }

    fn update_content_type_header(
        &mut self,
        body_type: BodyType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut headers = self.headers(cx);
        headers.retain(|h| h.key.to_lowercase() != "content-type");
        if let Some(content_type) = body_type.content_type() {
            headers.push(Header::new("Content-Type", content_type));
        }
        self.headers_panel = cx.new(|cx| RequestHeaders::new(headers, window, cx));
        cx.notify();
    }

    fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.is_loading = loading;
        cx.notify();
    }

    // === URL / header / param resolution ===

    /// The active environment's base URL for this service's workspace.
    pub fn base_url(&self, cx: &App) -> SharedString {
        self.service_state
            .read(cx)
            .env_state
            .read(cx)
            .active_base_url()
    }

    pub fn final_url(&self, cx: &Context<Self>) -> String {
        let path = self.url.read(cx).value().to_string();
        let base_url = self.base_url(cx);
        let mut resolved_url = url_utils::resolve_url(&base_url, &path, self.use_base_url);
        resolved_url = substitute_path_params(&resolved_url, &self.path_params(cx));
        let query_params = self.resolve_final_query_params(cx);
        resolved_url = build_url_with_query_params(&resolved_url, &query_params);
        self.resolve_final_url(resolved_url, cx)
    }

    fn get_folder_common_headers(&self, cx: &Context<Self>) -> Vec<Header> {
        self.with_folder(cx, |folder, this| {
            folder
                .common_headers
                .iter()
                .filter(|h| this.is_valid_header(h))
                .cloned()
                .collect()
        })
    }

    fn get_folder_common_search_params(&self, cx: &Context<Self>) -> Vec<SearchParameter> {
        self.with_folder(cx, |folder, this| {
            folder
                .common_search_params
                .iter()
                .filter(|p| this.is_valid_search_param(p))
                .cloned()
                .collect()
        })
    }

    fn get_folder_common_query_params(&self, cx: &Context<Self>) -> Vec<QueryParameter> {
        self.with_folder(cx, |folder, this| {
            folder
                .common_search_params
                .iter()
                .filter(|p| this.is_valid_search_param(p))
                .map(|p| QueryParameter::new(p.key.clone(), p.value.clone()))
                .collect()
        })
    }

    /// Runs `f` with this service's folder, returning the mapped value or empty.
    fn with_folder<T: Default>(
        &self,
        cx: &Context<Self>,
        f: impl FnOnce(&models::Folder, &Self) -> T,
    ) -> T {
        let Some(folder_id) = self.folder_id else {
            return T::default();
        };
        match self
            .service_state
            .read(cx)
            .folders
            .iter()
            .find(|f| f.id == folder_id)
        {
            Some(folder) => f(folder, self),
            None => T::default(),
        }
    }

    fn is_valid_header(&self, header: &Header) -> bool {
        let key = header.key.trim();
        let value = header.value.trim();
        if key.is_empty() || value.is_empty() {
            return false;
        }
        if key.len() > 100 || value.len() > 8192 {
            return false;
        }
        if !key
            .chars()
            .all(|c| c.is_ascii() && !c.is_control() && c != ':' && c != ' ')
        {
            return false;
        }
        value
            .chars()
            .all(|c| c.is_ascii() && (!c.is_control() || c == '\t'))
    }

    fn is_valid_search_param(&self, param: &SearchParameter) -> bool {
        let key = param.key.trim();
        let value = param.value.trim();
        if key.is_empty() || value.is_empty() {
            return false;
        }
        if key.len() > 100 || value.len() > 2048 {
            return false;
        }
        if !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return false;
        }
        !(value.contains('\n') || value.contains('\r'))
    }

    fn resolve_final_headers(&self, cx: &Context<Self>) -> Vec<Header> {
        let mut final_headers = self.get_folder_common_headers(cx);
        final_headers.extend(self.headers(cx));
        self.deduplicate_headers(final_headers)
    }

    fn resolve_final_query_params(&self, cx: &Context<Self>) -> Vec<QueryParameter> {
        let folder_params = self.get_folder_common_query_params(cx);
        let service_params = self.query_params(cx);
        merge_query_params(&folder_params, &service_params)
    }

    fn resolve_final_url(&self, base_url: String, cx: &Context<Self>) -> String {
        let folder_params = self.get_folder_common_search_params(cx);
        if folder_params.is_empty() {
            return base_url;
        }
        url_utils::build_url_with_params(&base_url, &folder_params)
    }

    fn deduplicate_headers(&self, headers: Vec<Header>) -> Vec<Header> {
        let mut final_headers = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        for header in headers.into_iter().rev() {
            let key_lower = header.key.to_lowercase();
            if seen_keys.insert(key_lower) {
                final_headers.push(header);
            }
        }
        final_headers.reverse();
        final_headers
    }

    fn validate_path_params(&self, cx: &Context<Self>) -> Result<(), String> {
        let url = self.url.read(cx).value().to_string();
        let url_after_substitution = substitute_path_params(&url, &self.path_params(cx));
        if url_after_substitution.contains('{') && url_after_substitution.contains('}') {
            let missing_params = super::param_utils::detect_path_params(&url_after_substitution);
            if !missing_params.is_empty() {
                return Err(format!(
                    "Missing required path parameters: {}",
                    missing_params.join(", ")
                ));
            }
        }
        Ok(())
    }

    // === Send ===

    /// Builds the human-readable "request info" text shown while sending.
    fn build_request_info(
        &self,
        method: HttpMethod,
        url: &str,
        headers: &[Header],
        body: Option<&String>,
        cx: &Context<Self>,
    ) -> String {
        let user_entered_url = self.url.read(cx).value().to_string();
        let base_url = self.base_url(cx);
        let mut text = format!("{:?} {}\n", method, url);
        if self.use_base_url
            && !base_url.is_empty()
            && !url_utils::is_absolute_url(&user_entered_url)
        {
            text.push_str(&format!("({}: {})\n", t!("env.base_url"), base_url));
        }
        text.push('\n');
        for h in headers {
            text.push_str(&format!("{}: {}\n", h.key, h.value));
        }
        if let Some(b) = body {
            text.push_str(&format!("\n{}", b));
        }
        text
    }

    pub fn send_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error_msg) = self.validate_path_params(cx) {
            self.response.update(cx, |state, cx| {
                state.set_value(
                    format!("Validation Error: {}\n\nPlease provide values for all path parameters before sending the request.", error_msg),
                    window,
                    cx,
                )
            });
            cx.notify();
            return;
        }

        self.set_loading(true, cx);
        self.response
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.response_headers
            .update(cx, |state, cx| state.set_value("", window, cx));

        let url = self.final_url(cx);
        let method = self.method();
        let headers = self.resolve_final_headers(cx);
        let body = self.body(cx);

        let request_info_text = self.build_request_info(method, &url, &headers, body.as_ref(), cx);
        self.request_info.update(cx, |state, cx| {
            state.set_value(request_info_text, window, cx)
        });
        cx.notify();

        let task = Self::spawn_request(cx, method, url, headers, body);

        let response = self.response.clone();
        let response_headers = self.response_headers.clone();
        cx.spawn_in(
            window,
            async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let result = task.await.unwrap();
                this.update_in(cx, |this, _, cx| this.set_loading(false, cx))
                    .ok();
                Self::handle_response(result, response, response_headers, cx).await;
            },
        )
        .detach();
    }

    /// Spawns the actual HTTP request on the Tokio runtime.
    fn spawn_request(
        cx: &mut Context<Self>,
        method: HttpMethod,
        url: String,
        headers: Vec<Header>,
        body: Option<String>,
    ) -> Task<Result<reqwest::Result<reqwest::Response>, gpui_tokio::JoinError>> {
        Tokio::spawn(cx, async move {
            let client = reqwest::Client::new();
            let mut req = match method {
                HttpMethod::Get => client.get(&url),
                HttpMethod::Post => client.post(&url),
                HttpMethod::Put => client.put(&url),
                HttpMethod::Delete => client.delete(&url),
                HttpMethod::Patch => client.patch(&url),
                HttpMethod::Head => client.head(&url),
                HttpMethod::Options => client.request(reqwest::Method::OPTIONS, &url),
            };
            for h in &headers {
                req = req.header(h.key.as_ref(), h.value.as_ref());
            }
            if let Some(b) = body {
                req = req.body(b);
            }
            req.send().await
        })
    }

    /// Writes the response (headers + body, streaming SSE if applicable) to the
    /// response/response_headers inputs.
    async fn handle_response(
        result: reqwest::Result<reqwest::Response>,
        response: Entity<InputState>,
        response_headers: Entity<InputState>,
        cx: &mut AsyncWindowContext,
    ) {
        let resp = match result {
            Ok(resp) => resp,
            Err(e) => {
                response
                    .update_in(cx, |state, window, cx| {
                        state.set_value(format!("Error: {}", e), window, cx);
                    })
                    .ok();
                return;
            }
        };

        let headers_text = resp
            .headers()
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n");
        response_headers
            .update_in(cx, |state, window, cx| {
                state.set_value(headers_text, window, cx);
            })
            .ok();

        let is_sse = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.contains("text/event-stream"))
            .unwrap_or(false);

        if is_sse {
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let (text, is_err) = match chunk {
                    Ok(bytes) => (String::from_utf8_lossy(&bytes).to_string(), false),
                    Err(e) => (format!("\n[Error: {}]", e), true),
                };
                response
                    .update_in(cx, |state, window, cx| state.insert(text, window, cx))
                    .ok();
                if is_err {
                    break;
                }
            }
        } else {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|e| format!("Error: {}", e));
            response
                .update_in(cx, |state, window, cx| {
                    state.set_value(text, window, cx);
                })
                .ok();
        }
    }
}
