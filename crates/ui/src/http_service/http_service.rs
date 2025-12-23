use super::request_body::BodyType;
use super::request_panel::{RequestPanel, RequestPanelEvent};
use super::response_panel::ResponsePanel;
use futures::StreamExt;
use gpui::*;
use gpui_component::input::{InputState, TabSize};
use gpui_component::StyledExt;
use gpui_tokio::Tokio;
use models::{HttpConfig, HttpMethod};
use smallvec::{smallvec, SmallVec};

pub struct HttpService {
    url: Entity<InputState>,
    response: Entity<InputState>,
    response_headers: Entity<InputState>,
    request_info: Entity<InputState>,
    request_panel: Entity<RequestPanel>,
    response_panel: Entity<ResponsePanel>,
    is_loading: bool,
    _subscriptions: SmallVec<[Subscription; 2]>,
}

impl HttpService {
    pub fn new(config: HttpConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = config.path.clone();
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
            cx.new(|cx| RequestPanel::new(config.method.clone(), url.clone(), window, cx));

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
            _subscriptions: smallvec![subscription],
        }
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

        let url = self.url.read(cx).value().to_string();
        let method = self.request_panel.read(cx).method(cx);
        let headers = self.request_panel.read(cx).headers(cx);
        let body = self.request_panel.read(cx).body(cx);

        // Build request info
        let mut request_info_text = format!("{:?} {}\n\n", method, url);
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

        div()
            .flex_1()
            .h_full()
            .py_2()
            .px_4()
            .v_flex()
            .gap_4()
            .child(self.request_panel.clone())
            .child(self.response_panel.clone())
    }
}
