use super::request_body::BodyType;
use super::request_panel::{RequestPanel, RequestPanelEvent};
use super::response_panel::ResponsePanel;
use super::url_utils;
use futures::StreamExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{InputState, TabSize};
use gpui_component::StyledExt;
use gpui_tokio::Tokio;
use models::{HttpConfig, HttpMethod, Header, SearchParameter};
use smallvec::{smallvec, SmallVec};
use uuid::Uuid;
use workspace;

pub struct HttpService {
    url: Entity<InputState>,
    response: Entity<InputState>,
    response_headers: Entity<InputState>,
    request_info: Entity<InputState>,
    request_panel: Entity<RequestPanel>,
    response_panel: Entity<ResponsePanel>,
    is_loading: bool,
    base_url: SharedString,
    use_base_url: bool,
    folder_id: Option<Uuid>,
    _subscriptions: SmallVec<[Subscription; 2]>,
}

impl HttpService {
    pub fn new(config: HttpConfig, base_url: Option<SharedString>, folder_id: Option<Uuid>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = config.path.clone();
        let use_base_url = config.use_base_url;
        let url = cx.new(|cx| InputState::new(window, cx).default_value(&url));
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
            ResponsePanel::new(response.clone(), response_headers.clone(), request_info.clone())
        });
        let request_panel =
            cx.new(|cx| RequestPanel::new(config.method.clone(), url.clone(), use_base_url, base_url.clone().unwrap_or_default(), window, cx));

        let subscription = cx.subscribe_in(&request_panel, window, |this, _, event, window, cx| {
            match event {
                RequestPanelEvent::Send => {
                    this.send_request(window, cx);
                }
                RequestPanelEvent::AddHeader => {
                    this.request_panel
                        .update(cx, |panel, cx| panel.add_header(window, cx));
                }
                RequestPanelEvent::BodyTypeChanged(body_type) => {
                    this.update_content_type_header(*body_type, window, cx);
                }
                RequestPanelEvent::MethodChanged(_) => {
                    // Method is managed by MethodSelector, no action needed here
                }
                RequestPanelEvent::ToggleBaseUrl => {
                    this.toggle_use_base_url(cx);
                }
            }
        });

        Self {
            url,
            response,
            response_headers,
            request_info,
            request_panel,
            response_panel,
            is_loading: false,
            base_url: base_url.unwrap_or_default(),
            use_base_url,
            folder_id,
            _subscriptions: smallvec![subscription],
        }
    }

    /// Updates the base_url when the environment changes
    pub fn set_base_url(&mut self, base_url: SharedString, cx: &mut Context<Self>) {
        self.base_url = base_url.clone();
        self.request_panel.update(cx, |panel, _cx| {
            panel.set_base_url(base_url);
        });
        cx.notify();
    }

    /// Toggles the use_base_url mode
    pub fn toggle_use_base_url(&mut self, cx: &mut Context<Self>) {
        self.use_base_url = !self.use_base_url;
        let use_base_url = self.use_base_url;
        self.request_panel.update(cx, |panel, _cx| {
            panel.set_use_base_url(use_base_url);
        });
        cx.notify();
    }

    /// Gets the final URL by resolving base_url and user-entered path
    pub fn final_url(&self, cx: &Context<Self>) -> String {
        let path = self.url.read(cx).value().to_string();
        let resolved_url = url_utils::resolve_url(&self.base_url, &path, self.use_base_url);
        self.resolve_final_url(resolved_url, cx)
    }

    /// Gets folder common headers from the workspace registry
    fn get_folder_common_headers(&self, cx: &Context<Self>) -> Vec<Header> {
        if let Some(folder_id) = self.folder_id {
            let registry = workspace::WorkspaceRegistry::global(cx);
            if let Some(workspace) = registry.read(cx).get_active_workspace() {
                if let Some(folder) = workspace.folders.iter().find(|f| f.id == folder_id) {
                    // Validate headers before using them
                    return folder.common_headers.iter()
                        .filter(|header| self.is_valid_header(header))
                        .cloned()
                        .collect();
                }
            }
        }
        Vec::new()
    }

    /// Gets folder common search parameters from the workspace registry
    fn get_folder_common_search_params(&self, cx: &Context<Self>) -> Vec<SearchParameter> {
        if let Some(folder_id) = self.folder_id {
            let registry = workspace::WorkspaceRegistry::global(cx);
            if let Some(workspace) = registry.read(cx).get_active_workspace() {
                if let Some(folder) = workspace.folders.iter().find(|f| f.id == folder_id) {
                    // Validate parameters before using them
                    return folder.common_search_params.iter()
                        .filter(|param| self.is_valid_search_param(param))
                        .cloned()
                        .collect();
                }
            }
        }
        Vec::new()
    }

    /// Validate header data to handle malformed data gracefully
    fn is_valid_header(&self, header: &Header) -> bool {
        let key = header.key.trim();
        let value = header.value.trim();
        
        // Basic validation
        if key.is_empty() || value.is_empty() {
            return false;
        }
        
        // Length checks
        if key.len() > 100 || value.len() > 8192 {
            return false;
        }
        
        // Format validation
        if !key.chars().all(|c| c.is_ascii() && !c.is_control() && c != ':' && c != ' ') {
            return false;
        }
        
        if !value.chars().all(|c| c.is_ascii() && (!c.is_control() || c == '\t')) {
            return false;
        }
        
        true
    }

    /// Validate search parameter data to handle malformed data gracefully
    fn is_valid_search_param(&self, param: &SearchParameter) -> bool {
        let key = param.key.trim();
        let value = param.value.trim();
        
        // Basic validation
        if key.is_empty() || value.is_empty() {
            return false;
        }
        
        // Length checks
        if key.len() > 100 || value.len() > 2048 {
            return false;
        }
        
        // Format validation
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            return false;
        }
        
        // Check for newlines in values
        if value.contains('\n') || value.contains('\r') {
            return false;
        }
        
        true
    }

    /// Resolves final headers by merging folder and service headers
    /// Service headers take priority over folder headers (Requirements 6.3, 6.4)
    fn resolve_final_headers(&self, cx: &Context<Self>) -> Vec<Header> {
        let mut final_headers = Vec::new();
        
        // Add folder common headers first (lower priority)
        let folder_headers = self.get_folder_common_headers(cx);
        final_headers.extend(folder_headers);
        
        // Add service-specific headers (higher priority, can override)
        let service_headers = self.request_panel.read(cx).headers(cx);
        final_headers.extend(service_headers);
        
        // Remove duplicates, keeping service-level headers
        self.deduplicate_headers(final_headers)
    }

    /// Resolves final URL by merging folder search parameters with the URL
    /// Service parameters take priority over folder parameters (Requirements 6.3, 6.4)
    fn resolve_final_url(&self, base_url: String, cx: &Context<Self>) -> String {
        let folder_params = self.get_folder_common_search_params(cx);
        
        // If no folder parameters, return the base URL as-is
        if folder_params.is_empty() {
            return base_url;
        }
        
        // Build URL with folder parameters
        url_utils::build_url_with_params(&base_url, &folder_params)
    }

    /// Removes duplicate headers, keeping the last occurrence (service headers override folder headers)
    fn deduplicate_headers(&self, headers: Vec<Header>) -> Vec<Header> {
        let mut final_headers = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        
        // Process headers in reverse order to keep the last occurrence
        for header in headers.into_iter().rev() {
            let key_lower = header.key.to_lowercase();
            if !seen_keys.contains(&key_lower) {
                seen_keys.insert(key_lower);
                final_headers.push(header);
            }
        }
        
        // Reverse to restore original order
        final_headers.reverse();
        final_headers
    }

    fn update_content_type_header(
        &mut self,
        body_type: BodyType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_panel.update(cx, |panel, cx| {
            // Get current headers, filter out Content-Type, add new one if needed
            let mut headers = panel.headers(cx);
            headers.retain(|h| h.key.to_lowercase() != "content-type");

            if let Some(content_type) = body_type.content_type() {
                headers.push(models::Header {
                    key: "Content-Type".into(),
                    value: content_type.into(),
                });
            }

            // Recreate headers panel with updated headers
            let headers_panel = cx.new(|cx| {
                super::request_headers::RequestHeaders::new(headers, window, cx)
            });
            panel.headers_panel = headers_panel;
            cx.notify();
        });
    }

    fn send_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_loading = true;
        self.response
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.response_headers
            .update(cx, |state, cx| state.set_value("", window, cx));

        let user_entered_url = self.url.read(cx).value().to_string();
        let url = self.final_url(cx);
        let method = self.request_panel.read(cx).method(cx);
        let headers = self.resolve_final_headers(cx);
        let body = self.request_panel.read(cx).body(cx);

        // Build request info with final URL and base_url usage indicator
        let mut request_info_text = format!("{:?} {}\n", method, url);
        if self.use_base_url && !self.base_url.is_empty() && !url_utils::is_absolute_url(&user_entered_url) {
            request_info_text.push_str(&format!("(Base URL: {})\n", self.base_url));
        }
        request_info_text.push('\n');
        for h in &headers {
            request_info_text.push_str(&format!("{}: {}\n", h.key, h.value));
        }
        if let Some(ref b) = body {
            request_info_text.push_str(&format!("\n{}", b));
        }
        self.request_info
            .update(cx, |state, cx| state.set_value(request_info_text, window, cx));

        cx.notify();

        let response = self.response.clone();
        let response_headers = self.response_headers.clone();

        let task = Tokio::spawn(cx, async move {
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
        });

        cx.spawn_in(
            window,
            async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let result = task.await.unwrap();

                match result {
                    Ok(resp) => {
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
                                    Ok(bytes) => {
                                        (String::from_utf8_lossy(&bytes).to_string(), false)
                                    }
                                    Err(e) => (format!("\n[Error: {}]", e), true),
                                };
                                response
                                    .update_in(cx, |state, window, cx| {
                                        state.insert(text, window, cx);
                                    })
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
                    Err(e) => {
                        let err = format!("Error: {}", e);
                        response
                            .update_in(cx, |state, window, cx| {
                                state.set_value(err, window, cx);
                            })
                            .ok();
                    }
                }

                this.update_in(cx, |this, _, cx| {
                    this.is_loading = false;
                    cx.notify();
                })
                .ok();
            },
        )
        .detach();
    }
}

