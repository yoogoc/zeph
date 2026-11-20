//! Logical workspaces own independent workbenches; switching never rebinds jobs.
use crate::{
    model::Project,
    storage::{WorkspaceCatalog, WorkspaceEntry, WorkspaceLayout},
    ui::{
        TITLEBAR_CONTROL_HEIGHT, TITLEBAR_CONTROL_RADIUS, TITLEBAR_LABEL_SIZE,
        WORKBENCH_TITLEBAR_HEIGHT, Workbench, brandmark, form_dialog_footer,
    },
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    assets::IconName as AssetIcon,
    base::{Tab, Tabs},
    component::{
        ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, TitleBar, WindowExt,
        button::{Button, ButtonVariants},
        dialog::DialogButtonProps,
        h_flex,
        input::{Input, InputState},
        v_flex,
    },
    *,
};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone)]
enum WorkspaceEvent {
    Select(Uuid),
    Close(Uuid),
    Create(String),
    Open(Uuid),
    LayoutChanged,
}
#[derive(Clone, PartialEq, Eq)]
struct WorkspaceLabel {
    id: Uuid,
    name: String,
    running: usize,
}

pub struct WorkspaceStrip {
    labels: Vec<WorkspaceLabel>,
    available: Vec<WorkspaceLabel>,
    active: Option<Uuid>,
    notice: String,
    busy: bool,
    manager_open: bool,
    layout: WorkspaceLayout,
}
impl EventEmitter<WorkspaceEvent> for WorkspaceStrip {}

fn titlebar_tab(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Tab {
    Tab::new(id)
        .accessibility_label(label)
        .selected(selected)
        .h(TITLEBAR_CONTROL_HEIGHT)
        .flex_shrink_0()
        .max_w(px(220.))
        .px_2()
        .gap_1()
        .rounded(TITLEBAR_CONTROL_RADIUS)
        .border_1()
        .border_color(cx.theme().transparent)
        .text_size(TITLEBAR_LABEL_SIZE)
        .text_color(cx.theme().foreground)
        .when(selected, |tab| {
            tab.bg(cx.theme().background).border_color(cx.theme().input)
        })
        .hover(|style| {
            if selected {
                style
            } else {
                style.bg(cx.theme().foreground.opacity(0.08))
            }
        })
}

impl Render for WorkspaceStrip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = Tabs::new("workspace-tabs")
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .px_1()
            .gap_1()
            .items_center()
            .overflow_x_scroll()
            .restrict_scroll_to_axis()
            .child(
                titlebar_tab("workspace-manager", "工作区", self.manager_open, cx)
                    .child(Icon::new(IconName::GalleryVerticalEnd).size_4())
                    .child("工作区")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.manager_open = true;
                        cx.notify();
                    })),
            );
        for label in &self.labels {
            let id = label.id;
            let active = Some(id) == self.active && !self.manager_open;
            let title = SharedString::from(if label.running > 0 {
                format!("{} · {} 运行中", label.name, label.running)
            } else {
                label.name.clone()
            });
            let close = Button::new(SharedString::from(format!("workspace-close-{id}")))
                .xsmall()
                .ghost()
                .size(px(20.))
                .icon(IconName::Close)
                .text_color(cx.theme().muted_foreground)
                .accessibility_label(format!("关闭工作区 {}", label.name))
                .tooltip("关闭标签，工作区仍保留在列表中")
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.stop_propagation();
                    cx.emit(WorkspaceEvent::Close(id));
                }));
            tabs = tabs.child(
                titlebar_tab(format!("workspace-tab-{id}"), title.clone(), active, cx)
                    .pl_3()
                    .pr_1()
                    .child(div().min_w_0().truncate().child(title))
                    .child(div().flex_shrink_0().child(close))
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(WorkspaceEvent::Select(id)))),
            );
        }
        h_flex()
            .h_full()
            .flex_1()
            .min_w_0()
            .gap_2()
            .items_center()
            .child(tabs)
            .when(
                cfg!(any(target_os = "windows", target_os = "linux")),
                |bar| {
                    bar.child(
                        Button::new("application-settings")
                            .small()
                            .ghost()
                            .h(TITLEBAR_CONTROL_HEIGHT)
                            .label("设置")
                            .tooltip("应用设置 · ⌘ ,")
                            .on_click(|_, _, cx| crate::ui_settings::open(None, cx)),
                    )
                },
            )
    }
}

