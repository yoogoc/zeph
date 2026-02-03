//! EditableInput widget that encapsulates InputState management.

use gpui::*;
use gpui_component::input::{Input, InputState};

/// Events emitted by EditableInput
pub enum EditableInputEvent {
    /// Value changed (emitted on each keystroke)
    Changed(SharedString),
    /// User pressed Enter to submit
    Submit(SharedString),
    /// User pressed Escape to cancel
    Cancel,
}

/// A widget that encapsulates InputState creation and common input behaviors.
///
/// This widget provides a standardized way to create input fields with:
/// - Placeholder text
/// - Validation error display
/// - Change and submit event handling
pub struct EditableInput {
    state: Entity<InputState>,
    validation_error: Option<SharedString>,
    width: Option<f32>,
}

impl EventEmitter<EditableInputEvent> for EditableInput {}

impl EditableInput {
    /// Creates a new EditableInput with the given placeholder.
    pub fn new(
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = placeholder.into();
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));

        Self {
            state,
            validation_error: None,
            width: None,
        }
    }

    /// Creates a new EditableInput with an initial value.
    pub fn with_value(
        value: impl Into<String>,
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = placeholder.into();
        let value = value.into();
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));

        state.update(cx, |state, cx| {
            state.set_value(value, window, cx);
        });

        Self {
            state,
            validation_error: None,
            width: None,
        }
    }

    /// Sets a fixed width for the input.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets a validation error message.
    pub fn set_error(&mut self, error: Option<impl Into<SharedString>>, cx: &mut Context<Self>) {
        self.validation_error = error.map(|e| e.into());
        cx.notify();
    }

    /// Clears the validation error.
    pub fn clear_error(&mut self, cx: &mut Context<Self>) {
        self.validation_error = None;
        cx.notify();
    }

    /// Gets the current value.
    pub fn value(&self, cx: &App) -> SharedString {
        self.state.read(cx).value()
    }

    /// Sets the value programmatically.
    pub fn set_value(&self, value: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.set_value(value.into(), window, cx);
        });
    }

    /// Returns a reference to the underlying InputState entity.
    pub fn state(&self) -> &Entity<InputState> {
        &self.state
    }

    /// Handle key events for submit/cancel
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "enter" => {
                let value = self.state.read(cx).value();
                cx.emit(EditableInputEvent::Submit(value));
            }
            "escape" => {
                cx.emit(EditableInputEvent::Cancel);
            }
            _ => {}
        }
    }
}

impl Render for EditableInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_error = self.validation_error.is_some();

        let mut input = Input::new(&self.state);

        if let Some(width) = self.width {
            input = input.w(px(width));
        } else {
            input = input.w_full();
        }

        if has_error {
            input = input.border_color(rgb(0xdc3545));
        }

        let mut container = div()
            .flex()
            .flex_col()
            .gap_1()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.handle_key_down(event, cx);
            }))
            .child(input);

        if let Some(error) = &self.validation_error {
            container = container.child(
                div()
                    .text_xs()
                    .text_color(rgb(0xdc3545))
                    .child(error.clone()),
            );
        }

        container
    }
}
