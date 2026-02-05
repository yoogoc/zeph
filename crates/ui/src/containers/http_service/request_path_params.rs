use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputState};
use gpui_component::StyledExt;
use models::PathParameter;

use super::param_utils::detect_path_params;

struct PathParamRow {
    name: SharedString,
    value: Entity<InputState>,
}

pub struct RequestPathParams {
    rows: Vec<PathParamRow>,
}

impl RequestPathParams {
    pub fn new(params: Vec<PathParameter>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let rows = params
            .into_iter()
            .map(|p| PathParamRow {
                name: p.name.clone(),
                value: cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&p.value)
                        .placeholder("Value")
                }),
            })
            .collect();

        Self { rows }
    }

    pub fn params(&self, cx: &App) -> Vec<PathParameter> {
        self.rows
            .iter()
            .map(|row| PathParameter::new(row.name.clone(), row.value.read(cx).value().to_string()))
            .collect()
    }

    /// Updates the path parameters based on URL changes
    /// Detects new placeholders and removes old ones
    pub fn update_from_url(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        let detected_params = detect_path_params(url);

        // Create a map of existing parameter values
        let existing_values: std::collections::HashMap<String, String> = self
            .rows
            .iter()
            .map(|row| (row.name.to_string(), row.value.read(cx).value().to_string()))
            .collect();

        // Create new rows based on detected parameters
        self.rows = detected_params
            .into_iter()
            .map(|name| {
                let value = existing_values.get(&name).cloned().unwrap_or_default();
                PathParamRow {
                    name: name.clone().into(),
                    value: cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(&value)
                            .placeholder("Value")
                    }),
                }
            })
            .collect();

        cx.notify();
    }
}

impl Render for RequestPathParams {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_1()
            .children(self.rows.iter().map(|row| {
                let has_value = !row.value.read(cx).value().is_empty();
                let is_required = true; // All path parameters are required

                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    // Read-only name label
                    .child(
                        div().flex_1().child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(gpui::rgb(0xf8f9fa))
                                .text_color(gpui::rgb(0x495057))
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{{{}}}", row.name)),
                        ),
                    )
                    // Value input
                    .child(div().flex_1().child(Input::new(&row.value)).when(
                        !has_value && is_required,
                        |el| {
                            el.child(
                                div()
                                    .absolute()
                                    .top(px(-20.0))
                                    .left_0()
                                    .text_xs()
                                    .text_color(gpui::rgb(0xdc3545))
                                    .child(shared_t!("path_params.required")),
                            )
                        },
                    ))
                    // Required indicator
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(if has_value {
                                gpui::rgb(0x28a745)
                            } else {
                                gpui::rgb(0xdc3545)
                            })
                            .child(if has_value { "✓" } else { "!" }),
                    )
            }))
            .when(self.rows.is_empty(), |el| {
                el.child(
                    div()
                        .p_4()
                        .text_center()
                        .text_color(gpui::rgb(0x6c757d))
                        .text_sm()
                        .child(shared_t!("path_params.empty")),
                )
            })
    }
}
