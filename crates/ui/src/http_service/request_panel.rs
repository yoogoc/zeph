use super::method_selector::{MethodSelector, MethodSelectorEvent};
use super::request_body::{BodyType, RequestBody, RequestBodyEvent};
use super::request_headers::RequestHeaders;
use super::url_utils;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{Disableable, IconName, StyledExt};
use models::{Header, HttpMethod};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum RequestTab {
    #[default]
    Body,
    Headers,
}

#[allow(unused)]
pub enum RequestPanelEvent {
    Send,
    AddHeader,
    BodyTypeChanged(BodyType),
    MethodChanged(HttpMethod),
    ToggleBaseUrl,
}

pub struct RequestPanel {
    url: Entity<InputState>,
    method_selector: Entity<MethodSelector>,
    body_panel: Entity<RequestBody>,
    pub headers_panel: Entity<RequestHeaders>,
    active_tab: RequestTab,
    is_loading: bool,
    use_base_url: bool,
    base_url: String,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<RequestPanelEvent> for RequestPanel {}

impl RequestPanel {
    pub fn new(
        method: HttpMethod,
        url: Entity<InputState>,
        use_base_url: bool,
        base_url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let method_selector = cx.new(|cx| MethodSelector::new(method, window, cx));
        let body_panel = cx.new(|cx| RequestBody::new(window, cx));
        let headers_panel = cx.new(|cx| RequestHeaders::new(Vec::new(), window, cx));

        let method_sub = cx.subscribe(&method_selector, |_this, _, event, cx| match event {
            MethodSelectorEvent::Changed(method) => {
                cx.emit(RequestPanelEvent::MethodChanged(method.clone()));
            }
        });

        let body_sub = cx.subscribe(&body_panel, |_this, _, event, cx| match event {
            RequestBodyEvent::BodyTypeChanged(body_type) => {
                cx.emit(RequestPanelEvent::BodyTypeChanged(*body_type));
            }
            RequestBodyEvent::BodyChanged(_) => {}
        });

        let headers_sub = cx.subscribe(&headers_panel, |_this, _, _event, _cx| {});

        Self {
            url,
            method_selector,
            body_panel,
            headers_panel,
            active_tab: RequestTab::default(),
            is_loading: false,
            use_base_url,
            base_url,
            _subscriptions: vec![method_sub, body_sub, headers_sub],
        }
    }

    pub fn set_loading(&mut self, is_loading: bool) {
        self.is_loading = is_loading;
    }

    pub fn set_use_base_url(&mut self, use_base_url: bool) {
        self.use_base_url = use_base_url;
    }

    pub fn set_base_url(&mut self, base_url: String) {
        self.base_url = base_url;
    }

    /// Gets the final URL by resolving base_url and user-entered path
    pub fn final_url(&self, cx: &App) -> String {
        let path = self.url.read(cx).value().to_string();
        url_utils::resolve_url(&self.base_url, &path, self.use_base_url)
    }

    pub fn add_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.headers_panel
            .update(cx, |panel, cx| panel.add_header(window, cx));
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

    pub fn method(&self, cx: &App) -> HttpMethod {
        self.method_selector.read(cx).method(cx)
    }
}

impl Render for RequestPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let is_loading = self.is_loading;
        let use_base_url = self.use_base_url;
        let base_url = self.base_url.clone();
        let show_final_url_preview = use_base_url && !base_url.is_empty() && !url_utils::is_absolute_url(&self.url.read(cx).value().to_string());

        let tooltip_text = if use_base_url {
            if base_url.is_empty() {
                "Base URL mode enabled (no base URL set)".to_string()
            } else {
                format!("Base URL: {}", base_url)
            }
        } else {
            "Base URL mode disabled".to_string()
        };

        div()
            .v_flex()
            .gap_4()
            .child(
                div()
                    .v_flex()
                    .gap_1()
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
                                    .child(self.method_selector.clone())
                                    .child(Input::new(&self.url))
                                    .child(
                                        Button::new("toggle_base_url")
                                            .icon(IconName::Globe)
                                            .ghost()
                                            .tooltip(tooltip_text)
                                            .when(use_base_url, |btn| btn.primary())
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                cx.emit(RequestPanelEvent::ToggleBaseUrl);
                                            })),
                                    ),
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
                    .when(show_final_url_preview, |el| {
                        let base_url_display = base_url.trim_end_matches('/').to_string();
                        let path_display = self.url.read(cx).value().to_string().trim_start_matches('/').to_string();
                        el.child(
                            div()
                                .h_flex()
                                .gap_1()
                                .px_2()
                                .text_xs()
                                .text_color(gpui::rgb(0x6c757d))
                                .child(
                                    div()
                                        .text_color(gpui::rgb(0x28a745))
                                        .child(base_url_display)
                                )
                                .child("/")
                                .child(path_display)
                        )
                    }),
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
