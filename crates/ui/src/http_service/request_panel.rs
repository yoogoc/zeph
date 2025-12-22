use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{Disableable, StyledExt};
use models::{Header, HttpMethod};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum RequestTab {
    #[default]
    Body,
    Headers,
}

pub enum RequestPanelEvent {
    Send,
    AddHeader,
    RemoveHeader(usize),
}

pub struct RequestPanel {
    method: HttpMethod,
    url: Entity<InputState>,
    headers: Vec<Header>,
    body: Option<SharedString>,
    active_tab: RequestTab,
    is_loading: bool,
}

impl EventEmitter<RequestPanelEvent> for RequestPanel {}

impl RequestPanel {
    pub fn new(method: HttpMethod, url: Entity<InputState>) -> Self {
        Self {
            method,
            url,
            headers: Vec::new(),
            body: None,
            active_tab: RequestTab::default(),
            is_loading: false,
        }
    }

    pub fn set_headers(&mut self, headers: Vec<Header>) {
        self.headers = headers;
    }

    pub fn set_body(&mut self, body: Option<SharedString>) {
        self.body = body;
    }

    pub fn set_loading(&mut self, is_loading: bool) {
        self.is_loading = is_loading;
    }

    fn render_method_selector(&self) -> impl IntoElement {
        let (color, label) = match self.method {
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
}

impl Render for RequestPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let is_loading = self.is_loading;

        div()
            .v_flex()
            .gap_4()
            .child(
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
                            .child(self.render_method_selector())
                            .child(Input::new(&self.url)),
                    )
                    .child(
                        Button::new("send_request")
                            .primary()
                            .label(if is_loading { "Sending..." } else { "Send" })
                            .disabled(is_loading)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(RequestPanelEvent::Send);
                            })),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        TabBar::new("request_tabs")
                            .child(Tab::new().label("Body"))
                            .child(Tab::new().label("Headers"))
                            .selected_index(active_tab as usize)
                            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                this.active_tab = if *ix == 0 {
                                    RequestTab::Body
                                } else {
                                    RequestTab::Headers
                                };
                                cx.notify();
                            }))
                            .suffix(
                                Button::new("add_header")
                                    .label("+ Add Header")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(RequestPanelEvent::AddHeader);
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
                                .when_some(self.body.clone(), |this, body| this.child(body)),
                        )
                    })
                    .when(active_tab == RequestTab::Headers, |el| {
                        el.child(
                            div()
                                .v_flex()
                                .gap_1()
                                .children(self.headers.iter().enumerate().map(|(i, header)| {
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
                                                .on_click(cx.listener(move |_, _, _, cx| {
                                                    cx.emit(RequestPanelEvent::RemoveHeader(i));
                                                })),
                                        )
                                }))
                                .when(self.headers.is_empty(), |el| {
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
                    }),
            )
    }
}
