//! SearchParamsPanel component - manages folder search parameters.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{Disableable, StyledExt};
use models::SearchParameter;
use std::collections::HashMap;

/// Events emitted by the search params panel
#[derive(Debug, Clone)]
pub enum SearchParamsPanelEvent {
    ParamsChanged(Vec<SearchParameter>),
}

/// Input row for search parameter key-value pairs
struct ParamInputRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
}

impl ParamInputRow {
    fn new(
        param: Option<&SearchParameter>,
        window: &mut Window,
        cx: &mut Context<SearchParamsPanel>,
    ) -> Self {
        let (key_value, value_value) = if let Some(param) = param {
            (param.key.to_string(), param.value.to_string())
        } else {
            (String::new(), String::new())
        };

        Self {
            key: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&key_value)
                    .placeholder(shared_t!("parameter.name"))
            }),
            value: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&value_value)
                    .placeholder(shared_t!("parameter.value"))
            }),
        }
    }

    fn to_param(&self, cx: &App) -> SearchParameter {
        SearchParameter {
            key: self.key.read(cx).value().to_string().into(),
            value: self.value.read(cx).value().to_string().into(),
        }
    }

    fn is_valid(&self, cx: &App) -> bool {
        let key_value = self.key.read(cx).value();
        let value_value = self.value.read(cx).value();
        let key = key_value.trim();
        let value = value_value.trim();
        !key.is_empty() && !value.is_empty()
    }
}

/// Panel for managing search parameters
pub struct SearchParamsPanel {
    param_rows: Vec<ParamInputRow>,
    stored_params: Vec<SearchParameter>,
    needs_add_button: bool,

    // Error handling and validation
    is_saving: bool,
    save_error: Option<String>,
    validation_errors: HashMap<usize, String>,
}

impl EventEmitter<SearchParamsPanelEvent> for SearchParamsPanel {}

impl SearchParamsPanel {
    pub fn new() -> Self {
        Self {
            param_rows: Vec::new(),
            stored_params: Vec::new(),
            needs_add_button: true,
            is_saving: false,
            save_error: None,
            validation_errors: HashMap::new(),
        }
    }

    pub fn set_params(&mut self, params: Vec<SearchParameter>) {
        self.stored_params = params;
        self.param_rows.clear(); // Will be recreated in render
        self.validation_errors.clear();
        self.save_error = None;
    }

    /// Set saving state
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.is_saving = saving;
        if saving {
            self.save_error = None;
        }
        cx.notify();
    }

    /// Set save error
    pub fn set_save_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.save_error = Some(error);
        self.is_saving = false;
        cx.notify();
    }

    #[allow(unused)]
    fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_row = ParamInputRow::new(None, window, cx);
        self.param_rows.push(new_row);
        cx.notify();
    }

    fn remove_param(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.param_rows.len() {
            self.param_rows.remove(index);
            self.emit_params_changed(cx);
            cx.notify();
        }
    }

    fn emit_params_changed(&self, cx: &mut Context<Self>) {
        // Validate all parameters before emitting
        let mut valid_params = Vec::new();
        let mut has_errors = false;

        for (index, row) in self.param_rows.iter().enumerate() {
            if let Some(_error) = self.get_validation_error(index, cx) {
                has_errors = true;
                // Don't emit if there are validation errors
                continue;
            }

            if row.is_valid(cx) {
                valid_params.push(row.to_param(cx));
            }
        }

        // Only emit if no validation errors
        if !has_errors {
            cx.emit(SearchParamsPanelEvent::ParamsChanged(valid_params));
        }
    }

    /// Enhanced parameter name validation
    fn validate_param_name(name: &str) -> Result<(), SharedString> {
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(shared_t!("parameter.validate.name.not_empty"));
        }

        if trimmed.len() > 100 {
            return Err(shared_t!("parameter.validate.name.too_long"));
        }

        // URL parameter name validation
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(shared_t!("parameter.validate.name.syntax_error"));
        }

        // Check for reserved parameter names that might conflict with common URL patterns
        if matches!(
            trimmed.to_lowercase().as_str(),
            "callback" | "jsonp" | "_" | "__proto__" | "constructor"
        ) {
            return Err(shared_t!("parameter.validate.name.reserved"));
        }

        Ok(())
    }

    /// Enhanced parameter value validation
    fn validate_param_value(value: &str) -> Result<(), SharedString> {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Err(shared_t!("parameter.validate.value.not_empty"));
        }

        if trimmed.len() > 2048 {
            return Err(shared_t!("parameter.validate.value.too_long"));
        }

        // Check for potentially problematic characters
        if trimmed.contains('\n') || trimmed.contains('\r') {
            return Err(shared_t!("parameter.validate.value.syntax_error"));
        }

        Ok(())
    }

    /// Check for duplicate parameter names
    fn check_duplicate_params(&self, cx: &App) -> HashMap<usize, SharedString> {
        let mut name_counts = HashMap::new();
        let mut errors = HashMap::new();

        // Count occurrences of each parameter name
        for (index, row) in self.param_rows.iter().enumerate() {
            let key_value = row.key.read(cx).value();
            let key = key_value.trim().to_lowercase();

            if !key.is_empty() {
                name_counts.entry(key).or_insert_with(Vec::new).push(index);
            }
        }

        // Mark duplicates as errors
        for (name, indices) in name_counts {
            if indices.len() > 1 {
                for &index in &indices {
                    errors.insert(
                        index,
                        shared_t!("parameter.validate.name.duplicate", name = name),
                    );
                }
            }
        }

        errors
    }

    fn get_validation_error(&self, index: usize, cx: &App) -> Option<SharedString> {
        if let Some(row) = self.param_rows.get(index) {
            let key_value = row.key.read(cx).value();
            let value_value = row.value.read(cx).value();
            let key = key_value.trim();
            let value = value_value.trim();

            // Check parameter name
            if let Err(error) = Self::validate_param_name(key) {
                return Some(error);
            }

            // Check parameter value
            if let Err(error) = Self::validate_param_value(value) {
                return Some(error);
            }

            // Check for duplicates
            let duplicate_errors = self.check_duplicate_params(cx);
            if let Some(error) = duplicate_errors.get(&index) {
                return Some(error.clone());
            }
        }
        None
    }
}

