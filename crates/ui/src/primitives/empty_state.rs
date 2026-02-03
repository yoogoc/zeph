//! EmptyState component for placeholder content.

use gpui::*;
use gpui_component::button::Button;
use gpui_component::Icon;

/// An empty state component that displays a centered message with optional icon and action.
///
/// This is a stateless primitive component that renders based on props only.
#[derive(IntoElement)]
pub struct EmptyState {
    message: SharedString,
    description: Option<SharedString>,
    icon: Option<Icon>,
    action_label: Option<SharedString>,
    action_id: Option<ElementId>,
}

impl EmptyState {
    /// Creates a new EmptyState with the given message.
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            description: None,
            icon: None,
            action_label: None,
            action_id: None,
        }
    }

    /// Sets a description below the main message.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets an icon to display above the message.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Sets an action button.
    pub fn action(mut self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        self.action_id = Some(id.into());
        self.action_label = Some(label.into());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .py_8()
            .gap_3();

        // Icon
        if let Some(icon) = self.icon {
            container = container.child(
                div()
                    .size(px(48.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(0xf5f5f5))
                    .child(icon.size_6().text_color(rgb(0x999999))),
            );
        }

        // Message
        container = container.child(
            div()
                .text_base()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0x666666))
                .text_center()
                .child(self.message),
        );

        // Description
        if let Some(description) = self.description {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(rgb(0x999999))
                    .text_center()
                    .max_w(px(300.0))
                    .child(description),
            );
        }

        // Action button
        if let (Some(id), Some(label)) = (self.action_id, self.action_label) {
            container = container.child(div().mt_2().child(Button::new(id).label(label)));
        }

        container
    }
}
