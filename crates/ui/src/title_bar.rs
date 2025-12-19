use gpui::*;

pub struct TitleBar {
}

impl TitleBar {
    pub fn new() -> Self {
        Self {}
    }
}
impl Default for TitleBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for TitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}