mod collections;
mod docs;
mod histories;
mod settings;
mod tab;

use collections::Collections;
use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel};
use tab::Tab;

#[derive(IntoElement)]
pub struct Workspace {
    #[allow(unused)]
    pub id: usize,
}

impl Workspace {
    pub fn new(id: usize) -> Self {
        Self { id }
    }
}

impl RenderOnce for Workspace {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().id("workspace").p_1().size_full().child(
            h_resizable("ws-resize")
                .child(
                    resizable_panel()
                        .size(px(50.))
                        .child(Tab::new().h_full().bg(white())),
                )
                .child(Collections::new().into_any_element()),
        )
    }
}
