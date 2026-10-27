//! Logical workspaces own independent workbenches; switching never rebinds jobs.
use crate::{
    model::Project,
    storage::{WorkspaceCatalog, WorkspaceEntry},
    ui::{WORKBENCH_TITLEBAR_HEIGHT, Workbench, brandmark},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, IconName, Selectable, Sizable, TitleBar,
        button::{Button, ButtonVariants},
        h_flex,
        input::{Input, InputState},
        v_flex,
    },
    *,
};
use std::{
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
    new_name: Entity<InputState>,
}
impl EventEmitter<WorkspaceEvent> for WorkspaceStrip {}
impl Render for WorkspaceStrip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = h_flex().h_full().gap_1().items_center().child(
            Button::new("workspace-manager-tab")
                .small()
                .ghost()
                .h(px(30.))
                .icon(IconName::GalleryVerticalEnd)
                .label("工作区")
                .selected(self.manager_open)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.manager_open = true;
                    cx.notify();
                })),
        );
        for label in &self.labels {
            let id = label.id;
            tabs = tabs.child(
                h_flex()
                    .flex_shrink_0()
                    .h(px(30.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(if Some(id) == self.active && !self.manager_open {
                        cx.theme().border
                    } else {
                        transparent_black()
                    })
                    .bg(if Some(id) == self.active && !self.manager_open {
                        cx.theme().background
                    } else {
                        transparent_black()
                    })
                    .gap_1()
                    .items_center()
                    .child(
                        Button::new(SharedString::from(format!("workspace-tab-{id}")))
                            .small()
                            .ghost()
                            .max_w(px(180.))
                            .tooltip(label.name.clone())
                            .label(format!(
                                "{}{}",
                                label.name,
                                if label.running > 0 {
                                    format!(" · {} 运行中", label.running)
                                } else {
                                    String::new()
                                }
                            ))
                            .on_click(
                                cx.listener(move |_, _, _, cx| cx.emit(WorkspaceEvent::Select(id))),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("workspace-close-{id}")))
                            .xsmall()
                            .ghost()
                            .icon(IconName::Close)
                            .accessibility_label(format!("关闭工作区 {}", label.name))
                            .tooltip("关闭标签，工作区仍保留在列表中")
                            .on_click(
                                cx.listener(move |_, _, _, cx| cx.emit(WorkspaceEvent::Close(id))),
                            ),
                    ),
            );
        }
        h_flex()
            .h_full()
            .flex_1()
            .min_w_0()
            .gap_2()
            .items_center()
            .child(
                div()
                    .id("workspace-tabs-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_x_scroll()
                    .child(tabs),
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
    fn render_panel(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut content = v_flex()
            .w_full()
            .flex_1()
            .min_h_0()
            .p(px(36.))
            .gap_4()
            .bg(cx.theme().background);
        if self.manager_open {
            let mut list = v_flex().gap_3();
            for label in &self.available {
                let id = label.id;
                let opened = self.labels.iter().any(|l| l.id == id);
                list = list.child(
                    h_flex()
                        .h(px(64.))
                        .px_4()
                        .gap_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .items_center()
                        .child(
                            gpui_kit::component::Icon::new(IconName::GalleryVerticalEnd)
                                .text_color(cx.theme().primary),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_1()
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(label.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(if opened {
                                            "已在标签页中打开"
                                        } else {
                                            "本地工作区"
                                        }),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!("workspace-list-open-{id}")))
                                .small()
                                .ghost()
                                .disabled(self.busy)
                                .label(if opened { "切换" } else { "打开" })
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(WorkspaceEvent::Open(id))
                                })),
                        ),
                );
            }
            content = content.child(
                v_flex()
                    .px_4()
                    .py_3()
                    .gap_4()
                    .flex_1()
                    .min_h_0()
                    .max_w(px(880.))
                    .child(
                        h_flex().gap_2().items_center().child(
                            div()
                                .flex_1()
                                .text_size(px(26.))
                                .font_weight(FontWeight::BOLD)
                                .child("工作区"),
                        ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "每个工作区独立管理请求、环境与历史。关闭标签后，可在此重新打开。",
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Input::new(&self.new_name).small().flex_1())
                            .child(
                                Button::new("workspace-create")
                                    .small()
                                    .primary()
                                    .label(if self.busy {
                                        "处理中…"
                                    } else {
                                        "创建工作区"
                                    })
                                    .disabled(self.busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.emit(WorkspaceEvent::Create(
                                            this.new_name.read(cx).value().to_string(),
                                        ));
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("workspace-catalog-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(list),
                    ),
            );
        }
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
        let strip = cx.new(|cx| WorkspaceStrip {
            labels: vec![],
            available: vec![],
            active: None,
            notice,
            busy: false,
            manager_open: false,
            new_name: cx
                .new(|cx| InputState::new(window, cx).placeholder("工作区名称，例如：商城服务")),
        });
        let subscription =
            cx.subscribe_in(&strip, window, |this, _, event, window, cx| match event {
                WorkspaceEvent::Select(id) => this.select(*id, window, cx),
                WorkspaceEvent::Close(id) => this.close(*id, window, cx),
                WorkspaceEvent::Create(name) => this.create_workspace(name.clone(), window, cx),
                WorkspaceEvent::Open(id) => this.open_workspace(*id, window, cx),
            });
        let strip_observer = cx.observe_in(&strip, window, |_, _, window, cx| {
            window.refresh();
            window.on_next_frame(|window, _| window.refresh());
            cx.notify();
        });
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
                            s.new_name
                                .update(cx, |input, cx| input.set_value("", window, cx));
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
        if !self.strip.read(cx).manager_open
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
        }
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
