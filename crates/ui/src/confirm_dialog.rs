use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{h_flex, v_flex, StyledExt};
use i18n::t;

/// Event emitted by the confirmation dialog
pub enum ConfirmDialogEvent {
    /// User clicked the primary action button (e.g., "Save")
    Confirm,
    /// User clicked the secondary action button (e.g., "Don't Save")
    Discard,
    /// User clicked cancel or closed the dialog
    Cancel,
}

/// A generic confirmation dialog with three options
pub struct ConfirmDialog {
    title: SharedString,
    message: SharedString,
    confirm_label: SharedString,
    discard_label: SharedString,
    cancel_label: SharedString,
}

impl EventEmitter<ConfirmDialogEvent> for ConfirmDialog {}

impl ConfirmDialog {
    /// Creates a new confirmation dialog for unsaved changes
    pub fn unsaved_changes() -> Self {
        Self {
            title: t!("service.confirm_close.title").into(),
            message: t!("service.confirm_close.message").into(),
            confirm_label: t!("service.confirm_close.save").into(),
            discard_label: t!("service.confirm_close.dont_save").into(),
            cancel_label: t!("service.confirm_close.cancel").into(),
        }
    }

    /// Creates a new confirmation dialog with custom labels
    pub fn new(
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        confirm_label: impl Into<SharedString>,
        discard_label: impl Into<SharedString>,
        cancel_label: impl Into<SharedString>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_label: confirm_label.into(),
            discard_label: discard_label.into(),
            cancel_label: cancel_label.into(),
        }
    }
}

impl Render for ConfirmDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(400.))
            .bg(white())
            .rounded_lg()
            .shadow_lg()
            .overflow_hidden()
            .child(
                // Header
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(
                        div()
                            .text_lg()
                            .font_bold()
                            .text_color(rgb(0x333333))
                            .child(self.title.clone()),
                    ),
            )
            .child(
                // Body
                div().px_4().py_4().child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x666666))
                        .child(self.message.clone()),
                ),
            )
            .child(
                // Footer with buttons
                div()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("confirm-dialog-cancel")
                                    .label(self.cancel_label.clone())
                                    .on_click(cx.listener(|_this, _, _, cx| {
                                        cx.emit(ConfirmDialogEvent::Cancel);
                                    })),
                            )
                            .child(
                                Button::new("confirm-dialog-discard")
                                    .label(self.discard_label.clone())
                                    .on_click(cx.listener(|_this, _, _, cx| {
                                        cx.emit(ConfirmDialogEvent::Discard);
                                    })),
                            )
                            .child(
                                Button::new("confirm-dialog-confirm")
                                    .label(self.confirm_label.clone())
                                    .primary()
                                    .on_click(cx.listener(|_this, _, _, cx| {
                                        cx.emit(ConfirmDialogEvent::Confirm);
                                    })),
                            ),
                    ),
            )
    }
}