impl Render for HttpService {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.request_panel.update(cx, |panel, cx| {
            panel.set_loading(self.is_loading);
            cx.notify();
        });

        // Get folder information for navigation link
        let folder_info = if let Some(folder_id) = self.folder_id {
            let registry = workspace::WorkspaceRegistry::global(cx);
            if let Some(workspace) = registry.read(cx).get_active_workspace() {
                workspace.folders.iter()
                    .find(|f| f.id == folder_id)
                    .map(|folder| (folder.id, folder.name.to_string()))
            } else {
                None
            }
        } else {
            None
        };

        div()
            .flex_1()
            .h_full()
            .py_2()
            .px_4()
            .v_flex()
            .gap_4()
            .when(folder_info.is_some(), |container: gpui::Div| {
                let (folder_id, folder_name) = folder_info.unwrap();
                container.child(
                    // Navigation breadcrumb to parent folder
                    div()
                        .h_flex()
                        .items_center()
                        .gap_2()
                        .pb_2()
                        .border_b_1()
                        .border_color(gpui::rgb(0xe0e0e0))
                        .child(
                            gpui_component::button::Button::new(format!("navigate_to_folder_{}", folder_id))
                                .ghost()
                                .compact()
                                .icon(gpui_component::IconName::Folder)
                                .label(format!("Folder: {}", folder_name))
                                .on_click(cx.listener(move |_this, _, _, cx| {
                                    // Emit event to navigate to folder detail
                                    let registry = workspace::WorkspaceRegistry::global(cx);
                                    registry.update(cx, |registry, cx| {
                                        if let Some(workspace) = registry.get_active_workspace_mut() {
                                            workspace.current_view = models::WorkspaceView::FolderDetail(folder_id);
                                            cx.notify();
                                        }
                                    });
                                }))
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(gpui::rgb(0x666666))
                                .child("Click to view folder settings")
                        )
                )
            })
            .child(self.request_panel.clone())
            .child(self.response_panel.clone())
    }
}
