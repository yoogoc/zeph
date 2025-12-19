use gpui::*;

pub fn init(window: &mut Window, cx: &mut App) -> Entity<Application> {
    cx.new(|cx| Application::new(window, cx))
}

#[derive(Debug)]
pub struct Application {
    title_bar: Entity<ui::TitleBar>
}

impl Application {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            title_bar: cx.new(|_| ui::TitleBar::new())
        }
    }
}

impl Render for Application {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}