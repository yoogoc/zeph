use crate::ui::workspace::collections::TabBar;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{
    IconName, Sizable, h_flex,
    tab::{Tab as GTab, TabBar as GTabBar},
};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct CollectionBar {
    selected_index: Option<usize>,
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
    tabs: Rc<Vec<TabBar>>,
}

impl CollectionBar {
    pub fn new(tabs: Rc<Vec<TabBar>>) -> Self {
        Self {
            on_click: None,
            selected_index: None,
            tabs,
        }
    }
    pub fn on_click<F>(mut self, on_click: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = Some(index);
        self
    }
}

impl RenderOnce for CollectionBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_click = self.on_click.clone();

        h_flex()
            .w_full()
            .pr_2()
            .justify_between()
            .border_1()
            .rounded_xl()
            .child(
                GTabBar::new("tabs")
                    .border_1()
                    .rounded_xl()
                    .when_some(self.selected_index, |this, selected_ix| {
                        this.selected_index(selected_ix)
                    })
                    .when_some(on_click, |this, on_click| {
                        this.on_click(move |x, window, cx| {
                            on_click(x, window, cx);
                        })
                    })
                    .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                        let (name, _typ) = match tab {
                            TabBar::HTTP(http) => (&http.name, "http"),
                            TabBar::GRPC(grpc) => (&grpc.name, "grpc"),
                            TabBar::WS(ws) => (&ws.name, "ws"),
                        };
                        // fixme style
                        GTab::new()
                            .when(i == 0, |tab| tab.border_l_1().rounded_l_xl())
                            .when(i == self.tabs.len() - 1, |tab| {
                                tab.border_r_1().rounded_r_xl()
                            })
                            .label(name)
                            // .prefix(typ)
                            .suffix(Button::new("close").ghost().xsmall().icon(IconName::Close))
                        // .id("")
                    })),
            )
        // .child(Button::new("settings").ghost().icon(IconName::Settings))
    }
}
