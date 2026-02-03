//! CommonHeadersPanel component - manages folder common headers.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{h_flex, v_flex, Disableable, StyledExt};
use models::Header;
use std::collections::HashMap;

/// Events emitted by the common headers panel
#[derive(Debug, Clone)]
pub enum CommonHeadersPanelEvent {
    HeadersChanged(Vec<Header>),
}

/// Input row for header key-value pairs
struct HeaderInputRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
}

impl HeaderInputRow {
    fn new(
        header: Option<&Header>,
        window: &mut Window,
        cx: &mut Context<CommonHeadersPanel>,
    ) -> Self {
        let (key_value, value_value) = if let Some(header) = header {
            (header.key.to_string(), header.value.to_string())
        } else {
            (String::new(), String::new())
        };

        Self {
            key: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&key_value)
                    .placeholder(shared_t!("header.name"))
            }),
            value: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&value_value)
                    .placeholder(shared_t!("header.value"))
            }),
        }
    }

    fn to_header(&self, cx: &App) -> Header {
        Header {
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

/// Panel for managing common headers
pub struct CommonHeadersPanel {
    header_rows: Vec<HeaderInputRow>,
    stored_headers: Vec<Header>,
    needs_add_button: bool,

    // Error handling and validation
    is_saving: bool,
    save_error: Option<String>,
    validation_errors: HashMap<usize, String>,
}

impl EventEmitter<CommonHeadersPanelEvent> for CommonHeadersPanel {}

impl CommonHeadersPanel {
    pub fn new() -> Self {
        Self {
            header_rows: Vec::new(),
            stored_headers: Vec::new(),
            needs_add_button: true,
            is_saving: false,
            save_error: None,
            validation_errors: HashMap::new(),
        }
    }

    pub fn set_headers(&mut self, headers: Vec<Header>) {
        self.stored_headers = headers;
        self.header_rows.clear(); // Will be recreated in render
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
    fn add_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_row = HeaderInputRow::new(None, window, cx);
        self.header_rows.push(new_row);
        cx.notify();
    }

    fn remove_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.header_rows.len() {
            self.header_rows.remove(index);
            self.emit_headers_changed(cx);
            cx.notify();
        }
    }

    fn emit_headers_changed(&self, cx: &mut Context<Self>) {
        // Validate all headers before emitting
        let mut valid_headers = Vec::new();
        let mut has_errors = false;

        for (index, row) in self.header_rows.iter().enumerate() {
            if let Some(_error) = self.get_validation_error(index, cx) {
                has_errors = true;
                // Don't emit if there are validation errors
                continue;
            }

            if row.is_valid(cx) {
                valid_headers.push(row.to_header(cx));
            }
        }

        // Only emit if no validation errors
        if !has_errors {
            cx.emit(CommonHeadersPanelEvent::HeadersChanged(valid_headers));
        }
    }

    /// Enhanced header name validation
    fn validate_header_name(name: &str) -> Result<(), SharedString> {
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(shared_t!("header.validate.name.not_empty"));
        }

        if trimmed.len() > 100 {
            return Err(shared_t!("header.validate.name.too_long"));
        }

        // HTTP header name validation (RFC 7230)
        if !trimmed.chars().all(|c| {
            c.is_ascii()
                && (c.is_alphanumeric() || c == '-' || c == '_')
                && !c.is_control()
                && c != ':'
                && c != ' '
        }) {
            return Err(shared_t!("header.validate.name.syntax_error"));
        }

        // Check for reserved headers
        let lower_name = trimmed.to_lowercase();
        if matches!(
            lower_name.as_str(),
            "host"
                | "connection"
                | "content-length"
                | "transfer-encoding"
                | "upgrade"
                | "proxy-connection"
                | "te"
                | "trailer"
        ) {
            return Err(shared_t!("header.validate.name.reserved", name = trimmed));
        }

        Ok(())
    }

    /// Enhanced header value validation
    fn validate_header_value(value: &str) -> Result<(), SharedString> {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Err(shared_t!("header.validate.name.not_empty"));
        }

        if trimmed.len() > 8192 {
            return Err(shared_t!("header.validate.name.too_long"));
        }

        // HTTP header value validation (RFC 7230)
        if !trimmed
            .chars()
            .all(|c| c.is_ascii() && !c.is_control() || c == '\t')
        {
            return Err(shared_t!("header.validate.name.syntax_error"));
        }

        Ok(())
    }

    /// Check for duplicate header names
    fn check_duplicate_headers(&self, cx: &App) -> HashMap<usize, SharedString> {
        let mut name_counts = HashMap::new();
        let mut errors = HashMap::new();

        // Count occurrences of each header name
        for (index, row) in self.header_rows.iter().enumerate() {
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
                    errors.insert(index, shared_t!("duplicate", name = name));
                }
            }
        }

        errors
    }

    fn get_validation_error(&self, index: usize, cx: &App) -> Option<SharedString> {
        if let Some(row) = self.header_rows.get(index) {
            let key_value = row.key.read(cx).value();
            let value_value = row.value.read(cx).value();
            let key = key_value.trim();
            let value = value_value.trim();

            // Check the header name
            if let Err(error) = Self::validate_header_name(key) {
                return Some(error);
            }

            // Check header value
            if let Err(error) = Self::validate_header_value(value) {
                return Some(error);
            }

            // Check for duplicates
            let duplicate_errors = self.check_duplicate_headers(cx);
            if let Some(error) = duplicate_errors.get(&index) {
                return Some(error.into());
            }
        }
        None
    }
}

impl Render for CommonHeadersPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Initialize header rows from stored headers if needed
        if self.header_rows.is_empty() && !self.stored_headers.is_empty() {
            for header in &self.stored_headers {
                let row = HeaderInputRow::new(Some(header), window, cx);
                self.header_rows.push(row);
            }
        }

        // Handle add button request
        if self.needs_add_button {
            let row = HeaderInputRow::new(None, window, cx);
            self.header_rows.push(row);
            self.needs_add_button = false;
        }

        // Ensure we have at least one empty row if no headers exist
        if self.header_rows.is_empty() {
            let row = HeaderInputRow::new(None, window, cx);
            self.header_rows.push(row);
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
                            .child(shared_t!("folders.tab.common_headers")),
                    )
                    .child(
                        Button::new("add_header")
                            .primary()
                            .label(shared_t!("header.add"))
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
                            .text_color(rgb(0x666666))
                            .child(shared_t!("header.description")),
                    )
                    .child(div().v_flex().gap_2().children(
                        self.header_rows.iter().enumerate().map(|(index, row)| {
                            let validation_error = self.get_validation_error(index, cx);
                            let has_error = validation_error.is_some();

                            v_flex()
                                .gap_1()
                                .child(
                                    h_flex()
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
                                            Button::new(format!("delete_header_{}", index))
                                                .ghost()
                                                .compact()
                                                .icon(gpui_component::IconName::Delete)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.remove_header(index, cx);
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
                        Button::new("save_headers")
                            .primary()
                            .when(self.is_saving, |btn| btn.disabled(true))
                            .label(if self.is_saving {
                                shared_t!("common.saving")
                            } else {
                                shared_t!("common.save")
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                // Validate all headers before saving
                                let mut has_errors = false;
                                for i in 0..this.header_rows.len() {
                                    if this.get_validation_error(i, cx).is_some() {
                                        has_errors = true;
                                        break;
                                    }
                                }

                                if !has_errors {
                                    this.set_saving(true, cx);
                                    this.emit_headers_changed(cx);
                                } else {
                                    this.set_save_error(t!("common.save_invalid").to_string(), cx);
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
}
