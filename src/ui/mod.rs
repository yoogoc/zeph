mod main;
mod title_bar;
mod workspace;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{h_flex, v_flex};
use main::Main;
use title_bar::TabBar;
use title_bar::TitleBar;
use workspace::Workspace;

pub struct Root {}

impl Root {
    pub fn new() -> Self {
        Self {}
    }
}

struct RootState {
    current_tab: TabBar,
}

impl Render for RootState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = window.use_state(cx, |_, _| RootState {
            current_tab: TabBar::Home,
        });

        h_flex().size_full().child(
            v_flex()
                .size_full()
                .bg(opaque_grey(0.8,0.8))
                .child(TitleBar::new().on_click(window.listener_for(
                    &state,
                    |state, tab_bar, _window, _cx| {
                        state.current_tab = *tab_bar;
                    },
                )))
                .map(|this| match state.read(cx).current_tab {
                    TabBar::Home => this.child(Main::new()),
                    TabBar::Workspace(ws) => this.child(Workspace::new(ws)),
                }),
        )
    }
}
