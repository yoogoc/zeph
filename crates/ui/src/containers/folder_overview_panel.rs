//! FolderOverviewPanel component - displays basic folder information with edit support.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{Disableable, StyledExt};
use models::Folder;
use std::collections::HashMap;

/// Events emitted by the folder overview panel
#[derive(Debug, Clone)]
pub enum FolderOverviewPanelEvent {
    SaveChanges(String, Option<String>), // name, description
}

/// Overview panel showing basic folder information
pub struct FolderOverviewPanel {
    folder: Option<Folder>,
    is_editing: bool,
    name_input: Option<Entity<InputState>>,
    description_input: Option<Entity<InputState>>,
    validation_errors: HashMap<String, SharedString>,
    is_saving: bool,
}

impl EventEmitter<FolderOverviewPanelEvent> for FolderOverviewPanel {}

impl FolderOverviewPanel {
    pub fn new() -> Self {
        Self {
            folder: None,
            is_editing: false,
            name_input: None,
            description_input: None,
            validation_errors: HashMap::new(),
            is_saving: false,
        }
    }

    pub fn set_folder(&mut self, folder: Folder) {
        self.folder = Some(folder);
        // Reset edit mode when folder changes
        self.is_editing = false;
        self.name_input = None;
        self.description_input = None;
        self.validation_errors.clear();
        self.is_saving = false;
    }

