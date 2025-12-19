use gpui::*;
use gpui_component::{button::*, *};
use std::sync::Arc;

pub struct HelloWorld;

impl Render for HelloWorld {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(|_, _, _| println!("Clicked!")),
            )
    }
}

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let app = Application::new()
        .with_assets(gpui_component_assets::Assets)
        .with_http_client(Arc::new(reqwest_client::ReqwestClient::new()));

    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        init(cx);

        let bounds = Bounds::centered(None, size(px(920.0), px(700.0)), cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            cx.activate(true);

            gpui_tokio::init(cx);

            let view = cx.new(|_| HelloWorld);
            // This first level on the window, should be a Root.
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("Failed to open window. Please restart the application.");
    });
}
