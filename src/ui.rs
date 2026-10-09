use crate::{
    model::*,
    storage::ProjectStore,
    transport::{self, RunRecord, SessionCommand, SessionEvent},
};
use gpui_kit::component::{
    ActiveTheme, Disableable, IndexPath, Selectable, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Editor, EditorState, Input, InputEvent, InputState},
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::*;
use prost_reflect::DescriptorPool;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

gpui_kit::actions!(zeph, [SendRequest, SaveProject, Quit]);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Query,
    Path,
    Headers,
    Body,
    Auth,
    Settings,
}
impl Tab {
    fn label(self) -> &'static str {
        match self {
            Self::Query => "Query",
            Self::Path => "Path",
            Self::Headers => "Headers",
            Self::Body => "Body",
            Self::Auth => "Auth",
            Self::Settings => "设置",
        }
    }
}
#[derive(Clone, Copy)]
enum Field {
    Name,
    Url,
    Body,
    Token,
    User,
    Password,
    Timeout,
    Proxy,
    Environment,
    Search,
}
struct ParamInputs {
    name: Entity<InputState>,
    value: Entity<InputState>,
}
struct Job {
    id: Uuid,
    request: Uuid,
    cancel: CancellationToken,
    commands: Option<tokio::sync::mpsc::Sender<SessionCommand>>,
    half_closed: bool,
}
enum WorkerEvent {
    Record(Uuid, RunRecord),
    Session(Uuid, Uuid, SessionEvent),
    Done(Uuid),
    Pool(Uuid, Result<DescriptorPool, String>, String),
    Imported(Result<crate::openapi::ImportedSpec, String>),
    Saved(u64, Result<(), String>),
    Notice(String),
}

