use gpui_kit::*;
use std::sync::Arc;
use zeph::{
    storage::ProjectStore,
    ui::{Quit, SaveProject, SendRequest, Workbench},
};
fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let directory = match args.next().as_deref() {
        Some("--project") => std::path::PathBuf::from(
            args.next()
                .ok_or_else(|| anyhow::anyhow!("--project 需要一个目录"))?,
        ),
        Some("--help") => {
            println!("Zeph — local native API workbench\n\ncargo run -- [--project DIRECTORY]");
            return Ok(());
        }
        Some(arg) => anyhow::bail!("未知参数: {arg}"),
        None => ProjectStore::default_directory(),
    };
    let store = ProjectStore::new(directory);
    let project = store.load()?;
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?,
    );
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            zeph::ui::apply_theme(cx);
            cx.bind_keys([
                KeyBinding::new("cmd-enter", SendRequest, Some("Zeph")),
                KeyBinding::new("cmd-s", SaveProject, Some("Zeph")),
                KeyBinding::new("cmd-q", Quit, None),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.set_menus([Menu::new("Zeph").items([MenuItem::action("Quit Zeph", Quit)])]);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Zeph — Local API Workbench".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::centered(size(px(1440.), px(960.)), cx)),
                window_min_size: Some(size(px(1100.), px(720.))),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| Workbench::new(project, store, runtime, window, cx))
            })
            .expect("无法打开 Zeph 窗口");
            cx.activate(true);
        });
    Ok(())
}
