//! Card component for container styling with optional sections.

use gpui::*;

/// A card component that provides a styled container with optional header, body, and footer.
///
/// This is a stateless primitive component that renders based on props only.
#[derive(IntoElement)]
pub struct Card {
    header: Option<AnyElement>,
    body: Option<AnyElement>,
    footer: Option<AnyElement>,
    padding: bool,
}

impl Card {
    /// Creates a new empty Card.
    pub fn new() -> Self {
        Self {
            header: None,
            body: None,
            footer: None,
            padding: true,
        }
    }

    /// Sets the header content.
    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// Sets the body content.
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    /// Sets the footer content.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// Disables default padding.
    pub fn no_padding(mut self) -> Self {
        self.padding = false;
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div()
            .bg(white())
            .rounded_lg()
            .border_1()
            .border_color(rgb(0xe0e0e0))
            .overflow_hidden();

        if let Some(header) = self.header {
            let header_div = div()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(0xe0e0e0))
                .child(header);
            container = container.child(header_div);
        }

        if let Some(body) = self.body {
            let body_div = if self.padding {
                div().px_4().py_3().child(body)
            } else {
                div().child(body)
            };
            container = container.child(body_div);
        }

        if let Some(footer) = self.footer {
            let footer_div = div()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(rgb(0xe0e0e0))
                .child(footer);
            container = container.child(footer_div);
        }

        container
    }
}
