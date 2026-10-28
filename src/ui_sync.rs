use super::*;
use crate::apifox::{
    self, Change, ChangeKind, Resolution, Selection, Snapshot, SourceConfig, SyncPlan, SyncSource,
};
use std::collections::HashSet;
use zeroize::Zeroizing;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SyncTab {
    Changes,
    Settings,
    History,
}
pub(super) struct SyncUi {
    pub open: bool,
    selected: Option<Uuid>,
    tab: SyncTab,
    plan: Option<SyncPlan>,
    error: Option<String>,
    preview_revision: Option<u64>,
    selection: Selection,
    active: Option<String>,
    busy: HashSet<Uuid>,
    configuring: bool,
    verifying: Option<(Uuid, Entity<InputState>)>,
    last_tick: Instant,
}
impl Default for SyncUi {
    fn default() -> Self {
        Self {
            open: false,
            selected: None,
            tab: SyncTab::Changes,
            plan: None,
            error: None,
            preview_revision: None,
            selection: Selection::default(),
            active: None,
            busy: HashSet::new(),
            configuring: false,
            verifying: None,
            last_tick: Instant::now(),
        }
    }
}
pub(super) struct ConnectedSource {
    source: SyncSource,
    snapshot: Snapshot,
}
#[derive(Clone)]
struct ConnectionForm {
    name: Entity<InputState>,
    project: Entity<InputState>,
    branch: Entity<InputState>,
    module: Entity<InputState>,
    token: Entity<InputState>,
    target: Entity<SelectState<Vec<String>>>,
    destinations: Vec<Option<Uuid>>,
    status: Entity<InputState>,
    existing: Option<SyncSource>,
}
impl ConnectionForm {
    fn read(&self, cx: &App) -> anyhow::Result<(SyncSource, Zeroizing<String>, Option<Uuid>)> {
        let number = |input: &Entity<InputState>| -> anyhow::Result<Option<u64>> {
            let s = input.read(cx).value();
            let s = s.trim();
            if s.is_empty() {
                Ok(None)
            } else {
                Ok(Some(s.parse().map_err(|_| {
                    anyhow::anyhow!("分支和模块 ID 必须为正整数")
                })?))
            }
        };
        let config = SourceConfig {
            name: self.name.read(cx).value().trim().into(),
            project_id: self.project.read(cx).value().trim().into(),
            branch_id: number(&self.branch)?,
            module_id: number(&self.module)?,
        };
        config.validate()?;
        let target = self
            .target
            .read(cx)
            .selected_index(cx)
            .and_then(|i| self.destinations.get(i.row))
            .copied()
            .flatten();
        let reuse = self
            .existing
            .as_ref()
            .filter(|s| s.config.same_scope(&config) && s.root_folder == target);
        let mut source = SyncSource::new(config);
        source.root_folder = target;
        if let Some(old) = reuse {
            source.id = old.id;
        }
        let token = Zeroizing::new(self.token.read(cx).value().trim().to_owned());
        if token.is_empty() && self.existing.is_none() {
            anyhow::bail!("请输入 Apifox 访问令牌");
        }
        Ok((source, token, self.existing.as_ref().map(|s| s.id)))
    }
}

async fn pull_with_credential(
    source: &SyncSource,
    provided: Zeroizing<String>,
    read_id: Option<Uuid>,
) -> Result<(Snapshot, Zeroizing<String>), String> {
    let token = if provided.is_empty() {
        let id = read_id.ok_or("请输入访问令牌")?;
        tokio::task::spawn_blocking(move || apifox::read_token(id))
            .await
            .map_err(|_| "系统凭据读取失败".to_owned())?
            .map_err(|e| e.to_string())?
    } else {
        provided
    };
    let snapshot = apifox::fetch(&source.config, &token)
        .await
        .map_err(|e| e.message)?;
    Ok((snapshot, token))
}

