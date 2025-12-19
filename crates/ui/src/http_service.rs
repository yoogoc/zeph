use gpui::*;
use gpui_component::{button::{Button, ButtonVariants}, *};
use models::{HttpConfig, HttpMethod, Header};

pub struct HttpService {
    config: HttpConfig,
    response_body: Option<String>,
    is_loading: bool,
}

impl HttpService {
    pub fn new(config: HttpConfig) -> Self {
        Self {
            config,
            response_body: None,
            is_loading: false,
        }
    }
    
    fn add_header(&mut self, _cx: &mut Context<Self>) {
        self.config.headers.push(Header {
            key: "Content-Type".to_string(),
            value: "application/json".to_string(),
        });
    }
    
    fn remove_header(&mut self, index: usize, _cx: &mut Context<Self>) {
        if index < self.config.headers.len() {
            self.config.headers.remove(index);
        }
    }
    
    fn send_request(&mut self, _cx: &mut Context<Self>) {
        self.is_loading = true;
        println!("Sending {} request to {}", 
            match self.config.method {
                HttpMethod::Get => "GET",
                HttpMethod::Post => "POST",
                HttpMethod::Put => "PUT",
                HttpMethod::Delete => "DELETE",
                _ => "HTTP",
            },
            self.config.url
        );
    }
    
    fn render_method_selector(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child("Method:")
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .bg(match self.config.method {
                        HttpMethod::Get => gpui::rgb(0x28a745),
                        HttpMethod::Post => gpui::rgb(0x007bff),
                        HttpMethod::Put => gpui::rgb(0xffc107),
                        HttpMethod::Delete => gpui::rgb(0xdc3545),
                        _ => gpui::rgb(0x6c757d),
                    })
                    .text_color(gpui::rgb(0xffffff))
                    .rounded_md()
                    .text_sm()
                    .font_semibold()
                    .child(match self.config.method {
                        HttpMethod::Get => "GET",
                        HttpMethod::Post => "POST",
                        HttpMethod::Put => "PUT",
                        HttpMethod::Delete => "DELETE",
                        _ => "HTTP",
                    })
            )
    }
    
    fn render_url_input(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child("URL:")
            )
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .p_3()
                            .bg(gpui::rgb(0xffffff))
                            .border_1()
                            .border_color(gpui::rgb(0xdee2e6))
                            .rounded_md()
                            .text_sm()
                            .child(self.config.url.clone())
                    )
                    .child(
                        Button::new("send_request")
                            .primary()
                            .label(if self.is_loading { "Sending..." } else { "Send" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.send_request(cx);
                            }))
                    )
            )
    }
    
    fn render_headers(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child("Headers:")
                    )
                    .child(
                        Button::new("add_header")
                            .label("+ Add Header")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_header(cx);
                                cx.notify();
                            }))
                    )
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .children(
                        self.config.headers.iter().enumerate().map(|(i, header)| {
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
                                        .child(header.key.clone())
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
                                        .child(header.value.clone())
                                )
                                .child(
                                    Button::new(format!("remove_header_{}", i))
                                        .label("×")
                                        .w(px(24.0))
                                        .h(px(24.0))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.remove_header(i, cx);
                                            cx.notify();
                                        }))
                                )
                        })
                    )
                    .child(
                        if self.config.headers.is_empty() {
                            div()
                                .p_4()
                                .text_center()
                                .text_color(gpui::rgb(0x6c757d))
                                .text_sm()
                                .child("No headers added")
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    )
            )
    }
    
    fn render_body(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child("Request Body:")
            )
            .child(
                div()
                    .min_h(px(120.0))
                    .p_3()
                    .bg(gpui::rgb(0xffffff))
                    .border_1()
                    .border_color(gpui::rgb(0xdee2e6))
                    .rounded_md()
                    .text_sm()
                    .child(
                        self.config.body.as_ref()
                            .unwrap_or(&"".to_string())
                            .clone()
                    )
            )
    }
    
    fn render_response(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child("Response:")
            )
            .child(
                div()
                    .min_h(px(200.0))
                    .p_3()
                    .bg(gpui::rgb(0xf8f9fa))
                    .border_1()
                    .border_color(gpui::rgb(0xdee2e6))
                    .rounded_md()
                    .text_sm()
                    .child(
                        self.response_body.as_ref()
                            .unwrap_or(&"Click 'Send' to see the response".to_string())
                            .clone()
                    )
            )
    }
}

impl Render for HttpService {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .h_full()
            .p_4()
            .v_flex()
            .gap_4()
            .child(self.render_method_selector(cx))
            .child(self.render_url_input(cx))
            .child(self.render_headers(cx))
            .child(self.render_body(cx))
            .child(self.render_response(cx))
    }
}
