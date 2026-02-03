//! Badge component for displaying HTTP method labels with appropriate colors.

use gpui::*;
use models::HttpMethod;

/// A badge component that displays HTTP method labels with color coding.
///
/// This is a stateless primitive component that renders based on props only.
#[derive(IntoElement)]
pub struct Badge {
    method: HttpMethod,
    compact: bool,
}

impl Badge {
    /// Creates a new Badge for the given HTTP method.
    pub fn new(method: HttpMethod) -> Self {
        Self {
            method,
            compact: false,
        }
    }

    /// Creates a Badge from a method string (e.g., "GET", "POST").
    /// Falls back to GET if the string is not recognized.
    pub fn from_str(method: &str) -> Self {
        let method = match method.to_uppercase().as_str() {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            "PATCH" => HttpMethod::Patch,
            "HEAD" => HttpMethod::Head,
            "OPTIONS" => HttpMethod::Options,
            _ => HttpMethod::Get,
        };
        Self::new(method)
    }

    /// Sets compact mode (smaller padding).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Returns the background color for the HTTP method.
    fn background_color(&self) -> Rgba {
        match self.method {
            HttpMethod::Get => rgb(0x28a745),     // Green
            HttpMethod::Post => rgb(0x007bff),    // Blue
            HttpMethod::Put => rgb(0xffc107),     // Yellow/Orange
            HttpMethod::Delete => rgb(0xdc3545),  // Red
            HttpMethod::Patch => rgb(0x6f42c1),   // Purple
            HttpMethod::Head => rgb(0x6c757d),    // Gray
            HttpMethod::Options => rgb(0x17a2b8), // Cyan
        }
    }

    /// Returns the text color (white for all methods).
    fn text_color(&self) -> Rgba {
        rgb(0xffffff)
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (px_val, py_val) = if self.compact { (4.0, 1.0) } else { (6.0, 2.0) };

        div()
            .px(px(px_val))
            .py(px(py_val))
            .rounded(px(4.0))
            .bg(self.background_color())
            .text_color(self.text_color())
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .child(self.method.as_str())
    }
}