impl Workbench {
    pub(super) fn open_sync(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        self.stash_environment(cx);
        self.environment_open = false;
        self.history_open = false;
        self.definitions_open = false;
        self.encoding_open = false;
        self.sync_ui.open = true;
        self.sync_ui.selected = id
            .or(self.sync_ui.selected)
            .filter(|id| self.project.sync_sources.iter().any(|s| s.id == *id))
            .or_else(|| self.project.sync_sources.first().map(|s| s.id));
        self.sync_ui.preview_revision = None;
        cx.notify();
    }
    pub(super) fn show_sync_mapping(
        &mut self,
        folder: Option<Uuid>,
        request: Option<Uuid>,
        cx: &mut Context<Self>,
    ) -> bool {
        let source = request
            .and_then(|id| apifox::binding_for_request(&self.project, id).map(|(s, _)| s.id))
            .or_else(|| {
                folder.and_then(|id| {
                    self.project
                        .sync_sources
                        .iter()
                        .find(|s| {
                            s.root_folder == Some(id) || s.folders.iter().any(|f| f.folder_id == id)
                        })
                        .map(|s| s.id)
                })
            });
        if let Some(id) = source {
            self.open_sync(Some(id), cx);
            true
        } else {
            false
        }
    }
    fn refresh_sync_preview(&mut self) {
        if self.sync_ui.preview_revision == Some(self.revision) {
            return;
        }
        self.sync_ui.preview_revision = Some(self.revision);
        self.sync_ui.plan = None;
        self.sync_ui.error = None;
        if let Some(id) = self.sync_ui.selected {
            match apifox::plan(&self.project, id) {
                Ok(plan) => {
                    self.sync_ui.selection = Selection::safe(&plan);
                    if !plan
                        .changes
                        .iter()
                        .any(|c| Some(&c.key) == self.sync_ui.active.as_ref())
                    {
                        self.sync_ui.active = plan
                            .changes
                            .iter()
                            .find(|c| c.kind == ChangeKind::Conflict)
                            .or_else(|| plan.changes.first())
                            .map(|c| c.key.clone());
                    }
                    self.sync_ui.plan = Some(plan);
                }
                Err(e) => self.sync_ui.error = Some(e.to_string()),
            }
        }
    }
    pub(super) fn sync_tick(&mut self, cx: &mut Context<Self>) {
        if self.sync_ui.last_tick.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.sync_ui.last_tick = Instant::now();
        let now = chrono::Utc::now().timestamp();
        let due = self
            .project
            .sync_sources
            .iter()
            .filter(|s| s.due(now) && !self.sync_ui.busy.contains(&s.id))
            .map(|s| s.id)
            .collect::<Vec<_>>();
        for id in due {
            self.check_sync(id, cx);
        }
    }
    fn check_sync(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.sync_ui.busy.contains(&id) {
            return;
        }
        let Some(source) = self.project.sync_sources.iter().find(|s| s.id == id) else {
            return;
        };
        let config = source.config.clone();
        self.sync_ui.busy.insert(id);
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let token = tokio::task::spawn_blocking(move || apifox::read_token(id)).await;
            let result = match token {
                Ok(Ok(token)) => apifox::fetch(&config, &token).await,
                _ => Err(apifox::ApiFailure::credentials()),
            };
            let _ = tx.send(WorkerEvent::SyncFetched(id, result)).await;
        });
        self.notice = "正在检查 Apifox 更新…".into();
        cx.notify();
    }
    pub(super) fn sync_fetched(
        &mut self,
        id: Uuid,
        result: Result<Snapshot, apifox::ApiFailure>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_ui.busy.remove(&id);
        let Some(source) = self.project.sync_sources.iter_mut().find(|s| s.id == id) else {
            return;
        };
        let auto = source.auto_apply && !source.paused;
        match result {
            Ok(snapshot) => {
                source.received(snapshot);
                self.notice = "Apifox 快照已更新，可查看变更".into();
            }
            Err(error) => {
                source.failed(&error);
                self.notice = error.message;
                self.dirty(cx);
                return;
            }
        }
        self.dirty(cx);
        if auto && let Ok(plan) = apifox::plan(&self.project, id) {
            let selection = Selection::safe(&plan);
            if !selection.included.is_empty() {
                self.apply_sync_plan(&plan, &selection, window, cx);
            }
        }
    }
    pub(super) fn sync_configured(
        &mut self,
        id: Uuid,
        result: Result<Box<ConnectedSource>, String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_ui.configuring = false;
        self.sync_ui.busy.remove(&id);
        match result {
            Ok(result) => {
                let ConnectedSource { source, snapshot } = *result;
                let id = source.id;
                if source
                    .root_folder
                    .is_some_and(|id| self.project.folder(id).is_err())
                {
                    self.notice = "目标文件夹已删除，请重新配置连接".into();
                    cx.notify();
                    return;
                }
                if let Some(existing) = self.project.sync_sources.iter_mut().find(|s| s.id == id) {
                    existing.config = source.config;
                    existing.root_folder = source.root_folder;
                    existing.received(snapshot);
                } else {
                    let mut source = source;
                    source.received(snapshot);
                    self.project.sync_sources.push(source);
                }
                self.open_sync(Some(id), cx);
                self.sync_ui.tab = SyncTab::Changes;
                self.notice = "Apifox 连接已验证并保存，请预览首次同步内容".into();
                self.dirty(cx);
            }
            Err(error) => {
                self.notice = format!("连接未保存：{error}");
                cx.notify();
            }
        }
    }
    pub(super) fn sync_verified(
        &mut self,
        id: Uuid,
        result: Result<usize, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((pending, status)) = self.sync_ui.verifying.take() {
            if pending == id {
                let text = match result {
                    Ok(count) => format!("连接成功 · {count} 个 HTTP 接口；尚未保存"),
                    Err(error) => format!("连接失败：{error}"),
                };
                status.update(cx, |s, cx| s.set_value(text, window, cx));
            } else {
                self.sync_ui.verifying = Some((pending, status));
            }
        }
        cx.notify();
    }
    fn edit_sync_source(&mut self, id: Option<Uuid>, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync_ui.configuring {
            self.notice = "连接正在验证并保存，请稍候".into();
            cx.notify();
            return;
        }
        let existing = id.and_then(|id| {
            self.project
                .sync_sources
                .iter()
                .find(|s| s.id == id)
                .cloned()
        });
        let make =
            |text: String, hint: &'static str, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    let mut s = InputState::new(window, cx).placeholder(hint);
                    s.set_value(text, window, cx);
                    s
                })
            };
        let name = make(
            existing
                .as_ref()
                .map(|s| s.config.name.clone())
                .unwrap_or("Apifox 项目".into()),
            "连接名称",
            window,
            cx,
        );
        let project = make(
            existing
                .as_ref()
                .map(|s| s.config.project_id.clone())
                .unwrap_or_default(),
            "Apifox 项目 ID",
            window,
            cx,
        );
        let branch = make(
            existing
                .as_ref()
                .and_then(|s| s.config.branch_id)
                .map(|n| n.to_string())
                .unwrap_or_default(),
            "留空使用主分支",
            window,
            cx,
        );
        let module = make(
            existing
                .as_ref()
                .and_then(|s| s.config.module_id)
                .map(|n| n.to_string())
                .unwrap_or_default(),
            "留空使用默认模块",
            window,
            cx,
        );
        let token = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(if existing.is_some() {
                    "留空保留现有令牌"
                } else {
                    "Apifox API 访问令牌"
                })
        });
        let status = make(String::new(), "", window, cx);
        let mut destinations = vec![None];
        let mut labels = vec!["新建独立同步文件夹".to_owned()];
        let mut folders = self
            .project
            .folders
            .iter()
            .map(|f| (self.project.folder_path(f.id), f.id))
            .collect::<Vec<_>>();
        folders.sort();
        for (path, id) in folders {
            labels.push(path);
            destinations.push(Some(id));
        }
        let selected = destinations
            .iter()
            .position(|d| *d == existing.as_ref().and_then(|s| s.root_folder))
            .unwrap_or(0);
        let target = cx.new(|cx| {
            SelectState::new(labels, Some(IndexPath::new(selected)), window, cx).searchable(true)
        });
        let form = ConnectionForm {
            name,
            project,
            branch,
            module,
            token,
            target,
            destinations,
            status,
            existing,
        };
        let view = cx.entity().downgrade();
        let focus = form.project.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let form = form.clone();
            let view = view.clone();
            let test_form = form.clone();
            let test_view = view.clone();
            let test_status = form.status.clone();
            dialog
                .title(if id.is_some() {
                    "编辑 Apifox 连接"
                } else {
                    "连接 Apifox"
                })
                .w(px(540.))
                .footer(folder_dialog_footer("保存并预览"))
                .child(
                    v_flex()
                        .gap_3()
                        .child(sync_form_field(
                            "连接名称",
                            Input::new(&form.name).into_any_element(),
                        ))
                        .child(sync_form_field(
                            "项目 ID",
                            Input::new(&form.project).into_any_element(),
                        ))
                        .child(sync_form_field(
                            "访问令牌",
                            Input::new(&form.token).into_any_element(),
                        ))
                        .child(
                            div()
                                .text_xs()
                                .child("令牌存入系统凭据存储，不写入工作区文件。"),
                        )
                        .child(sync_form_field(
                            "分支 ID",
                            Input::new(&form.branch).into_any_element(),
                        ))
                        .child(sync_form_field(
                            "模块 ID",
                            Input::new(&form.module).into_any_element(),
                        ))
                        .child(sync_form_field(
                            "目标文件夹",
                            Select::new(&form.target).w_full().into_any_element(),
                        ))
                        .child(
                            div()
                                .text_xs()
                                .child("修改项目、分支、模块或目标会创建新数据源，保留原连接。"),
                        )
                        .child(
                            Button::new("sync-verify")
                                .small()
                                .w(px(112.))
                                .ghost()
                                .label("验证连接")
                                .on_click(move |_, window, cx| {
                                    let result = test_form.read(cx);
                                    match result {
                                        Ok((source, secret, read_id)) => {
                                            let _ = test_view.update(cx, |this, cx| {
                                                let nonce = Uuid::new_v4();
                                                this.sync_ui.verifying =
                                                    Some((nonce, test_status.clone()));
                                                test_status.update(cx, |s, cx| {
                                                    s.set_value("正在验证…", window, cx)
                                                });
                                                let tx = this.tx.clone();
                                                this.runtime.spawn(async move {
                                                    let result = pull_with_credential(
                                                        &source, secret, read_id,
                                                    )
                                                    .await
                                                    .map(|(snapshot, _)| snapshot.endpoints.len());
                                                    let _ = tx
                                                        .send(WorkerEvent::SyncVerified(
                                                            nonce, result,
                                                        ))
                                                        .await;
                                                });
                                            });
                                        }
                                        Err(error) => test_status.update(cx, |s, cx| {
                                            s.set_value(error.to_string(), window, cx)
                                        }),
                                    }
                                }),
                        )
                        .child(
                            Input::new(&form.status)
                                .readonly(true)
                                .appearance(false)
                                .h(px(40.)),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    let (source, secret, read_id) = match form.read(cx) {
                        Ok(result) => result,
                        Err(error) => {
                            form.status
                                .update(cx, |s, cx| s.set_value(error.to_string(), window, cx));
                            return false;
                        }
                    };
                    if view
                        .read_with(cx, |this, _| this.sync_ui.busy.contains(&source.id))
                        .unwrap_or(false)
                    {
                        form.status.update(cx, |s, cx| {
                            s.set_value("此数据源正在检查，请稍后保存连接", window, cx)
                        });
                        return false;
                    }
                    let _ = view.update(cx, |this, cx| {
                        this.sync_ui.configuring = true;
                        this.sync_ui.busy.insert(source.id);
                        this.notice = "正在验证并保存 Apifox 连接…".into();
                        let tx = this.tx.clone();
                        this.runtime.spawn(async move {
                            let source_id = source.id;
                            let result = async {
                                let (snapshot, token) =
                                    pull_with_credential(&source, secret, read_id).await?;
                                let id = source.id;
                                tokio::task::spawn_blocking(move || apifox::save_token(id, &token))
                                    .await
                                    .map_err(|_| "凭据保存任务失败".to_owned())?
                                    .map_err(|e| e.to_string())?;
                                Ok(Box::new(ConnectedSource { source, snapshot }))
                            }
                            .await;
                            let _ = tx
                                .send(WorkerEvent::SyncConfigured(source_id, result))
                                .await;
                        });
                        cx.notify();
                    });
                    form.token.update(cx, |s, cx| s.set_value("", window, cx));
                    true
                })
        });
        focus.update(cx, |s, cx| s.focus(window, cx));
    }
    fn apply_sync_plan(
        &mut self,
        plan: &SyncPlan,
        selection: &Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match apifox::apply(&mut self.project, plan, selection) {
            Ok(count) => {
                if count == 0 {
                    return;
                }
                if self.project.tabs().is_empty()
                    && let Some(request) = self.project.requests.first()
                {
                    let id = request.id;
                    self.project.open_request(id);
                }
                if !self.project.tabs().is_empty() {
                    self.load_request(window, cx);
                }
                self.notice = format!("已同步 {count} 项变更，本地调试值已保留");
                self.dirty(cx);
            }
            Err(error) => {
                self.notice = error.to_string();
                self.sync_ui.preview_revision = None;
                cx.notify();
            }
        }
    }
    fn apply_sync_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(plan) = self.sync_ui.plan.clone() {
            let selection = self.sync_ui.selection.clone();
            self.apply_sync_plan(&plan, &selection, window, cx);
        }
    }
    pub(super) fn sync_ui_select(&mut self, id: Uuid) {
        self.sync_ui.selected = Some(id);
        self.sync_ui.preview_revision = None;
    }
    pub(super) fn sync_show_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self
            .sync_ui
            .selected
            .and_then(|id| self.project.sync_sources.iter().find(|s| s.id == id))
        else {
            return;
        };
        let Some(snapshot) = &source.latest else {
            return;
        };
        let text = serde_json::to_string_pretty(&snapshot.document).unwrap_or_default();
        self.definition
            .update(cx, |s, cx| s.set_value(text, window, cx));
        self.definition_path = None;
        self.definitions_open = true;
        self.sync_ui.open = false;
        cx.notify();
    }
    fn sync_diagnostics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self
            .sync_ui
            .selected
            .and_then(|id| self.project.sync_sources.iter().find(|s| s.id == id))
        else {
            return;
        };
        let text = source
            .latest
            .as_ref()
            .map(|s| s.diagnostics.join("\n\n"))
            .unwrap_or_default();
        self.encoding
            .update(cx, |s, cx| s.set_value(text, window, cx));
        self.encoding_open = true;
        self.sync_ui.open = false;
        cx.notify();
    }
    pub(super) fn sync_page(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.sync_ui.selected.is_none() {
            self.sync_ui.selected = self.project.sync_sources.first().map(|s| s.id);
            self.sync_ui.preview_revision = None;
        }
        self.refresh_sync_preview();
        let mut page = v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .p_5()
                    .gap_3()
                    .items_center()
                    .child(
                        v_flex()
                            .flex_1()
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("数据源"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Apifox → 当前工作区 · 本地快照"),
                            ),
                    )
                    .child(
                        Button::new("sync-add")
                            .small()
                            .icon(IconName::Plus)
                            .label("添加数据源")
                            .disabled(self.sync_ui.configuring)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit_sync_source(None, window, cx)
                            })),
                    ),
            );
        if self.project.sync_sources.is_empty() {
            return page
                .child(
                    v_flex()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(
                            Icon::new(gpui_kit::assets::IconName::Plug)
                                .size(px(28.))
                                .text_color(cx.theme().primary),
                        )
                        .child("连接 Apifox 项目")
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("拉取接口定义和目录，保留本地调试内容。"),
                        ),
                )
                .into_any_element();
        }
        let mut source_tabs = h_flex().px_5().gap_2();
        for source in &self.project.sync_sources {
            let id = source.id;
            source_tabs = source_tabs.child(
                Button::new(SharedString::from(format!("source-{id}")))
                    .small()
                    .ghost()
                    .selected(self.sync_ui.selected == Some(id))
                    .label(source.config.name.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sync_ui.selected = Some(id);
                        this.sync_ui.preview_revision = None;
                        cx.notify();
                    })),
            );
        }
        if self.project.sync_sources.len() > 1 {
            page = page.child(source_tabs);
        }
        let Some(source) = self
            .sync_ui
            .selected
            .and_then(|id| self.project.sync_sources.iter().find(|s| s.id == id))
        else {
            return page.into_any_element();
        };
        let id = source.id;
        let busy = self.sync_ui.busy.contains(&id);
        let pending = self
            .sync_ui
            .plan
            .as_ref()
            .map(|p| p.changes.len())
            .unwrap_or(0);
        let status = if busy {
            "正在检查…"
        } else if source.authorization_required {
            "需要更新令牌"
        } else if source.paused {
            "已暂停"
        } else if pending > 0 {
            "有待处理变更"
        } else if source.latest.is_some() {
            "已同步"
        } else {
            "尚未检查"
        };
        let meta = format!(
            "项目 {} · {} · {}",
            source.config.project_id,
            source
                .config
                .branch_id
                .map(|n| format!("分支 {n}"))
                .unwrap_or("主分支".into()),
            source
                .config
                .module_id
                .map(|n| format!("模块 {n}"))
                .unwrap_or("默认模块".into())
        );
        let target = source
            .root_folder
            .map(|id| self.project.folder_path(id))
            .unwrap_or_else(|| format!("首次应用时新建 / {}", source.config.name));
        page = page.child(
            div()
                .mx_5()
                .mb_2()
                .p_4()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(px(8.))
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Icon::new(gpui_kit::assets::IconName::CloudDownload)
                                .size(px(22.))
                                .text_color(cx.theme().primary),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(source.config.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(meta),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate()
                                        .child(format!("→ {target}")),
                                ),
                        )
                        .child(
                            v_flex()
                                .items_end()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(if source.authorization_required {
                                            rgb(0xbb545e).into()
                                        } else {
                                            cx.theme().primary
                                        })
                                        .child(status),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            source
                                                .last_checked_at
                                                .as_deref()
                                                .and_then(|s| {
                                                    chrono::DateTime::parse_from_rfc3339(s).ok()
                                                })
                                                .map(|d| {
                                                    format!(
                                                        "上次检查 {}",
                                                        d.with_timezone(&chrono::Local)
                                                            .format("%m-%d %H:%M")
                                                    )
                                                })
                                                .unwrap_or("首次连接".into()),
                                        ),
                                ),
                        )
                        .child(
                            Button::new("sync-edit")
                                .small()
                                .ghost()
                                .icon(IconName::Settings2)
                                .accessibility_label("编辑 Apifox 连接")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.edit_sync_source(Some(id), window, cx)
                                })),
                        ),
                ),
        );
        if let Some(error) = &source.last_error {
            page = page.child(
                div()
                    .mx_5()
                    .mb_2()
                    .text_sm()
                    .text_color(rgb(0xbb545e))
                    .child(error.clone()),
            );
        }
        let diagnostics = source
            .latest
            .as_ref()
            .map(|s| s.diagnostics.len())
            .unwrap_or(0);
        let drafts = source
            .bindings
            .iter()
            .filter(|b| !b.compatibility.is_empty())
            .count();
        if diagnostics > 0 || drafts > 0 {
            page = page.child(
                h_flex()
                    .mx_5()
                    .mb_2()
                    .gap_3()
                    .when(diagnostics > 0, |row| {
                        row.child(
                            Button::new("sync-diagnostics")
                                .small()
                                .ghost()
                                .label(format!("{diagnostics} 条导入诊断"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.sync_diagnostics(window, cx)
                                })),
                        )
                    })
                    .when(drafts > 0, |row| {
                        row.child(
                            Button::new("sync-draft-notes")
                                .small()
                                .ghost()
                                .label(format!("{drafts} 份草稿需要检查"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sync_ui.tab = SyncTab::Settings;
                                    cx.notify();
                                })),
                        )
                    }),
            );
        }
        let mut tabs = h_flex()
            .mx_5()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border);
        for (tab, label) in [
            (SyncTab::Changes, format!("变更预览 {pending}")),
            (SyncTab::Settings, "同步设置".into()),
            (SyncTab::History, "同步记录".into()),
        ] {
            tabs = tabs.child(
                div()
                    .border_b_2()
                    .border_color(if self.sync_ui.tab == tab {
                        cx.theme().primary
                    } else {
                        transparent_black()
                    })
                    .child(
                        Button::new(SharedString::from(format!("sync-tab-{label}")))
                            .small()
                            .ghost()
                            .h(px(36.))
                            .label(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.sync_ui.tab = tab;
                                cx.notify();
                            })),
                    ),
            );
        }
        page = page.child(tabs);
        page.child(match self.sync_ui.tab {
            SyncTab::Changes => self.sync_changes(cx),
            SyncTab::Settings => self.sync_settings(cx),
            SyncTab::History => self.sync_history(cx),
        })
        .into_any_element()
    }
    fn sync_changes(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(id) = self.sync_ui.selected else {
            return div().into_any_element();
        };
        let mut content = v_flex().flex_1().min_h_0().p_5().gap_3();
        let count = self
            .sync_ui
            .plan
            .as_ref()
            .map(|p| p.changes.len())
            .unwrap_or(0);
        content = content.child(
            h_flex()
                .gap_3()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{count} 项变更 · 远端移除默认保留")),
                )
                .child(
                    Button::new("sync-document")
                        .small()
                        .ghost()
                        .label("查看原文")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.sync_show_document(window, cx)),
                        ),
                )
                .child(
                    Button::new("sync-check")
                        .small()
                        .icon(IconName::RotateCw)
                        .label(if self.sync_ui.busy.contains(&id) {
                            "正在检查…"
                        } else {
                            "检查更新"
                        })
                        .disabled(self.sync_ui.busy.contains(&id))
                        .on_click(cx.listener(move |this, _, _, cx| this.check_sync(id, cx))),
                ),
        );
        if let Some(error) = &self.sync_ui.error {
            return content
                .child(
                    div()
                        .p_4()
                        .text_color(cx.theme().muted_foreground)
                        .child(error.clone()),
                )
                .into_any_element();
        }
        let Some(plan) = &self.sync_ui.plan else {
            return content.into_any_element();
        };
        if plan.changes.is_empty() {
            return content
                .child(
                    v_flex()
                        .flex_1()
                        .justify_center()
                        .items_center()
                        .gap_3()
                        .child(
                            Icon::new(IconName::Check)
                                .size(px(24.))
                                .text_color(cx.theme().primary),
                        )
                        .child("接口定义已是最新")
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("本地调试值、历史和标签页均独立保留。"),
                        ),
                )
                .into_any_element();
        }
        let mut rows = v_flex().p_1().gap_1();
        for change in &plan.changes {
            let key = change.key.clone();
            let choose = key.clone();
            let selected = self.sync_ui.selection.included.contains(&key);
            let unresolved = change.fields.iter().any(|f| {
                f.conflict
                    && !self
                        .sync_ui
                        .selection
                        .resolutions
                        .contains_key(&(key.clone(), f.path.clone()))
            }) || (change.kind == ChangeKind::LocalDeleted
                && !self
                    .sync_ui
                    .selection
                    .resolutions
                    .contains_key(&(key.clone(), "$".into())));
            rows = rows.child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .rounded(px(5.))
                    .items_center()
                    .bg(if self.sync_ui.active.as_ref() == Some(&key) {
                        cx.theme().primary.opacity(0.07)
                    } else {
                        transparent_black()
                    })
                    .child(
                        Checkbox::new(SharedString::from(format!("sync-select-{key}")))
                            .accessibility_label(format!("选择同步变更 {}", change.name))
                            .checked(selected)
                            .disabled(unresolved)
                            .on_click(cx.listener(move |this, value, _, cx| {
                                if *value {
                                    this.sync_ui.selection.included.insert(choose.clone());
                                } else {
                                    this.sync_ui.selection.included.remove(&choose);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("sync-detail-{key}")))
                            .accessibility_label(format!(
                                "查看变更 {} {}，{}",
                                change.method,
                                change.name,
                                change.kind.label()
                            ))
                            .ghost()
                            .h(px(46.))
                            .flex_1()
                            .min_w_0()
                            .child(
                                v_flex()
                                    .w_full()
                                    .items_start()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_size(px(12.))
                                                    .child(change.name.clone()),
                                            )
                                            .child(sync_badge(change.kind, cx)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(change.method.clone()),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.sync_ui.active = Some(key.clone());
                                cx.notify();
                            })),
                    ),
            );
        }
        let detail = plan
            .changes
            .iter()
            .find(|c| Some(&c.key) == self.sync_ui.active.as_ref())
            .map(|c| self.sync_detail(c, cx))
            .unwrap_or_else(|| div().into_any_element());
        content = content.child(
            h_flex()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(px(8.))
                .overflow_hidden()
                .child(
                    div()
                        .id("sync-change-list")
                        .w(px(245.))
                        .flex_shrink_0()
                        .h_full()
                        .overflow_y_scroll()
                        .border_r_1()
                        .border_color(cx.theme().border)
                        .child(rows),
                )
                .child(
                    div()
                        .id("sync-change-detail")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_y_scroll()
                        .child(detail),
                ),
        );
        let selected = self.sync_ui.selection.included.len();
        let conflicts = plan
            .changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Conflict || c.kind == ChangeKind::LocalDeleted)
            .count();
        content
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("已选 {selected} 项 · {conflicts} 项需处理")),
                    )
                    .child(
                        Button::new("sync-apply")
                            .small()
                            .primary()
                            .label(format!("应用 {selected} 项变更"))
                            .disabled(selected == 0)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.apply_sync_selection(window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
    fn sync_detail(&self, change: &Change, cx: &mut Context<Self>) -> AnyElement {
        let source = self
            .sync_ui
            .selected
            .and_then(|id| self.project.sync_sources.iter().find(|s| s.id == id));
        let path = change
            .remote
            .and_then(|index| {
                source
                    .and_then(|s| s.latest.as_ref())
                    .and_then(|s| s.endpoints.get(index))
            })
            .and_then(|endpoint| endpoint.definition["path"].as_str())
            .map(str::to_owned)
            .or_else(|| {
                change
                    .binding
                    .and_then(|index| source.and_then(|s| s.bindings.get(index)))
                    .and_then(|b| b.baseline["path"].as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| {
                change
                    .directory
                    .as_ref()
                    .map(|p| p.join(" / "))
                    .unwrap_or_default()
            });
        let mut detail = v_flex()
            .p_4()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::MEDIUM)
                            .child(change.name.clone()),
                    )
                    .child(sync_badge(change.kind, cx)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} {}", change.method, path)),
            );
        if change.kind == ChangeKind::Added {
            detail = detail.child(
                div()
                    .text_sm()
                    .child("首次应用后添加接口定义，并生成一份可编辑的本地调试草稿。"),
            );
        }
        if change.kind == ChangeKind::LocalDeleted {
            detail = detail.child(
                div()
                    .text_sm()
                    .child("此接口已在本地删除。选择保留删除状态，或显式恢复接口。"),
            );
            for (resolution, label) in [
                (Resolution::Local, "保留本地删除"),
                (Resolution::Remote, "恢复接口"),
            ] {
                let key = change.key.clone();
                let selected = self
                    .sync_ui
                    .selection
                    .resolutions
                    .get(&(key.clone(), "$".into()))
                    == Some(&resolution);
                detail = detail.child(
                    Button::new(SharedString::from(format!("sync-deleted-{label}")))
                        .small()
                        .ghost()
                        .selected(selected)
                        .label(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.sync_ui
                                .selection
                                .resolutions
                                .insert((key.clone(), "$".into()), resolution);
                            this.sync_ui.selection.included.insert(key.clone());
                            cx.notify();
                        })),
                );
            }
        }
        if change.kind == ChangeKind::Removed {
            detail =
                detail.child(div().text_sm().child(
                    "接口在本次完整快照中缺失。默认保留本地副本；归档只移动分组，不删除请求。",
                ));
            for (archive, label) in [(false, "保留并标记"), (true, "归档到已移除")] {
                let key = change.key.clone();
                let selected = self.sync_ui.selection.archive.contains(&key) == archive;
                detail = detail.child(
                    Button::new(SharedString::from(format!("sync-removed-{archive}")))
                        .small()
                        .ghost()
                        .selected(selected)
                        .label(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if archive {
                                this.sync_ui.selection.archive.insert(key.clone());
                            } else {
                                this.sync_ui.selection.archive.remove(&key);
                            }
                            this.sync_ui.selection.included.insert(key.clone());
                            cx.notify();
                        })),
                );
            }
        }
        for field in change
            .fields
            .iter()
            .enumerate()
            .filter(|(index, field)| *index < 40 || field.conflict)
            .map(|(_, field)| field)
        {
            let mut row = v_flex()
                .gap_2()
                .pb_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .child(field_label(&field.path)),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(sync_value("当前本地", field.local.as_ref(), cx))
                        .child(sync_value("Apifox 最新", field.remote.as_ref(), cx)),
                );
            if field.conflict {
                row = row.child(sync_value("上次同步", field.base.as_ref(), cx));
                let mut choices = h_flex().gap_2();
                for (resolution, label) in [
                    (Resolution::Local, "保留本地"),
                    (Resolution::Remote, "采用远端"),
                ] {
                    let key = change.key.clone();
                    let path = field.path.clone();
                    let selected = self
                        .sync_ui
                        .selection
                        .resolutions
                        .get(&(key.clone(), path.clone()))
                        == Some(&resolution);
                    choices = choices.child(
                        Button::new(SharedString::from(format!("resolve-{key}-{path}-{label}")))
                            .small()
                            .selected(selected)
                            .when(selected, |button| button.icon(IconName::Check))
                            .label(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.sync_ui
                                    .selection
                                    .resolutions
                                    .insert((key.clone(), path.clone()), resolution);
                                if let Some(change) = this
                                    .sync_ui
                                    .plan
                                    .as_ref()
                                    .and_then(|p| p.changes.iter().find(|c| c.key == key))
                                    && change.fields.iter().filter(|f| f.conflict).all(|f| {
                                        this.sync_ui
                                            .selection
                                            .resolutions
                                            .contains_key(&(key.clone(), f.path.clone()))
                                    })
                                {
                                    this.sync_ui.selection.included.insert(key.clone());
                                }
                                cx.notify();
                            })),
                    );
                }
                row = row.child(choices);
            }
            if [
                field.local.as_ref(),
                field.remote.as_ref(),
                field.base.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|value| value.to_string().chars().count() > 900)
            {
                let field = field.clone();
                row = row.child(
                    Button::new(SharedString::from(format!("full-diff-{}", field.path)))
                        .small()
                        .ghost()
                        .label("查看完整对比")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.sync_full_diff(&field, window, cx)
                        })),
                );
            }
            detail = detail.child(row);
        }
        if change.fields.len() > 40 {
            detail = detail.child(div().text_xs().child(format!(
                "另有 {} 个字段，请查看完整原文。",
                change.fields.len() - 40
            )));
        }
        detail.child(div().text_xs().text_color(cx.theme().muted_foreground).child("调试参数值、Body、认证、上传文件、响应历史和请求 ID 均保留。定义变化后请检查草稿兼容性。")).into_any_element()
    }
    fn sync_full_diff(
        &mut self,
        field: &apifox::FieldChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor =
            |value: &Option<serde_json::Value>, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    let mut state = EditorState::new(window, cx).language("json");
                    state.set_value(
                        serde_json::to_string_pretty(&value).unwrap_or_default(),
                        window,
                        cx,
                    );
                    state
                })
            };
        let local = editor(&field.local, window, cx);
        let remote = editor(&field.remote, window, cx);
        let title = field_label(&field.path);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(title.clone()).w(px(960.)).child(
                h_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child("当前本地")
                            .child(Editor::new(&local).readonly(true).h(px(360.))),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child("Apifox 最新")
                            .child(Editor::new(&remote).readonly(true).h(px(360.))),
                    ),
            )
        });
    }
    fn sync_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(source) = self
            .sync_ui
            .selected
            .and_then(|id| self.project.sync_sources.iter().find(|s| s.id == id))
        else {
            return div().into_any_element();
        };
        let id = source.id;
        let mut frequency = h_flex().gap_2();
        for (minutes, label) in [
            (0, "仅手动"),
            (5, "5 分钟"),
            (15, "15 分钟"),
            (30, "30 分钟"),
        ] {
            frequency = frequency.child(
                Button::new(("sync-frequency", minutes as usize))
                    .small()
                    .ghost()
                    .selected(source.check_minutes == minutes)
                    .label(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(s) = this.project.sync_sources.iter_mut().find(|s| s.id == id) {
                            s.check_minutes = minutes;
                            s.next_check_at =
                                chrono::Utc::now().timestamp() + i64::from(minutes) * 60;
                        }
                        this.dirty(cx);
                    })),
            )
        }
        let paused = source.paused;
        let auto = Checkbox::new("sync-auto")
            .label("自动应用无冲突的定义更新")
            .checked(source.auto_apply)
            .on_click(cx.listener(move |this, value, _, cx| {
                if let Some(s) = this.project.sync_sources.iter_mut().find(|s| s.id == id) {
                    s.auto_apply = *value;
                }
                this.dirty(cx);
            }));
        let follow = Checkbox::new("sync-follow-folders")
            .label("跟随远端目录，无冲突时更新")
            .checked(source.follow_folders)
            .on_click(cx.listener(move |this, value, _, cx| {
                if let Some(s) = this.project.sync_sources.iter_mut().find(|s| s.id == id) {
                    s.follow_folders = *value;
                }
                this.dirty(cx);
            }));
        let pause = Button::new("sync-pause")
            .small()
            .label(if paused {
                "恢复检查"
            } else {
                "暂停检查"
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(s) = this.project.sync_sources.iter_mut().find(|s| s.id == id) {
                    s.paused = !s.paused;
                }
                this.dirty(cx);
            }));
        let disconnect = Button::new("sync-disconnect")
            .small()
            .ghost()
            .label("断开连接，保留本地接口")
            .disabled(self.sync_ui.busy.contains(&id))
            .on_click(cx.listener(move |this, _, _, cx| this.disconnect_sync(id, cx)));
        let note = |text: &'static str| {
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(text)
        };
        let mut settings = v_flex()
            .id("sync-settings-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_5()
            .gap_5()
            .child(
                v_flex()
                    .gap_2()
                    .child("检查频率")
                    .child(note(
                        "仅在应用运行时检查；断网重试使用退避。令牌失效时自动暂停。",
                    ))
                    .child(frequency),
            )
            .child(auto)
            .child(note("冲突、远端移除和本地已删除项始终留待处理。"))
            .child(follow)
            .child(h_flex().gap_3().child(pause).child(disconnect))
            .child(note(
                "同步 HTTP 定义、Schema、示例和目录。gRPC、WS、完整环境与脚本不通过此接口同步。",
            ));
        for binding in source
            .bindings
            .iter()
            .filter(|b| !b.compatibility.is_empty())
        {
            let request_id = binding.request_id;
            let name = self
                .project
                .requests
                .iter()
                .find(|r| r.id == request_id)
                .map(|r| r.name.clone())
                .unwrap_or("已删除请求".into());
            settings = settings.child(
                v_flex()
                    .gap_2()
                    .child(format!("{name} · 草稿兼容性"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(binding.compatibility.join("\n")),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new(SharedString::from(format!("draft-open-{request_id}")))
                                    .small()
                                    .label("打开请求")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some(index) = this
                                            .project
                                            .requests
                                            .iter()
                                            .position(|r| r.id == request_id)
                                        {
                                            this.switch_request(index, window, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "draft-checked-{request_id}"
                                )))
                                .small()
                                .ghost()
                                .label("标记已检查")
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if let Some(source) = this
                                            .project
                                            .sync_sources
                                            .iter_mut()
                                            .find(|s| s.id == id)
                                            && let Some(binding) = source
                                                .bindings
                                                .iter_mut()
                                                .find(|b| b.request_id == request_id)
                                        {
                                            binding.compatibility.clear();
                                        }
                                        this.dirty(cx);
                                    },
                                )),
                            ),
                    ),
            );
        }
        settings.into_any_element()
    }
    fn disconnect_sync(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.project.sync_sources.retain(|s| s.id != id);
        for request in &mut self.project.requests {
            if request
                .openapi
                .as_ref()
                .is_some_and(|m| m.contract_id == id)
            {
                request.openapi = None;
                request.source = None;
            }
        }
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            if let Err(error) = apifox::remove_token(id) {
                let _ = tx.blocking_send(WorkerEvent::Notice(format!("连接已断开；{error}")));
            }
        });
        self.sync_ui.selected = self.project.sync_sources.first().map(|s| s.id);
        self.sync_ui.preview_revision = None;
        self.notice = "已断开连接，本地接口和调试数据已保留".into();
        self.dirty(cx);
    }
    fn sync_history(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(source) = self
            .sync_ui
            .selected
            .and_then(|id| self.project.sync_sources.iter().find(|s| s.id == id))
        else {
            return div().into_any_element();
        };
        let id = source.id;
        let mut history = v_flex()
            .id("sync-history-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_5()
            .gap_3();
        if source.runs.is_empty() {
            history = history.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("尚未应用同步变更。成功应用后保存记录和撤销信息。"),
            );
        }
        for run in &source.runs {
            let run_id = run.id;
            history = history.child(
                h_flex()
                    .gap_3()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Icon::new(IconName::Check)
                            .size(px(16.))
                            .text_color(cx.theme().primary),
                    )
                    .child(
                        v_flex().flex_1().gap_1().child(run.summary.clone()).child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    chrono::DateTime::parse_from_rfc3339(&run.at)
                                        .map(|d| {
                                            d.with_timezone(&chrono::Local)
                                                .format("%m-%d %H:%M:%S")
                                                .to_string()
                                        })
                                        .unwrap_or(run.at.clone()),
                                ),
                        ),
                    )
                    .when(
                        run.undo.is_some() && run.generation == source.generation,
                        |row| {
                            row.child(
                                Button::new(SharedString::from(format!("sync-undo-{run_id}")))
                                    .small()
                                    .ghost()
                                    .label("撤销本批变更")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        match apifox::undo(&mut this.project, id, run_id) {
                                            Ok(()) => {
                                                if !this.project.tabs().is_empty() {
                                                    this.load_request(window, cx);
                                                }
                                                this.notice =
                                                    "已撤销本批定义改动，本地调试值保留".into();
                                                this.dirty(cx);
                                            }
                                            Err(error) => {
                                                this.notice = error.to_string();
                                                cx.notify();
                                            }
                                        }
                                    })),
                            )
                        },
                    ),
            );
        }
        history.into_any_element()
    }
}

