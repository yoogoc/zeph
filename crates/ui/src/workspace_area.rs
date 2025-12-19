use gpui::*;

pub enum WorkspaceArea {
    Home,
}

impl WorkspaceArea {
    pub fn home() -> Self {
        Self::Home
    }
    pub fn workspace(window: &mut Window, cx: &mut App) -> Self {
        Self::Home
    }
}
impl Default for WorkspaceArea {
    fn default() -> Self {
        Self::home()
    }
}

impl Render for WorkspaceArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}