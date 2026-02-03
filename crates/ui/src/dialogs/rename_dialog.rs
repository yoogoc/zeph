use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{IconName, StyledExt};

/// Type of entity being renamed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    Service,
    Folder,
}

impl EntityType {
    /// Get the display name for the entity type
    pub fn display_name(&self) -> &'static str {
        match self {
            EntityType::Service => "Service",
            EntityType::Folder => "Folder",
        }
    }

    /// Get the localized title for the rename dialog
    pub fn dialog_title(&self) -> &'static str {
        match self {
            EntityType::Service => "重命名 Service",
            EntityType::Folder => "重命名 Folder",
        }
    }
}

/// Events emitted by the RenameDialog
#[derive(Debug, Clone)]
pub enum RenameDialogEvent {
    /// User confirmed the rename with the new name
    Confirm(SharedString),
    /// User cancelled the rename operation
    Cancel,
}

/// Validation error types for name input
/// Implements Requirements 6.1, 6.2 for input validation
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Name is empty or contains only whitespace
    EmptyName,
    /// Name exceeds maximum length (255 characters)
    TooLong,
    /// Name contains invalid characters
    InvalidCharacters(String),
}

impl ValidationError {
    /// Get the error message for display
    pub fn message(&self) -> String {
        match self {
            ValidationError::EmptyName => "名称不能为空".to_string(),
            ValidationError::TooLong => "名称长度不能超过 255 个字符".to_string(),
            ValidationError::InvalidCharacters(chars) => {
                format!("名称包含非法字符: {}", chars)
            }
        }
    }
}

/// A reusable dialog component for renaming Services and Folders
/// Implements Requirements 1.2, 2.2, 6.1, 6.2
pub struct RenameDialog {
    /// Input field for the new name
    input: Entity<InputState>,
    /// Original name (for display and comparison)
    #[allow(dead_code)]
    original_name: SharedString,
    /// Type of entity being renamed
    entity_type: EntityType,
    /// Current validation error, if any
    validation_error: Option<ValidationError>,
}

impl EventEmitter<RenameDialogEvent> for RenameDialog {}

impl RenameDialog {
    /// Create a new RenameDialog
    ///
    /// # Arguments
    /// * `original_name` - The current name of the entity
    /// * `entity_type` - The type of entity being renamed (Service or Folder)
    /// * `window` - The window context
    /// * `cx` - The component context
    pub fn new(
        original_name: SharedString,
        entity_type: EntityType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Create input field pre-filled with the original name
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));

        // Set the initial value to the original name
        let name_string = original_name.to_string();
        input.update(cx, |state, cx| {
            state.set_value(name_string, window, cx);
        });

        Self {
            input,
            original_name,
            entity_type,
            validation_error: None,
        }
    }

    /// Validate the input name
    /// Implements Requirements 6.1, 6.2 for name validation
    ///
    /// Returns Ok(()) if valid, or Err(ValidationError) if invalid
    fn validate_name(&self, name: &str) -> Result<(), ValidationError> {
        // Check for empty or whitespace-only names
        // Implements Requirement 6.1
        if name.trim().is_empty() {
            return Err(ValidationError::EmptyName);
        }

        // Check for maximum length (255 characters)
        // Implements Requirement 6.2
        if name.len() > 255 {
            return Err(ValidationError::TooLong);
        }

        // Check for invalid characters (optional - can be extended)
        // For now, we'll allow most characters but could add restrictions
        // like no control characters, etc.
        let invalid_chars: Vec<char> = name
            .chars()
            .filter(|c| c.is_control() && *c != '\n' && *c != '\r' && *c != '\t')
            .collect();

        if !invalid_chars.is_empty() {
            let chars_str = invalid_chars
                .iter()
                .map(|c| format!("U+{:04X}", *c as u32))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(ValidationError::InvalidCharacters(chars_str));
        }

        Ok(())
    }

    /// Handle the confirm action
    fn handle_confirm(&mut self, cx: &mut Context<Self>) {
        let new_name = self.input.read(cx).value().to_string();

        // Validate the name
        match self.validate_name(&new_name) {
            Ok(()) => {
                // Clear any previous errors
                self.validation_error = None;
                // Emit the confirm event with the new name
                cx.emit(RenameDialogEvent::Confirm(new_name.into()));
            }
            Err(error) => {
                // Store the validation error for display
                self.validation_error = Some(error);
                cx.notify();
            }
        }
    }

    /// Handle the cancel action
    fn handle_cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(RenameDialogEvent::Cancel);
    }

    /// Render the dialog header
    fn render_header(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_4()
            .border_b_1()
            .border_color(rgb(0xe9ecef))
            .h_flex()
            .items_center()
            .gap_2()
            .child(
                gpui_component::Icon::new(IconName::File)
                    .size_4()
                    .text_color(rgb(0x007bff)),
            )
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(rgb(0x212529))
                    .child(self.entity_type.dialog_title()),
            )
    }

    /// Render the input field with validation error display
    /// Implements Requirements 6.1, 6.2, 6.4 for error display
    fn render_input(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(rgb(0x495057))
                    .child("名称"),
            )
            .child(Input::new(&self.input).w_full().when(
                self.validation_error.is_some(),
                |input| {
                    // Add red border for invalid input
                    input.border_color(rgb(0xdc3545))
                },
            ))
            .when_some(self.validation_error.as_ref(), |el, error| {
                // Display validation error message
                // Implements Requirement 6.4 for specific error display
                el.child(
                    div()
                        .h_flex()
                        .items_start()
                        .gap_2()
                        .child(
                            gpui_component::Icon::new(IconName::Close)
                                .size_3()
                                .text_color(rgb(0xdc3545))
                                .mt(px(2.0)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0xdc3545))
                                .child(error.message()),
                        ),
                )
            })
    }

    /// Render the action buttons (Cancel and Confirm)
    fn render_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("cancel_rename")
                    .ghost()
                    .child("取消")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.handle_cancel(cx);
                    })),
            )
            .child(
                Button::new("confirm_rename")
                    .primary()
                    .child("确认")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.handle_confirm(cx);
                    })),
            )
    }
}

impl Render for RenameDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(450.0))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xdee2e6))
            .rounded_lg()
            .shadow_lg()
            .v_flex()
            .child(self.render_header(cx))
            .child(
                // Content area
                div().p_4().v_flex().gap_4().child(self.render_input(cx)),
            )
            .child(
                // Footer with actions
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(rgb(0xe9ecef))
                    .child(self.render_actions(cx)),
            )
    }
}
