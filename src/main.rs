use gpui_kit::component::TitleBar;
use gpui_kit::*;
use std::sync::Arc;
use zeph::{
    storage::{ProjectStore, WorkspaceCatalog, WorkspaceSession},
    ui::{
        CloseRequest, NewRequest, Quit, SaveProject, SearchRequests, SendRequest, UndoDelete,
        WORKBENCH_TITLEBAR_HEIGHT,
    },
    workspace::WorkspaceApp,
};
fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    // --project is a compatibility/development entry, never a workspace identity.
    let external = match args.next().as_deref() {
        Some("--project") => Some(std::path::PathBuf::from(
            args.next()
                .ok_or_else(|| anyhow::anyhow!("--project 需要一个目录"))?,
        )),
        Some("--help") => {
            println!("Zeph — local native API workbench\n\ncargo run -- [--project DIRECTORY]");
            return Ok(());
        }
        Some(arg) => anyhow::bail!("未知参数: {arg}"),
        None => None,
    };
    let external = external
        .map(|path| -> anyhow::Result<_> {
            let store = ProjectStore::new(path);
            // Retain compatibility with the original CLI initialization behavior.
            store.load()?;
            Ok(std::fs::canonicalize(&store.directory)?)
        })
        .transpose()?;
    let catalog_path = external
        .as_ref()
        .map(|path| path.join(".zeph/workspaces.json"))
        .unwrap_or_else(WorkspaceCatalog::default_path);
    let legacy = ProjectStore::new(
        external
            .clone()
            .unwrap_or_else(ProjectStore::default_directory),
    );
    let session = if !catalog_path.exists()
        && external.is_none()
        && WorkspaceSession::default_path().exists()
    {
        Some(WorkspaceSession::load(&WorkspaceSession::default_path())?)
    } else {
        None
    };
    let catalog = WorkspaceCatalog::initialize(&catalog_path, &legacy, session)?;
    let mut notice = String::new();
    let mut workspaces = vec![];
    for id in &catalog.opened {
        let entry = catalog.entry(*id)?.clone();
        match ProjectStore::open_existing(entry.store().directory) {
            Ok((_, project)) => workspaces.push((entry, project)),
            Err(error) => notice.push_str(&format!("无法恢复工作区“{}”: {error}\n", entry.name)),
        }
    }
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
                KeyBinding::new("cmd-k", SearchRequests, Some("Zeph")),
                KeyBinding::new("cmd-w", CloseRequest, Some("Zeph")),
                KeyBinding::new("cmd-n", NewRequest, Some("Zeph")),
                KeyBinding::new("cmd-shift-z", UndoDelete, Some("Zeph")),
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
                    traffic_light_position: Some(point(
                        px(9.),
                        (WORKBENCH_TITLEBAR_HEIGHT - px(14.)) / 2.,
                    )),
                    ..TitleBar::title_bar_options()
                }),
                window_bounds: Some(WindowBounds::centered(size(px(1440.), px(960.)), cx)),
                window_min_size: Some(size(px(1100.), px(720.))),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| {
                    WorkspaceApp::new(
                        catalog,
                        catalog_path,
                        workspaces,
                        notice,
                        runtime,
                        window,
                        cx,
                    )
                })
            })
            .expect("无法打开 Zeph 窗口");
            cx.activate(true);
        });
    Ok(())
}
