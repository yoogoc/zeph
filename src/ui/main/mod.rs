use gpui::*;
use gpui_component::button::{Button, ButtonVariants};

#[derive(IntoElement)]
pub struct Main {}

impl Main {
    pub fn new() -> Self {
        Self {}
    }
}

impl RenderOnce for Main {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id("window-body")
            .p_5()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, Main!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
    }
}
