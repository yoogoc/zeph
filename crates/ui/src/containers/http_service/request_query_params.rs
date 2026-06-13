use super::http_state::HttpState;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Disableable, StyledExt};
use models::QueryParameter;

struct QueryParamRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
    enabled: bool,
    /// `Change` subscriptions on key/value that push edits back to `HttpState`.
    _subscriptions: [Subscription; 2],
}

pub struct RequestQueryParams {
    http_state: Entity<HttpState>,
    rows: Vec<QueryParamRow>,
    folder_params: Vec<QueryParameter>,
}

impl RequestQueryParams {
    pub fn new(
        http_state: Entity<HttpState>,
        params: Vec<QueryParameter>,
        folder_params: Vec<QueryParameter>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let rows = params
            .into_iter()
            .map(|p| {
                let key = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&p.key)
                        .placeholder("Key")
                });
                let value = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&p.value)
                        .placeholder("Value")
                });
                Self::make_row(key, value, p.enabled, cx)
            })
            .collect();

        Self {
            http_state,
            rows,
            folder_params,
        }
    }

    /// Builds a row, wiring `Change` subscriptions that sync edits to `HttpState`.
    fn make_row(
        key: Entity<InputState>,
        value: Entity<InputState>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> QueryParamRow {
        let key_sub = cx.subscribe(&key, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.sync(cx);
            }
        });
        let value_sub = cx.subscribe(&value, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.sync(cx);
            }
        });
        QueryParamRow {
            key,
            value,
            enabled,
            _subscriptions: [key_sub, value_sub],
        }
    }

    /// Pushes the current rows to `HttpState`.
    fn sync(&self, cx: &mut Context<Self>) {
        let params = self.params(cx);
        self.http_state
            .update(cx, |s, cx| s.set_query_params(params, cx));
    }

    pub fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("common.key")));
        let value = cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("common.value")));
        let row = Self::make_row(key, value, true, cx);
        self.rows.push(row);
        self.sync(cx);
        cx.notify();
    }

    pub fn params(&self, cx: &App) -> Vec<QueryParameter> {
        self.rows
            .iter()
            .map(|row| {
                QueryParameter::new(
                    row.key.read(cx).value().to_string(),
                    row.value.read(cx).value().to_string(),
                )
                .with_enabled(row.enabled)
            })
            .collect()
    }

    pub fn toggle_enabled(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get_mut(index) {
            row.enabled = !row.enabled;
            self.sync(cx);
            cx.notify();
        }
    }
}

impl Render for RequestQueryParams {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_1()
            // Display folder-inherited parameters with visual indicator
            .children(self.folder_params.iter().enumerate().map(|(i, param)| {
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Checkbox::new(format!("folder_param_enabled_{}", i))
                            .checked(param.enabled)
                            .disabled(true),
                    )
                    .child(
                        div().flex_1().child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0xf8f9fa))
                                .text_color(rgb(0x6c757d))
                                .text_sm()
                                .child(param.key.to_string()),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0xf8f9fa))
                                .text_color(rgb(0x6c757d))
                                .text_sm()
                                .child(param.value.to_string()),
                        ),
                    )
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(0x6c757d))
                            .child("📁"),
                    )
            }))
            // Display service-level parameters
            .children(self.rows.iter().enumerate().map(|(i, row)| {
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Checkbox::new(format!("param_enabled_{}", i))
                            .checked(row.enabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_enabled(i, cx);
                            })),
                    )
                    .child(div().flex_1().child(Input::new(&row.key)))
                    .child(div().flex_1().child(Input::new(&row.value)))
                    .child(
                        Button::new(format!("remove_param_{}", i))
                            .label("×")
                            .w(px(24.0))
                            .h(px(24.0))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.rows.remove(i);
                                this.sync(cx);
                                cx.notify();
                            })),
                    )
            }))
            .when(
                self.rows.is_empty() && self.folder_params.is_empty(),
                |el| {
                    el.child(
                        div()
                            .p_4()
                            .text_center()
                            .text_color(rgb(0x6c757d))
                            .text_sm()
                            .child(shared_t!("query_params.empty")),
                    )
                },
            )
    }
}
