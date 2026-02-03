//! TreeItem component for hierarchical list nodes.

use gpui::*;
use gpui_component::Icon;

/// A tree item component that displays a single node in a tree structure.
///
/// This is a stateless primitive component that renders based on props only.
#[derive(IntoElement)]
pub struct TreeItem {
    label: SharedString,
    icon: Option<Icon>,
    has_children: bool,
    is_expanded: bool,
    is_selected: bool,
    depth: usize,
}

impl TreeItem {
    /// Creates a new TreeItem with the given label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            has_children: false,
            is_expanded: false,
            is_selected: false,
            depth: 0,
        }
    }

    /// Sets an icon for the tree item.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Indicates whether this item has children (shows expand/collapse indicator).
    pub fn has_children(mut self, has_children: bool) -> Self {
        self.has_children = has_children;
        self
    }

    /// Sets the expanded state (only relevant if has_children is true).
    pub fn is_expanded(mut self, is_expanded: bool) -> Self {
        self.is_expanded = is_expanded;
        self
    }

    /// Sets the selected state.
    pub fn is_selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected;
        self
    }

    /// Sets the indentation depth.
    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }
}

impl RenderOnce for TreeItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let indent = px(self.depth as f32 * 16.0);
        let bg_color = if self.is_selected {
            rgb(0xe3f2fd)
        } else {
            rgb(0xffffff) // transparent background
        };

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .ml(indent)
            .rounded(px(4.0))
            .bg(bg_color)
            .hover(|s| s.bg(rgb(0xf5f5f5)));

        // Expand/collapse indicator
        if self.has_children {
            let chevron_icon = if self.is_expanded {
                "icons/chevron_down.svg"
            } else {
                "icons/chevron_right.svg"
            };
            row = row.child(
                div()
                    .size(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(chevron_icon)
                            .size(px(12.0))
                            .text_color(rgb(0x666666)),
                    ),
            );
        } else {
            // Spacer for alignment
            row = row.child(div().size(px(16.0)));
        }

        // Optional icon
        if let Some(icon) = self.icon {
            row = row.child(icon.size_4().text_color(rgb(0x666666)));
        }

        // Label
        row = row.child(
            div()
                .text_sm()
                .text_color(rgb(0x333333))
                .text_ellipsis()
                .overflow_hidden()
                .child(self.label),
        );

        row
    }
}
