mod model;
mod ui;

use gpui::*;
use gpui_component::{Root as GRoot, TitleBar};

use ui::Root;

#[tokio::main]
async fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        cx.spawn(async move |cx| {
            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::Maximized(Bounds::default())),
                titlebar: Some(TitleBar::title_bar_options()),
                ..Default::default()
            };

            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| Root::new());
                cx.new(|cx| GRoot::new(view, window, cx))
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
