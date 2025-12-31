use gpui::*;
use gpui_component::{v_flex, Root, StyledExt, TitleBar};

pub fn open_settings(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitleBar::title_bar_options()),
        ..Default::default()
    };
    cx.open_window(window_options, |window, cx| {
        cx.activate(true);

        let settings = cx.new(|_| GlobalSettings {});
        cx.new(|cx| Root::new(settings, window, cx))
    })
    .expect("Failed to open window. Please restart the application.");
}

pub struct GlobalSettings {}

impl Render for GlobalSettings {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(TitleBar::new().items_center().h_flex().justify_center().child("Settings").border_b_0())
            .child("Setting coming soon......")
    }
}
