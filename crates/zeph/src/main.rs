mod application;
mod theme;

use gpui::*;
use gpui_component::*;
use rust_i18n::t;
use std::sync::Arc;

i18n::init!();

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    rust_i18n::set_locale("zh-CN");

    let app = Application::new()
        .with_assets(gpui_component_assets::Assets)
        .with_http_client(Arc::new(reqwest_client::ReqwestClient::new()));

    app.run(move |cx| {
        // Initialize gpui-component
        init(cx);
        theme::init(cx);

        // Initialize data models and registries
        workspace::init(cx);

        gpui_tokio::init(cx);

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            cx.activate(true);

            let app_view = application::init(window, cx);
            cx.new(|cx| Root::new(app_view, window, cx))
        })
        .expect(t!("common.failed_open_window").as_ref());
    });
}
