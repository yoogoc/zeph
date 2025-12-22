use super::request_body::RequestBody;
use super::request_headers::{RequestHeaders, RequestHeadersEvent};
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
    body_panel: Entity<RequestBody>,
    headers_panel: Entity<RequestHeaders>,
    active_tab: RequestTab,
    is_loading: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<RequestPanelEvent> for RequestPanel {}

impl RequestPanel {
    pub fn new(method: HttpMethod, url: Entity<InputState>, cx: &mut Context<Self>) -> Self {
        let body_panel = cx.new(|_cx| RequestBody::new(None));
        let headers_panel = cx.new(|_cx| RequestHeaders::new(Vec::new()));

        let subscription = cx.subscribe(&headers_panel, |_this, _, event, cx| match event {
            RequestHeadersEvent::Remove(index) => {
                cx.emit(RequestPanelEvent::RemoveHeader(*index));
            }
        });

        Self {
            method,
            url,
            headers: Vec::new(),
            body: None,
            body_panel,
            headers_panel,
            active_tab: RequestTab::default(),
            is_loading: false,
            _subscriptions: vec![subscription],
        }
    }

    pub fn set_headers(&mut self, headers: Vec<Header>, cx: &mut Context<Self>) {
        self.headers = headers.clone();
        self.headers_panel.update(cx, |panel, cx| {
            *panel = RequestHeaders::new(headers);
            cx.notify();
        });
    }

    pub fn set_body(&mut self, body: Option<SharedString>, cx: &mut Context<Self>) {
        self.body = body.clone();
        self.body_panel.update(cx, |panel, cx| {
            *panel = RequestBody::new(body);
            cx.notify();
        });
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
                        el.child(self.body_panel.clone())
                    })
                    .when(active_tab == RequestTab::Headers, |el| {
                        el.child(self.headers_panel.clone())
                    }),
            )
    }
}
