use crate::model::Workspace;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{
    IconName, Sizable, TitleBar as GTitleBar, h_flex,
    tab::{Tab as GTab, TabBar as GTabBar},
};
use std::rc::Rc;

#[derive(Copy, Clone, PartialEq)]
pub enum TabBar {
    Home,
    Workspace(usize),
}

pub enum Tab {
    Workspace(Workspace),
}

#[derive(IntoElement)]
pub struct TitleBar {
    on_click: Option<Rc<dyn Fn(&TabBar, &mut Window, &mut App) + 'static>>,
}

impl TitleBar {
    pub fn new() -> Self {
        Self { on_click: None }
    }
    pub fn on_click<F>(mut self, on_click: F) -> Self
    where
        F: Fn(&TabBar, &mut Window, &mut App) + 'static,
    {
        self.on_click = Some(Rc::new(on_click));
        self
    }
}

struct TitleBarState {
    tabs: Vec<Tab>,
    tab_index: usize,
}

impl Render for TitleBarState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_state(cx, |_, _| TitleBarState {
            tabs: vec![
                Tab::Workspace(Workspace {
                    id: 1,
                    name: "workspace 1".to_string(),
                    ..Workspace::default()
                }),
                Tab::Workspace(Workspace {
                    id: 2,
                    name: "workspace 2".to_string(),
                    ..Workspace::default()
                }),
                Tab::Workspace(Workspace {
                    id: 3,
                    name: "workspace 3".to_string(),
                    ..Workspace::default()
                }),
            ],
            tab_index: 0,
        });
        let on_click = self.on_click.clone();

        GTitleBar::new().child(
            h_flex()
                .w_full()
                .pr_2()
                .justify_between()
                .child(
                    GTabBar::new("tabs")
                        .selected_index(state.read(cx).tab_index)
                        .on_click(
                            window.listener_for(&state, move |state, index, window, cx| {
                                state.tab_index = *index;
                                if let Some(ref on_click) = on_click {
                                    let tab_bar = if *index == 0 {
                                        TabBar::Home
                                    } else {
                                        let tab = &state.tabs[*index - 1];
                                        match tab {
                                            Tab::Workspace(ws) => TabBar::Workspace(ws.id),
                                        }
                                    };
                                    on_click(&tab_bar, window, cx)
                                }
                            }),
                        )
                        .child(GTab::new().label("Home"))
                        .children(state.read(cx).tabs.iter().map(|tab| match tab {
                            Tab::Workspace(ws) => GTab::new().label(&ws.name).suffix(
                                Button::new("close").ghost().xsmall().icon(IconName::Close),
                            ),
                        })),
                )
                .child(Button::new("settings").ghost().icon(IconName::Settings)),
        )
    }
}
