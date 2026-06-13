use super::http_state::{HttpState, RequestTab};
use super::param_utils::detect_path_params;
use super::url_utils;
use crate::http_service::method_selector::MethodSelector;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{h_flex, v_flex, Disableable, IconName};

/// Pure-render request panel: reads `HttpState` and drives it via `update`.
pub struct RequestPanel {
    http_state: Entity<HttpState>,
    method_selector: Entity<MethodSelector>,
}

impl RequestPanel {
    pub fn new(http_state: Entity<HttpState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hs = http_state.clone();
        let method_selector = cx.new(|cx| MethodSelector::new(hs, window, cx));

        Self {
            http_state,
            method_selector,
        }
    }
}

impl Render for RequestPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.http_state.read(cx);
        let active_tab = state.active_tab;
        let is_loading = state.is_loading;
        let use_base_url = state.use_base_url;
        let is_modified = state.is_modified(cx);
        let url = state.url.clone();

        let body_panel = state.body_panel.clone();
        let headers_panel = state.headers_panel.clone();
        let query_params_panel = state.query_params_panel.clone();
        let path_params_panel = state.path_params_panel.clone();

        let base_url = state.base_url(cx);
        let url_value = url.read(cx).value().to_string();
        let show_final_url_preview =
            use_base_url && !base_url.is_empty() && !url_utils::is_absolute_url(&url_value);
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
                            .child(Input::new(&url))
                            .child(
                                Button::new("toggle_base_url")
                                    .icon(IconName::Globe)
                                    .ghost()
                                    .tooltip(tooltip_text)
                                    .when(use_base_url, |btn| btn.primary())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.http_state
                                            .update(cx, |s, cx| s.toggle_use_base_url(cx));
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
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.http_state
                                    .update(cx, |s, cx| s.send_request(window, cx));
                            })),
                    )
                    .child(
                        Button::new("save_request")
                            .label(shared_t!("common.save"))
                            .disabled(!is_modified)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.http_state.update(cx, |s, cx| s.save(cx));
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child({
                        let mut tab_bar =
                            TabBar::new("request_tabs").with_variant(TabVariant::Underline);

                        if has_path_params {
                            tab_bar = tab_bar.child(Tab::new().label(shared_t!("request.path")));
                        }

                        tab_bar = tab_bar
                            .child(Tab::new().label(shared_t!("request.query")))
                            .child(Tab::new().label(shared_t!("request.body")))
                            .child(Tab::new().label(shared_t!("request.header")));

                        tab_bar
                            .selected_index(if has_path_params {
                                active_tab as usize
                            } else {
                                match active_tab {
                                    RequestTab::Query => 0,
                                    RequestTab::Body => 1,
                                    RequestTab::Headers => 2,
                                    _ => 0,
                                }
                            })
                            .on_click(cx.listener(move |this, ix: &usize, _, cx| {
                                let tab = if has_path_params {
                                    match *ix {
                                        0 => RequestTab::Path,
                                        1 => RequestTab::Query,
                                        2 => RequestTab::Body,
                                        3 => RequestTab::Headers,
                                        _ => RequestTab::Path,
                                    }
                                } else {
                                    match *ix {
                                        0 => RequestTab::Query,
                                        1 => RequestTab::Body,
                                        2 => RequestTab::Headers,
                                        _ => RequestTab::Query,
                                    }
                                };
                                this.http_state
                                    .update(cx, |s, cx| s.set_active_tab(tab, cx));
                            }))
                            .suffix(match active_tab {
                                RequestTab::Headers => Button::new("add_header")
                                    .icon(IconName::Plus)
                                    .label(shared_t!("request.add_header"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.http_state
                                            .update(cx, |s, cx| s.add_header(window, cx));
                                    }))
                                    .into_any_element(),
                                RequestTab::Query => Button::new("add_query_param")
                                    .icon(IconName::Plus)
                                    .label(shared_t!("request.add_param"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.http_state
                                            .update(cx, |s, cx| s.add_query_param(window, cx));
                                    }))
                                    .into_any_element(),
                                _ => div().into_any_element(),
                            })
                    })
                    .when(active_tab == RequestTab::Body, |el| el.child(body_panel))
                    .when(active_tab == RequestTab::Headers, |el| {
                        el.child(headers_panel)
                    })
                    .when(active_tab == RequestTab::Query, |el| {
                        el.child(query_params_panel)
                    })
                    .when(active_tab == RequestTab::Path && has_path_params, |el| {
                        el.child(path_params_panel)
                    }),
            )
    }
}