/// Dedicated workspace management page, selected by the titlebar tab.
#[derive(IntoElement)]
pub struct WorkspacePanel(pub Entity<WorkspaceStrip>);
impl RenderOnce for WorkspacePanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.0
            .update(cx, |strip, cx| strip.render_panel(window, cx))
    }
}
impl WorkspaceStrip {
    fn create_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx));
        let focus = input.clone();
        let view = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let input = input.clone();
            let view = view.clone();
            dialog
                .title("新建工作区")
                .w(px(360.))
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_xs().child("工作区名称"))
                        .child(Input::new(&input).aria_label("工作区名称")),
                )
                .footer(form_dialog_footer(
                    "创建并打开",
                    input.read(cx).value().trim().is_empty(),
                ))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("创建并打开")
                        .cancel_text("取消")
                        .show_cancel(true),
                )
                .on_ok(move |_, _, cx| {
                    let name = input.read(cx).value();
                    let name = name.trim();
                    if name.is_empty() {
                        return false;
                    }
                    let _ =
                        view.update(cx, |_, cx| cx.emit(WorkspaceEvent::Create(name.to_owned())));
                    true
                })
        });
        focus.update(cx, |input, cx| input.focus(window, cx));
    }

    fn layout_switcher(&self, cx: &Context<Self>) -> Div {
        let mut switcher = h_flex()
            .gap_0p5()
            .p_0p5()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted);
        for (layout, label, icon) in [
            (WorkspaceLayout::List, "清单", AssetIcon::List),
            (WorkspaceLayout::Tiles, "平铺", AssetIcon::LayoutGrid),
        ] {
            switcher = switcher.child(
                Button::new(label)
                    .small()
                    .ghost()
                    .icon(icon)
                    .label(label)
                    .selected(self.layout == layout)
                    .when(self.layout == layout, |b| {
                        b.bg(cx.theme().background).text_color(cx.theme().primary)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.layout != layout {
                            this.layout = layout;
                            cx.emit(WorkspaceEvent::LayoutChanged);
                            cx.notify();
                        }
                    })),
            );
        }
        switcher
    }

    fn workspace_item(&self, label: &WorkspaceLabel, opened: bool, cx: &Context<Self>) -> Button {
        let id = label.id;
        let theme = cx.theme();
        let colors = [
            theme.primary,
            theme.chart_2,
            theme.chart_3,
            theme.chart_4,
            theme.muted_foreground,
        ];
        let ink = colors[id.as_bytes()[0] as usize % colors.len()];
        let name = div()
            .min_w_0()
            .truncate()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.name.clone());
        let button = Button::new(format!("workspace-open-{id}"))
            .ghost()
            .w_full()
            .min_w_0()
            .disabled(self.busy)
            .accessibility_label(format!(
                "{}工作区 {}",
                if opened { "切换" } else { "打开" },
                label.name
            ))
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(WorkspaceEvent::Open(id))));
        match self.layout {
            WorkspaceLayout::List => button.h(px(48.)).px_2().child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_3()
                    .items_center()
                    .child(div().size(px(6.)).flex_shrink_0().rounded_full().bg(ink))
                    .child(name),
            ),
            WorkspaceLayout::Tiles => button
                .h(px(96.))
                .p_0()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .child(
                    v_flex()
                        .size_full()
                        .min_w_0()
                        .items_start()
                        .justify_between()
                        .p_3()
                        .child(
                            h_flex()
                                .size(px(32.))
                                .justify_center()
                                .items_center()
                                .rounded_md()
                                .bg(ink.opacity(0.10))
                                .child(Icon::new(AssetIcon::Layers2).size_4().text_color(ink)),
                        )
                        .child(name.max_w_full()),
                ),
        }
    }

    fn render_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let opened: HashSet<_> = self.labels.iter().map(|label| label.id).collect();
        let (in_tabs, closed): (Vec<_>, Vec<_>) = self
            .available
            .iter()
            .partition(|label| opened.contains(&label.id));
        let width = window.viewport_size().width;
        let columns = if width < px(440.) {
            1
        } else if width < px(680.) {
            2
        } else {
            3
        };
        let mut groups = v_flex().gap_5().pb_4();
        for (title, members, is_open) in [("已打开", in_tabs, true), ("未打开", closed, false)]
        {
            if members.is_empty() {
                continue;
            }
            let mut entries = match self.layout {
                WorkspaceLayout::List => v_flex(),
                WorkspaceLayout::Tiles => div().grid().grid_cols(columns).gap_3(),
            };
            for (index, label) in members.into_iter().enumerate() {
                if self.layout == WorkspaceLayout::List && index > 0 {
                    entries = entries.child(div().h(px(1.)).bg(cx.theme().border.opacity(0.6)));
                }
                entries = entries.child(self.workspace_item(label, is_open, cx));
            }
            groups = groups.child(
                v_flex()
                    .gap_2()
                    .flex_shrink_0()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(cx.theme().muted_foreground)
                                    .child(title),
                            )
                            .child(div().flex_1().h(px(1.)).bg(cx.theme().border)),
                    )
                    .child(entries),
            );
        }
        let header = h_flex()
            .items_center()
            .gap_3()
            .flex_shrink_0()
            .child(
                div()
                    .flex_1()
                    .text_size(px(26.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("工作区"),
            )
            .child(self.layout_switcher(cx))
            .child(
                Button::new("workspace-create")
                    .small()
                    .primary()
                    .icon(IconName::Plus)
                    .label("新建")
                    .accessibility_label("新建工作区")
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, window, cx| this.create_dialog(window, cx))),
            );
        let content = v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .items_center()
            .pt(px(34.))
            .px_5()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(780.))
                    .flex_1()
                    .min_h_0()
                    .gap_5()
                    .child(header)
                    .child(
                        div()
                            .id("workspace-catalog-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(groups),
                    ),
            );
        content
            .when(!self.notice.is_empty(), |row| {
                row.child(
                    h_flex()
                        .px_3()
                        .gap_2()
                        .items_center()
                        .child(div().flex_1().text_xs().child(self.notice.clone()))
                        .child(
                            Button::new("workspace-notice-close")
                                .small()
                                .ghost()
                                .label("×")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.notice.clear();
                                    cx.notify();
                                })),
                        ),
                )
            })
            .into_any_element()
    }
}

