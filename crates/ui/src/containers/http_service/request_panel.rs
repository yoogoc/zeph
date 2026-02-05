use super::method_selector::{MethodSelector, MethodSelectorEvent};
use super::param_utils::detect_path_params;
use super::request_body::{BodyType, RequestBody, RequestBodyEvent};
use super::request_headers::RequestHeaders;
use super::request_path_params::RequestPathParams;
use super::request_query_params::RequestQueryParams;
use super::url_utils;
use crate::url_utils::get_base_url;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{h_flex, v_flex, Disableable, IconName};
use models::{Header, HttpMethod, PathParameter, QueryParameter};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum RequestTab {
    Path,
    #[default]
    Query,
    Body,
    Headers,
}

pub enum RequestPanelEvent {
    Send,
    Save,
    AddHeader,
    AddQueryParam,
    BodyTypeChanged(BodyType),
    #[allow(unused)]
    MethodChanged(HttpMethod),
    ToggleBaseUrl,
}

pub struct RequestPanel {
    url: Entity<InputState>,
    method_selector: Entity<MethodSelector>,
    body_panel: Entity<RequestBody>,
    pub headers_panel: Entity<RequestHeaders>,
    query_params_panel: Entity<RequestQueryParams>,
    path_params_panel: Entity<RequestPathParams>,
    active_tab: RequestTab,
    is_loading: bool,
    is_modified: bool,
    use_base_url: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<RequestPanelEvent> for RequestPanel {}

impl RequestPanel {
    pub fn new(
        method: HttpMethod,
        url: Entity<InputState>,
        headers: Vec<Header>,
        query_params: Vec<QueryParameter>,
        path_params: Vec<PathParameter>,
        folder_query_params: Vec<QueryParameter>,
        use_base_url: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let method_selector = cx.new(|cx| MethodSelector::new(method, window, cx));
        let body_panel = cx.new(|cx| RequestBody::new(window, cx));
        let headers_panel = cx.new(|cx| RequestHeaders::new(headers, window, cx));
        let query_params_panel =
            cx.new(|cx| RequestQueryParams::new(query_params, folder_query_params, window, cx));
        let path_params_panel = cx.new(|cx| RequestPathParams::new(path_params, window, cx));

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

        Self {
            url,
            method_selector,
            body_panel,
            headers_panel,
            query_params_panel,
            path_params_panel,
            active_tab: RequestTab::default(),
            is_loading: false,
            is_modified: false,
            use_base_url,
            _subscriptions: vec![method_sub, body_sub],
        }
    }

    pub fn set_loading(&mut self, is_loading: bool) {
        self.is_loading = is_loading;
    }

    pub fn set_use_base_url(&mut self, use_base_url: bool) {
        self.use_base_url = use_base_url;
    }

    pub fn set_modified(&mut self, is_modified: bool) {
        self.is_modified = is_modified;
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

    pub fn query_params(&self, cx: &App) -> Vec<QueryParameter> {
        self.query_params_panel.read(cx).params(cx)
    }

    pub fn path_params(&self, cx: &App) -> Vec<PathParameter> {
        self.path_params_panel.read(cx).params(cx)
    }

    pub fn add_query_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query_params_panel
            .update(cx, |panel, cx| panel.add_param(window, cx));
    }

    pub fn update_path_params_from_url(
        &mut self,
        url: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.path_params_panel.update(cx, |panel, cx| {
            panel.update_from_url(url, window, cx);
        });
        cx.notify();
    }
}

impl Render for RequestPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let is_loading = self.is_loading;
        let use_base_url = self.use_base_url;
        let base_url = get_base_url(cx);
        let show_final_url_preview = use_base_url
            && !base_url.is_empty()
            && !url_utils::is_absolute_url(&self.url.read(cx).value().to_string());

        // Detect if path parameters exist in the URL
        let url_value = self.url.read(cx).value().to_string();
        let has_path_params = !detect_path_params(&url_value).is_empty();

        let tooltip_text = if use_base_url {
            if base_url.is_empty() {
                t!("request.base_url.enabled_but_empty").to_string()
            } else {
                format!("{}: {}", t!("env.base_url"), base_url)
            }
        } else {
            t!("request.base_url.disabled").to_string()
        };

        v_flex()
            .w_full()
            .gap_4()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .flex_1()
                            .p_2()
                            .gap_1()
                            .bg(white())
                            .border_1()
                            .border_color(rgb(0xdee2e6))
                            .rounded_md()
                            .text_sm()
                            .child(self.method_selector.clone())
                            .when(show_final_url_preview, |el| {
                                let base_url_display = base_url.trim_end_matches('/').to_string();
                                el.child(
                                    div()
                                        .text_sm()
                                        .text_color(gpui::rgb(0x28a745))
                                        .child(base_url_display),
                                )
                            })
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
                            .label(if is_loading {
                                shared_t!("request.sending")
                            } else {
                                shared_t!("request.send")
                            })
                            .disabled(is_loading)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(RequestPanelEvent::Send);
                            })),
                    )
                    .child(
                        Button::new("save_request")
                            .label(shared_t!("common.save"))
                            .disabled(!self.is_modified)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(RequestPanelEvent::Save);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child({
                        let mut tab_bar =
                            TabBar::new("request_tabs").with_variant(TabVariant::Underline);

                        // Only show Path tab when path parameters are detected
                        if has_path_params {
                            tab_bar = tab_bar.child(Tab::new().label(shared_t!("request.path")));
                        }

                        tab_bar = tab_bar
                            .child(Tab::new().label(shared_t!("request.query")))
                            .child(Tab::new().label(shared_t!("request.body")))
                            .child(Tab::new().label(shared_t!("request.header")));

                        tab_bar
                            .selected_index(active_tab as usize)
                            .on_click(cx.listener(move |this, ix: &usize, _, cx| {
                                this.active_tab = match *ix {
                                    0 if has_path_params => RequestTab::Path,
                                    1 => RequestTab::Query,
                                    2 => RequestTab::Body,
                                    3 => RequestTab::Headers,
                                    _ => {
                                        if has_path_params {
                                            RequestTab::Path
                                        } else {
                                            RequestTab::Query
                                        }
                                    }
                                };
                                cx.notify();
                            }))
                            .suffix(match active_tab {
                                RequestTab::Headers => Button::new("add_header")
                                    .icon(IconName::Plus)
                                    .label(shared_t!("request.add_header"))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(RequestPanelEvent::AddHeader);
                                    })),
                                RequestTab::Query => Button::new("add_query_param")
                                    .icon(IconName::Plus)
                                    .label(shared_t!("request.add_param"))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(RequestPanelEvent::AddQueryParam);
                                    })),
                                _ => Button::new("placeholder").label("").disabled(true),
                            })
                    })
                    .when(active_tab == RequestTab::Body, |el| {
                        el.child(self.body_panel.clone())
                    })
                    .when(active_tab == RequestTab::Headers, |el| {
                        el.child(self.headers_panel.clone())
                    })
                    .when(active_tab == RequestTab::Query, |el| {
                        el.child(self.query_params_panel.clone())
                    })
                    .when(active_tab == RequestTab::Path && has_path_params, |el| {
                        el.child(self.path_params_panel.clone())
                    }),
            )
    }
}
