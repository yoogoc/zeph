use crate::global_setting::open_settings;
use crate::state::ApplicationState;
use gpui::*;
use gpui_component::button::{Button, ButtonRounded};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{h_flex, IconName, Sizable, Size, TitleBar as GTitleBar};

pub struct TitleBar {
    tabs_state: Entity<ApplicationState>,
}

impl TitleBar {
    pub fn new(
        _window: &mut Window,
        _cx: &mut Context<Self>,
        tabs_state: Entity<ApplicationState>,
    ) -> Self {
        Self { tabs_state }
    }
}

impl Render for TitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ts = self.tabs_state.read(cx);
        GTitleBar::new()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .pr_4()
                    .child(
                        TabBar::new("workspace_tabs")
                            .with_variant(TabVariant::Pill)
                            .menu(true)
                            .selected_index(ts.active_idx)
                            .on_click({
                                let ts = self.tabs_state.clone();
                                cx.listener(move |_this, index: &usize, _, cx| {
                                    ts.update(cx, |ts, cx| {
                                        ts.active_idx = *index;
                                        cx.notify();
                                    });
                                })
                            })
                            .items_center()
                            .gap_2()
                            .with_size(Size::Small)
                            .children(ts.tabs.iter().map(|tab| {
                                Tab::new()
                                    .label(tab.title.clone())
                                    .pr_1()
                                    .suffix(
                                        Button::new(format!("close_{}", tab.id))
                                            .with_size(px(13.))
                                            .rounded(ButtonRounded::Large)
                                            .icon(IconName::Close)
                                            .on_click({
                                                let ts = self.tabs_state.clone();
                                                let tab_id = tab.id.clone();
                                                cx.listener(move |_this, _, _, cx| {
                                                    ts.update(cx, |ts, _cx| {
                                                        ts.close_tab(tab_id);
                                                    });
                                                    cx.stop_propagation();
                                                })
                                            }),
                                    )
                                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                                        window.prevent_default()
                                    })
                            }))
                            .h_full()
                            .border_b_0(),
                    )
                    .child(
                        h_flex().items_center().justify_center().child(
                            Button::new("settings")
                                .small()
                                .icon(IconName::Settings)
                                .on_click(cx.listener(|_, _, _, cx| {
                                    open_settings(cx);
                                })),
                        ),
                    ),
            )
            .border_b_0()
    }
}
