use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::{Disableable, StyledExt};
use models::QueryParameter;

struct QueryParamRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
    enabled: bool,
    #[allow(unused)]
    is_folder_param: bool,
}

pub struct RequestQueryParams {
    rows: Vec<QueryParamRow>,
    folder_params: Vec<QueryParameter>,
}

impl RequestQueryParams {
    pub fn new(
        params: Vec<QueryParameter>,
        folder_params: Vec<QueryParameter>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let rows = params
            .into_iter()
            .map(|p| QueryParamRow {
                key: cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&p.key)
                        .placeholder("Key")
                }),
                value: cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&p.value)
                        .placeholder("Value")
                }),
                enabled: p.enabled,
                is_folder_param: false,
            })
            .collect();

        Self {
            rows,
            folder_params,
        }
    }

    pub fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rows.push(QueryParamRow {
            key: cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("common.key"))),
            value: cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("common.value"))),
            enabled: true,
            is_folder_param: false,
        });
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