    /// Enter edit mode - replace read-only fields with input boxes
    pub fn enter_edit_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(folder) = &self.folder {
            // Create input fields with current values
            self.name_input = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&folder.name)
                    .placeholder(shared_t!("folders.fields.name"))
            }));

            let description_value = folder
                .description
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_default();

            self.description_input = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&description_value)
                    .placeholder(shared_t!("folders.fields.description"))
                    .multi_line(true)
            }));

            self.is_editing = true;
            cx.notify();
        }
    }

    /// Exit edit mode - restore read-only display
    pub fn exit_edit_mode(&mut self, cx: &mut Context<Self>) {
        self.is_editing = false;
        self.name_input = None;
        self.description_input = None;
        self.validation_errors.clear();
        self.is_saving = false;
        cx.notify();
    }

    /// Validate inputs before saving
    fn validate_inputs(&self, cx: &App) -> Result<(), HashMap<String, SharedString>> {
        let mut errors = HashMap::new();

        // Validate name
        if let Some(name_input) = &self.name_input {
            let name_value = name_input.read(cx).value();
            let name = name_value.trim();

            if name.is_empty() {
                errors.insert(
                    "name".to_string(),
                    shared_t!("folder.validate.name.not_empty"),
                );
            } else if name.len() > 255 {
                errors.insert(
                    "name".to_string(),
                    shared_t!("folder.validate.name.too_long"),
                );
            }
        }

        // Validate description
        if let Some(desc_input) = &self.description_input {
            let desc_value = desc_input.read(cx).value();
            let desc = desc_value.trim();

            if desc.len() > 1000 {
                errors.insert(
                    "description".to_string(),
                    shared_t!("folder.validate.description.too_long"),
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Save changes to folder
    fn save_changes(&mut self, cx: &mut Context<Self>) {
        // Validate inputs first
        match self.validate_inputs(cx) {
            Ok(()) => {
                // Clear any previous validation errors
                self.validation_errors.clear();

                // Get the values from inputs
                if let (Some(name_input), Some(desc_input)) =
                    (&self.name_input, &self.description_input)
                {
                    let name_value = name_input.read(cx).value();
                    let name = name_value.trim().to_string();

                    let desc_value = desc_input.read(cx).value();
                    let desc = desc_value.trim();
                    let description = if desc.is_empty() {
                        None
                    } else {
                        Some(desc.to_string())
                    };

                    // Set saving state
                    self.is_saving = true;
                    cx.notify();

                    // Emit event to parent
                    cx.emit(FolderOverviewPanelEvent::SaveChanges(name, description));
                }
            }
            Err(errors) => {
                // Store validation errors and notify
                self.validation_errors = errors;
                cx.notify();
            }
        }
    }

    /// Set saving state (called by parent after save completes)
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.is_saving = saving;
        cx.notify();
    }

    /// Handle save success (called by parent)
    pub fn handle_save_success(&mut self, cx: &mut Context<Self>) {
        self.is_saving = false;
        self.exit_edit_mode(cx);
    }

    /// Handle save error (called by parent)
    pub fn handle_save_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.is_saving = false;
        self.validation_errors
            .insert("save".to_string(), error.into());
        cx.notify();
    }

    fn format_timestamp(timestamp: i64) -> String {
        // TODO Convert timestamp to readable format
        // In a real implementation, you'd use chrono or similar
        format!("Timestamp: {}", timestamp)
    }
}

impl Render for FolderOverviewPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(folder) = &self.folder else {
            return div().child(shared_t!("folders.no_info")).into_any_element();
        };

        let folder_name = folder.name.to_string();
        let folder_type_str = match folder.folder_type {
            models::FolderType::Service => shared_t!("folders.type.service"),
            models::FolderType::Schema => shared_t!("folders.type.schema"),
        };
        let description = folder.description.clone();
        let created_at = folder.created_at;
        let updated_at = folder.updated_at;
        let headers_count = folder.common_headers.len();
        let params_count = folder.common_search_params.len();

        div()
            .v_flex()
            .gap_4()
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .child(shared_t!("folders.info")),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .when(!self.is_editing, |container| {
                                        // Edit button - only show when not in edit mode
                                        container.child(
                                            Button::new("edit_folder")
                                                .primary()
                                                .compact()
                                                .label(shared_t!("folder.overview.edit"))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.enter_edit_mode(window, cx);
                                                })),
                                        )
                                    })
                                    .when(self.is_editing, |container| {
                                        // Save and Cancel buttons - only show in edit mode
                                        container
                                            .child(
                                                Button::new("cancel_edit")
                                                    .ghost()
                                                    .compact()
                                                    .label(shared_t!("common.cancel"))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.exit_edit_mode(cx);
                                                    })),
                                            )
                                            .child(
                                                Button::new("save_folder")
                                                    .primary()
                                                    .compact()
                                                    .when(self.is_saving, |btn| btn.disabled(true))
                                                    .label(if self.is_saving {
                                                        shared_t!("common.saving")
                                                    } else {
                                                        shared_t!("common.save")
                                                    })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.save_changes(cx);
                                                    })),
                                            )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_3()
                            .when(self.validation_errors.contains_key("save"), |container| {
                                // Show save error at the top
                                container.child(
                                    div()
                                        .p_2()
                                        .bg(gpui::rgb(0xfef2f2))
                                        .border_1()
                                        .border_color(gpui::rgb(0xfecaca))
                                        .rounded_md()
                                        .text_sm()
                                        .text_color(gpui::rgb(0xdc2626))
                                        .child(self.validation_errors.get("save").unwrap().clone()),
                                )
                            })
                            .child(
                                // Name field - editable or read-only
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_medium()
                                            .child(format!("{}:", t!("folders.fields.name"))),
                                    )
                                    .child(if self.is_editing {
                                        if let Some(input) = &self.name_input {
                                            div()
                                                .v_flex()
                                                .gap_1()
                                                .child(Input::new(input))
                                                .when(
                                                    self.validation_errors.contains_key("name"),
                                                    |container| {
                                                        container.child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(gpui::rgb(0xdc2626))
                                                                .child(
                                                                    self.validation_errors
                                                                        .get("name")
                                                                        .unwrap()
                                                                        .clone(),
                                                                ),
                                                        )
                                                    },
                                                )
                                                .into_any_element()
                                        } else {
                                            div().child("Error: No input").into_any_element()
                                        }
                                    } else {
                                        div().child(folder_name.clone()).into_any_element()
                                    }),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_medium()
                                            .child(format!("{}:", t!("folders.fields.type"))),
                                    )
                                    .child(div().child(folder_type_str)),
                            )
                            .child(
                                // Description field - editable or read-only
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div().font_medium().child(format!(
                                            "{}:",
                                            t!("folders.fields.description")
                                        )),
                                    )
                                    .child(if self.is_editing {
                                        if let Some(input) = &self.description_input {
                                            div()
                                                .v_flex()
                                                .gap_1()
                                                .child(Input::new(input))
                                                .when(
                                                    self.validation_errors
                                                        .contains_key("description"),
                                                    |container| {
                                                        container.child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(gpui::rgb(0xdc2626))
                                                                .child(
                                                                    self.validation_errors
                                                                        .get("description")
                                                                        .unwrap()
                                                                        .clone(),
                                                                ),
                                                        )
                                                    },
                                                )
                                                .into_any_element()
                                        } else {
                                            div().child("Error: No input").into_any_element()
                                        }
                                    } else {
                                        div()
                                            .child(description.unwrap_or_else(|| {
                                                shared_t!("folders.no_description")
                                            }))
                                            .into_any_element()
                                    }),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_medium()
                                            .child(format!("{}:", t!("folders.fields.created_at"))),
                                    )
                                    .child(div().child(Self::format_timestamp(created_at))),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_medium()
                                            .child(format!("{}:", t!("folders.fields.updated_at"))),
                                    )
                                    .child(div().child(Self::format_timestamp(updated_at))),
                            ),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child(shared_t!("folders.statistics")),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        div().font_medium().child(format!(
                                            "{}:",
                                            t!("folders.tab.common_headers")
                                        )),
                                    )
                                    .child(div().child(format!("{}", headers_count))),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        div().font_medium().child(format!(
                                            "{}:",
                                            t!("folders.tab.search_parameters")
                                        )),
                                    )
                                    .child(div().child(format!("{}", params_count))),
                            ),
                    ),
            )
            .into_any_element()
    }
}
