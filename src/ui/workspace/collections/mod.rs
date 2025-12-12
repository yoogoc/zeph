mod grpc;
mod http;
mod list;
mod tab;
mod websocket;

use crate::model::{GRPC, Http, WebSocket};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button};
use gpui_component::resizable::{h_resizable, resizable_panel};
use list::List;
use std::rc::Rc;
use tab::CollectionBar;

#[derive(Debug)]
pub enum TabBar {
    HTTP(Http),
    GRPC(GRPC),
    WS(WebSocket),
}

#[derive(IntoElement)]
pub struct Collections {}

impl Collections {
    pub fn new() -> Self {
        Self {}
    }
}

struct CollectionBarState {
    tabs: Rc<Vec<TabBar>>,
    tab_index: usize,
}

impl Render for CollectionBarState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl RenderOnce for Collections {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_state(cx, |_, _| CollectionBarState {
            tabs: Rc::new(vec![
                TabBar::HTTP(Http {
                    id: 0,
                    name: "http".to_string(),
                    ..Default::default()
                }),
                TabBar::GRPC(GRPC {
                    id: 0,
                    name: "grpc".to_string(),
                    ..Default::default()
                }),
                TabBar::WS(WebSocket {
                    id: 0,
                    name: "ws".to_string(),
                    ..Default::default()
                }),
            ]),
            tab_index: 0,
        });

        div().id("collection").pl_1().size_full().child(
            h_resizable("collection-resize")
                .child(
                    resizable_panel()
                        .size(px(30.))
                        // TODO 点击以后，将tab加入tabs，并且重置tab_index
                        .child(List::new().h_full().bg(white())),
                )
                .child(
                    div()
                        .id("collection-content")
                        .size_full()
                        .ml_1()
                        .bg(white())
                        .justify_between()
                        .border_1()
                        .rounded_xl()
                        .child(
                            CollectionBar::new(state.read(cx).tabs.clone())
                                .selected_index(state.read(cx).tab_index)
                                .on_click(window.listener_for(&state, |state, index, _, _| {
                                    state.tab_index = *index;
                                    let tab = state.tabs.get(*index);
                                    println!("Tab: {:?}", tab);
                                })),
                        )
                        .map(|base| {
                            let state = state.read(cx);
                            let tab = state.tabs.get(state.tab_index);
                            match tab {
                                None => base.child(Button::new("empty").label("empty")),
                                Some(tab) => match tab {
                                    TabBar::HTTP(_tab) => {
                                        base.child(http::Page::new().into_any_element())
                                    }
                                    TabBar::GRPC(_tab) => {
                                        base.child(grpc::Page::new().into_any_element())
                                    }
                                    TabBar::WS(_tab) => {
                                        base.child(websocket::Page::new().into_any_element())
                                    }
                                },
                            }
                        })
                        .into_any_element(),
                ),
        )
    }
}
