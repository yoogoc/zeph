mod application;
mod theme;

use gpui::*;
use gpui_component::*;
use std::sync::Arc;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Initialize i18n system with runtime loading
    if let Err(e) = i18n::init() {
        tracing::error!("Failed to initialize i18n system: {}", e);
        tracing::warn!("Continuing with degraded i18n support");
    }

    i18n::set_locale("zh-CN");

    let app = gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .with_http_client(Arc::new(reqwest_client::ReqwestClient::new()));

    app.run(move |cx| {
        // Initialize gpui-component
        init(cx);
        theme::init(cx);

        gpui_tokio::init(cx);

        register::storage::init(cx);

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
        .expect(&i18n::t("common.failed_open_window"));
    });
}

fn db_path() -> std::path::PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zeph");
    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("zeph.db")
}
