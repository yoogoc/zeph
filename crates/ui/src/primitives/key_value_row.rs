//! KeyValueRow component for displaying label-value pairs.

use gpui::*;

/// A row component that displays a key-value pair with consistent styling.
///
/// This is a stateless primitive component that renders based on props only.
#[derive(IntoElement)]
pub struct KeyValueRow {
    key: SharedString,
    value: SharedString,
    key_width: Option<f32>,
}

impl KeyValueRow {
    /// Creates a new KeyValueRow with the given key and value.
    pub fn new(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            key_width: None,
        }
    }

    /// Sets a fixed width for the key column.
    pub fn key_width(mut self, width: f32) -> Self {
        self.key_width = Some(width);
        self
    }
}

impl RenderOnce for KeyValueRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let key_div = {
            let mut d = div()
                .text_sm()
                .text_color(rgb(0x666666))
                .font_weight(FontWeight::MEDIUM)
                .child(self.key);
            if let Some(width) = self.key_width {
                d = d.w(px(width));
            } else {
                d = d.min_w(px(80.0));
            }
            d
        };

        let value_div = div()
            .flex_1()
            .text_sm()
            .text_color(rgb(0x333333))
            .child(self.value);

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .py_1()
            .child(key_div)
            .child(value_div)
    }
}
