use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    resizable::{resizable_panel, v_resizable},
    select::{Select, SelectState},
    tab::{Tab, TabBar},
    v_flex, IconName, IndexPath, Sizable,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum RequestTab {
    #[default]
    Params,
    Headers,
    Body,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum ResponseTab {
    #[default]
    Body,
    Headers,
}

struct KeyValue {
    key: Entity<InputState>,
    value: Entity<InputState>,
    enabled: bool,
}

struct PageState {
    method_state: Entity<SelectState<Vec<&'static str>>>,
    url_state: Entity<InputState>,
    request_tab: RequestTab,
    response_tab: ResponseTab,
    params: Vec<KeyValue>,
    headers: Vec<KeyValue>,
    body_state: Entity<InputState>,
    response_body: String,
    response_headers: Vec<(String, String)>,
    response_status: Option<u16>,
}

impl Render for PageState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
pub struct Page {}

impl Page {
    pub fn new() -> Self {
        Self {}
    }
}

fn create_kv(window: &mut Window, cx: &mut App) -> KeyValue {
    KeyValue {
        key: cx.new(|cx| InputState::new(window, cx)),
        value: cx.new(|cx| InputState::new(window, cx)),
        enabled: true,
    }
}

impl RenderOnce for Page {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let methods = vec!["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];

        let state = window.use_state(cx, |window, cx| {
            let method_state = cx.new(|cx| {
                SelectState::new(methods.clone(), Some(IndexPath::default().row(0)), window, cx)
            });
            let url_state = cx.new(|cx| {
                let mut s = InputState::new(window, cx);
                s.set_placeholder("Enter URL...", window, cx);
                s
            });
            let body_state = cx.new(|cx| {
                let mut s = InputState::new(window, cx);
                s.set_placeholder("Request body...", window, cx);
                s
            });
            PageState {
                method_state,
                url_state,
                request_tab: RequestTab::default(),
                response_tab: ResponseTab::default(),
                params: vec![create_kv(window, cx)],
                headers: vec![create_kv(window, cx)],
                body_state,
                response_body: String::new(),
                response_headers: vec![],
                response_status: None,
            }
        });

        let page_state = state.read(cx);

        v_flex()
            .id("http-page")
            .size_full()
            .p_2()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(Select::new(&page_state.method_state).w(px(110.)).small())
                    .child(Input::new(&page_state.url_state).w_full().small().cleanable(true))
                    .child(
                        Button::new("send")
                            .primary()
                            .small()
                            .label("Send")
                            .on_click(|_, _, _| println!("Send request")),
                    ),
            )
            .child(
                v_resizable("http-panels")
                    .child(
                        resizable_panel()
                            .size(px(200.))
                            .child(render_request_panel(window, cx, &state)),
                    )
                    .child(
                        resizable_panel()
                            .size(px(200.))
                            .child(render_response_panel(window, cx, &state)),
                    ),
            )
    }
}

fn render_request_panel(window: &mut Window, cx: &mut App, state: &Entity<PageState>) -> impl IntoElement {
    let ps = state.read(cx);
    let tab = ps.request_tab;

    v_flex()
        .size_full()
        .border_1()
        .rounded_md()
        .child(
            TabBar::new("request-tabs")
                .selected_index(tab as usize)
                .on_click(window.listener_for(state, |s, i, _, _| {
                    s.request_tab = match i {
                        0 => RequestTab::Params,
                        1 => RequestTab::Headers,
                        _ => RequestTab::Body,
                    };
                }))
                .child(Tab::new().label("Params"))
                .child(Tab::new().label("Headers"))
                .child(Tab::new().label("Body")),
        )
        .child(
            div()
                .size_full()
                .p_2()
                .overflow_y_hidden()
                .child(match tab {
                    RequestTab::Params => render_kv_editor(window, cx, state, true).into_any_element(),
                    RequestTab::Headers => render_kv_editor(window, cx, state, false).into_any_element(),
                    RequestTab::Body => Input::new(&ps.body_state).h_full().into_any_element(),
                }),
        )
}

fn render_kv_editor(
    window: &mut Window,
    cx: &mut App,
    state: &Entity<PageState>,
    is_params: bool,
) -> impl IntoElement {
    let ps = state.read(cx);
    let items = if is_params { &ps.params } else { &ps.headers };

    v_flex()
        .size_full()
        .gap_1()
        .child(
            h_flex()
                .w_full()
                .gap_2()
                .child(div().w(px(16.)))
                .child(div().flex_1().text_sm().text_color(hsla(0., 0., 0.5, 1.)).child("Key"))
                .child(div().flex_1().text_sm().text_color(hsla(0., 0., 0.5, 1.)).child("Value"))
                .child(div().w(px(24.))),
        )
        .children(items.iter().enumerate().map(|(i, kv)| {
            h_flex()
                .w_full()
                .gap_2()
                .child(
                    div()
                        .w(px(16.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(12.))
                                .h(px(12.))
                                .rounded(px(2.))
                                .border_1()
                                .when(kv.enabled, |d| d.bg(hsla(0.6, 0.8, 0.5, 1.))),
                        ),
                )
                .child(Input::new(&kv.key).small().flex_1())
                .child(Input::new(&kv.value).small().flex_1())
                .child(
                    Button::new(("del", i))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Minus),
                )
        }))
        .child(
            Button::new("add-row")
                .ghost()
                .xsmall()
                .icon(IconName::Plus)
                .on_click(window.listener_for(state, move |s, _, window, cx| {
                    let kv = create_kv(window, cx);
                    if is_params {
                        s.params.push(kv);
                    } else {
                        s.headers.push(kv);
                    }
                })),
        )
}

fn render_response_panel(window: &mut Window, cx: &mut App, state: &Entity<PageState>) -> impl IntoElement {
    let ps = state.read(cx);
    let tab = ps.response_tab;

    v_flex()
        .size_full()
        .border_1()
        .rounded_md()
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .child(
                    TabBar::new("response-tabs")
                        .selected_index(tab as usize)
                        .on_click(window.listener_for(state, |s, i, _, _| {
                            s.response_tab = match i {
                                0 => ResponseTab::Body,
                                _ => ResponseTab::Headers,
                            };
                        }))
                        .child(Tab::new().label("Body"))
                        .child(Tab::new().label("Headers")),
                )
                .when_some(ps.response_status, |el, status| {
                    el.child(
                        div()
                            .px_2()
                            .text_sm()
                            .text_color(if status < 400 {
                                hsla(0.33, 0.8, 0.4, 1.0)
                            } else {
                                hsla(0.0, 0.8, 0.5, 1.0)
                            })
                            .child(format!("Status: {}", status)),
                    )
                }),
        )
        .child(
            div()
                .size_full()
                .p_2()
                .overflow_y_hidden()
                .child(match tab {
                    ResponseTab::Body => {
                        if ps.response_body.is_empty() {
                            "Response will appear here".into_any_element()
                        } else {
                            ps.response_body.clone().into_any_element()
                        }
                    }
                    ResponseTab::Headers => {
                        if ps.response_headers.is_empty() {
                            "No headers".into_any_element()
                        } else {
                            v_flex()
                                .gap_1()
                                .children(ps.response_headers.iter().map(|(k, v)| {
                                    h_flex()
                                        .gap_2()
                                        .child(div().text_sm().font_weight(FontWeight::BOLD).child(k.clone()))
                                        .child(div().text_sm().child(v.clone()))
                                }))
                                .into_any_element()
                        }
                    }
                }),
        )
}