fn sync_form_field(label: &'static str, input: AnyElement) -> AnyElement {
    h_flex()
        .items_center()
        .gap_3()
        .child(div().w(px(90.)).flex_shrink_0().text_sm().child(label))
        .child(div().flex_1().min_w_0().child(input))
        .into_any_element()
}
fn sync_badge(kind: ChangeKind, cx: &App) -> AnyElement {
    div()
        .text_size(px(10.))
        .px_1()
        .rounded(px(3.))
        .bg(cx.theme().muted)
        .text_color(match kind {
            ChangeKind::Conflict | ChangeKind::LocalDeleted => rgb(0x9c741b).into(),
            ChangeKind::Removed => rgb(0xaa535d).into(),
            _ => cx.theme().primary,
        })
        .child(kind.label())
        .into_any_element()
}
fn sync_value(label: &'static str, value: Option<&serde_json::Value>, cx: &App) -> AnyElement {
    let text = value
        .map(|v| {
            if let Some(s) = v.as_str() {
                s.to_owned()
            } else {
                serde_json::to_string_pretty(v).unwrap_or_default()
            }
        })
        .unwrap_or("（不存在）".into());
    let mut shortened = text.chars().take(900).collect::<String>();
    if shortened.len() < text.len() {
        shortened.push_str("\n…请查看完整原文");
    }
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .p_3()
        .rounded(px(5.))
        .bg(cx.theme().muted)
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(
            div()
                .text_size(px(12.))
                .font_family("Menlo")
                .child(shortened),
        )
        .into_any_element()
}
fn field_label(path: &str) -> String {
    match path {
        "/name" => "接口名称".into(),
        "/url" => "地址模板".into(),
        "/path" => "接口路径".into(),
        "/method" => "HTTP Method".into(),
        "/folder" => "逻辑文件夹".into(),
        "/description" => "接口说明".into(),
        "/schema" => "请求体 Schema".into(),
        _ => {
            let mut label = path.to_owned();
            for (prefix, title) in [
                ("/contract/operation/requestBody", "请求体定义"),
                ("/contract/operation/responses", "响应定义"),
                ("/contract/operation/parameters", "请求参数定义"),
                ("/contract/operation/tags", "接口标签"),
                ("/contract/pathParameters", "路径级参数"),
                ("/contract/dependencies", "引用模型"),
                ("/contract/security", "认证方案"),
                ("/contract/operation", "接口定义"),
                ("/schema", "请求体 Schema"),
            ] {
                if let Some(suffix) = path.strip_prefix(prefix) {
                    label = format!("{title}{suffix}");
                    break;
                }
            }
            label
                .trim_start_matches('/')
                .split('/')
                .map(|s| s.replace("~1", "/").replace("~0", "~"))
                .collect::<Vec<_>>()
                .join(" / ")
        }
    }
}
