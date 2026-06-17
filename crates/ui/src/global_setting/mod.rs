mod lang;
mod menu;
mod theme;

use crate::global_setting::lang::LangSettings;
use crate::global_setting::theme::ThemeSettings;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::list::{List, ListState};
use gpui_component::{h_flex, v_flex, Root, StyledExt, TitleBar};
use menu::{Menu, MenuList};

pub fn open_settings(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitleBar::title_bar_options()),
        ..Default::default()
    };
    cx.open_window(window_options, |window, cx| {
        cx.activate(true);

        let settings = cx.new(|cx| GlobalSettings::new(window, cx));
        cx.new(|cx| Root::new(settings, window, cx))
    })
    .expect(t!("common.failed_open_window").as_ref());
}

pub struct GlobalSettings {
    menu_state: Entity<ListState<MenuList>>,
    lang: Entity<LangSettings>,
    theme: Entity<ThemeSettings>,
}

impl GlobalSettings {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = MenuList::new();
        let state = cx.new(|cx| ListState::new(delegate, window, cx));
        let lang = cx.new(|cx| LangSettings::new(window, cx));
        let theme = cx.new(|cx| ThemeSettings::new(cx));
        Self {
            menu_state: state,
            lang,
            theme,
        }
    }
}

impl Render for GlobalSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.menu_state.read(cx);
        v_flex()
            .size_full()
            .child(
                TitleBar::new()
                    .items_center()
                    .h_flex()
                    .justify_center()
                    .child(shared_t!("global_settings.title"))
                    .border_b_0(),
            )
            .child(
                h_flex()
                    .bg(rgb(0xf5f5f5))
                    .size_full()
                    .items_center()
                    .p_4()
                    .gap_2()
                    .child(
                        h_flex()
                            .border_1()
                            .rounded_2xl()
                            .h_full()
                            .bg(white())
                            .w_40()
                            .child(List::new(&self.menu_state))
                            .p_2(),
                    )
                    .child(
                        div()
                            .border_1()
                            .rounded_2xl()
                            .size_full()
                            .bg(white())
                            .when(
                                menu.delegate().selected_item == Some(Menu::Request),
                                |this| {
                                    this.child(
                                        div()
                                            .child("Request")
                                            .text_center()
                                            .text_2xl()
                                            .font_bold()
                                            .p_2(),
                                    )
                                },
                            )
                            .when(menu.delegate().selected_item == Some(Menu::I18n), |this| {
                                this.child(self.lang.clone())
                            })
                            .when(menu.delegate().selected_item == Some(Menu::Theme), |this| {
                                this.child(self.theme.clone())
                            })
                            .when(menu.delegate().selected_item == Some(Menu::Proxy), |this| {
                                this.child(
                                    div()
                                        .child("Proxy")
                                        .text_center()
                                        .text_2xl()
                                        .font_bold()
                                        .p_2(),
                                )
                            }),
                    ),
            )
    }
}
