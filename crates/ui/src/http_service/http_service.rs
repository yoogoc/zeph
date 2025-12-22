use super::response_panel::ResponsePanel;
use futures::StreamExt;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputState, TabSize};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{
    button::{Button, ButtonVariants},
    *,
};
use gpui_tokio::Tokio;
use models::{Header, HttpConfig, HttpMethod};
use smallvec::{smallvec, SmallVec};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum RequestTab {
    #[default]
    Body,
    Headers,
}

pub struct HttpService {
    config: HttpConfig,
    url: Entity<InputState>,
    response: Entity<InputState>,
    response_headers: Entity<InputState>,
    request_info: Entity<InputState>,
    response_panel: Entity<ResponsePanel>,
    request_tab: RequestTab,
    is_loading: bool,
    _subscriptions: SmallVec<[Subscription; 2]>,
}

impl HttpService {
    pub fn new(config: HttpConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = config.url.clone();
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

        let subscriptions = smallvec![];

        Self {
            config,
            url,
            response,
            response_headers,
            request_info,
            response_panel,
            request_tab: RequestTab::default(),
            is_loading: false,
            _subscriptions: subscriptions,
        }
    }

    fn add_header(&mut self, _cx: &mut Context<Self>) {
        self.config.headers.push(Header {
            key: "Content-Type".into(),
            value: "application/json".into(),
        });
    }

    fn remove_header(&mut self, index: usize, _cx: &mut Context<Self>) {
        if index < self.config.headers.len() {
            self.config.headers.remove(index);
        }
    }

    fn send_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_loading = true;
        self.response
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.response_headers
            .update(cx, |state, cx| state.set_value("", window, cx));

        let url = self.url.read(cx).value().to_string();
        let method = self.config.method.clone();
        let headers = self.config.headers.clone();
        let body = self.config.body.clone();

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
                req = req.body(b.to_string());
            }

            req.send().await
        });

        cx.spawn_in(
            window,
            async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let result = task.await.unwrap();

                match result {
                    Ok(resp) => {
                        // Format headers
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

    fn render_method_selector(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let (color, label) = match self.config.method {
            HttpMethod::Get => (0x28a745, "GET"),
            HttpMethod::Post => (0x007bff, "POST"),
            HttpMethod::Put => (0xffc107, "PUT"),
            HttpMethod::Delete => (0xdc3545, "DELETE"),
            HttpMethod::Patch => (0x17a2b8, "PATCH"),
            HttpMethod::Head => (0x6c757d, "HEAD"),
            HttpMethod::Options => (0x6c757d, "OPTIONS"),
        };
        div()
            .px_3()
            .py_1()
            .bg(gpui::rgb(color))
            .text_color(gpui::rgb(0xffffff))
            .rounded_md()
            .text_sm()
            .font_semibold()
            .child(label)
    }

    fn render_url_input(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .h_flex()
                    .p_2()
                    .gap_1()
                    .bg(gpui::rgb(0xffffff))
                    .border_1()
                    .border_color(gpui::rgb(0xdee2e6))
                    .rounded_md()
                    .text_sm()
                    .child(self.render_method_selector(cx))
                    .child(Input::new(&self.url)),
            )
            .child(
                Button::new("send_request")
                    .primary()
                    .label(if self.is_loading {
                        "Sending..."
                    } else {
                        "Send"
                    })
                    .disabled(self.is_loading)
                    .on_click(cx.listener(|this, _, window, cx| this.send_request(window, cx))),
            )
    }

    fn render_request_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.request_tab;

        div()
            .v_flex()
            .gap_2()
            .child(
                TabBar::new("request_tabs")
                    .child(Tab::new().label("Body"))
                    .child(Tab::new().label("Headers"))
                    .selected_index(active_tab as usize)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.request_tab = if *ix == 0 {
                            RequestTab::Body
                        } else {
                            RequestTab::Headers
                        };
                        cx.notify();
                    }))
                    .suffix(
                        Button::new("add_header")
                            .label("+ Add Header")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_header(cx);
                                cx.notify();
                            })),
                    ),
            )
            .when(active_tab == RequestTab::Body, |el| {
                el.child(
                    div()
                        .min_h(px(120.0))
                        .p_3()
                        .bg(gpui::rgb(0xffffff))
                        .border_1()
                        .border_color(gpui::rgb(0xdee2e6))
                        .rounded_md()
                        .text_sm()
                        .when_some(self.config.body.clone(), |this, body| this.child(body)),
                )
            })
            .when(active_tab == RequestTab::Headers, |el| {
                el.child(
                    div()
                        .v_flex()
                        .gap_1()
                        .children(self.config.headers.iter().enumerate().map(|(i, header)| {
                            div()
                                .h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .p_2()
                                        .bg(gpui::rgb(0xf8f9fa))
                                        .border_1()
                                        .border_color(gpui::rgb(0xdee2e6))
                                        .rounded_md()
                                        .text_sm()
                                        .child(header.key.clone()),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .p_2()
                                        .bg(gpui::rgb(0xf8f9fa))
                                        .border_1()
                                        .border_color(gpui::rgb(0xdee2e6))
                                        .rounded_md()
                                        .text_sm()
                                        .child(header.value.clone()),
                                )
                                .child(
                                    Button::new(format!("remove_header_{}", i))
                                        .label("×")
                                        .w(px(24.0))
                                        .h(px(24.0))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.remove_header(i, cx);
                                            cx.notify();
                                        })),
                                )
                        }))
                        .when(self.config.headers.is_empty(), |el| {
                            el.child(
                                div()
                                    .p_4()
                                    .text_center()
                                    .text_color(gpui::rgb(0x6c757d))
                                    .text_sm()
                                    .child("No headers added"),
                            )
                        }),
                )
            })
    }
}

impl Render for HttpService {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .py_2()
            .px_4()
            .v_flex()
            .gap_4()
            .child(self.render_url_input(cx))
            .child(self.render_request_panel(cx))
            .child(self.response_panel.clone())
    }
}
