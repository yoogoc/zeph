use gpui::*;
use gpui_component::{button::{Button, ButtonVariants}, StyledExt};

#[derive(IntoElement)]
pub struct List {
    style: StyleRefinement,
}

impl List {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
        }
    }
}

impl RenderOnce for List {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id("workspace-tab")
            .p_2()
            .size_full()
            .items_center()
            .justify_center()
            .border_1()
            .rounded_xl()
            .child(
                Button::new("collection")
                    .ghost()
                    .label("collection")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
            .child(
                Button::new("history")
                    .ghost()
                    .label("history")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
            .child(
                Button::new("doc")
                    .ghost()
                    .label("doc")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
            .child(
                Button::new("settings")
                    .ghost()
                    .label("settings")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
            .refine_style(&self.style)
    }
}

impl Styled for List {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}