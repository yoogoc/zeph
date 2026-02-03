//! OverlayDialog widget that provides a modal container with backdrop.

use gpui::*;

/// Events emitted by OverlayDialog
pub enum OverlayDialogEvent {
    /// User clicked the backdrop to dismiss
    Dismiss,
}

/// A widget that renders a semi-transparent overlay with centered dialog content.
///
/// This widget provides a standardized way to display modal dialogs with:
/// - Semi-transparent backdrop
/// - Centered content
/// - Backdrop click to dismiss
#[derive(IntoElement)]
pub struct OverlayDialog {
    content: AnyElement,
    dismissable: bool,
    backdrop_color: Rgba,
}

impl OverlayDialog {
    /// Creates a new OverlayDialog with the given content.
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            content: content.into_any_element(),
            dismissable: true,
            backdrop_color: rgba(0x00000080), // 50% black
        }
    }

    /// Sets whether clicking the backdrop dismisses the dialog.
    pub fn dismissable(mut self, dismissable: bool) -> Self {
        self.dismissable = dismissable;
        self
    }

    /// Sets a custom backdrop color.
    pub fn backdrop_color(mut self, color: Rgba) -> Self {
        self.backdrop_color = color;
        self
    }

    /// Creates a darker backdrop (70% opacity).
    pub fn dark_backdrop(mut self) -> Self {
        self.backdrop_color = rgba(0x000000b3);
        self
    }

    /// Creates a lighter backdrop (30% opacity).
    pub fn light_backdrop(mut self) -> Self {
        self.backdrop_color = rgba(0x0000004d);
        self
    }
}

impl RenderOnce for OverlayDialog {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(self.backdrop_color)
            .child(
                // Content container - stops click propagation
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(self.content),
            )
    }
}

/// Helper trait for adding overlay dialog support to elements.
///
/// This provides a convenient `.with_overlay_dialog()` method that can be
/// chained onto any element.
pub trait OverlayDialogExt {
    /// Conditionally renders an overlay dialog on top of this element.
    fn with_overlay_dialog<D: IntoElement>(self, dialog: Option<D>) -> Self;
}

impl OverlayDialogExt for Div {
    fn with_overlay_dialog<D: IntoElement>(self, dialog: Option<D>) -> Self {
        match dialog {
            Some(content) => self.child(OverlayDialog::new(content)),
            None => self,
        }
    }
}
