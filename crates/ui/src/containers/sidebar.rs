use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{v_flex, IconName};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct Sidebar {
    active_idx: usize,
    tabs: Vec<(SharedString, SharedString, IconName)>,
    on_switch_view: Option<Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
}

impl Sidebar {
    pub fn new(active_idx: usize, tabs: Vec<(SharedString, SharedString, IconName)>) -> Self {
        Self {
            active_idx,
            tabs,
            on_switch_view: None,
        }
    }

    pub fn on_switch_view(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_switch_view = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_switch_view = self.on_switch_view;

        v_flex()
            .items_center()
            .justify_start()
            .w(px(40.0))
            .h_full()
            .bg(rgb(0xf5f5f5))
            .px_4()
            .pb_4()
            .pt_2()
            .child(
                v_flex()
                    .gap_2()
                    .children(
                        self.tabs
                            .into_iter()
                            .enumerate()
                            .map(|(idx, (id, title, icon))| {
                                let is_active = idx == self.active_idx;
                                let handler = on_switch_view.clone();
                                Button::new(id)
                                    .icon(icon)
                                    .tooltip(title)
                                    .when(is_active, |btn: Button| btn.primary())
                                    .on_click(move |_, window, cx| {
                                        if let Some(ref handler) = handler {
                                            handler(idx, window, cx);
                                        }
                                    })
                            }),
                    ),
            )
    }
}
