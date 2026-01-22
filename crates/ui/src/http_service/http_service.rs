use super::param_utils::{build_url_with_query_params, merge_query_params, substitute_path_params};
use super::request_body::BodyType;
use super::request_panel::{RequestPanel, RequestPanelEvent};
use super::response_panel::ResponsePanel;
use super::url_utils;
use futures::StreamExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{InputState, TabSize};
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::spinner::Spinner;
use gpui_component::{v_flex, Sizable};
use gpui_tokio::Tokio;
use models::{Header, HttpConfig, HttpMethod, QueryParameter, SearchParameter};
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
    pub fn new(
        config: HttpConfig,
        base_url: Option<SharedString>,
        folder_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
            ResponsePanel::new(
                response.clone(),
                response_headers.clone(),
                request_info.clone(),
            )
        });

        // Get folder query parameters for inheritance
        let folder_query_params = if let Some(folder_id) = folder_id {
            let registry = workspace::WorkspaceRegistry::global(cx);
            if let Some(workspace) = registry.read(cx).get_active_workspace() {
                if let Some(folder) = workspace.folders.iter().find(|f| f.id == folder_id) {
                    folder
                        .common_search_params
                        .iter()
                        .map(|param| QueryParameter {
                            key: param.key.clone(),
                            value: param.value.clone(),
                            enabled: true,
                            description: None,
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let request_panel = cx.new(|cx| {
            RequestPanel::new(
                config.method.clone(),
                url.clone(),
                Vec::new(), // Headers are managed separately, not stored in HttpConfig
                config.query_params.clone(),
                config.path_params.clone(),
                folder_query_params,
                use_base_url,
                base_url.clone().unwrap_or_default(),
                window,
                cx,
            )
        });

        // Subscribe to URL changes to update path parameters
        let url_subscription = cx.subscribe_in(
            &url,
            window,
            |this, url_entity, _event: &gpui_component::input::InputEvent, window, cx| {
                let url_value = url_entity.read(cx).value().to_string();
                this.request_panel.update(cx, |panel, cx| {
                    panel.update_path_params_from_url(&url_value, window, cx);
                });
            },
        );

        let subscription = cx.subscribe_in(&request_panel, window, |this, _, event, window, cx| {
            match event {
                RequestPanelEvent::Send => {
                    this.send_request(window, cx);
                }
                RequestPanelEvent::AddHeader => {
                    this.request_panel
                        .update(cx, |panel, cx| panel.add_header(window, cx));
                }
                RequestPanelEvent::AddQueryParam => {
                    this.request_panel
                        .update(cx, |panel, cx| panel.add_query_param(window, cx));
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
            _subscriptions: smallvec![subscription, url_subscription],
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

    /// Gets the final URL by resolving base_url, path params, and query params
    pub fn final_url(&self, cx: &Context<Self>) -> String {
        let path = self.url.read(cx).value().to_string();
        let mut resolved_url = url_utils::resolve_url(&self.base_url, &path, self.use_base_url);

        // Apply path parameter substitution
        let path_params = self.request_panel.read(cx).path_params(cx);
        resolved_url = substitute_path_params(&resolved_url, &path_params);

        // Apply query parameters (both folder and service level)
        let query_params = self.resolve_final_query_params(cx);
        resolved_url = build_url_with_query_params(&resolved_url, &query_params);

        // Apply legacy folder search parameters (for backward compatibility)
        resolved_url = self.resolve_final_url(resolved_url, cx);

        resolved_url
    }

    /// Gets folder common headers from the workspace registry
    fn get_folder_common_headers(&self, cx: &Context<Self>) -> Vec<Header> {
        if let Some(folder_id) = self.folder_id {
            let registry = workspace::WorkspaceRegistry::global(cx);
            if let Some(workspace) = registry.read(cx).get_active_workspace() {
                if let Some(folder) = workspace.folders.iter().find(|f| f.id == folder_id) {
                    // Validate headers before using them
                    return folder
                        .common_headers
                        .iter()
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
                    return folder
                        .common_search_params
                        .iter()
                        .filter(|param| self.is_valid_search_param(param))
                        .cloned()
                        .collect();
                }
            }
        }
        Vec::new()
    }

    /// Gets folder common query parameters from the workspace registry
    /// Converts SearchParameter to QueryParameter format
    fn get_folder_common_query_params(&self, cx: &Context<Self>) -> Vec<QueryParameter> {
        if let Some(folder_id) = self.folder_id {
            let registry = workspace::WorkspaceRegistry::global(cx);
            if let Some(workspace) = registry.read(cx).get_active_workspace() {
                if let Some(folder) = workspace.folders.iter().find(|f| f.id == folder_id) {
                    // Convert SearchParameter to QueryParameter
                    return folder
                        .common_search_params
                        .iter()
                        .filter(|param| self.is_valid_search_param(param))
                        .map(|param| QueryParameter {
                            key: param.key.clone(),
                            value: param.value.clone(),
                            enabled: true,
                            description: None,
                        })
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
        if !key
            .chars()
            .all(|c| c.is_ascii() && !c.is_control() && c != ':' && c != ' ')
        {
            return false;
        }

        if !value
            .chars()
            .all(|c| c.is_ascii() && (!c.is_control() || c == '\t'))
        {
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
        if !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
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

    /// Resolves final query parameters by merging folder and service params
    /// Service parameters override folder parameters with same key (Requirements 5.1, 5.2, 5.3)
    fn resolve_final_query_params(&self, cx: &Context<Self>) -> Vec<QueryParameter> {
        let folder_params = self.get_folder_common_query_params(cx);
        let service_params = self.request_panel.read(cx).query_params(cx);
        merge_query_params(&folder_params, &service_params)
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

    /// Validates path parameters are complete
    /// Returns error if any required path parameter is missing a value (Requirements 6.4, 9.3)
    fn validate_path_params(&self, cx: &Context<Self>) -> Result<(), String> {
        let url = self.url.read(cx).value().to_string();
        let path_params = self.request_panel.read(cx).path_params(cx);

        // Check if URL contains any unresolved placeholders
        let url_after_substitution = substitute_path_params(&url, &path_params);

        // If there are still placeholders in the URL, some params are missing
        if url_after_substitution.contains('{') && url_after_substitution.contains('}') {
            // Extract the missing parameter names
            let missing_params: Vec<String> =
                super::param_utils::detect_path_params(&url_after_substitution);

            if !missing_params.is_empty() {
                return Err(format!(
                    "Missing required path parameters: {}",
                    missing_params.join(", ")
                ));
            }
        }

        Ok(())
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
                headers.push(Header {
                    key: "Content-Type".into(),
                    value: content_type.into(),
                });
            }

            // Recreate a headers panel with updated headers
            let headers_panel =
                cx.new(|cx| super::request_headers::RequestHeaders::new(headers, window, cx));
            panel.headers_panel = headers_panel;
            cx.notify();
        });
    }

    fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.is_loading = loading;
        self.request_panel.update(cx, |panel, cx| {
            panel.set_loading(loading);
            cx.notify();
        });
    }

    fn send_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Validate path parameters before sending (Requirements 6.4, 9.3)
        if let Err(error_msg) = self.validate_path_params(cx) {
            // Display validation error in response panel
            self.response.update(cx, |state, cx| {
                state.set_value(
                    format!("Validation Error: {}\n\nPlease provide values for all path parameters before sending the request.", error_msg),
                    window,
                    cx
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

        let user_entered_url = self.url.read(cx).value().to_string();
        let url = self.final_url(cx);
        let method = self.request_panel.read(cx).method(cx);
        let headers = self.resolve_final_headers(cx);
        let body = self.request_panel.read(cx).body(cx);

        // Build request info with final resolved URL (Requirements 6.1, 6.2, 9.3)
        let mut request_info_text = format!("{:?} {}\n", method, url);
        if self.use_base_url
            && !self.base_url.is_empty()
            && !url_utils::is_absolute_url(&user_entered_url)
        {
            request_info_text.push_str(&format!("({}: {})\n", t!("env.base_url"), self.base_url));
        }
        request_info_text.push('\n');
        for h in &headers {
            request_info_text.push_str(&format!("{}: {}\n", h.key, h.value));
        }
        if let Some(ref b) = body {
            request_info_text.push_str(&format!("\n{}", b));
        }
        self.request_info.update(cx, |state, cx| {
            state.set_value(request_info_text, window, cx)
        });

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

                this.update_in(cx, |this, _, cx| {
                    this.set_loading(false, cx);
                })
                .ok();

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
            },
        )
        .detach();
    }
}

impl Render for HttpService {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .py_2()
            .px_4()
            .gap_4()
            .relative()
            .child(
                v_resizable("http-service")
                    .child(self.request_panel.clone().into_any_element())
                    .child(
                        resizable_panel()
                            .child(div().pt_2().size_full().child(self.response_panel.clone())),
                    ),
            )
            .when(self.is_loading, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0., 0., 1., 0.5))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| cx.stop_propagation())
                        .child(
                            v_flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .child(Spinner::new().large())
                                .child(
                                    // TODO: cancel action
                                    Button::new("cancel-http-service").label("cancel").primary(),
                                ),
                        ),
                )
            })
    }
}