pub struct Workbench {
    project: Project,
    store: ProjectStore,
    runtime: Arc<tokio::runtime::Runtime>,
    tx: mpsc::Sender<WorkerEvent>,
    rx: mpsc::Receiver<WorkerEvent>,
    name: Entity<InputState>,
    url: Entity<InputState>,
    search: Entity<InputState>,
    token: Entity<InputState>,
    user: Entity<InputState>,
    password: Entity<InputState>,
    timeout: Entity<InputState>,
    proxy: Entity<InputState>,
    body: Entity<EditorState>,
    response: Entity<EditorState>,
    environment: Entity<EditorState>,
    schema: Entity<EditorState>,
    method: Entity<SelectState<Vec<String>>>,
    body_kind: Entity<SelectState<Vec<String>>>,
    rpc_method: Entity<SelectState<Vec<String>>>,
    env_select: Entity<SelectState<Vec<String>>>,
    auth_kind: Entity<SelectState<Vec<String>>>,
    params: Vec<ParamInputs>,
    subscriptions: Vec<Subscription>,
    param_subscriptions: Vec<Subscription>,
    tab: Tab,
    loading: bool,
    environment_open: bool,
    history_open: bool,
    response_headers: bool,
    pools: HashMap<Uuid, DescriptorPool>,
    records: HashMap<Uuid, RunRecord>,
    logs: HashMap<Uuid, String>,
    history: Vec<RunRecord>,
    job: Option<Job>,
    notice: String,
    revision: u64,
    saved_revision: u64,
    save_pending: bool,
    edited_at: Instant,
    _poll: Task<()>,
    focus: FocusHandle,
}
impl Workbench {
    pub fn new(
        project: Project,
        store: ProjectStore,
        runtime: Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = |window: &mut Window, cx: &mut Context<Self>, hint: &'static str| {
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        let name = input(window, cx, "请求名称");
        let url = input(window, cx, "https://api.example.com");
        let search = input(window, cx, "搜索请求…");
        let token = input(window, cx, "Token 或 {{token}}");
        let user = input(window, cx, "Username");
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Password")
        });
        let timeout = input(window, cx, "秒");
        let proxy = input(window, cx, "http://localhost:8888");
        let body = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let response = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let environment = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let schema = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let method = cx.new(|cx| {
            SelectState::new(
                HTTP_METHODS
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let body_kind = cx.new(|cx| {
            SelectState::new(
                BodyKind::ALL
                    .iter()
                    .map(|s| s.label().to_string())
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let rpc_method =
            cx.new(|cx| SelectState::new(Vec::<String>::new(), None, window, cx).searchable(true));
        let env_select = cx.new(|cx| {
            SelectState::new(
                project
                    .environments
                    .iter()
                    .map(|e| e.name.clone())
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(project.active_environment)),
                window,
                cx,
            )
        });
        let auth_kind = cx.new(|cx| {
            SelectState::new(
                vec!["None".into(), "Bearer".into(), "Basic".into()],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let (tx, rx) = mpsc::channel(32);
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| this.poll(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let history = store.history().unwrap_or_default();
        let mut this = Self {
            project,
            store,
            runtime,
            tx,
            rx,
            name,
            url,
            search,
            token,
            user,
            password,
            timeout,
            proxy,
            body,
            response,
            environment,
            schema,
            method,
            body_kind,
            rpc_method,
            env_select,
            auth_kind,
            params: vec![],
            subscriptions: vec![],
            param_subscriptions: vec![],
            tab: Tab::Query,
            loading: false,
            environment_open: false,
            history_open: false,
            response_headers: false,
            pools: HashMap::new(),
            records: HashMap::new(),
            logs: HashMap::new(),
            history,
            job: None,
            notice: "仅本地 · 请求尚未发送".into(),
            revision: 0,
            saved_revision: 0,
            save_pending: false,
            edited_at: Instant::now(),
            _poll: poll,
            focus: cx.focus_handle(),
        };
        for (entity, field) in [
            (this.name.clone(), Field::Name),
            (this.url.clone(), Field::Url),
            (this.search.clone(), Field::Search),
            (this.token.clone(), Field::Token),
            (this.user.clone(), Field::User),
            (this.password.clone(), Field::Password),
            (this.timeout.clone(), Field::Timeout),
            (this.proxy.clone(), Field::Proxy),
        ] {
            this.subscriptions.push(cx.subscribe_in(
                &entity,
                window,
                move |this, entity, event, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.edit(field, entity.read(cx).value().to_string(), window, cx);
                    }
                },
            ));
        }
        for (entity, field) in [
            (this.body.clone(), Field::Body),
            (this.environment.clone(), Field::Environment),
        ] {
            this.subscriptions.push(cx.subscribe_in(
                &entity,
                window,
                move |this, entity, event, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.edit(field, entity.read(cx).value().to_string(), window, cx);
                    }
                },
            ));
        }
        this.subscriptions.push(cx.subscribe_in(
            &this.method,
            window,
            |this, _, event: &SelectEvent<Vec<String>>, _, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                if !this.loading {
                    this.current_mut().method = value.clone();
                    this.dirty(cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.body_kind,
            window,
            |this, _, event: &SelectEvent<Vec<String>>, window, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                if !this.loading {
                    this.current_mut().body.kind = BodyKind::ALL
                        .into_iter()
                        .find(|k| k.label() == value)
                        .unwrap_or_default();
                    this.set_body_language(cx);
                    this.rebuild_params(window, cx);
                    this.dirty(cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.env_select,
            window,
            |this, _, event: &SelectEvent<Vec<String>>, window, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                if !this.loading
                    && let Some(i) = this
                        .project
                        .environments
                        .iter()
                        .position(|e| &e.name == value)
                {
                    this.project.active_environment = i;
                    this.load_environment(window, cx);
                    this.dirty(cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.rpc_method,
            window,
            |this, _, event: &SelectEvent<Vec<String>>, window, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                if !this.loading {
                    if this.current().grpc_method != *value {
                        this.current_mut().grpc_method = value.clone();
                        this.template(window, cx);
                    }
                    this.update_schema(window, cx);
                    this.dirty(cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.auth_kind,
            window,
            |this, _, event: &SelectEvent<Vec<String>>, _, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                if !this.loading {
                    this.current_mut().auth = match value.as_str() {
                        "Bearer" => Auth::Bearer(this.token.read(cx).value().to_string()),
                        "Basic" => Auth::Basic {
                            username: this.user.read(cx).value().to_string(),
                            password: this.password.read(cx).value().to_string(),
                        },
                        _ => Auth::None,
                    };
                    this.dirty(cx);
                }
            },
        ));
        for r in &this.project.requests {
            if r.protocol == Protocol::Grpc
                && r.proto_path.is_empty()
                && r.grpc_method.starts_with("shop.v1.OrderService/")
            {
                this.pools.insert(r.id, transport::grpc::example_pool());
            }
        }
        this.tab = if this.current().protocol == Protocol::Http {
            Tab::Query
        } else {
            Tab::Body
        };
        this.load_request(window, cx);
        this.load_environment(window, cx);
        this
    }
    fn current(&self) -> &Request {
        &self.project.requests[self.project.active_request]
    }
    fn current_mut(&mut self) -> &mut Request {
        &mut self.project.requests[self.project.active_request]
    }
    fn dirty(&mut self, cx: &mut Context<Self>) {
        self.revision += 1;
        self.edited_at = Instant::now();
        cx.notify();
    }
    fn edit(&mut self, field: Field, value: String, _window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        match field {
            Field::Search => {
                cx.notify();
                return;
            }
            Field::Environment => {
                cx.notify();
                return;
            }
            Field::Name => self.current_mut().name = value,
            Field::Url => self.current_mut().url = value,
            Field::Body => self.current_mut().body.text = value,
            Field::Token => {
                if let Auth::Bearer(token) = &mut self.current_mut().auth {
                    *token = value;
                }
            }
            Field::User => {
                if let Auth::Basic { username, .. } = &mut self.current_mut().auth {
                    *username = value;
                }
            }
            Field::Password => {
                if let Auth::Basic { password, .. } = &mut self.current_mut().auth {
                    *password = value;
                }
            }
            Field::Timeout => {
                if let Ok(seconds) = value.parse::<u64>() {
                    self.current_mut().timeout_secs = seconds.max(1);
                }
            }
            Field::Proxy => self.current_mut().proxy = value,
        }
        self.dirty(cx);
    }
    fn load_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        let r = self.current().clone();
        for (entity, value) in [
            (self.name.clone(), r.name),
            (self.url.clone(), r.url),
            (self.timeout.clone(), r.timeout_secs.to_string()),
            (self.proxy.clone(), r.proxy),
        ] {
            entity.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        self.body
            .update(cx, |state, cx| state.set_value(r.body.text, window, cx));
        self.method.update(cx, |state, cx| {
            state.set_selected_value(&r.method, window, cx)
        });
        self.body_kind.update(cx, |state, cx| {
            state.set_selected_value(&r.body.kind.label().to_string(), window, cx)
        });
        let (kind, token, user, password) = match r.auth {
            Auth::None => ("None", String::new(), String::new(), String::new()),
            Auth::Bearer(token) => ("Bearer", token, String::new(), String::new()),
            Auth::Basic { username, password } => ("Basic", String::new(), username, password),
        };
        self.auth_kind.update(cx, |state, cx| {
            state.set_selected_value(&kind.to_string(), window, cx)
        });
        for (entity, value) in [
            (self.token.clone(), token),
            (self.user.clone(), user),
            (self.password.clone(), password),
        ] {
            entity.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        // Re-open saved local descriptors; never silently substitute a different service.
        if r.protocol == Protocol::Grpc
            && !self.pools.contains_key(&r.id)
            && !r.proto_path.is_empty()
        {
            match transport::grpc::load_proto(&PathBuf::from(&r.proto_path)) {
                Ok(pool) => {
                    self.pools.insert(r.id, pool);
                }
                Err(e) => self.notice = format!("Proto 无法读取: {e}"),
            }
        }
        self.update_rpc_select(window, cx);
        self.update_schema(window, cx);
        self.update_response(window, cx);
        self.loading = false;
        self.set_body_language(cx);
        self.rebuild_params(window, cx);
        cx.notify();
    }
    fn load_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        let value = serde_json::to_string_pretty(&self.project.variables()).unwrap_or_default();
        self.environment
            .update(cx, |s, cx| s.set_value(value, window, cx));
        self.loading = false;
    }
    fn update_rpc_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let r = self.current();
        let selected = r.grpc_method.clone();
        let items = self
            .pools
            .get(&r.id)
            .map(transport::grpc::methods)
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.path)
            .collect();
        self.rpc_method.update(cx, |s, cx| {
            s.set_items(items, window, cx);
            s.set_selected_value(&selected, window, cx);
        });
    }
    fn update_schema(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let r = self.current();
        let text = if let Some(pool) = self.pools.get(&r.id) {
            match transport::grpc::method(pool, &r.grpc_method) {
                Ok(m) => format!(
                    "{}\n\nRequest: {}\n{}\n\nResponse: {}\n{}",
                    if m.is_client_streaming() {
                        if m.is_server_streaming() {
                            "Bidirectional streaming"
                        } else {
                            "Client streaming"
                        }
                    } else if m.is_server_streaming() {
                        "Server streaming"
                    } else {
                        "Unary"
                    },
                    m.input().full_name(),
                    transport::grpc::message_template(m.input()),
                    m.output().full_name(),
                    transport::grpc::message_template(m.output())
                ),
                Err(e) => e.to_string(),
            }
        } else if let Some(schema) = &r.schema {
            serde_json::to_string_pretty(schema).unwrap_or_default()
        } else if r.protocol == Protocol::Grpc {
            "导入 .proto 或 descriptor set，\n或从目标服务 Reflection 读取。\n\n随后可在 Method 中选择方法。".into()
        } else {
            "导入 OpenAPI 后显示请求 Schema。\n\n支持 Swagger 2.0、OpenAPI 3.0、3.1\n未实现的特性会显示导入诊断。".into()
        };
        self.schema
            .update(cx, |s, cx| s.set_value(text, window, cx));
    }
    fn param_data(&self) -> &Vec<Parameter> {
        match self.tab {
            Tab::Query => &self.current().query,
            Tab::Path => &self.current().path,
            Tab::Headers => &self.current().headers,
            _ => &self.current().body.fields,
        }
    }
    fn param_data_mut(&mut self) -> &mut Vec<Parameter> {
        match self.tab {
            Tab::Query => &mut self.current_mut().query,
            Tab::Path => &mut self.current_mut().path,
            Tab::Headers => &mut self.current_mut().headers,
            _ => &mut self.current_mut().body.fields,
        }
    }
    fn rebuild_params(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        self.param_subscriptions.clear();
        self.params.clear();
        let data = self.param_data().clone();
        for (i, p) in data.into_iter().enumerate() {
            let name = cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder("Key");
                s.set_value(p.name, window, cx);
                s
            });
            let value = cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder("Value / {{variable}}");
                s.set_value(p.value, window, cx);
                s
            });
            for (entity, is_name) in [(name.clone(), true), (value.clone(), false)] {
                self.param_subscriptions.push(cx.subscribe_in(
                    &entity,
                    window,
                    move |this, entity, event, _, cx| {
                        if matches!(event, InputEvent::Change) && !this.loading {
                            let text = entity.read(cx).value().to_string();
                            if let Some(p) = this.param_data_mut().get_mut(i) {
                                if is_name {
                                    p.name = text;
                                } else {
                                    p.value = text;
                                }
                                this.dirty(cx);
                            }
                        }
                    },
                ));
            }
            self.params.push(ParamInputs { name, value });
        }
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.save_pending || self.revision == self.saved_revision {
            return;
        }
        self.save_pending = true;
        let project = self.project.clone();
        let store = self.store.clone();
        let tx = self.tx.clone();
        let revision = self.revision;
        self.runtime.spawn_blocking(move || {
            let result = store
                .save_versioned(&project, revision)
                .map_err(|e| e.to_string());
            let _ = tx.blocking_send(WorkerEvent::Saved(revision, result));
        });
        cx.notify();
    }
    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = false;
        for _ in 0..100 {
            let Ok(event) = self.rx.try_recv() else {
                break;
            };
            changed = true;
            match event {
                WorkerEvent::Saved(rev, result) => {
                    self.save_pending = false;
                    match result {
                        Ok(()) => {
                            self.saved_revision = rev;
                        }
                        Err(e) => {
                            self.notice = format!("保存失败: {e}");
                            self.edited_at = Instant::now() + Duration::from_secs(5);
                        }
                    }
                }
                WorkerEvent::Notice(text) => self.notice = text,
                WorkerEvent::Record(job, record) => {
                    if self.job.as_ref().is_some_and(|j| j.id == job) {
                        self.job = None;
                    }
                    self.notice = format!("{} · {} ms", record.status, record.elapsed_ms);
                    self.records.insert(record.request_id, record.clone());
                    self.history.insert(0, record);
                    self.history.truncate(100);
                    self.update_response(window, cx);
                }
                WorkerEvent::Session(job, id, event) => {
                    if self.job.as_ref().is_some_and(|j| j.id == job) {
                        let log = self.logs.entry(id).or_default();
                        if log.len() + event_text(&event).len() + 40 < 1024 * 1024 {
                            log.push_str(&format!(
                                "{}  {}\n\n",
                                chrono::Local::now().format("%H:%M:%S%.3f"),
                                event_text(&event)
                            ));
                        }
                        if id == self.current().id {
                            self.update_response(window, cx);
                        }
                    }
                }
                WorkerEvent::Done(job) => {
                    if self.job.as_ref().is_some_and(|j| j.id == job) {
                        self.job = None;
                    }
                }
                WorkerEvent::Pool(id, result, source) => match result {
                    Ok(pool) => {
                        let count = transport::grpc::methods(&pool).len();
                        let valid = transport::grpc::method(
                            &pool,
                            &self
                                .project
                                .requests
                                .iter()
                                .find(|r| r.id == id)
                                .map(|r| r.grpc_method.clone())
                                .unwrap_or_default(),
                        )
                        .is_ok();
                        let first = transport::grpc::methods(&pool)
                            .first()
                            .map(|m| m.path.clone())
                            .unwrap_or_default();
                        self.pools.insert(id, pool);
                        if let Some(r) = self.project.requests.iter_mut().find(|r| r.id == id) {
                            r.proto_path = source;
                            if !valid {
                                r.grpc_method = first;
                            }
                        }
                        self.notice = format!("已读取 {count} 个 gRPC 方法");
                        if id == self.current().id {
                            self.loading = true;
                            self.update_rpc_select(window, cx);
                            self.update_schema(window, cx);
                            self.loading = false;
                        }
                        self.dirty(cx);
                    }
                    Err(e) => self.notice = e,
                },
                WorkerEvent::Imported(result) => match result {
                    Ok(import) => {
                        let count = import.requests.len();
                        self.project.requests.extend(import.requests);
                        self.notice = format!(
                            "已导入 OpenAPI {} · {count} 个接口{}",
                            import.version,
                            if import.diagnostics.is_empty() {
                                String::new()
                            } else {
                                format!(" · {} 条诊断（点击导入诊断）", import.diagnostics.len())
                            }
                        );
                        self.logs
                            .insert(Uuid::nil(), import.diagnostics.join("\n\n"));
                        self.dirty(cx);
                    }
                    Err(e) => self.notice = format!("导入失败: {e}"),
                },
            }
        }
        if self.revision != self.saved_revision
            && !self.save_pending
            && Instant::now().saturating_duration_since(self.edited_at) > Duration::from_millis(500)
        {
            self.save(cx);
        }
        if changed {
            cx.notify();
        }
    }
    fn update_response(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.current().id;
        let text = if self.response_headers {
            self.records
                .get(&id)
                .map(|r| {
                    r.headers
                        .iter()
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default()
        } else if self.current().protocol != Protocol::Http {
            self.logs
                .get(&id)
                .cloned()
                .or_else(|| self.records.get(&id).map(|r| r.body.clone()))
                .unwrap_or_else(|| {
                    "还没有连接。连接后这里显示真实的消息、Metadata 和结束状态。".into()
                })
        } else {
            self.records
                .get(&id)
                .map(|r| r.body.clone())
                .unwrap_or_else(|| {
                    "还没有响应。\n\n配置 URL 和参数后，点击发送或按 ⌘ Enter。".into()
                })
        };
        self.response
            .update(cx, |s, cx| s.set_value(text, window, cx));
    }
    fn send(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.job.is_some() {
            self.notice = "已有请求正在执行，先停止当前请求。".into();
            cx.notify();
            return;
        }
        let mut request = self.current().clone();
        // Programmatic Editor updates deliberately do not emit InputEvent::Change.
        request.body.text = self.body.read(cx).value().to_string();
        let vars = self.project.variables();
        let environment = self.project.environments[self.project.active_environment]
            .name
            .clone();
        let tx = self.tx.clone();
        let store = self.store.clone();
        let cancel = CancellationToken::new();
        let job = Uuid::new_v4();
        let id = request.id;
        if let Err(e) = request.resolved_url(&vars) {
            self.notice = e.to_string();
            cx.notify();
            return;
        }
        if request.protocol == Protocol::Http {
            let token = cancel.clone();
            self.runtime.spawn(async move {
                let started = Instant::now();
                let record = match transport::http::execute(
                    request.clone(),
                    vars,
                    environment.clone(),
                    token,
                )
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        let mut r = RunRecord::error(&request, environment, format!("{e:#}"));
                        r.elapsed_ms = started.elapsed().as_millis();
                        r
                    }
                };
                let saved = store.record(&record);
                let _ = tx.send(WorkerEvent::Record(job, record)).await;
                if let Err(e) = saved {
                    let _ = tx
                        .send(WorkerEvent::Notice(format!("历史保存失败: {e}")))
                        .await;
                }
            });
            self.job = Some(Job {
                id: job,
                request: id,
                cancel,
                commands: None,
                half_closed: false,
            });
        } else {
            let pool = self.pools.get(&id).cloned();
            if request.protocol == Protocol::Grpc && pool.is_none() {
                self.notice = "先导入 proto 或读取 Reflection，再选择 Method。".into();
                cx.notify();
                return;
            }
            let (commands, rx) = tokio::sync::mpsc::channel(32);
            let (events, mut incoming) = tokio::sync::mpsc::channel(64);
            let token = cancel.clone();
            let task_request = request.clone();
            let end_tx = events.clone();
            self.runtime.spawn(async move {
                let result = if task_request.protocol == Protocol::Grpc {
                    transport::grpc::run(task_request, vars, pool.unwrap(), rx, events, token).await
                } else {
                    transport::websocket::run(task_request, vars, rx, events, token).await
                };
                if let Err(e) = result {
                    let _ = end_tx.send(SessionEvent::Failed(format!("{e:#}"))).await;
                }
            });
            self.runtime.spawn(async move {
                let started = Instant::now();
                let mut record = RunRecord::error(&request, environment, String::new());
                record.method = request.protocol.label().into();
                record.status = "已结束".into();
                record.error = false;
                while let Some(mut event) = incoming.recv().await {
                    record.truncated |= bound_event(&mut event);
                    match &event {
                        SessionEvent::Metadata(headers) => record.headers.extend(headers.clone()),
                        SessionEvent::Finished { code, trailers } => {
                            record.status = code.clone();
                            record.headers.extend(trailers.clone());
                        }
                        SessionEvent::Failed(error) => {
                            record.status = error.clone();
                            record.error = true;
                        }
                        _ => {}
                    }
                    let text = event_text(&event);
                    if record.body.len() + text.len() < 1024 * 1024 {
                        record.body.push_str(&text);
                        record.body.push_str("\n\n");
                    } else {
                        record.truncated = true;
                    }
                    let _ = tx.send(WorkerEvent::Session(job, id, event)).await;
                }
                record.elapsed_ms = started.elapsed().as_millis();
                record.bytes = record.body.len();
                let saved = store.record(&record);
                let _ = tx.send(WorkerEvent::Record(job, record)).await;
                let _ = tx.send(WorkerEvent::Done(job)).await;
                if let Err(e) = saved {
                    let _ = tx
                        .send(WorkerEvent::Notice(format!("历史保存失败: {e}")))
                        .await;
                }
            });
            self.logs.insert(id, String::new());
            self.job = Some(Job {
                id: job,
                request: id,
                cancel,
                commands: Some(commands),
                half_closed: false,
            });
        }
        self.notice = "正在执行…".into();
        cx.notify();
    }
    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.job {
            job.cancel.cancel();
        }
        self.notice = "正在停止…".into();
        cx.notify();
    }
    fn command(&mut self, command: SessionCommand, cx: &mut Context<Self>) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        if job.request != self.project.requests[self.project.active_request].id {
            return;
        }
        let half = matches!(command, SessionCommand::HalfClose);
        if let Some(sender) = &job.commands {
            match sender.try_send(command) {
                Ok(()) => {
                    if half {
                        job.half_closed = true;
                    }
                }
                Err(e) => self.notice = format!("消息未发送: {e}"),
            }
        }
        cx.notify();
    }
    fn import_openapi(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("OpenAPI", &["json", "yaml", "yml"])
            .pick_file()
        {
            let tx = self.tx.clone();
            let directory = self.store.directory.join("contracts");
            self.runtime.spawn_blocking(move || {
                let result = crate::openapi::import(&path)
                    .and_then(|import| {
                        crate::storage::save_contract(&directory, &import)?;
                        Ok(import)
                    })
                    .map_err(|e| format!("{e:#}"));
                let _ = tx.blocking_send(WorkerEvent::Imported(result));
            });
            self.notice = "正在解析本地 OpenAPI…".into();
            cx.notify();
        }
    }
    fn import_proto(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Protocol Buffers", &["proto", "bin", "protoset", "pb"])
            .pick_file()
        {
            let tx = self.tx.clone();
            let id = self.current().id;
            let directory = self.store.directory.join(".zeph/descriptors");
            self.runtime.spawn_blocking(move || {
                let source = directory.join(format!("{id}.bin"));
                let result = transport::grpc::load_proto(&path)
                    .and_then(|pool| {
                        std::fs::create_dir_all(&directory)?;
                        std::fs::write(&source, pool.encode_to_vec())?;
                        Ok(pool)
                    })
                    .map_err(|e| format!("{e:#}"));
                let _ = tx.blocking_send(WorkerEvent::Pool(
                    id,
                    result,
                    source.to_string_lossy().into_owned(),
                ));
            });
            self.notice = "正在读取 Proto…".into();
            cx.notify();
        }
    }
    fn reflect(&mut self, cx: &mut Context<Self>) {
        let request = self.current().clone();
        let vars = self.project.variables();
        let tx = self.tx.clone();
        let directory = self.store.directory.join(".zeph/descriptors");
        self.runtime.spawn(async move {
            let result = tokio::time::timeout(
                Duration::from_secs(request.timeout_secs.max(1)),
                transport::grpc::discover(&request, &vars),
            )
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| format!("{e:#}")));
            let source = directory.join(format!("{}.bin", request.id));
            let result = result.and_then(|pool| {
                std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                std::fs::write(&source, pool.encode_to_vec()).map_err(|e| e.to_string())?;
                Ok(pool)
            });
            let _ = tx
                .send(WorkerEvent::Pool(
                    request.id,
                    result,
                    source.to_string_lossy().into_owned(),
                ))
                .await;
        });
        self.notice = "正在读取服务 Reflection…".into();
        cx.notify();
    }
    fn new_request(&mut self, protocol: Protocol, window: &mut Window, cx: &mut Context<Self>) {
        let mut request = Request::new(protocol.clone(), "新请求");
        request.url = match protocol {
            Protocol::Http => "{{base_url}}",
            Protocol::Grpc => "{{grpc_host}}",
            Protocol::WebSocket => "{{ws_url}}",
        }
        .into();
        self.project.requests.push(request);
        self.project.active_request = self.project.requests.len() - 1;
        self.tab = if protocol == Protocol::Http {
            Tab::Query
        } else {
            Tab::Body
        };
        self.load_request(window, cx);
        self.dirty(cx);
    }
    fn choose_file(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            let text = path.to_string_lossy().into_owned();
            if let Some(i) = index {
                if let Some(p) = self.current_mut().body.fields.get_mut(i) {
                    p.value = text;
                    p.file = true;
                }
                self.rebuild_params(window, cx);
            } else {
                self.current_mut().body.file_path = text;
            }
            self.dirty(cx);
        }
    }
    fn set_body_language(&self, cx: &mut Context<Self>) {
        let language = if self.current().protocol == Protocol::Http
            && self.current().body.kind != BodyKind::Json
        {
            "text"
        } else {
            "json"
        };
        self.body
            .update(cx, |s, cx| s.set_highlighter(language, cx));
    }
    fn template(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let r = self.current();
        if let Some(pool) = self.pools.get(&r.id)
            && let Ok(m) = transport::grpc::method(pool, &r.grpc_method)
        {
            let text = transport::grpc::request_template(m.input());
            self.current_mut().body.text = text.clone();
            self.body.update(cx, |s, cx| s.set_value(text, window, cx));
            self.dirty(cx);
        }
    }
    fn save_environment(&mut self, cx: &mut Context<Self>) {
        match serde_json::from_str::<std::collections::BTreeMap<String, String>>(
            &self.environment.read(cx).value(),
        ) {
            Ok(vars) => {
                self.project.environments[self.project.active_environment].variables = vars;
                self.environment_open = false;
                self.dirty(cx);
                self.notice = "环境变量已更新".into();
            }
            Err(e) => {
                self.notice = format!("环境变量需要 JSON 字符串键值表: {e}");
                cx.notify();
            }
        }
    }
    fn switch_request(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.project.active_request = index;
        self.environment_open = false;
        self.history_open = false;
        self.tab = if self.current().protocol == Protocol::Http {
            Tab::Query
        } else {
            Tab::Body
        };
        self.load_request(window, cx);
        self.dirty(cx);
    }
    fn param_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let is_form = self.tab == Tab::Body;
        let is_multipart = is_form && self.current().body.kind == BodyKind::FormData;
        let mut rows = v_flex().gap_2().child(
            h_flex()
                .gap_3()
                .px_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(div().w(px(26.)).child("启用"))
                .child(div().w(px(170.)).child("KEY"))
                .child(div().flex_1().child(if is_multipart {
                    "VALUE / FILE PATH"
                } else {
                    "VALUE"
                })),
        );
        for (i, p) in self.param_data().iter().enumerate() {
            if let Some(inputs) = self.params.get(i) {
                let mut row =
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Checkbox::new(("enabled", i)).checked(p.enabled).on_click(
                            cx.listener(move |this, value, _, cx| {
                                if let Some(p) = this.param_data_mut().get_mut(i) {
                                    p.enabled = *value;
                                }
                                this.dirty(cx);
                            }),
                        ))
                        .child(Input::new(&inputs.name).small().w(px(170.)))
                        .child(Input::new(&inputs.value).small().flex_1());
                if is_multipart {
                    row = row
                        .child(
                            Button::new(("file-type", i))
                                .small()
                                .ghost()
                                .label(if p.file { "File" } else { "Text" })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(p) = this.current_mut().body.fields.get_mut(i) {
                                        p.file = !p.file;
                                    }
                                    this.dirty(cx);
                                })),
                        )
                        .child(
                            Button::new(("file-pick", i))
                                .small()
                                .ghost()
                                .label("选择")
                                .disabled(!p.file)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.choose_file(Some(i), window, cx)
                                })),
                        );
                }
                row = row.child(
                    Button::new(("delete-param", i))
                        .small()
                        .ghost()
                        .label("×")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.param_data_mut().remove(i);
                            this.rebuild_params(window, cx);
                            this.dirty(cx);
                        })),
                );
                rows = rows.child(row);
            }
        }
        rows.child(
            Button::new("add-param")
                .small()
                .ghost()
                .label("＋ 添加参数")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.param_data_mut().push(Parameter::new("", ""));
                    this.rebuild_params(window, cx);
                    this.dirty(cx);
                })),
        )
        .into_any_element()
    }
    fn request_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let r = self.current();
        let protocol = r.protocol.clone();
        let mut content = v_flex().size_full().min_h_0().gap_3().p_4();
        match self.tab {
            Tab::Query | Tab::Path | Tab::Headers => {
                content = content.child(
                    div()
                        .id("parameters-scroll")
                        .overflow_y_scroll()
                        .flex_1()
                        .child(self.param_editor(cx)),
                )
            }
            Tab::Body => {
                if protocol == Protocol::Http {
                    content = content.child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .w(px(215.))
                                    .h(px(28.))
                                    .child(Select::new(&self.body_kind).small().w_full()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("独立于 HTTP method 选择 Body"),
                            ),
                    );
                }
                if protocol == Protocol::Grpc {
                    let client_streaming = self
                        .pools
                        .get(&r.id)
                        .and_then(|p| transport::grpc::method(p, &r.grpc_method).ok())
                        .is_some_and(|m| m.is_client_streaming());
                    let active = self
                        .job
                        .as_ref()
                        .is_some_and(|j| j.request == r.id && !j.half_closed);
                    content =
                        content.child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("grpc-template")
                                        .small()
                                        .ghost()
                                        .label("生成消息模板")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.template(window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("grpc-message")
                                        .small()
                                        .primary()
                                        .label("发送消息")
                                        .disabled(!active || !client_streaming)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.command(
                                                SessionCommand::Send(
                                                    this.body.read(cx).value().to_string(),
                                                ),
                                                cx,
                                            )
                                        })),
                                )
                                .child(
                                    Button::new("grpc-half-close")
                                        .small()
                                        .label("结束发送 · Half-close")
                                        .disabled(!active || !client_streaming)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.command(SessionCommand::HalfClose, cx)
                                        })),
                                ),
                        );
                }
                if protocol == Protocol::WebSocket {
                    let active = self.job.as_ref().is_some_and(|j| j.request == r.id);
                    content = content.child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("ws-send")
                                    .small()
                                    .primary()
                                    .label("发送 Text")
                                    .disabled(!active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.command(
                                            SessionCommand::Send(
                                                this.body.read(cx).value().to_string(),
                                            ),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new("ws-binary")
                                    .small()
                                    .label("发送文件 Binary")
                                    .disabled(!active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                                            match std::fs::read(path) {
                                                Ok(bytes) => this
                                                    .command(SessionCommand::SendBinary(bytes), cx),
                                                Err(e) => {
                                                    this.notice = e.to_string();
                                                    cx.notify();
                                                }
                                            }
                                        }
                                    })),
                            )
                            .child(
                                Button::new("ws-close")
                                    .small()
                                    .label("关闭连接")
                                    .disabled(!active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.command(SessionCommand::Close, cx)
                                    })),
                            ),
                    );
                }
                if protocol != Protocol::Http
                    || matches!(r.body.kind, BodyKind::Json | BodyKind::Text | BodyKind::Xml)
                {
                    content = content.child(
                        Editor::new(&self.body)
                            .h(relative(1.))
                            .flex_1()
                            .min_h_0()
                            .bordered(true),
                    );
                } else {
                    match r.body.kind {
                        BodyKind::None => {
                            content = content.child(
                                v_flex()
                                    .flex_1()
                                    .justify_center()
                                    .items_center()
                                    .gap_2()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("此请求不发送 Body")
                                    .child(
                                        div()
                                            .text_xs()
                                            .child("Query、Path 和 Headers 仍然参与请求"),
                                    ),
                            )
                        }
                        BodyKind::FormData | BodyKind::UrlEncoded => {
                            content = content.child(
                                div()
                                    .id("form-scroll")
                                    .flex_1()
                                    .overflow_y_scroll()
                                    .child(self.param_editor(cx)),
                            )
                        }
                        BodyKind::Binary => {
                            content = content.child(
                                v_flex()
                                    .gap_3()
                                    .child(div().text_sm().child(if r.body.file_path.is_empty() {
                                        "未选择文件".to_string()
                                    } else {
                                        r.body.file_path.clone()
                                    }))
                                    .child(
                                        Button::new("binary-pick").label("选择本地文件").on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.choose_file(None, window, cx)
                                            }),
                                        ),
                                    ),
                            )
                        }
                        _ => {}
                    }
                }
            }
            Tab::Auth => {
                content = content.child(
                    div()
                        .w(px(190.))
                        .h(px(34.))
                        .child(Select::new(&self.auth_kind).w_full()),
                );
                match r.auth {
                    Auth::None => {
                        content = content.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("不附加 Authorization。可在 Headers 中单独配置。"),
                        )
                    }
                    Auth::Bearer(_) => content = content.child(Input::new(&self.token).w_full()),
                    Auth::Basic { .. } => {
                        content = content
                            .child(Input::new(&self.user))
                            .child(Input::new(&self.password))
                    }
                };
                content = content.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("凭据和环境变量保存在当前本地项目文件中。"),
                );
            }
            Tab::Settings => {
                content = content
                    .child(
                        h_flex()
                            .gap_4()
                            .child(div().w(px(140.)).child("超时（秒）"))
                            .child(Input::new(&self.timeout).w(px(100.))),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .child(div().w(px(140.)).child("HTTP 代理"))
                            .child(Input::new(&self.proxy).flex_1()),
                    )
                    .child(
                        Checkbox::new("redirects")
                            .label("跟随 HTTP 重定向（最多 10 次）")
                            .checked(r.follow_redirects)
                            .on_click(cx.listener(|this, v, _, cx| {
                                this.current_mut().follow_redirects = *v;
                                this.dirty(cx);
                            })),
                    )
                    .child(
                        Checkbox::new("tls")
                            .label("验证 TLS 证书")
                            .checked(r.tls_verify)
                            .on_click(cx.listener(|this, v, _, cx| {
                                this.current_mut().tls_verify = *v;
                                this.dirty(cx);
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("gRPC / WS 使用系统证书；当前尚不支持代理设置。"),
                    )
            }
        }
        content.into_any_element()
    }
    fn response_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let record = self.records.get(&self.current().id);
        let running = self
            .job
            .as_ref()
            .is_some_and(|j| j.request == self.current().id);
        let status = if running {
            "执行中…".to_string()
        } else {
            record
                .map(|r| {
                    format!(
                        "{}   {} ms   {} B{}",
                        r.status,
                        r.elapsed_ms,
                        r.bytes,
                        if r.truncated { " · 预览截断" } else { "" }
                    )
                })
                .unwrap_or("尚未发送".into())
        };
        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .h(px(43.))
                    .flex_shrink_0()
                    .px_4()
                    .gap_3()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("响应"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(if record.is_some_and(|r| r.error) {
                                rgb(0xd84f5a).into()
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(status),
                    )
                    .child(
                        Button::new("response-body")
                            .small()
                            .ghost()
                            .selected(!self.response_headers)
                            .label("Body / Messages")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.response_headers = false;
                                this.update_response(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("response-headers")
                            .small()
                            .ghost()
                            .selected(self.response_headers)
                            .label("Headers")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.response_headers = true;
                                this.update_response(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("response-copy")
                            .small()
                            .ghost()
                            .label("复制")
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    this.response.read(cx).value().to_string(),
                                ))
                            })),
                    ),
            )
            .child(
                Editor::new(&self.response)
                    .h(relative(1.))
                    .readonly(true)
                    .appearance(false)
                    .flex_1()
                    .min_h_0(),
            )
            .into_any_element()
    }
    fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let filter = self.search.read(cx).value().to_lowercase();
        let mut list = v_flex().gap_1().p_2();
        for (i, r) in self.project.requests.iter().enumerate().filter(|(_, r)| {
            filter.is_empty()
                || format!("{} {} {}", r.name, r.method, r.protocol.label())
                    .to_lowercase()
                    .contains(&filter)
        }) {
            let tag = if r.protocol == Protocol::Http {
                r.method.as_str()
            } else {
                r.protocol.label()
            };
            list = list.child(
                h_flex()
                    .id(("request", i))
                    .role(Role::Button)
                    .aria_label(format!("{tag} {}", r.name))
                    .on_a11y_action(AccessibleAction::Click, {
                        let view = cx.entity().downgrade();
                        move |_, window, cx| {
                            let _ = view.update(cx, |this, cx| this.switch_request(i, window, cx));
                        }
                    })
                    .h(px(39.))
                    .px_2()
                    .gap_2()
                    .rounded_md()
                    .items_center()
                    .cursor_pointer()
                    .bg(if i == self.project.active_request {
                        cx.theme().accent
                    } else {
                        cx.theme().background
                    })
                    .hover(|s| s.bg(cx.theme().muted))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.switch_request(i, window, cx)),
                    )
                    .child(
                        div()
                            .w(px(70.))
                            .flex_shrink_0()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(method_color(tag))
                            .child(tag.to_owned()),
                    )
                    .child(div().text_sm().truncate().child(r.name.clone())),
            );
        }
        v_flex()
            .w(px(248.))
            .flex_shrink_0()
            .h_full()
            .bg(cx.theme().background)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .px_4()
                    .h(px(52.))
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("请求集合"),
                    )
                    .child(
                        Button::new("new-http")
                            .small()
                            .ghost()
                            .label("＋")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_request(Protocol::Http, window, cx)
                            })),
                    ),
            )
            .child(div().px_3().pb_3().child(Input::new(&self.search).small()))
            .child(
                h_flex()
                    .px_3()
                    .pb_2()
                    .gap_1()
                    .child(
                        Button::new("new-grpc")
                            .small()
                            .ghost()
                            .label("＋ gRPC")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_request(Protocol::Grpc, window, cx)
                            })),
                    )
                    .child(
                        Button::new("new-ws")
                            .small()
                            .ghost()
                            .label("＋ WS")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_request(Protocol::WebSocket, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .id("requests-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("history-open")
                            .ghost()
                            .label(format!("历史记录 · {}", self.history.len()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.history_open = !this.history_open;
                                this.environment_open = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("import-openapi")
                            .label("导入 OpenAPI")
                            .on_click(cx.listener(|this, _, _, cx| this.import_openapi(cx))),
                    ),
            )
            .into_any_element()
    }
    fn history_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = v_flex().gap_2().p_4();
        for (i, record) in self.history.iter().enumerate() {
            list = list.child(
                h_flex()
                    .id(("history", i))
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .gap_3()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let record = this.history[i].clone();
                        if let Some(index) = this
                            .project
                            .requests
                            .iter()
                            .position(|r| r.id == record.request_id)
                        {
                            this.switch_request(index, window, cx);
                            this.records.insert(record.request_id, record.clone());
                            this.logs.insert(record.request_id, record.body);
                            this.update_response(window, cx);
                        } else {
                            this.notice = "这条历史对应的请求已不存在。".into();
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(70.))
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(method_color(&record.method))
                            .child(record.method.clone()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(div().text_sm().child(record.request_name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{} · {}",
                                        record.environment,
                                        record
                                            .started_at
                                            .with_timezone(&chrono::Local)
                                            .format("%m-%d %H:%M:%S")
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .child(format!("{} · {} ms", record.status, record.elapsed_ms)),
                    ),
            );
        }
        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                h_flex()
                    .h(px(50.))
                    .px_4()
                    .items_center()
                    .child(div().flex_1().child("本地执行历史"))
                    .child(
                        Button::new("close-history")
                            .small()
                            .ghost()
                            .label("返回")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.history_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("history-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list),
            )
            .into_any_element()
    }
}
impl Render for Workbench {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let r = self.current();
        let protocol = r.protocol.clone();
        let current_running = self.job.as_ref().is_some_and(|j| j.request == r.id);
        let top = h_flex()
            .h(px(60.))
            .flex_shrink_0()
            .px_4()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().text_lg().font_weight(FontWeight::BOLD).child("zeph."))
            .child(
                div()
                    .pl_3()
                    .border_l_1()
                    .border_color(cx.theme().border)
                    .text_sm()
                    .child(self.project.name.clone()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("LOCAL WORKSPACE"),
            )
            .child(
                div()
                    .w(px(135.))
                    .h(px(28.))
                    .child(Select::new(&self.env_select).small().w_full()),
            )
            .child(
                Button::new("environment-open")
                    .small()
                    .ghost()
                    .label("变量")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.environment_open = !this.environment_open;
                        this.history_open = false;
                        this.load_environment(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("layout-toggle")
                    .small()
                    .ghost()
                    .label(if self.project.horizontal {
                        "左右布局"
                    } else {
                        "上下布局"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.project.horizontal = !this.project.horizontal;
                        this.dirty(cx);
                    })),
            )
            .child(
                Button::new("theme-toggle")
                    .small()
                    .ghost()
                    .label(if cx.theme().is_dark() {
                        "浅色"
                    } else {
                        "深色"
                    })
                    .on_click(cx.listener(|_, _, window, cx| {
                        let mode = if cx.theme().is_dark() {
                            ThemeMode::Light
                        } else {
                            ThemeMode::Dark
                        };
                        Theme::change(mode, Some(window), cx);
                        apply_theme(cx);
                    })),
            )
            .child(
                Button::new("save-project")
                    .small()
                    .label("保存")
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            );
        let main: AnyElement = if self.environment_open {
            v_flex().flex_1().min_h_0().p_5().gap_4().child(h_flex().gap_3().child(div().flex_1().child(format!("{} 环境变量",self.project.environments[self.project.active_environment].name))).child(Button::new("save-environment").primary().label("应用变量").on_click(cx.listener(|this,_,_,cx|this.save_environment(cx))))).child(div().text_sm().text_color(cx.theme().muted_foreground).child("用 JSON 字符串键值表编辑。在 URL、参数、Headers 和 Body 中使用 {{variable}}。" )).child(Editor::new(&self.environment).h(relative(1.)).flex_1()).into_any_element()
        } else if self.history_open {
            self.history_panel(cx)
        } else {
            let mut heading = h_flex()
                .h(px(53.))
                .flex_shrink_0()
                .px_4()
                .gap_3()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(method_color(protocol.label()))
                        .child(protocol.label()),
                )
                .child(Input::new(&self.name).small().appearance(false).flex_1())
                .child(
                    Button::new("duplicate-request")
                        .small()
                        .ghost()
                        .label("复制请求")
                        .on_click(cx.listener(|this, _, window, cx| {
                            let mut r = this.current().clone();
                            r.id = Uuid::new_v4();
                            r.name.push_str(" 副本");
                            if let Some(pool) = this.pools.get(&this.current().id).cloned() {
                                this.pools.insert(r.id, pool);
                            }
                            this.project.requests.push(r);
                            this.switch_request(this.project.requests.len() - 1, window, cx);
                        })),
                )
                .child(
                    Button::new("schema-toggle")
                        .small()
                        .ghost()
                        .selected(self.project.show_schema)
                        .label("Schema")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.project.show_schema = !this.project.show_schema;
                            this.dirty(cx);
                        })),
                );
            if protocol == Protocol::Grpc {
                heading = heading
                    .child(
                        Button::new("proto-import")
                            .small()
                            .label("导入 Proto")
                            .on_click(cx.listener(|this, _, _, cx| this.import_proto(cx))),
                    )
                    .child(
                        Button::new("grpc-reflection")
                            .small()
                            .label("Reflection")
                            .on_click(cx.listener(|this, _, _, cx| this.reflect(cx))),
                    );
            }
            let mut url_row = h_flex().px_4().pb_3().gap_2().items_center();
            if protocol == Protocol::Http {
                url_row = url_row.child(
                    div()
                        .w(px(118.))
                        .h(px(34.))
                        .child(Select::new(&self.method).w_full()),
                );
            }
            url_row = url_row.child(Input::new(&self.url).flex_1()).child(
                Button::new("send-request")
                    .primary()
                    .label(if protocol == Protocol::Http {
                        "发送  ⌘ ↵"
                    } else if protocol == Protocol::Grpc {
                        "调用"
                    } else {
                        "连接"
                    })
                    .disabled(self.job.is_some())
                    .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
            );
            if self.job.is_some() {
                url_row = url_row.child(
                    Button::new("stop-request")
                        .label("停止")
                        .on_click(cx.listener(|this, _, _, cx| this.stop(cx))),
                );
            }
            let mut editor = v_flex().flex_1().min_w_0().min_h_0();
            if protocol == Protocol::Grpc {
                editor = editor.child(
                    h_flex()
                        .px_4()
                        .pb_3()
                        .gap_3()
                        .items_center()
                        .child(div().text_sm().child("Method"))
                        .child(
                            div()
                                .flex_1()
                                .h(px(34.))
                                .child(Select::new(&self.rpc_method).w_full()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    self.pools
                                        .get(&r.id)
                                        .and_then(|p| {
                                            transport::grpc::method(p, &r.grpc_method).ok()
                                        })
                                        .map(|m| {
                                            match (m.is_client_streaming(), m.is_server_streaming())
                                            {
                                                (false, false) => "Unary",
                                                (false, true) => "Server stream",
                                                (true, false) => "Client stream",
                                                _ => "Bidi stream",
                                            }
                                        })
                                        .unwrap_or("未载入定义"),
                                ),
                        ),
                );
            }
            let mut tabs = h_flex()
                .px_3()
                .gap_1()
                .h(px(41.))
                .flex_shrink_0()
                .items_center()
                .border_b_1()
                .border_color(cx.theme().border);
            for (i, tab) in [
                Tab::Query,
                Tab::Path,
                Tab::Headers,
                Tab::Body,
                Tab::Auth,
                Tab::Settings,
            ]
            .into_iter()
            .enumerate()
            {
                if protocol != Protocol::Http && matches!(tab, Tab::Query | Tab::Path) {
                    continue;
                }
                tabs = tabs.child(
                    Button::new(("request-tab", i))
                        .small()
                        .ghost()
                        .selected(self.tab == tab)
                        .label(tab.label())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.tab = tab;
                            this.rebuild_params(window, cx);
                            cx.notify();
                        })),
                );
            }
            let request = v_flex()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(tabs)
                .child(self.request_editor(cx));
            let panels = if self.project.horizontal {
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(request)
                    .child(self.response_panel(cx))
                    .into_any_element()
            } else {
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(request)
                    .child(self.response_panel(cx))
                    .into_any_element()
            };
            editor = editor.child(panels);
            let mut work = h_flex().items_stretch().flex_1().min_h_0().child(editor);
            if self.project.show_schema {
                work = work.child(
                    v_flex()
                        .w(px(258.))
                        .flex_shrink_0()
                        .min_h_0()
                        .border_l_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .px_4()
                                .py_3()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("REQUEST SCHEMA / PROTO"),
                        )
                        .child(
                            Editor::new(&self.schema)
                                .h(relative(1.))
                                .readonly(true)
                                .appearance(false)
                                .flex_1()
                                .min_h_0(),
                        ),
                );
            }
            v_flex()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(heading)
                .child(url_row)
                .child(work)
                .into_any_element()
        };
        let status = if self.save_pending {
            "保存中…"
        } else if self.revision == self.saved_revision {
            "已保存到本地"
        } else {
            "待保存"
        };
        v_flex()
            .id("workbench")
            .track_focus(&self.focus)
            .key_context("Zeph")
            .on_action(cx.listener(|this, _: &SendRequest, window, cx| this.send(window, cx)))
            .on_action(cx.listener(|this, _: &SaveProject, _, cx| this.save(cx)))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(".AppleSystemUIFont")
            .text_sm()
            .child(top)
            .child(
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(main),
            )
            .child(
                h_flex()
                    .h(px(30.))
                    .flex_shrink_0()
                    .px_4()
                    .gap_3()
                    .items_center()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().child(if current_running {
                        "● 正在执行"
                    } else {
                        "● 本地工作区"
                    }))
                    .child(div().flex_1().truncate().child(self.notice.clone()))
                    .child(
                        Button::new("import-diagnostics")
                            .small()
                            .ghost()
                            .label("导入诊断")
                            .on_click(cx.listener(|this, _, window, cx| {
                                let text = this
                                    .logs
                                    .get(&Uuid::nil())
                                    .cloned()
                                    .unwrap_or("暂无导入诊断".into());
                                this.response
                                    .update(cx, |s, cx| s.set_value(text, window, cx));
                                cx.notify();
                            })),
                    )
                    .child(div().child(status)),
            )
    }
}
impl Drop for Workbench {
    fn drop(&mut self) {
        self.rx.close();
        if let Some(job) = &self.job {
            job.cancel.cancel();
        }
        if self.revision != self.saved_revision
            && let Err(e) = self.store.save_versioned(&self.project, self.revision)
        {
            log::error!("退出保存失败: {e}");
        }
    }
}
fn method_color(method: &str) -> Hsla {
    rgb(match method {
        "GET" | "HTTP" => 0x1b9b77,
        "POST" => 0xcb941d,
        "PUT" => 0x368dde,
        "PATCH" => 0x945ddb,
        "DELETE" => 0xdc5f6c,
        "HEAD" => 0x428f98,
        "OPTIONS" => 0x8371c4,
        "TRACE" => 0xc57638,
        "CONNECT" => 0x827c73,
        "gRPC" => 0x855bce,
        "WS" => 0x328eaa,
        _ => 0x6a7587,
    })
    .into()
}
fn event_text(event: &SessionEvent) -> String {
    match event {
        SessionEvent::Opened => "● 连接已建立".into(),
        SessionEvent::Sent(text) => format!("↑ Sent\n{text}"),
        SessionEvent::Received(text) => format!("↓ Received\n{text}"),
        SessionEvent::Metadata(headers) => format!(
            "Metadata\n{}",
            headers
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        SessionEvent::Finished { code, trailers } => format!(
            "■ {code}\n{}",
            trailers
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        SessionEvent::Failed(error) => format!("✕ {error}"),
    }
}

pub fn apply_theme(cx: &mut App) {
    let dark = cx.theme().is_dark();
    Theme::update(cx, |theme| {
        theme.colors.primary = rgb(0x19846f).into();
        theme.colors.primary_hover = rgb(0x14745f).into();
        theme.colors.primary_active = rgb(0x106451).into();
        theme.colors.primary_foreground = rgb(0xffffff).into();
        theme.colors.button_primary = rgb(0x19846f).into();
        theme.colors.button_primary_hover = rgb(0x14745f).into();
        theme.colors.button_primary_active = rgb(0x106451).into();
        theme.colors.button_primary_foreground = rgb(0xffffff).into();
        theme.colors.ring = rgb(0x19846f).into();
        theme.colors.accent = rgb(if dark { 0x193c35 } else { 0xeaf4f0 }).into();
        theme.colors.accent_foreground = rgb(if dark { 0xa8d8c9 } else { 0x1d6b56 }).into();
    });
}

fn bound_event(event: &mut SessionEvent) -> bool {
    let text = match event {
        SessionEvent::Sent(t) | SessionEvent::Received(t) | SessionEvent::Failed(t) => t,
        _ => return false,
    };
    if text.len() <= 65536 {
        return false;
    }
    let mut end = 65536;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("\n… [单条消息预览超过 64 KiB，已截断]");
    true
}