pub struct WorkspaceApp {
    workbenches: Vec<(Uuid, Entity<Workbench>)>,
    observers: Vec<Subscription>,
    active: Option<Uuid>,
    strip: Entity<WorkspaceStrip>,
    runtime: Arc<tokio::runtime::Runtime>,
    catalog: WorkspaceCatalog,
    catalog_path: PathBuf,
    revision: u64,
    saved_revision: Arc<Mutex<u64>>,
    _subscription: Subscription,
    _strip_observer: Subscription,
    settings: Entity<crate::ui_settings::ApplicationSettings>,
    _settings_subscription: Subscription,
}
impl WorkspaceApp {
    pub fn new(
        catalog: WorkspaceCatalog,
        catalog_path: PathBuf,
        workspaces: Vec<(WorkspaceEntry, Project)>,
        notice: String,
        runtime: Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let strip = cx.new(|_| WorkspaceStrip {
            labels: vec![],
            available: vec![],
            active: None,
            notice,
            busy: false,
            manager_open: false,
            layout: catalog.layout,
        });
        let subscription =
            cx.subscribe_in(&strip, window, |this, _, event, window, cx| match event {
                WorkspaceEvent::Select(id) => this.select(*id, window, cx),
                WorkspaceEvent::Close(id) => this.close(*id, window, cx),
                WorkspaceEvent::Create(name) => this.create_workspace(name.clone(), window, cx),
                WorkspaceEvent::Open(id) => this.open_workspace(*id, window, cx),
                WorkspaceEvent::LayoutChanged => this.save_catalog(window, cx),
            });
        let strip_observer = cx.observe_in(&strip, window, |_, _, window, cx| {
            window.refresh();
            window.on_next_frame(|window, _| window.refresh());
            cx.notify();
        });
        let settings = crate::ui_settings::store(cx);
        let settings_subscription = cx.subscribe_in(
            &settings,
            window,
            |this, _, _: &crate::ui_settings::RestartRequested, _, cx| {
                let result = (|| -> anyhow::Result<()> {
                    anyhow::ensure!(
                        !this.strip.read(cx).busy,
                        "工作区正在创建或打开，请完成后再重启"
                    );
                    for (_, view) in &this.workbenches {
                        view.update(cx, |w, _| w.flush())?;
                    }
                    {
                        let mut last = this
                            .saved_revision
                            .lock()
                            .map_err(|_| anyhow::anyhow!("工作区列表保存锁不可用"))?;
                        this.catalog.save(&this.catalog_path)?;
                        *last = this.revision + 1;
                    }
                    this.settings
                        .update(cx, |settings, _| settings.launch_install())?;
                    Ok(())
                })();
                match result {
                    Ok(()) => cx.quit(),
                    Err(e) => this.settings.update(cx, |s, cx| {
                        s.installation_error(format!("无法重启，工作区保持打开：{e}"), cx);
                    }),
                }
            },
        );
        let active = catalog.active;
        let mut this = Self {
            workbenches: vec![],
            observers: vec![],
            active: None,
            strip,
            runtime,
            catalog,
            catalog_path,
            revision: 0,
            saved_revision: Arc::new(Mutex::new(0)),
            _subscription: subscription,
            _strip_observer: strip_observer,
            settings,
            _settings_subscription: settings_subscription,
        };
        for (entry, project) in workspaces {
            this.attach(entry, project, window, cx);
        }
        if let Some(id) = active.filter(|id| this.workbenches.iter().any(|(i, _)| i == id)) {
            this.active = Some(id);
        }
        if let Some((_, view)) = this
            .workbenches
            .iter()
            .find(|(id, _)| Some(*id) == this.active)
        {
            view.update(cx, |w, cx| w.activate(window, cx));
        }
        window.on_next_frame(|window, _| window.refresh());
        this
    }
    fn attach(
        &mut self,
        entry: WorkspaceEntry,
        project: Project,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = entry.id;
        if self.workbenches.iter().any(|(i, _)| *i == id) {
            self.select(id, window, cx);
            return;
        }
        let runtime = self.runtime.clone();
        let strip = self.strip.clone();
        let view = cx.new(|cx| {
            let mut workbench = Workbench::new(project, entry.store(), runtime, window, cx);
            workbench.set_workspace_strip(strip);
            workbench
        });
        self.observers
            .push(cx.observe_in(&view, window, move |this, view, window, cx| {
                let name = view.read(cx).workspace_name().to_owned();
                if let Some(entry) = this.catalog.workspaces.iter_mut().find(|e| e.id == id)
                    && entry.name != name
                {
                    entry.name = name;
                    this.save_catalog(window, cx);
                }
                window.refresh();
                window.on_next_frame(|window, _| window.refresh());
                cx.notify();
            }));
        self.workbenches.push((id, view));
        self.active = Some(id);
        cx.notify();
    }
    fn select(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, view)) = self.workbenches.iter().find(|(i, _)| *i == id) {
            self.active = Some(id);
            self.strip.update(cx, |s, cx| {
                s.manager_open = false;
                cx.notify();
            });
            view.update(cx, |w, cx| w.activate(window, cx));
            self.save_catalog(window, cx);
            cx.notify();
        }
    }
    fn close(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.workbenches.iter().position(|(i, _)| *i == id) else {
            return;
        };
        if let Err(error) = self.workbenches[index].1.update(cx, |w, _| w.flush()) {
            self.notice(format!("关闭前保存失败，工作区已保留: {error}"), cx);
            return;
        }
        if let Some(entry) = self.catalog.workspaces.iter_mut().find(|e| e.id == id) {
            entry.name = self.workbenches[index].1.read(cx).workspace_name().into();
        }
        self.workbenches[index]
            .1
            .update(cx, |w, cx| w.close_environment_window(cx));
        self.workbenches.remove(index);
        drop(self.observers.remove(index));
        if self.active == Some(id) {
            self.active = self
                .workbenches
                .get(index.min(self.workbenches.len().saturating_sub(1)))
                .map(|(id, _)| *id);
        }
        if let Some((_, view)) = self
            .workbenches
            .iter()
            .find(|(i, _)| Some(*i) == self.active)
        {
            view.update(cx, |w, cx| w.activate(window, cx));
        }
        self.save_catalog(window, cx);
        cx.notify();
    }
    fn notice(&self, notice: String, cx: &mut Context<Self>) {
        self.strip.update(cx, |s, cx| {
            s.notice = notice;
            cx.notify();
        });
    }
    fn create_workspace(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.strip.read(cx).busy {
            return;
        }
        if name.trim().is_empty() {
            self.notice("请输入工作区名称".into(), cx);
            return;
        }
        self.strip.update(cx, |s, cx| {
            s.busy = true;
            cx.notify();
        });
        let path = self.catalog_path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { WorkspaceCatalog::default().create(&path, &name) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.strip.update(cx, |s, cx| {
                    s.busy = false;
                    cx.notify();
                });
                match result {
                    Ok((entry, project)) => {
                        this.catalog.workspaces.push(entry.clone());
                        this.attach(entry, project, window, cx);
                        this.save_catalog(window, cx);
                        this.strip.update(cx, |s, cx| {
                            s.manager_open = false;
                            s.notice.clear();
                            cx.notify();
                        });
                    }
                    Err(error) => this.notice(format!("新建工作区失败: {error:#}"), cx),
                }
            });
        })
        .detach();
    }
    fn open_workspace(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self.strip.read(cx).busy {
            return;
        }
        if self.workbenches.iter().any(|(i, _)| *i == id) {
            self.select(id, window, cx);
            self.strip.update(cx, |s, cx| {
                s.manager_open = false;
                cx.notify();
            });
            return;
        }
        let Ok(entry) = self.catalog.entry(id).cloned() else {
            return;
        };
        self.strip.update(cx, |s, cx| {
            s.busy = true;
            cx.notify();
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let store = entry.store();
                    crate::storage::ProjectStore::open_existing(&store.directory)
                        .map(|(_, project)| (entry, project))
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.strip.update(cx, |s, cx| {
                    s.busy = false;
                    cx.notify();
                });
                match result {
                    Ok((entry, project)) => {
                        this.attach(entry, project, window, cx);
                        this.save_catalog(window, cx);
                        this.strip.update(cx, |s, cx| {
                            s.manager_open = false;
                            s.notice.clear();
                            cx.notify();
                        });
                    }
                    Err(error) => this.notice(format!("打开工作区失败: {error:#}"), cx),
                }
            });
        })
        .detach();
    }
    fn snapshot(&mut self, cx: &App) -> WorkspaceCatalog {
        for (id, view) in &self.workbenches {
            if let Some(entry) = self.catalog.workspaces.iter_mut().find(|e| e.id == *id) {
                entry.name = view.read(cx).workspace_name().into();
            }
        }
        self.catalog.opened = self.workbenches.iter().map(|(id, _)| *id).collect();
        self.catalog.active = self.active;
        self.catalog.layout = self.strip.read(cx).layout;
        self.catalog.clone()
    }
    fn save_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let catalog = self.snapshot(cx);
        let path = self.catalog_path.clone();
        self.revision += 1;
        let revision = self.revision;
        let saved = self.saved_revision.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut last = saved
                        .lock()
                        .map_err(|_| anyhow::anyhow!("工作区列表保存锁不可用"))?;
                    if revision >= *last {
                        catalog.save(&path)?;
                        *last = revision;
                    }
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            if let Err(error) = result {
                let _ = this.update_in(cx, |this, _, cx| {
                    this.notice(format!("工作区列表保存失败: {error}"), cx)
                });
            }
        })
        .detach();
    }
}
impl Render for WorkspaceApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let labels = self
            .workbenches
            .iter()
            .map(|(id, view)| {
                let workbench = view.read(cx);
                WorkspaceLabel {
                    id: *id,
                    name: workbench.workspace_name().into(),
                    running: workbench.running_count(),
                }
            })
            .collect::<Vec<_>>();
        let available = self
            .catalog
            .workspaces
            .iter()
            .map(|e| WorkspaceLabel {
                id: e.id,
                name: e.name.clone(),
                running: 0,
            })
            .collect::<Vec<_>>();
        self.strip.update(cx, |s, cx| {
            if self.workbenches.is_empty() {
                s.manager_open = true;
            }
            if s.labels != labels || s.available != available || s.active != self.active {
                s.labels = labels;
                s.available = available;
                s.active = self.active;
                cx.notify();
            }
        });
        let content = if !self.strip.read(cx).manager_open
            && let Some((_, workbench)) = self
                .workbenches
                .iter()
                .find(|(id, _)| Some(*id) == self.active)
        {
            div()
                .size_full()
                .child(workbench.clone())
                .into_any_element()
        } else {
            v_flex()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    TitleBar::new()
                        .h(WORKBENCH_TITLEBAR_HEIGHT)
                        .bg(cx.theme().sidebar)
                        .child(
                            h_flex()
                                .h_full()
                                .flex_1()
                                .min_w_0()
                                .pr_4()
                                .gap_3()
                                .items_center()
                                .child(brandmark(cx))
                                .child(self.strip.clone()),
                        ),
                )
                .child(WorkspacePanel(self.strip.clone()))
                .into_any_element()
        };
        div()
            .size_full()
            .on_action(|_: &crate::ui_settings::OpenSettings, _, cx| {
                crate::ui_settings::open(None, cx)
            })
            .on_action(|_: &crate::ui_settings::CheckUpdates, _, cx| {
                crate::ui_settings::open_updates(cx)
            })
            .child(content)
    }
}
impl Drop for WorkspaceApp {
    fn drop(&mut self) {
        if self.revision == 0 {
            return;
        }
        match self.saved_revision.lock() {
            Ok(mut last) => {
                if let Err(error) = self.catalog.save(&self.catalog_path) {
                    log::error!("退出时工作区列表保存失败: {error}");
                }
                *last = self.revision + 1;
            }
            Err(error) => log::error!("退出时工作区列表保存锁不可用: {error}"),
        }
    }
}