impl Render for SearchParamsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Initialize param rows from stored params if needed
        if self.param_rows.is_empty() && !self.stored_params.is_empty() {
            for param in &self.stored_params {
                let row = ParamInputRow::new(Some(param), window, cx);
                self.param_rows.push(row);
            }
        }

        // Handle add button request
        if self.needs_add_button {
            let row = ParamInputRow::new(None, window, cx);
            self.param_rows.push(row);
            self.needs_add_button = false;
        }

        // Ensure we have at least one empty row if no params exist
        if self.param_rows.is_empty() {
            let row = ParamInputRow::new(None, window, cx);
            self.param_rows.push(row);
        }

        div()
            .v_flex()
            .gap_4()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child(shared_t!("folders.tab.search_parameters")),
                    )
                    .child(
                        Button::new("add_param")
                            .primary()
                            .label(shared_t!("parameter.add"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.needs_add_button = true;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::rgb(0x666666))
                            .child(shared_t!("parameter.description")),
                    )
                    .child(div().v_flex().gap_2().children(
                        self.param_rows.iter().enumerate().map(|(index, row)| {
                            let validation_error = self.get_validation_error(index, cx);
                            let has_error = validation_error.is_some();

                            div()
                                .v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap_3()
                                        .p_3()
                                        .bg(gpui::rgb(0xffffff))
                                        .border_1()
                                        .when(has_error, |div| {
                                            div.border_color(gpui::rgb(0xdc3545))
                                        })
                                        .when(!has_error, |div| {
                                            div.border_color(gpui::rgb(0xe0e0e0))
                                        })
                                        .rounded_md()
                                        .child(
                                            div()
                                                .flex_1()
                                                .h_flex()
                                                .gap_3()
                                                .child(
                                                    div().w(px(200.0)).child(Input::new(&row.key)),
                                                )
                                                .child(
                                                    div().flex_1().child(Input::new(&row.value)),
                                                ),
                                        )
                                        .child(
                                            Button::new(format!("delete_param_{}", index))
                                                .ghost()
                                                .compact()
                                                .icon(gpui_component::IconName::Delete)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.remove_param(index, cx);
                                                })),
                                        ),
                                )
                                .when(has_error, |container| {
                                    container.child(
                                        div()
                                            .text_xs()
                                            .text_color(gpui::rgb(0xdc3545))
                                            .pl_3()
                                            .child(validation_error.unwrap()),
                                    )
                                })
                        }),
                    )),
            )
            .child(
                div()
                    .h_flex()
                    .justify_end()
                    .gap_2()
                    .when(self.save_error.is_some(), |container| {
                        container.child(
                            div()
                                .text_xs()
                                .text_color(gpui::rgb(0xdc3545))
                                .child(self.save_error.as_ref().unwrap().clone()),
                        )
                    })
                    .child(
                        Button::new("save_params")
                            .primary()
                            .when(self.is_saving, |btn| btn.disabled(true))
                            .label(if self.is_saving {
                                shared_t!("common.saving")
                            } else {
                                shared_t!("common.save")
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                // Validate all parameters before saving
                                let mut has_errors = false;
                                for i in 0..this.param_rows.len() {
                                    if this.get_validation_error(i, cx).is_some() {
                                        has_errors = true;
                                        break;
                                    }
                                }

                                if !has_errors {
                                    this.set_saving(true, cx);
                                    this.emit_params_changed(cx);
                                } else {
                                    this.set_save_error(t!("common.save_invalid").to_string(), cx);
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
}
