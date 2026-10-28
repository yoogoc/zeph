use crate::{
    collections::{CollectionRow, DeletedFolder},
    model::*,
    storage::{ContractInfo, ProjectStore},
    transport::{self, RunRecord, SessionCommand, SessionEvent, SessionMessage, TimingMark},
    workspace::WorkspaceStrip,
};
use gpui_kit::base::ElementExt;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, IconName, IndexPath, Selectable, Sizable, Theme, TitleBar,
    WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::{DialogAction, DialogButtonProps, DialogClose, DialogFooter},
    h_flex,
    input::{Editor, EditorState, Input, InputEvent, InputState},
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    select::{Select, SelectEvent, SelectState},
    tab::{Tab as RequestTab, TabBar},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use prost_reflect::DescriptorPool;
use std::{
    cell::Cell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[path = "ui_scope_settings.rs"]
mod scope_settings;
#[path = "ui_structure.rs"]
mod structure_ui;
#[path = "ui_sync.rs"]
mod sync_ui;
#[path = "ui_value.rs"]
mod value_ui;

gpui_kit::actions!(
    zeph,
    [
        SendRequest,
        SaveProject,
        Quit,
        SearchRequests,
        CloseRequest,
        NewRequest,
        UndoDelete
    ]
);

pub const WORKBENCH_TITLEBAR_HEIGHT: Pixels = px(48.);

const SIDEBAR_GUTTER: f32 = 12.;
const SIDEBAR_ROW_INSET: f32 = 8.;
const SIDEBAR_ROW_HEIGHT: f32 = 32.;
const SIDEBAR_METHOD_WIDTH: f32 = 62.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Query,
    Path,
    Headers,
    Body,
    Auth,
    Settings,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResponseTab {
    Body,
    Headers,
    Trailers,
    Timeline,
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
    ProjectName,
    Name,
    Url,
    Method,
    Body,
    Token,
    User,
    Password,
    Timeout,
    Proxy,
    Environment,
    Search,
}
#[derive(Clone, Copy)]
enum Split {
    Sidebar,
    Schema,
    Vertical,
    Horizontal,
}

enum DeletedItem {
    Request(usize, Box<Request>),
    Folder(Box<DeletedFolder>),
}

struct ParamInputs {
    name: Entity<InputState>,
    value: Entity<InputState>,
}
struct Job {
    id: Uuid,
    cancel: CancellationToken,
    commands: Option<tokio::sync::mpsc::Sender<SessionCommand>>,
    half_closed: bool,
}
#[derive(Clone, Copy)]
enum WorkbenchTab {
    Request(usize),
    Settings(scope_settings::Scope),
}
enum WorkerEvent {
    RequestStructure(
        Uuid,
        u64,
        Result<Box<crate::structure::RequestStructure>, String>,
    ),
    SyncFetched(
        Uuid,
        Result<crate::apifox::Snapshot, crate::apifox::ApiFailure>,
    ),
    SyncConfigured(Uuid, Result<Box<sync_ui::ConnectedSource>, String>),
    SyncVerified(Uuid, Result<usize, String>),
    Record(Uuid, RunRecord),
    Session(Uuid, Uuid, u128, SessionEvent),
    Done(Uuid),
    Pool(Uuid, Result<DescriptorPool, String>, String),
    Imported(Result<crate::openapi::ImportedSpec, String>),
    Saved(u64, Result<(), String>),
    Notice(String),
    Contracts(Result<Vec<ContractInfo>, String>),
    Definition(PathBuf, Result<String, String>),
}

pub struct Workbench {
    structure_ui: structure_ui::StructureUi,
    value_ui: value_ui::ValueUi,
    active_scope: Option<scope_settings::Scope>,
    scope_editors: Vec<scope_settings::ScopeEditor>,
    sync_ui: sync_ui::SyncUi,
    workspace_strip: Option<Entity<WorkspaceStrip>>,
    empty_request: Request,
    project_name: Entity<InputState>,
    environment_name: Entity<InputState>,
    project: Project,
    store: ProjectStore,
    runtime: Arc<tokio::runtime::Runtime>,
    tx: mpsc::Sender<WorkerEvent>,
    rx: mpsc::Receiver<WorkerEvent>,
    name: Entity<InputState>,
    url: Entity<InputState>,
    url_editing: bool,
    split_sizes: Rc<Cell<[Size<Pixels>; 4]>>,
    resizing: Option<(Split, Point<Pixels>, f32)>,
    selected_messages: HashMap<Uuid, usize>,
    search: Entity<InputState>,
    token: Entity<InputState>,
    user: Entity<InputState>,
    password: Entity<InputState>,
    timeout: Entity<InputState>,
    proxy: Entity<InputState>,
    body: Entity<EditorState>,
    response: Entity<EditorState>,
    response_override: bool,
    environment: Entity<EditorState>,
    schema: Entity<EditorState>,
    method: Entity<InputState>,
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
    definitions_open: bool,
    contracts: Vec<ContractInfo>,
    definition_path: Option<PathBuf>,
    definition: Entity<EditorState>,
    history_response: Entity<EditorState>,
    history_preview: bool,
    encoding_open: bool,
    encoding: Entity<EditorState>,
    response_tab: ResponseTab,
    request_views: HashMap<Uuid, (Tab, ResponseTab)>,
    request_tab_scroll: ScrollHandle,
    revealed_request_tab: Cell<Option<(Uuid, usize, usize)>>,
    deleted_items: Vec<DeletedItem>,
    selected_folder: Option<Uuid>,
    deleted_environment: Option<(usize, Environment)>,
    environment_drafts: HashMap<String, (String, String)>,
    pools: HashMap<Uuid, DescriptorPool>,
    records: HashMap<Uuid, RunRecord>,
    logs: HashMap<Uuid, String>,
    history: Vec<RunRecord>,
    jobs: HashMap<Uuid, Job>,
    session_records: HashMap<Uuid, RunRecord>,
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
        let project_name = input(window, cx, "工作区名称");
        project_name.update(cx, |s, cx| s.set_value(project.name.clone(), window, cx));
        let environment_name = input(window, cx, "环境名称");
        let url = input(window, cx, "https://api.example.com");
        let search = input(window, cx, "搜索请求…");
        let token = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Token 或 {{token}}")
        });
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
        let definition = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let history_response = cx.new(|cx| EditorState::new(window, cx).language("text"));
        let encoding = cx.new(|cx| EditorState::new(window, cx).language("text"));
        let method = input(window, cx, "METHOD");
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
                vec![
                    "继承上级".into(),
                    "无认证".into(),
                    "Bearer".into(),
                    "Basic".into(),
                ],
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
        let selected_folder = project
            .requests
            .get(project.active_request)
            .and_then(|r| r.folder_id);
        let mut this = Self {
            structure_ui: structure_ui::StructureUi::default(),
            value_ui: value_ui::ValueUi::default(),
            active_scope: None,
            scope_editors: Vec::new(),
            sync_ui: sync_ui::SyncUi::default(),
            workspace_strip: None,
            empty_request: Request::new(Protocol::Http, ""),
            project_name,
            environment_name,
            project,
            store,
            runtime,
            tx,
            rx,
            name,
            url,
            url_editing: false,
            split_sizes: Rc::new(Cell::new([Size::default(); 4])),
            resizing: None,
            selected_messages: HashMap::new(),
            search,
            token,
            user,
            password,
            timeout,
            proxy,
            body,
            response,
            response_override: false,
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
            definitions_open: false,
            contracts: vec![],
            definition_path: None,
            definition,
            history_response,
            history_preview: false,
            encoding_open: false,
            encoding,
            response_tab: ResponseTab::Body,
            request_views: HashMap::new(),
            request_tab_scroll: ScrollHandle::new(),
            revealed_request_tab: Cell::new(None),
            deleted_items: vec![],
            selected_folder,
            deleted_environment: None,
            environment_drafts: HashMap::new(),
            pools: HashMap::new(),
            records: HashMap::new(),
            logs: HashMap::new(),
            history,
            jobs: HashMap::new(),
            session_records: HashMap::new(),
            notice: "就绪 · 请求尚未发送".into(),
            revision: 0,
            saved_revision: 0,
            save_pending: false,
            edited_at: Instant::now(),
            _poll: poll,
            focus: cx.focus_handle(),
        };
        for (entity, field) in [
            (this.name.clone(), Field::Name),
            (this.project_name.clone(), Field::ProjectName),
            (this.url.clone(), Field::Url),
            (this.method.clone(), Field::Method),
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
            &this.url,
            window,
            |this, _, event, window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.url_editing = false;
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.focus.focus(window, cx);
                    }
                    cx.notify();
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
                    this.stash_environment(cx);
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
                    this.current_mut().auth_inherit = Some(value == "继承上级");
                    if value != "继承上级" {
                        this.current_mut().auth = match value.as_str() {
                            "Bearer" => Auth::Bearer(this.token.read(cx).value().to_string()),
                            "Basic" => Auth::Basic {
                                username: this.user.read(cx).value().to_string(),
                                password: this.password.read(cx).value().to_string(),
                            },
                            _ => Auth::None,
                        };
                    }
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
        self.project
            .requests
            .get(self.project.active_request)
            .unwrap_or(&self.empty_request)
    }
    fn current_mut(&mut self) -> &mut Request {
        self.project
            .requests
            .get_mut(self.project.active_request)
            .unwrap_or(&mut self.empty_request)
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
            Field::ProjectName => {
                if !value.trim().is_empty() {
                    self.project.name = value.trim().to_owned();
                }
            }
            Field::Name => self.current_mut().name = value,
            Field::Url => self.current_mut().url = value,
            Field::Method => self.current_mut().method = value.trim().to_owned(),
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
        let inherited = r.inherits_auth();
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
        self.url_editing = false;
        self.method
            .update(cx, |state, cx| state.set_value(r.method, window, cx));
        self.body_kind.update(cx, |state, cx| {
            state.set_selected_value(&r.body.kind.label().to_string(), window, cx)
        });
        let (kind, token, user, password) = match r.auth {
            Auth::None => ("无认证", String::new(), String::new(), String::new()),
            Auth::Bearer(token) => ("Bearer", token, String::new(), String::new()),
            Auth::Basic { username, password } => ("Basic", String::new(), username, password),
        };
        self.auth_kind.update(cx, |state, cx| {
            state.set_selected_value(
                &if inherited {
                    "继承上级".into()
                } else {
                    kind.to_string()
                },
                window,
                cx,
            )
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
        let name = self.project.environments[self.project.active_environment]
            .name
            .clone();
        let (draft_name, value) =
            self.environment_drafts
                .get(&name)
                .cloned()
                .unwrap_or_else(|| {
                    (
                        name.clone(),
                        serde_json::to_string_pretty(&self.project.variables()).unwrap_or_default(),
                    )
                });
        self.environment
            .update(cx, |s, cx| s.set_value(value, window, cx));
        self.environment_name
            .update(cx, |s, cx| s.set_value(draft_name, window, cx));
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
        if self.current().protocol == Protocol::Http {
            self.update_http_structure(window, cx);
            return;
        }
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
        self.value_ui = value_ui::ValueUi::default();
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
                    move |this, entity, event, window, cx| {
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
                        if !is_name && !this.loading {
                            this.parameter_value_event(i, entity, event, window, cx);
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
                WorkerEvent::RequestStructure(id, generation, result) => {
                    self.received_structure(id, generation, result, window, cx)
                }
                WorkerEvent::SyncFetched(id, result) => self.sync_fetched(id, result, window, cx),
                WorkerEvent::SyncConfigured(id, result) => {
                    self.sync_configured(id, result, window, cx)
                }
                WorkerEvent::SyncVerified(id, result) => self.sync_verified(id, result, window, cx),
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
                WorkerEvent::Contracts(result) => match result {
                    Ok(contracts) => self.contracts = contracts,
                    Err(error) => self.notice = format!("接口定义读取失败: {error}"),
                },
                WorkerEvent::Definition(path, result) => {
                    if self.definition_path.as_ref() == Some(&path) {
                        match result {
                            Ok(text) => self
                                .definition
                                .update(cx, |s, cx| s.set_value(text, window, cx)),
                            Err(error) => self.notice = format!("接口定义读取失败: {error}"),
                        }
                    }
                }
                WorkerEvent::Record(job, record) => {
                    if self
                        .jobs
                        .get(&record.request_id)
                        .is_some_and(|j| j.id == job)
                    {
                        self.jobs.remove(&record.request_id);
                        self.session_records.remove(&record.request_id);
                        self.notice = format!("{} · {} ms", record.status, record.elapsed_ms);
                        self.records.insert(record.request_id, record.clone());
                        if record.request_id == self.current().id {
                            self.update_response(window, cx);
                        }
                    }
                    self.history.insert(0, record);
                    self.history.truncate(100);
                }
                WorkerEvent::Session(job, id, elapsed_ms, event) => {
                    if self.jobs.get(&id).is_some_and(|j| j.id == job) {
                        let log = self.logs.entry(id).or_default();
                        if log.len() + event_text(&event).len() + 40 < 1024 * 1024 {
                            log.push_str(&format!(
                                "{}  {}\n\n",
                                chrono::Local::now().format("%H:%M:%S%.3f"),
                                event_text(&event)
                            ));
                        }
                        if let Some(record) = self.session_records.get_mut(&id) {
                            record.elapsed_ms = elapsed_ms;
                            record_session_event(record, &event, elapsed_ms);
                            record.bytes = record.messages.iter().map(|m| m.body.len()).sum();
                        }
                        if id == self.current().id {
                            self.update_response(window, cx);
                        }
                    }
                }
                WorkerEvent::Done(job) => {
                    self.jobs.retain(|_, j| j.id != job);
                }
                WorkerEvent::Pool(id, result, source) => {
                    if !self.project.requests.iter().any(|r| r.id == id) {
                        continue;
                    }
                    match result {
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
                    }
                }
                WorkerEvent::Imported(result) => match result {
                    Ok(import) => {
                        let count = import.requests.len();
                        let first = import.requests.first().map(|r| r.id);
                        if let Err(error) = self
                            .project
                            .merge_collection(import.folders, import.requests)
                        {
                            self.notice = format!("导入失败: {error}");
                            continue;
                        }
                        if self.project.tabs().is_empty()
                            && let Some(id) = first
                        {
                            self.project.open_request(id);
                            self.restore_view();
                            self.load_request(window, cx);
                        }
                        self.refresh_contracts();
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
        self.sync_tick(cx);
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
        self.response_override = false;
        let id = self.current().id;
        let record = self
            .session_records
            .get(&id)
            .or_else(|| self.records.get(&id));
        let text = match self.response_tab {
            ResponseTab::Headers => record.map(|r| headers_text(&r.headers)).unwrap_or_else(|| "尚未收到 Headers / Metadata".into()),
            ResponseTab::Trailers => record.map(|r| {
                if r.trailers.is_empty() { if self.jobs.contains_key(&id) { "等待调用结束".into() } else { format!("调用状态: {}\n没有额外 Trailers", r.status) } }
                else { format!("调用状态: {}\n{}", r.status, headers_text(&r.trailers)) }
            }).unwrap_or_else(|| "尚未调用".into()),
            ResponseTab::Timeline => record.map(|r| format!("开始: {}\n环境: {}\n状态: {}\n总耗时: {} ms\n预览: {} B{}\n\n{}", r.started_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S%.3f"), r.environment, r.status, r.elapsed_ms, r.bytes, if r.truncated { "（已截断）" } else { "" }, r.timeline.iter().map(|m| format!("{:>7} ms  {}", m.elapsed_ms, m.label)).collect::<Vec<_>>().join("\n")))
                .unwrap_or_else(|| "发送后显示实际观测到的时间线。HTTP 的连接与服务端等待包含在 Headers 到达耗时中。".into()),
            ResponseTab::Body if self.current().protocol != Protocol::Http => record.and_then(|r| {
                let index = self.selected_messages.get(&id).copied().filter(|i| *i < r.messages.len())
                    .unwrap_or_else(|| r.messages.len().saturating_sub(1));
                r.messages.get(index).map(|m| pretty_payload(&m.body))
            }).or_else(|| record.filter(|r| r.messages.is_empty() && !r.body.is_empty()).map(|r| r.body.clone()))
                .unwrap_or_default(),
            ResponseTab::Body => record.map(|r| r.body.clone()).unwrap_or_else(|| "还没有响应。\n\n配置 URL 和参数后，点击发送或按 ⌘ Enter。".into()),
        };
        self.response.update(cx, |s, cx| {
            s.set_highlighter(
                if self.response_tab == ResponseTab::Body
                    && serde_json::from_str::<serde_json::Value>(&text).is_ok()
                {
                    "json"
                } else {
                    "text"
                },
                cx,
            );
            s.set_value(text, window, cx);
        });
    }
    fn send(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.jobs.contains_key(&self.current().id) {
            self.notice = "该请求正在执行，请先停止。".into();
            cx.notify();
            return;
        }
        if self.project.tabs().is_empty() || self.project.requests.is_empty() {
            return;
        }
        let mut request = match self.project.prepare_request(self.current()) {
            Ok(request) => request,
            Err(error) => {
                self.notice = error.to_string();
                cx.notify();
                return;
            }
        };
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
                let started_at = chrono::Utc::now();
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
                        r.started_at = started_at;
                        r.elapsed_ms = started.elapsed().as_millis();
                        r.timeline = vec![TimingMark {
                            elapsed_ms: r.elapsed_ms,
                            label: "执行失败或取消".into(),
                        }];
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
            self.jobs.insert(
                id,
                Job {
                    id: job,
                    cancel,
                    commands: None,
                    half_closed: false,
                },
            );
        } else {
            let pool = self.pools.get(&id).cloned();
            if request.protocol == Protocol::Grpc && pool.is_none() {
                self.notice = "先导入 proto 或读取 Reflection，再选择 Method。".into();
                cx.notify();
                return;
            }
            let mut initial = RunRecord::error(&request, environment.clone(), String::new());
            initial.method = request.protocol.label().into();
            initial.status = "连接中…".into();
            initial.error = false;
            self.selected_messages.remove(&id);
            self.session_records.insert(id, initial);
            self.records.remove(&id);
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
                    record_session_event(&mut record, &event, started.elapsed().as_millis());
                    let text = event_text(&event);
                    if record.body.len() + text.len() < 1024 * 1024 {
                        record.body.push_str(&text);
                        record.body.push_str("\n\n");
                    } else {
                        record.truncated = true;
                    }
                    let _ = tx
                        .send(WorkerEvent::Session(
                            job,
                            id,
                            started.elapsed().as_millis(),
                            event,
                        ))
                        .await;
                }
                record.elapsed_ms = started.elapsed().as_millis();
                record.bytes = record.messages.iter().map(|m| m.body.len()).sum();
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
            self.jobs.insert(
                id,
                Job {
                    id: job,
                    cancel,
                    commands: Some(commands),
                    half_closed: false,
                },
            );
        }
        self.notice = "正在执行…".into();
        self.update_response(_window, cx);
        cx.notify();
    }
    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = self.jobs.get(&self.current().id) {
            job.cancel.cancel();
        }
        self.notice = "正在停止…".into();
        cx.notify();
    }
    fn command(&mut self, command: SessionCommand, cx: &mut Context<Self>) {
        let id = self.current().id;
        let Some(job) = self.jobs.get_mut(&id) else {
            return;
        };
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
        let request = match self.project.prepare_request(self.current()) {
            Ok(request) => request,
            Err(error) => {
                self.notice = error.to_string();
                cx.notify();
                return;
            }
        };
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
        self.active_scope = None;
        let mut request = Request::new(protocol.clone(), format!("新 {} 请求", protocol.label()));
        if let Some(settings) = crate::ui_settings::maybe_store(cx) {
            settings
                .read(cx)
                .preferences
                .apply_request_defaults(&mut request);
        }
        request.url = match protocol {
            Protocol::Http => "{{base_url}}",
            Protocol::Grpc => "{{grpc_host}}",
            Protocol::WebSocket => "{{ws_url}}",
        }
        .into();
        if protocol != Protocol::Http {
            request.body.kind = BodyKind::Json;
        }
        request.folder_id = self
            .selected_folder
            .filter(|id| self.project.folder(*id).is_ok());
        self.project.expand_folder_ancestors(request.folder_id);
        let id = request.id;
        let first = self.project.requests.is_empty();
        self.project.requests.push(request);
        if first {
            self.request_views.insert(
                id,
                (
                    if protocol == Protocol::Http {
                        Tab::Query
                    } else {
                        Tab::Body
                    },
                    ResponseTab::Body,
                ),
            );
        }
        self.switch_request(self.project.requests.len() - 1, window, cx);
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
    fn stash_environment(&mut self, cx: &Context<Self>) {
        if self.environment_open {
            let key = self.project.environments[self.project.active_environment]
                .name
                .clone();
            self.environment_drafts.insert(
                key,
                (
                    self.environment_name.read(cx).value().to_string(),
                    self.environment.read(cx).value().to_string(),
                ),
            );
        }
    }
    fn refresh_environments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        self.env_select.update(cx, |s, cx| {
            s.set_items(
                self.project
                    .environments
                    .iter()
                    .map(|e| e.name.clone())
                    .collect(),
                window,
                cx,
            );
            s.set_selected_index(
                Some(IndexPath::new(self.project.active_environment)),
                window,
                cx,
            );
        });
        self.loading = false;
        self.load_environment(window, cx);
    }
    fn new_environment(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.stash_environment(cx);
        let mut environment = if duplicate {
            self.project.environments[self.project.active_environment].clone()
        } else {
            Environment {
                name: String::new(),
                variables: std::collections::BTreeMap::new(),
            }
        };
        let base = if duplicate {
            format!("{} 副本", environment.name)
        } else {
            "新环境".into()
        };
        let mut name = base.clone();
        let mut suffix = 2;
        while self.project.environments.iter().any(|e| e.name == name) {
            name = format!("{base} {suffix}");
            suffix += 1;
        }
        environment.name = name;
        self.project.environments.push(environment);
        self.project.active_environment = self.project.environments.len() - 1;
        self.refresh_environments(window, cx);
        self.dirty(cx);
    }
    fn delete_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.environments.len() <= 1 {
            return;
        }
        self.stash_environment(cx);
        let index = self.project.active_environment;
        self.deleted_environment = Some((index, self.project.environments.remove(index)));
        self.project.active_environment = index.min(self.project.environments.len() - 1);
        self.refresh_environments(window, cx);
        self.notice = "环境已删除，可恢复上次删除的环境".into();
        self.dirty(cx);
    }
    fn save_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .environment_name
            .read(cx)
            .value()
            .to_string()
            .trim()
            .to_owned();
        if name.is_empty()
            || self
                .project
                .environments
                .iter()
                .enumerate()
                .any(|(i, e)| i != self.project.active_environment && e.name == name)
        {
            self.notice = "环境名称不能为空或重复".into();
            cx.notify();
            return;
        }
        match serde_json::from_str::<std::collections::BTreeMap<String, String>>(
            self.environment.read(cx).value().as_ref(),
        ) {
            Ok(vars) => {
                let environment = &mut self.project.environments[self.project.active_environment];
                self.environment_drafts.remove(&environment.name);
                environment.name = name;
                environment.variables = vars;
                self.refresh_environments(window, cx);
                self.environment_open = false;
                self.dirty(cx);
                self.notice = "环境已更新".into();
            }
            Err(error) => {
                self.notice = format!("环境变量需要 JSON 字符串键值表: {error}");
                cx.notify();
            }
        }
    }
    fn environment_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex().flex_1().min_h_0().p_5().gap_4()
            .child(h_flex().gap_3().items_center().child(div().flex_1().child("工作区与环境"))
                .child(Button::new("environment-back").small().ghost().label("返回").on_click(cx.listener(|this, _, _, cx| { this.stash_environment(cx); this.environment_open = false; cx.notify(); })))
                .child(Button::new("save-environment").primary().label("应用变量").on_click(cx.listener(|this, _, window, cx| this.save_environment(window, cx)))))
            .child(h_flex().gap_3().items_center().child(div().w(px(80.)).text_color(cx.theme().muted_foreground).child("工作区名称"))
                .child(Input::new(&self.project_name).w(px(280.))))
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("请求、环境与执行历史均独立保存在此工作区。"))
            .child(h_flex().gap_2().items_center().child(div().child("环境名称")).child(Input::new(&self.environment_name).w(px(220.)))
                .child(Button::new("environment-new").small().label("新建环境").on_click(cx.listener(|this, _, window, cx| this.new_environment(false, window, cx))))
                .child(Button::new("environment-copy").small().ghost().label("复制环境").on_click(cx.listener(|this, _, window, cx| this.new_environment(true, window, cx))))
                .child(Button::new("environment-delete").small().ghost().label("删除环境").disabled(self.project.environments.len() <= 1).on_click(cx.listener(|this, _, window, cx| this.delete_environment(window, cx)))))
            .when(self.deleted_environment.is_some(), |column| column.child(Button::new("environment-restore").small().ghost().label("恢复上次删除的环境")
                .on_click(cx.listener(|this, _, window, cx| {
                    if let Some((index, mut environment)) = this.deleted_environment.take() {
                        while this.project.environments.iter().any(|e| e.name == environment.name) { environment.name.push_str(" 恢复"); }
                        let index = index.min(this.project.environments.len());
                        this.project.environments.insert(index, environment);
                        this.project.active_environment = index;
                        this.refresh_environments(window, cx); this.dirty(cx);
                    }
                }))))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("用 JSON 字符串键值表编辑。在 URL、参数、Headers 和 Body 中使用 {{variable}}。"))
            .child(Editor::new(&self.environment).h(relative(1.)).flex_1()).into_any_element()
    }
    fn remember_view(&mut self) {
        if !self.project.tabs().contains(&self.current().id) {
            return;
        }
        self.request_views
            .insert(self.current().id, (self.tab, self.response_tab));
    }
    fn restore_view(&mut self) {
        let default = if self.current().protocol == Protocol::Http {
            Tab::Query
        } else {
            Tab::Body
        };
        (self.tab, self.response_tab) = self
            .request_views
            .get(&self.current().id)
            .copied()
            .unwrap_or((default, ResponseTab::Body));
    }
    fn switch_request(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active_scope = None;
        let Some(id) = self.project.requests.get(index).map(|r| r.id) else {
            return;
        };
        self.remember_view();
        self.project.open_request(id);
        self.selected_folder = self.project.requests[index].folder_id;
        self.project.expand_folder_ancestors(self.selected_folder);
        self.stash_environment(cx);
        self.environment_open = false;
        self.history_open = false;
        self.definitions_open = false;
        self.encoding_open = false;
        self.sync_ui.open = false;
        self.restore_view();
        self.load_request(window, cx);
        self.dirty(cx);
    }
    fn close_request(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        self.encoding_open = false;
        self.remember_view();
        self.project.close_request(id);
        if !self.project.tabs().is_empty() {
            self.restore_view();
            self.load_request(window, cx);
        }
        self.dirty(cx);
    }
    fn delete_request(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        self.encoding_open = false;
        self.remember_view();
        if let Some(removed) = self.project.delete_request(id) {
            if let Some(job) = self.jobs.remove(&id) {
                job.cancel.cancel();
            }
            self.session_records.remove(&id);
            self.notice = format!("已删除“{}”，可撤销", removed.1.name);
            self.deleted_items
                .push(DeletedItem::Request(removed.0, Box::new(removed.1)));
            if !self.project.tabs().is_empty() {
                self.restore_view();
                self.load_request(window, cx);
            }
            self.dirty(cx);
        }
    }
    fn undo_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.deleted_items.pop() {
            Some(DeletedItem::Request(index, mut request)) => {
                if self.project.requests.iter().any(|r| r.id == request.id) {
                    self.notice = "此请求已恢复，无需重复撤销删除".into();
                    cx.notify();
                    return;
                }
                if request
                    .folder_id
                    .is_some_and(|id| self.project.folder(id).is_err())
                {
                    request.folder_id = None;
                }
                let index = index.min(self.project.requests.len());
                self.project.requests.insert(index, *request);
                self.switch_request(index, window, cx);
                self.notice = "请求已恢复".into();
            }
            Some(DeletedItem::Folder(folder)) => {
                match self.project.restore_folder((*folder).clone()) {
                    Ok(id) => {
                        self.selected_folder = Some(id);
                        self.notice = "文件夹已恢复".into();
                        self.dirty(cx);
                    }
                    Err(e) => {
                        self.deleted_items.push(DeletedItem::Folder(folder));
                        self.notice = e.to_string();
                        cx.notify();
                    }
                }
            }
            None => {}
        }
    }
    fn duplicate_request(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut request) = self.project.requests.iter().find(|r| r.id == id).cloned() else {
            return;
        };
        request.id = Uuid::new_v4();
        if crate::apifox::binding_for_request(&self.project, id).is_some() {
            request.openapi = None;
            request.source = None;
        }
        request.name.push_str(" 副本");
        if let Some(pool) = self.pools.get(&id).cloned() {
            self.pools.insert(request.id, pool);
        }
        self.project.requests.push(request);
        self.switch_request(self.project.requests.len() - 1, window, cx);
    }
    pub fn set_workspace_strip(&mut self, strip: Entity<WorkspaceStrip>) {
        self.workspace_strip = Some(strip);
    }
    fn move_request(&mut self, id: Uuid, offset: isize, cx: &mut Context<Self>) {
        let Some(index) = self.project.requests.iter().position(|r| r.id == id) else {
            return;
        };
        let siblings = self
            .project
            .requests
            .iter()
            .enumerate()
            .filter(|(_, r)| r.folder_id == self.project.requests[index].folder_id)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let position = siblings.iter().position(|i| *i == index).unwrap();
        let Some(next) = position
            .checked_add_signed(offset)
            .and_then(|i| siblings.get(i))
            .copied()
        else {
            return;
        };
        let active = self.current().id;
        self.project.requests.swap(index, next);
        self.project.active_request = self
            .project
            .requests
            .iter()
            .position(|r| r.id == active)
            .unwrap_or(0);
        self.dirty(cx);
    }
    fn save_response(&mut self, cx: &mut Context<Self>) {
        let text = self.response.read(cx).value().to_string();
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("response-preview.txt")
                .save_file()
            {
                let notice = match std::fs::write(&path, text) {
                    Ok(()) => format!("预览已保存: {}", path.display()),
                    Err(error) => format!("保存预览失败: {error}"),
                };
                let _ = tx.blocking_send(WorkerEvent::Notice(notice));
            }
        });
    }
    fn refresh_contracts(&self) {
        let store = self.store.clone();
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            let _ = tx.blocking_send(WorkerEvent::Contracts(
                store.contracts().map_err(|e| e.to_string()),
            ));
        });
    }
    fn definitions_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = v_flex().gap_2().p_3();
        for source in self
            .project
            .sync_sources
            .iter()
            .filter(|s| s.latest.is_some())
        {
            let id = source.id;
            list = list.child(
                Button::new(SharedString::from(format!("sync-contract-{id}")))
                    .ghost()
                    .label(format!("{} · Apifox", source.config.name))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.sync_ui_select(id);
                        this.sync_show_document(window, cx);
                    })),
            );
        }
        for (i, contract) in self.contracts.iter().enumerate() {
            let path = contract.path.clone();
            list = list.child(Button::new(("contract", i)).ghost().label(format!("{} · {}", contract.label, contract.version))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.definition_path = Some(path.clone());
                    let path = path.clone(); let tx = this.tx.clone();
                    this.runtime.spawn_blocking(move || {
                        let result = (|| -> anyhow::Result<String> {
                            let document: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
                            let sources: serde_json::Value = serde_json::from_slice(&std::fs::read(path.with_file_name("sources.json"))?)?;
                            Ok(serde_json::to_string_pretty(&serde_json::json!({"document": document, "source": sources.get("source"), "diagnostics": sources.get("diagnostics"), "localReferences": sources.get("files")}))?)
                        })().map_err(|e| e.to_string());
                        let _ = tx.blocking_send(WorkerEvent::Definition(path, result));
                    });
                    cx.notify();
                })));
        }
        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                h_flex()
                    .p_4()
                    .gap_3()
                    .child(div().flex_1().child("接口定义 · 文档原文与诊断"))
                    .child(
                        Button::new("definition-import")
                            .small()
                            .label("导入 OpenAPI")
                            .on_click(cx.listener(|this, _, _, cx| this.import_openapi(cx))),
                    ),
            )
            .when(
                self.contracts.is_empty()
                    && !self.project.sync_sources.iter().any(|s| s.latest.is_some()),
                |column| {
                    column.child(
                        div()
                            .p_4()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "尚未导入 OpenAPI。gRPC 的本地 Proto 与方法可在对应请求中查看。",
                            ),
                    )
                },
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(
                        div()
                            .id("definitions-scroll")
                            .w(px(250.))
                            .overflow_y_scroll()
                            .child(list),
                    )
                    .child(
                        Editor::new(&self.definition)
                            .readonly(true)
                            .appearance(false)
                            .h(relative(1.))
                            .flex_1(),
                    ),
            )
            .into_any_element()
    }
    pub fn workspace_name(&self) -> &str {
        &self.project.name
    }
    pub fn running_count(&self) -> usize {
        self.jobs.len()
    }
    pub fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub fn prepare_restart(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        self.stash_environment(cx);
        self.flush()
    }
    pub fn flush(&mut self) -> anyhow::Result<()> {
        if self.revision != self.saved_revision {
            self.store.save_versioned(&self.project, self.revision)?;
            self.saved_revision = self.revision;
        }
        Ok(())
    }
    fn request_tabs(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut bar = TabBar::new(SharedString::from(format!(
            "request-tabs-{:?}",
            cx.entity_id()
        )))
        .track_scroll(&self.request_tab_scroll)
        .segmented()
        .menu(true)
        .max_width(px(232.))
        .bg(cx.theme().sidebar)
        .rounded_none()
        .px_2()
        .gap(px(4.));
        let mut targets = Vec::new();
        for id in self.project.tabs() {
            let Some((index, request)) = self
                .project
                .requests
                .iter()
                .enumerate()
                .find(|(_, r)| r.id == *id)
            else {
                continue;
            };
            let id = *id;
            let active = self.active_scope.is_none() && index == self.project.active_request;
            if active {
                let position = targets.len();
                bar = bar.selected_index(position);
                let reveal = (id, position, self.project.tabs().len());
                if self.revealed_request_tab.get() != Some(reveal) {
                    let scroll = self.request_tab_scroll.clone();
                    // The new tab must be measured before ScrollHandle can reveal it.
                    window.on_next_frame(move |window, _| {
                        scroll.scroll_to_item(position);
                        window.refresh();
                    });
                    self.revealed_request_tab.set(Some(reveal));
                }
            }
            targets.push(WorkbenchTab::Request(index));
            let tag = if request.protocol == Protocol::Http {
                request.method.as_str()
            } else {
                request.protocol.label()
            };
            let mut color = method_color(tag);
            color.l = if cx.theme().is_dark() {
                color.l.max(0.64)
            } else {
                color.l.min(0.34)
            };
            let group = SharedString::from(format!("request-tab-{id}"));
            let running = self.jobs.contains_key(&id);
            let close = Button::new(("request-tab-close", index))
                .xsmall()
                .ghost()
                .size(px(20.))
                .icon(IconName::Close)
                .text_color(cx.theme().muted_foreground)
                .accessibility_label(format!("关闭请求标签 {}", request.name))
                .tooltip("关闭标签，保留请求和运行中的会话")
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.close_request(id, window, cx)
                }));
            bar = bar.child(
                RequestTab::new()
                    .group(group.clone())
                    .aria_label(format!(
                        "{}{}",
                        request.name,
                        if running { "，运行中" } else { "" }
                    ))
                    .label(request.name.clone())
                    .prefix(
                        div()
                            .ml(px(8.))
                            .w(px(46.))
                            .h(px(18.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .bg(color.opacity(0.08))
                            .text_color(color)
                            .font_family("Menlo")
                            .text_size(px(9.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(relative(1.))
                            .truncate()
                            .child(tag.to_owned()),
                    )
                    .suffix(
                        h_flex()
                            .mr(px(6.))
                            .gap_1()
                            .items_center()
                            .when(running, |row| {
                                row.child(div().size(px(5.)).rounded_full().bg(cx.theme().primary))
                            })
                            .child(
                                div()
                                    .w(px(20.))
                                    .h(px(20.))
                                    .when(!active, |item| {
                                        item.invisible().group_hover(group, |s| s.visible())
                                    })
                                    .child(close),
                            ),
                    ),
            );
        }
        for (index, editor) in self.scope_editors.iter().enumerate() {
            let scope = editor.scope;
            let (title, icon) = match scope {
                scope_settings::Scope::Workspace => ("工作区设置".to_owned(), IconName::Settings2),
                scope_settings::Scope::Folder(id) => {
                    let Ok(folder) = self.project.folder(id) else {
                        continue;
                    };
                    (format!("{} · 设置", folder.name), IconName::Folder)
                }
            };
            let active = self.active_scope == Some(scope);
            let position = targets.len();
            targets.push(WorkbenchTab::Settings(scope));
            if active {
                bar = bar.selected_index(position);
                let id = match scope {
                    scope_settings::Scope::Workspace => Uuid::nil(),
                    scope_settings::Scope::Folder(id) => id,
                };
                let reveal = (
                    id,
                    position,
                    self.project.tabs().len() + self.scope_editors.len(),
                );
                if self.revealed_request_tab.get() != Some(reveal) {
                    let scroll = self.request_tab_scroll.clone();
                    window.on_next_frame(move |window, _| {
                        scroll.scroll_to_item(position);
                        window.refresh();
                    });
                    self.revealed_request_tab.set(Some(reveal));
                }
            }
            let group = SharedString::from(format!("settings-tab-{index}"));
            bar = bar.child(
                RequestTab::new()
                    .group(group.clone())
                    .aria_label(title.clone())
                    .label(title.clone())
                    .prefix(
                        div().ml(px(10.)).child(
                            Icon::new(icon)
                                .size_4()
                                .text_color(cx.theme().muted_foreground),
                        ),
                    )
                    .suffix(
                        div()
                            .mr(px(6.))
                            .w(px(20.))
                            .h(px(20.))
                            .when(!active, |item| {
                                item.invisible().group_hover(group, |s| s.visible())
                            })
                            .child(
                                Button::new(("settings-tab-close", index))
                                    .xsmall()
                                    .ghost()
                                    .size(px(20.))
                                    .icon(IconName::Close)
                                    .text_color(cx.theme().muted_foreground)
                                    .accessibility_label(format!("关闭设置标签 {title}"))
                                    .tooltip("关闭设置标签")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.close_scope_settings(scope, window, cx);
                                    })),
                            ),
                    ),
            )
        }
        div()
            .h(px(40.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(cx.theme().sidebar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                bar.w_full()
                    .on_click(cx.listener(move |this, selected: &usize, window, cx| {
                        match targets.get(*selected) {
                            Some(WorkbenchTab::Request(index)) => {
                                this.switch_request(*index, window, cx)
                            }
                            Some(WorkbenchTab::Settings(scope)) => {
                                this.open_scope_settings(*scope, window, cx)
                            }
                            None => {}
                        }
                    })),
            )
            .into_any_element()
    }
    fn pane_size(&self, kind: Split) -> f32 {
        let bounds = self.split_sizes.get()[kind as usize];
        let (value, min, max) = match kind {
            Split::Sidebar => (
                self.project.panes.sidebar,
                210.,
                (f32::from(bounds.width) - 365.).min(420.),
            ),
            Split::Schema => (
                self.project.panes.schema,
                180.,
                (f32::from(bounds.width) - 365.).min(620.),
            ),
            Split::Vertical => (
                self.project.panes.request_height,
                140.,
                f32::from(bounds.height) - 145.,
            ),
            Split::Horizontal => (
                self.project.panes.request_width,
                240.,
                f32::from(bounds.width) - 245.,
            ),
        };
        if bounds.width == px(0.) || bounds.height == px(0.) {
            value.max(min)
        } else {
            value.clamp(min, max.max(min))
        }
    }
    fn resize_to(&mut self, kind: Split, value: f32, cx: &mut Context<Self>) {
        match kind {
            Split::Sidebar => self.project.panes.sidebar = value,
            Split::Schema => self.project.panes.schema = value,
            Split::Vertical => self.project.panes.request_height = value,
            Split::Horizontal => self.project.panes.request_width = value,
        }
        let bounded = self.pane_size(kind);
        match kind {
            Split::Sidebar => self.project.panes.sidebar = bounded,
            Split::Schema => self.project.panes.schema = bounded,
            Split::Vertical => self.project.panes.request_height = bounded,
            Split::Horizontal => self.project.panes.request_width = bounded,
        }
        self.dirty(cx);
    }
    fn split(
        &self,
        kind: Split,
        first: AnyElement,
        second: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let vertical = matches!(kind, Split::Vertical);
        let size = px(self.pane_size(kind));
        let mut left = v_flex().min_h_0().min_w_0().overflow_hidden().child(first);
        let mut right = v_flex().min_h_0().min_w_0().overflow_hidden().child(second);
        if matches!(kind, Split::Schema) {
            left = left.flex_1();
            right = right.w(size).h_full().flex_shrink_0();
        } else if vertical {
            left = left.h(size).w_full().flex_shrink_0();
            right = right.flex_1();
        } else {
            left = left.w(size).h_full().flex_shrink_0();
            right = right.flex_1();
        }
        let separator = div()
            .id(("split-handle", kind as usize))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .hover(|s| s.bg(cx.theme().primary.opacity(0.15)))
            .map(|s| {
                if vertical {
                    s.h(px(5.)).w_full().cursor_row_resize()
                } else {
                    s.w(px(5.)).h_full().cursor_col_resize()
                }
            })
            .child(div().bg(cx.theme().border).map(|s| {
                if vertical {
                    s.h(px(1.)).w_full()
                } else {
                    s.w(px(1.)).h_full()
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    this.resizing = Some((kind, e.position, this.pane_size(kind)));
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
        let sizes = self.split_sizes.clone();
        let view = cx.entity().downgrade();
        let group = if vertical { v_flex() } else { h_flex() };
        group
            .id(("split-group", kind as usize))
            .items_stretch()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .on_prepaint(move |bounds, window, cx| {
                let mut all = sizes.get();
                if all[kind as usize] != bounds.size {
                    all[kind as usize] = bounds.size;
                    sizes.set(all);
                    let view = view.clone();
                    window.defer(cx, move |_, cx| {
                        let _ = view.update(cx, |_, cx| cx.notify());
                    });
                }
            })
            .child(left)
            .child(separator)
            .child(right)
            .into_any_element()
    }
    fn address(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.url_editing {
            return Input::new(&self.url)
                .appearance(false)
                .h(px(42.))
                .flex_1()
                .into_any_element();
        }
        let resolved = self.current().resolved_url(&self.project.variables());
        let (text, error) = match resolved {
            Ok(text) => (text, None),
            Err(e) => (self.current().url.clone(), Some(e.to_string())),
        };
        let mut highlights = Vec::new();
        let query_start = text.find('?').unwrap_or(text.len());
        if self.current().url.starts_with("{{") {
            let end = text
                .find("://")
                .map(|i| {
                    text[i + 3..]
                        .find('/')
                        .map(|j| i + 3 + j)
                        .unwrap_or(query_start)
                })
                .unwrap_or(query_start);
            highlights.push((
                0..end,
                HighlightStyle {
                    color: Some(cx.theme().primary),
                    ..Default::default()
                },
            ));
        }
        for param in self.current().path.iter().filter(|p| p.enabled) {
            if let Ok(value) = interpolate(&param.value, &self.project.variables()) {
                let value = encode_path_segment(&value);
                for (start, _) in text[..query_start]
                    .match_indices(&value)
                    .filter(|(_, v)| !v.is_empty())
                {
                    if start > 0 && text.as_bytes()[start - 1] == b'/' {
                        highlights.push((
                            start..start + value.len(),
                            HighlightStyle {
                                color: Some(method_color("HEAD")),
                                background_color: Some(method_color("HEAD").opacity(0.08)),
                                ..Default::default()
                            },
                        ));
                    }
                }
            }
        }
        if query_start < text.len() {
            let mut offset = query_start + 1;
            for pair in text[offset..].split('&') {
                if let Some(equal) = pair.find('=') {
                    highlights.push((
                        offset + equal + 1..offset + pair.len(),
                        HighlightStyle {
                            color: Some(method_color("POST")),
                            ..Default::default()
                        },
                    ));
                }
                offset += pair.len() + 1;
            }
        }
        highlights.sort_by_key(|(range, _)| range.start);
        // Skip overlaps where an environment expansion also contains a path value.
        let mut end = 0;
        highlights.retain(|(r, _)| {
            let keep = r.start >= end && !r.is_empty();
            if keep {
                end = r.end;
            }
            keep
        });
        let tip = error.unwrap_or_else(|| {
            format!(
                "模板：{}\n点击编辑 · 紫色：环境变量，青色：Path，金色：Query",
                self.current().url
            )
        });
        Button::new("address-edit")
            .accessibility_label(text.clone())
            .ghost()
            .h(px(42.))
            .flex_1()
            .min_w_0()
            .justify_start()
            .tooltip(tip)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .text_left()
                    .min_w_0()
                    .truncate()
                    .text_size(px(14.))
                    .child(StyledText::new(text).with_highlights(highlights)),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.url_editing = true;
                this.url.update(cx, |s, cx| s.focus(window, cx));
                cx.notify();
            }))
            .into_any_element()
    }
    fn format_json(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match serde_json::from_str::<serde_json::Value>(self.body.read(cx).value().as_ref()) {
            Ok(value) => {
                let text = serde_json::to_string_pretty(&value).unwrap();
                self.current_mut().body.text = text.clone();
                self.body.update(cx, |s, cx| s.set_value(text, window, cx));
                self.dirty(cx);
            }
            Err(error) => {
                self.notice = format!("JSON 格式错误: {error}");
                cx.notify();
            }
        }
    }
    fn param_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let is_form = self.tab == Tab::Body;
        let is_multipart = is_form && self.current().body.kind == BodyKind::FormData;
        let mut rows = v_flex()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(
                h_flex()
                    .h(px(32.))
                    .items_center()
                    .bg(cx.theme().muted)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .gap_3()
                    .px_3()
                    .text_size(px(10.))
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(px(26.)).child("启用"))
                    .child(div().w(px(140.)).child("KEY"))
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
                        .h(px(38.))
                        .px_3()
                        .gap_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .items_center()
                        .child(Checkbox::new(("enabled", i)).checked(p.enabled).on_click(
                            cx.listener(move |this, value, _, cx| {
                                if let Some(p) = this.param_data_mut().get_mut(i) {
                                    p.enabled = *value;
                                }
                                this.dirty(cx);
                            }),
                        ))
                        .child(
                            Input::new(&inputs.name)
                                .small()
                                .appearance(false)
                                .w(px(140.)),
                        )
                        .child(self.parameter_value_input(i, p, inputs, cx));
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
                        .icon(IconName::Close)
                        .accessibility_label("删除参数")
                        .tooltip("删除参数")
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
                .icon(IconName::Plus)
                .label("添加参数")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.param_data_mut().push(Parameter::new("", ""));
                    this.rebuild_params(window, cx);
                    this.dirty(cx);
                })),
        )
        .into_any_element()
    }
    fn format_json_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("format-json")
            .small()
            .ghost()
            .icon(IconName::CaseSensitive)
            .label("格式化 JSON")
            .on_click(cx.listener(|this, _, window, cx| this.format_json(window, cx)))
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
                                    .w(px(150.))
                                    .h(px(28.))
                                    .child(Select::new(&self.body_kind).small().w_full()),
                            )
                            .child(div().flex_1())
                            .when(r.body.kind == BodyKind::Json, |row| {
                                row.child(self.format_json_button(cx))
                            }),
                    );
                }
                if protocol == Protocol::Grpc {
                    let client_streaming = self
                        .pools
                        .get(&r.id)
                        .and_then(|p| transport::grpc::method(p, &r.grpc_method).ok())
                        .is_some_and(|m| m.is_client_streaming());
                    let active = self.jobs.get(&r.id).is_some_and(|j| !j.half_closed);
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
                    let active = self.jobs.contains_key(&r.id);
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
                    if protocol == Protocol::Grpc {
                        content = content
                            .child(h_flex().justify_end().child(self.format_json_button(cx)));
                    }
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
                        .w(px(210.))
                        .h(px(34.))
                        .child(Select::new(&self.auth_kind).w_full()),
                );
                if r.inherits_auth() {
                    content = content.child(self.inherited_auth_summary(cx));
                } else {
                    match &r.auth {
                        Auth::None => {
                            content = content.child(
                                div()
                                    .text_sm()
                                    .child("此请求不生成 Authorization，也不继承上级认证。"),
                            )
                        }
                        Auth::Bearer(_) => {
                            content = content.child(Input::new(&self.token).w_full())
                        }
                        Auth::Basic { .. } => {
                            content = content
                                .child(Input::new(&self.user))
                                .child(Input::new(&self.password))
                        }
                    }
                }
                content=content.child(div().text_xs().text_color(cx.theme().muted_foreground)
                    .child("支持 {{variable}}，发送时使用当前环境。认证更改在下次发送或重新连接时生效。"));
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
    fn message_view(&self, record: Option<&RunRecord>, cx: &mut Context<Self>) -> AnyElement {
        let Some(record) = record else {
            return empty_state(
                IconName::Inbox,
                "等待消息",
                "连接建立后，收发的消息会按顺序出现在这里。",
                cx,
            );
        };
        if record.messages.is_empty() {
            if !record.body.is_empty() && !self.session_records.contains_key(&record.request_id) {
                return Editor::new(&self.response)
                    .readonly(true)
                    .appearance(false)
                    .h(relative(1.))
                    .flex_1()
                    .into_any_element();
            }
            return empty_state(
                IconName::Inbox,
                "等待消息",
                "Metadata 和连接事件可在对应标签中查看。",
                cx,
            );
        }
        let id = record.request_id;
        let selected = self
            .selected_messages
            .get(&id)
            .copied()
            .filter(|i| *i < record.messages.len())
            .unwrap_or(record.messages.len() - 1);
        let mut list = v_flex().gap_1().p_2();
        for (index, message) in record.messages.iter().enumerate() {
            let color = if message.sent {
                cx.theme().primary
            } else {
                method_color("GET")
            };
            list = list.child(
                v_flex()
                    .id(("message", index))
                    .role(Role::Button)
                    .aria_label(format!(
                        "{}消息 #{}，{} B",
                        if message.sent { "发送" } else { "收到" },
                        index + 1,
                        message.body.len()
                    ))
                    .on_a11y_action(AccessibleAction::Click, {
                        let view = cx.entity().downgrade();
                        move |_, window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.selected_messages.insert(id, index);
                                this.update_response(window, cx);
                                cx.notify();
                            });
                        }
                    })
                    .px_2()
                    .py_2()
                    .gap_1()
                    .rounded(px(5.))
                    .border_1()
                    .border_color(if selected == index {
                        cx.theme().primary.opacity(0.25)
                    } else {
                        transparent_black()
                    })
                    .bg(if selected == index {
                        cx.theme().primary.opacity(0.06)
                    } else {
                        transparent_black()
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().muted))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(if message.sent {
                                    IconName::ArrowUp
                                } else {
                                    IconName::ArrowDown
                                })
                                .size(px(13.))
                                .text_color(color),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!(
                                        "{} #{}",
                                        if message.sent { "发送" } else { "收到" },
                                        index + 1
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "+{} ms · {} B",
                                message.elapsed_ms,
                                message.body.len()
                            )),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.selected_messages.insert(id, index);
                        this.update_response(window, cx);
                        cx.notify();
                    })),
            );
        }
        let message = &record.messages[selected];
        let json = serde_json::from_str::<serde_json::Value>(&message.body).is_ok();
        h_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .items_stretch()
            .child(
                div()
                    .id("message-list")
                    .w(px(156.))
                    .flex_shrink_0()
                    .overflow_y_scroll()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(list),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(
                        h_flex()
                            .h(px(32.))
                            .flex_shrink_0()
                            .px_3()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!(
                                        "{}消息 #{}",
                                        if message.sent { "发送" } else { "收到" },
                                        selected + 1
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if json { "JSON" } else { "TEXT" }),
                            ),
                    )
                    .child(
                        Editor::new(&self.response)
                            .readonly(true)
                            .appearance(false)
                            .h(relative(1.))
                            .flex_1()
                            .min_h_0(),
                    ),
            )
            .into_any_element()
    }
    fn response_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let id = self.current().id;
        let record = self
            .session_records
            .get(&id)
            .or_else(|| self.records.get(&id));
        let running = self.jobs.contains_key(&id);
        let status = if running {
            "执行中…".into()
        } else {
            record.map(|r| r.status.clone()).unwrap_or("未发送".into())
        };
        let status_color = if record.is_some_and(|r| r.error) {
            rgb(0xd85c6b).into()
        } else if record.is_some() {
            method_color("GET")
        } else {
            cx.theme().muted_foreground
        };
        let protocol = &self.current().protocol;
        let mut tabs = h_flex().h_full().items_center().gap_2();
        for (key, tab, label) in [
            (
                "response-body",
                ResponseTab::Body,
                if *protocol == Protocol::Http {
                    "Body"
                } else {
                    "消息"
                },
            ),
            (
                "response-headers",
                ResponseTab::Headers,
                if *protocol == Protocol::Grpc {
                    "Metadata"
                } else if *protocol == Protocol::WebSocket {
                    "握手"
                } else {
                    "Headers"
                },
            ),
            ("response-trailers", ResponseTab::Trailers, "Trailers"),
            ("response-timeline", ResponseTab::Timeline, "时间线"),
        ] {
            if tab == ResponseTab::Trailers && *protocol != Protocol::Grpc {
                continue;
            }
            tabs = tabs.child(
                Button::new(key)
                    .small()
                    .ghost()
                    .h_full()
                    .rounded_none()
                    .border_b_2()
                    .border_color(if self.response_tab == tab {
                        cx.theme().primary
                    } else {
                        transparent_black()
                    })
                    .text_color(if self.response_tab == tab {
                        cx.theme().foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .label(label)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.response_tab = tab;
                        this.update_response(window, cx);
                        cx.notify();
                    })),
            );
        }
        let content = if record.is_none()
            && !running
            && !self.logs.contains_key(&id)
            && !self.response_override
            && self.response_tab == ResponseTab::Body
        {
            empty_state(
                IconName::Inbox,
                "响应会显示在这里",
                "发送请求后，查看响应内容、Headers 与耗时。",
                cx,
            )
        } else if *protocol != Protocol::Http
            && self.response_tab == ResponseTab::Body
            && !self.response_override
        {
            self.message_view(record, cx)
        } else {
            Editor::new(&self.response)
                .h(relative(1.))
                .readonly(true)
                .appearance(false)
                .flex_1()
                .min_h_0()
                .into_any_element()
        };
        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().background)
            .overflow_hidden()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .h(px(42.))
                    .flex_shrink_0()
                    .px_4()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("响应"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(4.))
                            .text_size(px(10.))
                            .text_color(status_color)
                            .bg(status_color.opacity(0.08))
                            .child(status),
                    )
                    .when_some(record, |row, record| {
                        row.child(
                            div()
                                .text_size(px(11.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} ms   ·   {} B{}",
                                    record.elapsed_ms,
                                    record.bytes,
                                    if record.truncated {
                                        " · 预览截断"
                                    } else {
                                        ""
                                    }
                                )),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("layout-toggle")
                            .small()
                            .ghost()
                            .icon(if self.project.horizontal {
                                IconName::PanelBottom
                            } else {
                                IconName::PanelRight
                            })
                            .accessibility_label("切换请求与响应布局")
                            .tooltip("切换上下 / 左右布局")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.project.horizontal = !this.project.horizontal;
                                this.dirty(cx);
                            })),
                    )
                    .child(
                        Button::new("response-copy")
                            .small()
                            .ghost()
                            .icon(IconName::Copy)
                            .accessibility_label("复制响应")
                            .tooltip("复制当前响应预览")
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    this.response.read(cx).value().to_string(),
                                ))
                            })),
                    )
                    .child(
                        Button::new("response-save")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowDown)
                            .accessibility_label("保存响应预览")
                            .tooltip("保存响应预览")
                            .disabled(record.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.save_response(cx))),
                    ),
            )
            .child(
                div()
                    .id("response-tabs-scroll")
                    .h(px(32.))
                    .flex_shrink_0()
                    .px_4()
                    .overflow_x_scroll()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(tabs),
            )
            .child(content)
            .into_any_element()
    }
    fn toggle_folder(&mut self, id: Uuid, searching: bool, cx: &mut Context<Self>) {
        self.selected_folder = Some(id);
        if let Some(folder) = self.project.folders.iter_mut().find(|f| f.id == id) {
            if !searching {
                folder.expanded = !folder.expanded;
            }
            self.dirty(cx);
        }
    }
    fn edit_folder(
        &mut self,
        id: Option<Uuid>,
        parent: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let initial = id
            .and_then(|id| self.project.folder(id).ok())
            .map(|f| f.name.clone())
            .unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("文件夹名称"));
        name.update(cx, |s, cx| s.set_value(initial, window, cx));
        let error = cx.new(|cx| InputState::new(window, cx));
        let view = cx.entity().downgrade();
        let focus = name.clone();
        let location = parent
            .map(|id| self.project.folder_path(id))
            .unwrap_or("请求集合".into());
        window.open_dialog(cx, move |dialog, _, _| {
            let input = name.clone();
            let error = error.clone();
            let view = view.clone();
            dialog
                .footer(folder_dialog_footer(if id.is_some() {
                    "保存"
                } else {
                    "创建"
                }))
                .title(if id.is_some() {
                    "重命名文件夹"
                } else {
                    "新建文件夹"
                })
                .w(px(420.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(div().text_sm().child(format!("位置：{location}")))
                        .child(Input::new(&input))
                        .child(
                            div()
                                .text_xs()
                                .child("文件夹属于当前工作区。重命名保留原始 OpenAPI tag 映射。"),
                        )
                        .child(
                            Input::new(&error)
                                .appearance(false)
                                .readonly(true)
                                .h(px(24.))
                                .text_color(rgb(0xd85c6b)),
                        ),
                )
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(if id.is_some() { "保存" } else { "创建" })
                        .cancel_text("取消")
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    let value = input.read(cx).value().to_string();
                    view.update(cx, |this, cx| {
                        let result = if let Some(id) = id {
                            this.project.rename_folder(id, &value).map(|_| id)
                        } else {
                            this.project.add_folder(&value, parent)
                        };
                        match result {
                            Ok(id) => {
                                this.selected_folder = Some(id);
                                this.notice = "文件夹已保存".into();
                                this.dirty(cx);
                                true
                            }
                            Err(e) => {
                                error.update(cx, |s, cx| s.set_value(e.to_string(), window, cx));
                                false
                            }
                        }
                    })
                    .unwrap_or(true)
                })
        });
        focus.update(cx, |s, cx| s.focus(window, cx));
    }
    fn move_collection_item(
        &mut self,
        id: Uuid,
        is_folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = if is_folder {
            self.project.folder(id).ok().and_then(|f| f.parent_id)
        } else {
            self.project
                .requests
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.folder_id)
        };
        let mut destinations = vec![None];
        let mut labels = vec!["请求集合（根目录）".to_owned()];
        let mut folders = self
            .project
            .folders
            .iter()
            .filter(|f| !is_folder || !self.project.folder_contains(id, f.id))
            .map(|f| (self.project.folder_path(f.id), f.id))
            .collect::<Vec<_>>();
        folders.sort();
        for (path, id) in folders {
            labels.push(path);
            destinations.push(Some(id));
        }
        let selected = destinations.iter().position(|p| *p == current).unwrap_or(0);
        let select = cx.new(|cx| {
            SelectState::new(labels, Some(IndexPath::new(selected)), window, cx).searchable(true)
        });
        let error = cx.new(|cx| InputState::new(window, cx));
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let select = select.clone();
            let error = error.clone();
            let view = view.clone();
            let destinations = destinations.clone();
            dialog
                .footer(folder_dialog_footer("移动"))
                .title(if is_folder {
                    "移动文件夹"
                } else {
                    "移动请求"
                })
                .w(px(460.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(Select::new(&select).w_full())
                        .child(div().text_xs().child(
                            "移动只改变本地分组，保留请求内容、运行会话和原始 OpenAPI 映射。",
                        ))
                        .child(
                            Input::new(&error)
                                .appearance(false)
                                .readonly(true)
                                .h(px(24.))
                                .text_color(rgb(0xd85c6b)),
                        ),
                )
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("移动")
                        .cancel_text("取消")
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    let Some(parent) = select
                        .read(cx)
                        .selected_index(cx)
                        .and_then(|i| destinations.get(i.row))
                        .copied()
                    else {
                        return false;
                    };
                    view.update(cx, |this, cx| {
                        let result = if is_folder {
                            this.project.move_folder(id, parent)
                        } else {
                            this.project.move_request_to_folder(id, parent)
                        };
                        match result {
                            Ok(()) => {
                                this.selected_folder = if is_folder { Some(id) } else { parent };
                                this.notice = "已移动到目标文件夹".into();
                                this.dirty(cx);
                                true
                            }
                            Err(e) => {
                                error.update(cx, |s, cx| s.set_value(e.to_string(), window, cx));
                                false
                            }
                        }
                    })
                    .unwrap_or(true)
                })
        });
    }
    fn delete_folder(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .project
            .sync_sources
            .iter()
            .any(|s| s.root_folder == Some(id))
        {
            self.notice = "此文件夹是同步目标，请先在数据源中断开连接，再删除分组".into();
            cx.notify();
            return;
        }
        let parent = self.project.folder(id).ok().and_then(|f| f.parent_id);
        match self.project.remove_folder(id) {
            Ok(removed) => {
                self.close_scope_settings(scope_settings::Scope::Folder(id), window, cx);
                if self.selected_folder == Some(id) {
                    self.selected_folder = parent;
                }
                self.deleted_items
                    .push(DeletedItem::Folder(Box::new(removed)));
                self.notice = "文件夹已删除，内容移至上一级；可撤销".into();
                self.dirty(cx);
            }
            Err(e) => {
                self.notice = e.to_string();
                cx.notify();
            }
        }
    }
    fn show_openapi_mapping(
        &mut self,
        folder: Option<Uuid>,
        request: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        if self.show_sync_mapping(folder, request, cx) {
            return;
        }
        let binding = if let Some(id) = folder {
            self.project.folder(id).ok().and_then(|f| f.openapi.as_ref().map(|m| (m.contract_id,serde_json::json!({"folder":self.project.folder_path(id),"originalTag":m.tag,"originalDirectory":m.directory,"contractId":m.contract_id}))))
        } else {
            request.and_then(|id| self.project.requests.iter().find(|r| r.id == id)).and_then(|r| r.openapi.as_ref().map(|m| (m.contract_id,serde_json::json!({"request":r.name,"folder":r.folder_id.map(|id|self.project.folder_path(id)),"operation":m}))))
        };
        let Some((contract, mapping)) = binding else {
            self.notice = "此项尚未关联 OpenAPI。导入文档后，按 tags 分组并保留接口映射。".into();
            cx.notify();
            return;
        };
        let path = self
            .store
            .directory
            .join("contracts")
            .join(contract.to_string())
            .join("original.json");
        self.definition_path = Some(path.clone());
        self.definitions_open = true;
        self.sync_ui.open = false;
        self.environment_open = false;
        self.history_open = false;
        self.encoding_open = false;
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            let result = (|| -> anyhow::Result<String> {
                let document: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
                let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(path.with_file_name("sources.json"))?)?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({"mapping":mapping,"source":manifest.get("source"),"document":document,"localReferences":manifest.get("files")}))?)
            })().map_err(|e|format!("无法读取映射的本地契约: {e}"));
            let _ = tx.blocking_send(WorkerEvent::Definition(path,result));
        });
        self.refresh_contracts();
        cx.notify();
    }
    fn folder_row(
        &self,
        id: Uuid,
        depth: usize,
        count: usize,
        searching: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Ok(folder) = self.project.folder(id) else {
            return div().into_any_element();
        };
        let expanded = folder.expanded || searching;
        let view = cx.entity().downgrade();
        let menu_view = view.clone();
        let sync_root = self
            .project
            .sync_sources
            .iter()
            .any(|s| s.root_folder == Some(id));
        h_flex()
            .id(SharedString::from(format!("folder-{id}")))
            .role(Role::Button)
            .aria_label(format!(
                "{}文件夹 {}，{} 个请求",
                if expanded { "收起" } else { "展开" },
                folder.name,
                count
            ))
            .on_a11y_action(AccessibleAction::Click, {
                let view = view.clone();
                move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| this.toggle_folder(id, searching, cx));
                }
            })
            .h(px(32.))
            .w_full()
            .flex_shrink_0()
            .pr(px(7.))
            .pl(px(SIDEBAR_ROW_INSET + depth.min(8) as f32 * 12.))
            .gap(px(5.))
            .rounded(px(5.))
            .cursor_pointer()
            .bg(if self.selected_folder == Some(id) {
                cx.theme().primary.opacity(0.06)
            } else {
                transparent_black()
            })
            .hover(|s| s.bg(cx.theme().muted))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_folder(id, searching, cx)))
            .child(
                Icon::new(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size(px(12.))
                .text_color(cx.theme().muted_foreground),
            )
            .child(
                Icon::new(if expanded {
                    gpui_kit::assets::IconName::FolderOpen
                } else {
                    gpui_kit::assets::IconName::Folder
                })
                .size(px(14.))
                .text_color(if folder.openapi.is_some() {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(folder.name.clone()),
            )
            .when(
                folder
                    .openapi
                    .as_ref()
                    .is_some_and(|m| m.tag.is_none() && m.directory.is_none()),
                |row| {
                    row.child(
                        div()
                            .text_size(px(8.))
                            .text_color(cx.theme().primary)
                            .child("API"),
                    )
                },
            )
            .when(sync_root, |row| {
                row.child(
                    div()
                        .text_size(px(8.))
                        .text_color(cx.theme().primary)
                        .child("Apifox"),
                )
            })
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(cx.theme().muted_foreground)
                    .child(count.to_string()),
            )
            .child(
                Button::new(SharedString::from(format!("folder-menu-{id}")))
                    .xsmall()
                    .ghost()
                    .size(px(24.))
                    .icon(IconName::Ellipsis)
                    .accessibility_label(format!("文件夹操作 {}", folder.name))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .dropdown_menu(move |menu, _, _| folder_menu(menu, menu_view.clone(), id)),
            )
            .context_menu(move |menu, _, _| folder_menu(menu, view.clone(), id))
            .into_any_element()
    }
    fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let filter = self.search.read(cx).value().to_lowercase();
        let mut list = v_flex().gap(px(2.)).px(px(SIDEBAR_GUTTER)).pb_3();
        for row in self.project.collection_rows(&filter) {
            let (i, depth) = match row {
                CollectionRow::Folder { id, depth, count } => {
                    list = list.child(self.folder_row(id, depth, count, !filter.is_empty(), cx));
                    continue;
                }
                CollectionRow::Request { index, depth } => (index, depth),
            };
            let request = &self.project.requests[i];
            let id = request.id;
            let view = cx.entity().downgrade();
            let menu_view = view.clone();
            let tag = if request.protocol == Protocol::Http {
                request.method.as_str()
            } else {
                request.protocol.label()
            };
            let selected = self.active_scope.is_none()
                && i == self.project.active_request
                && self.project.tabs().contains(&id);
            let mut method_color = method_color(tag);
            // Keep the small method labels legible on both sidebar surfaces.
            method_color.l = if cx.theme().is_dark() {
                method_color.l.max(0.64)
            } else {
                method_color.l.min(0.28)
            };
            let hover_background = if selected {
                cx.theme().background
            } else {
                cx.theme().muted
            };
            list = list.child(
                h_flex()
                    .id(("request", i))
                    .role(Role::Button)
                    .aria_label(format!("{tag} {}", request.name))
                    .on_a11y_action(AccessibleAction::Click, {
                        let view = cx.entity().downgrade();
                        move |_, window, cx| {
                            let _ = view.update(cx, |this, cx| this.switch_request(i, window, cx));
                        }
                    })
                    .w_full()
                    .h(px(SIDEBAR_ROW_HEIGHT))
                    .flex_shrink_0()
                    // Keep the inner columns on the shared inset despite the 1 px row border.
                    .px(px(SIDEBAR_ROW_INSET - 1.))
                    .pl(px(SIDEBAR_ROW_INSET - 1. + depth.min(8) as f32 * 12.))
                    .gap_2()
                    .items_center()
                    .rounded(px(5.))
                    .border_1()
                    .border_color(if selected {
                        cx.theme().primary.opacity(0.22)
                    } else {
                        transparent_black()
                    })
                    .bg(if selected {
                        cx.theme().background
                    } else {
                        transparent_black()
                    })
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover_background))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.switch_request(i, window, cx)),
                    )
                    .child(
                        div()
                            .w(px(SIDEBAR_METHOD_WIDTH))
                            .h(px(20.))
                            .flex_shrink_0()
                            .rounded(px(4.))
                            .bg(method_color.opacity(0.08))
                            .text_color(method_color)
                            .font_family("Menlo")
                            .text_size(px(9.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(relative(1.))
                            .whitespace_nowrap()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(tag.to_owned()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .font_weight(if selected {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .truncate()
                            .child(request.name.clone()),
                    )
                    .child(
                        Button::new(("request-operations", i))
                            .xsmall()
                            .ghost()
                            .size(px(24.))
                            .flex_shrink_0()
                            .icon(IconName::Ellipsis)
                            .accessibility_label(format!("请求操作 {}", request.name))
                            .tooltip("请求操作")
                            .dropdown_menu(move |menu, _, _| {
                                request_menu(menu, menu_view.clone(), id)
                            }),
                    )
                    .context_menu(move |menu, _, _| request_menu(menu, view.clone(), id)),
            );
        }
        let new_request_view = cx.entity().downgrade();
        v_flex()
            .w_full()
            .flex_shrink_0()
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .h(px(44.))
                    .flex_shrink_0()
                    .px(px(SIDEBAR_GUTTER + SIDEBAR_ROW_INSET))
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("请求集合"),
                    )
                    .child(
                        div()
                            .min_w(px(20.))
                            .h(px(18.))
                            .px_1()
                            .rounded(px(4.))
                            .bg(cx.theme().border.opacity(0.45))
                            .text_color(cx.theme().muted_foreground)
                            .text_size(px(10.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(self.project.requests.len().to_string()),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("new-request")
                            .xsmall()
                            .ghost()
                            .size(px(24.))
                            .flex_shrink_0()
                            .icon(IconName::Plus)
                            .accessibility_label("新建请求")
                            .tooltip("新建请求或文件夹")
                            .dropdown_menu(move |menu, _, _| {
                                new_request_menu(menu, new_request_view.clone())
                            }),
                    ),
            )
            .child(
                div().px(px(SIDEBAR_GUTTER)).pb_2().child(
                    Input::new(&self.search)
                        .small()
                        .h(px(30.))
                        .w_full()
                        .prefix(Icon::new(IconName::Search).size(px(14.))),
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
                    .px(px(SIDEBAR_GUTTER))
                    .py_2()
                    .gap(px(2.))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        navigation_button("history-open", IconName::RotateCw, "历史记录")
                            .selected(self.history_open)
                            .accessibility_label(format!("历史记录，共 {} 条", self.history.len()))
                            .child(
                                div()
                                    .w(px(24.))
                                    .flex_shrink_0()
                                    .text_size(px(10.))
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center()
                                    .child(self.history.len().to_string()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.stash_environment(cx);
                                this.active_scope = None;
                                this.history_open = !this.history_open;
                                this.sync_ui.open = false;
                                this.environment_open = false;
                                this.definitions_open = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        navigation_button("workspace-settings", IconName::Settings2, "工作区设置")
                            .on_click(cx.listener(|s, _, window, cx| {
                                s.open_scope_settings(scope_settings::Scope::Workspace, window, cx)
                            })),
                    )
                    .child(
                        navigation_button("definitions-open", IconName::BookOpen, "接口定义")
                            .selected(self.definitions_open)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.stash_environment(cx);
                                this.environment_open = false;
                                this.history_open = false;
                                this.active_scope = None;
                                this.definitions_open = !this.definitions_open;
                                this.sync_ui.open = false;
                                this.refresh_contracts();
                                cx.notify();
                            })),
                    )
                    .child(
                        navigation_button(
                            "sources-open",
                            gpui_kit::assets::IconName::Plug,
                            "数据源",
                        )
                        .selected(self.sync_ui.open)
                        .child(
                            div()
                                .w(px(24.))
                                .text_size(px(10.))
                                .text_color(cx.theme().muted_foreground)
                                .text_center()
                                .child(self.project.sync_sources.len().to_string()),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.open_sync(None, cx))),
                    )
                    .child(
                        navigation_button("import-openapi", IconName::FileText, "导入 OpenAPI")
                            .on_click(cx.listener(|this, _, _, cx| this.import_openapi(cx))),
                    )
                    .when(!self.deleted_items.is_empty(), |column| {
                        column.child(
                            navigation_button("undo-delete", IconName::Undo, "撤销删除").on_click(
                                cx.listener(|this, _, window, cx| this.undo_delete(window, cx)),
                            ),
                        )
                    }),
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
                            this.history_preview = true;
                            this.history_response.update(cx, |s, cx| s.set_value(format!("{} · {}\n{} · {} ms\n\nHeaders / Metadata\n{}\n\nTrailers\n{}\n\n{}", record.request_name, record.environment, record.status, record.elapsed_ms, headers_text(&record.headers), headers_text(&record.trailers), record.body), window, cx));
                            this.notice = "请求已删除，保留的执行历史仍可查看".into();
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
            .when(self.history_preview, |column| {
                column.child(
                    Editor::new(&self.history_response)
                        .readonly(true)
                        .appearance(false)
                        .h(px(250.))
                        .flex_shrink_0(),
                )
            })
            .into_any_element()
    }
}
impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let r = self.current();
        let protocol = r.protocol.clone();
        let current_running = self.jobs.contains_key(&r.id);
        let top = h_flex()
            .h_full()
            .flex_1()
            .min_w_0()
            .pr_4()
            .gap_2()
            .items_center()
            .child(brandmark(cx))
            .children(self.workspace_strip.clone())
            .child(
                div()
                    .w(px(110.))
                    .h(px(28.))
                    .child(Select::new(&self.env_select).small().w_full()),
            )
            .child(
                Button::new("environment-open")
                    .small()
                    .ghost()
                    .icon(IconName::Settings2)
                    .label("变量")
                    .accessibility_label("工作区与环境")
                    .tooltip("工作区名称与环境变量")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.stash_environment(cx);
                        this.active_scope = None;
                        this.environment_open = !this.environment_open;
                        this.sync_ui.open = false;
                        this.history_open = false;
                        this.definitions_open = false;
                        this.load_environment(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("workbench-options")
                    .small()
                    .ghost()
                    .icon(IconName::Ellipsis)
                    .tooltip("更多选项")
                    .accessibility_label("更多选项")
                    .dropdown_menu({
                        let view = cx.entity().downgrade();
                        move |menu, _, _| {
                            let save = view.clone();
                            let encoding = view.clone();
                            menu.item(
                                PopupMenuItem::new("应用设置 · ⌘ ,")
                                    .on_click(|_, _, cx| crate::ui_settings::open(None, cx)),
                            )
                            .item(PopupMenuItem::new("工作区设置…").on_click({
                                let view = view.clone();
                                move |_, window, cx| {
                                    let _ = view.update(cx, |s, cx| {
                                        s.open_scope_settings(
                                            scope_settings::Scope::Workspace,
                                            window,
                                            cx,
                                        )
                                    });
                                }
                            }))
                            .item(
                                PopupMenuItem::new("检查更新…")
                                    .on_click(|_, _, cx| crate::ui_settings::open_updates(cx)),
                            )
                            .item(PopupMenuItem::new("保存工作区 · ⌘ S").on_click(
                                move |_, _, cx| {
                                    let _ = save.update(cx, |this, cx| this.save(cx));
                                },
                            ))
                            .separator()
                            .item(PopupMenuItem::new("请求编码预览").on_click(
                                move |_, window, cx| {
                                    let _ = encoding.update(cx, |this, cx| {
                                        this.active_scope = None;
                                        let text = this
                                            .project
                                            .prepare_request(this.current())
                                            .and_then(|r| {
                                                r.encoding_preview(&this.project.variables())
                                            })
                                            .unwrap_or_else(|e| format!("预览失败: {e}"));
                                        this.encoding
                                            .update(cx, |s, cx| s.set_value(text, window, cx));
                                        this.encoding_open = true;
                                        cx.notify();
                                    });
                                },
                            ))
                            .item(
                                PopupMenuItem::new("复制实际 URL").on_click({
                                    let view = view.clone();
                                    move |_, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            if let Ok(url) = this
                                                .current()
                                                .resolved_url(&this.project.variables())
                                            {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    url,
                                                ));
                                            }
                                        });
                                    }
                                }),
                            )
                        }
                    }),
            );
        let main: AnyElement = if self.active_scope.is_some() {
            self.scope_settings_panel(cx)
        } else if self.sync_ui.open {
            self.sync_page(window, cx)
        } else if self.environment_open {
            self.environment_panel(cx)
        } else if self.history_open {
            self.history_panel(cx)
        } else if self.definitions_open {
            self.definitions_panel(cx)
        } else if self.encoding_open {
            v_flex()
                .flex_1()
                .min_h_0()
                .p_4()
                .gap_3()
                .child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().child("请求参数与 Body 编码预览"))
                        .child(
                            Button::new("encoding-copy")
                                .small()
                                .ghost()
                                .label("复制预览")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        this.encoding.read(cx).value().to_string(),
                                    ))
                                })),
                        )
                        .child(
                            Button::new("encoding-back")
                                .small()
                                .label("返回请求")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.encoding_open = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    Editor::new(&self.encoding)
                        .readonly(true)
                        .appearance(false)
                        .h(relative(1.))
                        .flex_1(),
                )
                .into_any_element()
        } else if self.project.tabs().is_empty() || self.project.requests.is_empty() {
            v_flex()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    Icon::new(IconName::SquareTerminal)
                        .size(px(32.))
                        .text_color(cx.theme().primary),
                )
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("从一个请求开始"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child("打开左侧的请求，或新建 HTTP、gRPC、WebSocket 请求。"),
                )
                .child(
                    Button::new("empty-new-request")
                        .primary()
                        .icon(IconName::Plus)
                        .label("新建 HTTP 请求")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.new_request(Protocol::Http, window, cx)
                        })),
                )
                .into_any_element()
        } else {
            let mut heading = h_flex()
                .h(px(58.))
                .flex_shrink_0()
                .px_5()
                .gap_3()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child(protocol.label()),
                )
                .child(Input::new(&self.name).small().appearance(false).flex_1())
                .child(
                    Button::new("duplicate-request")
                        .small()
                        .ghost()
                        .icon(IconName::Copy)
                        .accessibility_label("复制请求")
                        .tooltip("复制请求")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.duplicate_request(this.current().id, window, cx);
                        })),
                )
                .child(
                    Button::new("delete-request")
                        .small()
                        .ghost()
                        .icon(IconName::Delete)
                        .accessibility_label("删除请求")
                        .tooltip("删除请求")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.delete_request(this.current().id, window, cx)
                        })),
                )
                .child(
                    Button::new("schema-toggle")
                        .small()
                        .ghost()
                        .selected(self.project.show_schema)
                        .icon(IconName::PanelRight)
                        .accessibility_label("请求结构面板")
                        .tooltip("显示 / 隐藏请求结构")
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
            let mut url_row = h_flex()
                .h(px(46.))
                .mx_5()
                .mb_2()
                .px_1()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().input)
                .bg(cx.theme().background)
                .gap_2()
                .items_center();
            if protocol == Protocol::Http {
                url_row = url_row.child(
                    h_flex()
                        .w(px(142.))
                        .h(px(42.))
                        .items_center()
                        .gap_0()
                        .border_r_1()
                        .border_color(cx.theme().border)
                        .child(
                            Input::new(&self.method)
                                .appearance(false)
                                .h(px(42.))
                                .flex_1()
                                .text_center()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(method_color(&self.current().method)),
                        )
                        .child(
                            Button::new("method-presets")
                                .ghost()
                                .small()
                                .icon(IconName::ChevronDown)
                                .tooltip("选择 Method，或在左侧直接输入")
                                .accessibility_label("HTTP method 预设")
                                .dropdown_menu({
                                    let view = cx.entity().downgrade();
                                    move |mut menu, _, _| {
                                        for method in HTTP_METHODS {
                                            let view = view.clone();
                                            menu = menu.item(PopupMenuItem::new(method).on_click(
                                                move |_, window, cx| {
                                                    let _ = view.update(cx, |this, cx| {
                                                        this.current_mut().method = method.into();
                                                        this.method.update(cx, |s, cx| {
                                                            s.set_value(method, window, cx)
                                                        });
                                                        this.dirty(cx);
                                                    });
                                                },
                                            ));
                                        }
                                        menu
                                    }
                                }),
                        ),
                );
            }
            url_row = url_row.child(self.address(cx)).child(
                Button::new("send-request")
                    .icon(IconName::ArrowRight)
                    .tooltip("发送请求 · ⌘ Enter")
                    .primary()
                    .label(if protocol == Protocol::Http {
                        "发送"
                    } else if protocol == Protocol::Grpc {
                        "调用"
                    } else {
                        "连接"
                    })
                    .disabled(self.jobs.contains_key(&self.current().id))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.active_scope.is_none() {
                            this.send(window, cx);
                        }
                    })),
            );
            if self.jobs.contains_key(&self.current().id) {
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
                        .px_5()
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
                        .h_full()
                        .rounded_none()
                        .border_b_2()
                        .border_color(if self.tab == tab {
                            cx.theme().primary
                        } else {
                            transparent_black()
                        })
                        .text_color(if self.tab == tab {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        })
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
            let panels = self.split(
                if self.project.horizontal {
                    Split::Horizontal
                } else {
                    Split::Vertical
                },
                request.into_any_element(),
                self.response_panel(cx),
                cx,
            );
            editor = editor.child(div().flex_1().min_h_0().child(panels));
            let work = if self.project.show_schema {
                let schema = self.structure_panel(&protocol, cx);
                self.split(
                    Split::Schema,
                    editor.into_any_element(),
                    schema.into_any_element(),
                    cx,
                )
            } else {
                editor.into_any_element()
            };
            v_flex()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(heading)
                .child(url_row)
                .child(div().flex_1().min_h_0().child(work))
                .into_any_element()
        };
        let main = v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(self.request_tabs(window, cx))
            .child(div().flex_1().min_h_0().child(main))
            .into_any_element();
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
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                let Some((kind, start, size)) = this.resizing else {
                    return;
                };
                if e.pressed_button != Some(MouseButton::Left) {
                    this.resizing = None;
                    return;
                }
                let delta = if matches!(kind, Split::Vertical) {
                    e.position.y - start.y
                } else {
                    e.position.x - start.x
                };
                this.resize_to(
                    kind,
                    size + f32::from(delta)
                        * if matches!(kind, Split::Schema) {
                            -1.
                        } else {
                            1.
                        },
                    cx,
                );
                cx.stop_propagation();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.resizing.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.resizing.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &SendRequest, window, cx| {
                if this.active_scope.is_none() {
                    this.send(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SaveProject, window, cx| {
                if this.active_scope.is_some() {
                    this.save_scope_settings(window, cx);
                } else {
                    this.save(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SearchRequests, window, cx| {
                this.search.read(cx).focus_handle(cx).focus(window, cx)
            }))
            .on_action(cx.listener(|this, _: &CloseRequest, window, cx| {
                if let Some(scope) = this.active_scope {
                    this.close_scope_settings(scope, window, cx);
                } else {
                    this.close_request(this.current().id, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NewRequest, window, cx| {
                this.new_request(Protocol::Http, window, cx)
            }))
            .on_action(cx.listener(|this, _: &UndoDelete, window, cx| this.undo_delete(window, cx)))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(".AppleSystemUIFont")
            .text_size(px(13.))
            .child(
                TitleBar::new()
                    .h(WORKBENCH_TITLEBAR_HEIGHT)
                    .bg(cx.theme().sidebar)
                    .border_color(cx.theme().border)
                    .child(top),
            )
            .child(div().flex_1().min_h_0().child(self.split(
                Split::Sidebar,
                self.sidebar(cx),
                main,
                cx,
            )))
            .child(
                h_flex()
                    .h(px(26.))
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
                        "● 仅本地"
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
                                this.response_override = true;
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
        for job in self.jobs.values() {
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
        "QUERY" => 0x368dde,
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

fn navigation_button(
    id: &'static str,
    icon: impl Into<Icon>,
    label: impl Into<SharedString>,
) -> Button {
    let label = label.into();
    Button::new(id)
        .small()
        .ghost()
        .w_full()
        .h(px(SIDEBAR_ROW_HEIGHT))
        .px(px(SIDEBAR_ROW_INSET))
        .rounded(px(5.))
        .accessibility_label(label.clone())
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .size(px(16.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(icon).size(px(14.))),
                )
                .child(div().text_size(px(12.)).truncate().child(label)),
        )
}

fn new_request_menu(mut menu: PopupMenu, view: WeakEntity<Workbench>) -> PopupMenu {
    for (protocol, label) in [
        (Protocol::Http, "HTTP 请求"),
        (Protocol::Grpc, "gRPC 请求"),
        (Protocol::WebSocket, "WebSocket 请求"),
    ] {
        let view = view.clone();
        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
            let _ = view.update(cx, |this, cx| {
                this.new_request(protocol.clone(), window, cx)
            });
        }));
    }
    let folder = view.clone();
    menu =
        menu.separator().item(
            PopupMenuItem::new("新建文件夹…").on_click(move |_, window, cx| {
                let _ = folder.update(cx, |this, cx| {
                    this.edit_folder(None, this.selected_folder, window, cx)
                });
            }),
        );
    menu = menu.item(
        PopupMenuItem::new("新建根文件夹…").on_click(move |_, window, cx| {
            let _ = view.update(cx, |this, cx| this.edit_folder(None, None, window, cx));
        }),
    );

    menu
}

fn empty_state(icon: IconName, title: &str, description: &str, cx: &App) -> AnyElement {
    v_flex()
        .flex_1()
        .min_h_0()
        .items_center()
        .justify_center()
        .gap_3()
        .p_5()
        .child(
            div()
                .size(px(46.))
                .rounded(px(14.))
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .size(px(22.))
                        .text_color(cx.theme().muted_foreground),
                ),
        )
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_size(px(11.))
                .text_color(cx.theme().muted_foreground)
                .child(description.to_owned()),
        )
        .into_any_element()
}

pub(crate) fn brandmark(cx: &App) -> AnyElement {
    h_flex()
        .gap_2()
        .items_center()
        .pr_3()
        .flex_shrink_0()
        .child(
            div()
                .size(px(23.))
                .rounded(px(7.))
                .bg(cx.theme().primary)
                .text_color(cx.theme().primary_foreground)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.))
                .font_weight(FontWeight::BOLD)
                .child("z"),
        )
        .child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::BOLD)
                .child("zeph"),
        )
        .into_any_element()
}

pub fn apply_theme(cx: &mut App) {
    let dark = cx.theme().is_dark();
    Theme::update(cx, |theme| {
        let color = |light, dark_color| rgb(if dark { dark_color } else { light }).into();
        theme.font_size = px(13.);
        theme.radius = px(6.);
        theme.radius_lg = px(9.);
        theme.colors.background = color(0xffffff, 0x171a23);
        theme.colors.foreground = color(0x262c3b, 0xdce1ec);
        theme.colors.border = color(0xe7e9ef, 0x2a2f3d);
        theme.colors.input = color(0xdde1ea, 0x343b4d);
        theme.colors.muted = color(0xf7f8fb, 0x202532);
        theme.colors.muted_foreground = color(0x7a8293, 0x929caf);
        theme.colors.sidebar = color(0xf5f6f9, 0x12151d);
        theme.colors.sidebar_foreground = theme.colors.foreground;
        theme.colors.title_bar = theme.colors.sidebar;
        theme.colors.title_bar_border = theme.colors.border;
        theme.colors.primary = color(0x6656c9, 0x9b8aed);
        theme.colors.primary_hover = color(0x5948b7, 0x8978dc);
        theme.colors.primary_active = color(0x4e3fa4, 0x7c6acb);
        theme.colors.primary_foreground = color(0xffffff, 0x181326);
        theme.colors.button_primary = theme.colors.primary;
        theme.colors.button_primary_hover = theme.colors.primary_hover;
        theme.colors.button_primary_active = theme.colors.primary_active;
        theme.colors.button_primary_foreground = theme.colors.primary_foreground;
        theme.colors.button = theme.colors.background;
        theme.colors.button_foreground = theme.colors.foreground;
        theme.colors.button_hover = theme.colors.muted;
        theme.colors.button_active = color(0xeeecf9, 0x302b49);
        theme.colors.ring = theme.colors.primary;
        theme.colors.accent = color(0xeeecf9, 0x302b49);
        theme.colors.accent_foreground = color(0x5d4caf, 0xc1b7f0);
        theme.colors.popover = theme.colors.background;
        theme.colors.popover_foreground = theme.colors.foreground;
        let editor = &mut Arc::make_mut(&mut theme.highlight_theme).style;
        editor.editor_background = Some(theme.colors.background);
        editor.editor_gutter_background = Some(theme.colors.background);
        editor.editor_foreground = Some(theme.colors.foreground);
        editor.editor_active_line = Some(theme.colors.muted);
        editor.editor_line_number = Some(theme.colors.muted_foreground);
        editor.editor_active_line_number = Some(theme.colors.foreground);
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

fn headers_text(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}
fn pretty_payload(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| text.to_owned())
}
fn record_session_event(record: &mut RunRecord, event: &SessionEvent, elapsed_ms: u128) {
    if let SessionEvent::Sent(body) | SessionEvent::Received(body) = event {
        let bytes = record.messages.iter().map(|m| m.body.len()).sum::<usize>();
        if record.messages.len() < 2048 && bytes + body.len() <= 1024 * 1024 {
            record.messages.push(SessionMessage {
                sent: matches!(event, SessionEvent::Sent(_)),
                elapsed_ms,
                body: body.clone(),
            });
        } else {
            record.truncated = true;
        }
    }
    let label = match event {
        SessionEvent::Opened => {
            record.status = "已连接".into();
            "连接已建立"
        }
        SessionEvent::Metadata(headers) => {
            record.headers.extend(headers.clone());
            "收到 Headers / Metadata"
        }
        SessionEvent::Finished { code, trailers } => {
            record.status = code.clone();
            record.trailers = trailers.clone();
            record.error = record.method == "gRPC" && code != "0 OK";
            "调用 / 连接已结束"
        }
        SessionEvent::Failed(error) => {
            record.status = error.clone();
            record.error = true;
            "执行失败"
        }
        SessionEvent::Sent(_) => "发送消息",
        SessionEvent::Received(_) => "收到消息",
    };
    if record.timeline.len() < 2048 {
        record.timeline.push(TimingMark {
            elapsed_ms,
            label: label.into(),
        });
    }
}
fn folder_dialog_footer(label: &'static str) -> AnyElement {
    DialogFooter::new()
        .child(
            div()
                .w(px(76.))
                .h(px(32.))
                .child(DialogClose::new().trigger(|button| button.label("取消").w_full())),
        )
        .child(
            div()
                .w(px(if label.chars().count() > 4 { 120. } else { 76. }))
                .h(px(32.))
                .child(
                    DialogAction::new().child(
                        Button::new("folder-dialog-confirm")
                            .primary()
                            .label(label)
                            .w_full(),
                    ),
                ),
        )
        .into_any_element()
}

fn folder_menu(mut menu: PopupMenu, view: WeakEntity<Workbench>, id: Uuid) -> PopupMenu {
    for (protocol, label) in [
        (Protocol::Http, "新建 HTTP 请求"),
        (Protocol::Grpc, "新建 gRPC 请求"),
        (Protocol::WebSocket, "新建 WebSocket 请求"),
    ] {
        let view = view.clone();
        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
            let _ = view.update(cx, |this, cx| {
                this.selected_folder = Some(id);
                this.new_request(protocol.clone(), window, cx);
            });
        }));
    }
    let settings = view.clone();
    let child = view.clone();
    let rename = view.clone();
    let move_to = view.clone();
    let mapping = view.clone();
    menu.separator()
        .item(
            PopupMenuItem::new("文件夹设置…").on_click(move |_, window, cx| {
                let _ = settings.update(cx, |s, cx| {
                    s.open_scope_settings(scope_settings::Scope::Folder(id), window, cx)
                });
            }),
        )
        .item(
            PopupMenuItem::new("新建子文件夹…").on_click(move |_, window, cx| {
                let _ = child.update(cx, |this, cx| this.edit_folder(None, Some(id), window, cx));
            }),
        )
        .item(
            PopupMenuItem::new("重命名文件夹…").on_click(move |_, window, cx| {
                let _ = rename.update(cx, |this, cx| {
                    let parent = this.project.folder(id).ok().and_then(|f| f.parent_id);
                    this.edit_folder(Some(id), parent, window, cx);
                });
            }),
        )
        .item(
            PopupMenuItem::new("移动文件夹…").on_click(move |_, window, cx| {
                let _ = move_to.update(cx, |this, cx| {
                    this.move_collection_item(id, true, window, cx)
                });
            }),
        )
        .item(
            PopupMenuItem::new("查看 OpenAPI 映射").on_click(move |_, _, cx| {
                let _ =
                    mapping.update(cx, |this, cx| this.show_openapi_mapping(Some(id), None, cx));
            }),
        )
        .separator()
        .item(
            PopupMenuItem::new("删除文件夹，保留内容").on_click(move |_, window, cx| {
                let _ = view.update(cx, |this, cx| this.delete_folder(id, window, cx));
            }),
        )
}

fn request_menu(menu: PopupMenu, view: WeakEntity<Workbench>, id: Uuid) -> PopupMenu {
    let open = view.clone();
    let rename = view.clone();
    let duplicate = view.clone();
    let close = view.clone();
    let move_up = view.clone();
    let move_down = view.clone();
    let move_to = view.clone();
    let mapping = view.clone();
    menu.item(
        PopupMenuItem::new("打开请求").on_click(move |_, window, cx| {
            let _ = open.update(cx, |this, cx| {
                if let Some(index) = this.project.requests.iter().position(|r| r.id == id) {
                    this.switch_request(index, window, cx);
                }
            });
        }),
    )
    .item(PopupMenuItem::new("重命名").on_click(move |_, window, cx| {
        let _ = rename.update(cx, |this, cx| {
            if let Some(index) = this.project.requests.iter().position(|r| r.id == id) {
                this.switch_request(index, window, cx);
                this.name.read(cx).focus_handle(cx).focus(window, cx);
            }
        });
    }))
    .item(
        PopupMenuItem::new("复制请求").on_click(move |_, window, cx| {
            let _ = duplicate.update(cx, |this, cx| this.duplicate_request(id, window, cx));
        }),
    )
    .item(
        PopupMenuItem::new("关闭标签").on_click(move |_, window, cx| {
            let _ = close.update(cx, |this, cx| this.close_request(id, window, cx));
        }),
    )
    .item(
        PopupMenuItem::new("移动到文件夹…").on_click(move |_, window, cx| {
            let _ = move_to.update(cx, |this, cx| {
                this.move_collection_item(id, false, window, cx)
            });
        }),
    )
    .item(
        PopupMenuItem::new("查看 OpenAPI 映射").on_click(move |_, _, cx| {
            let _ = mapping.update(cx, |this, cx| this.show_openapi_mapping(None, Some(id), cx));
        }),
    )
    .separator()
    .item(PopupMenuItem::new("上移").on_click(move |_, _, cx| {
        let _ = move_up.update(cx, |this, cx| this.move_request(id, -1, cx));
    }))
    .item(PopupMenuItem::new("下移").on_click(move |_, _, cx| {
        let _ = move_down.update(cx, |this, cx| this.move_request(id, 1, cx));
    }))
    .separator()
    .item(
        PopupMenuItem::new("删除请求（可撤销）").on_click(move |_, window, cx| {
            let _ = view.update(cx, |this, cx| this.delete_request(id, window, cx));
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{Protocol, Request, RunRecord, SessionEvent, record_session_event};
    #[test]
    fn grpc_metadata_and_trailers_stay_separate_in_records() {
        let request = Request::new(Protocol::Grpc, "rpc");
        let mut record = RunRecord::error(&request, "Local".into(), String::new());
        record.method = "gRPC".into();
        record_session_event(
            &mut record,
            &SessionEvent::Metadata(vec![("x-start".into(), "one".into())]),
            10,
        );
        record_session_event(&mut record, &SessionEvent::Received("response".into()), 20);
        record_session_event(
            &mut record,
            &SessionEvent::Finished {
                code: "0 OK".into(),
                trailers: vec![("x-end".into(), "two".into())],
            },
            30,
        );
        assert_eq!(record.headers, vec![("x-start".into(), "one".into())]);
        assert_eq!(record.trailers, vec![("x-end".into(), "two".into())]);
        assert!(!record.error);
        assert_eq!(record.messages.len(), 1);
        assert!(!record.messages[0].sent);
        assert_eq!(record.messages[0].body, "response");
        assert_eq!(record.messages[0].elapsed_ms, 20);
        assert_eq!(
            record
                .timeline
                .iter()
                .map(|m| m.elapsed_ms)
                .collect::<Vec<_>>(),
            vec![10, 20, 30]
        );
        record_session_event(
            &mut record,
            &SessionEvent::Finished {
                code: "1 CANCELLED".into(),
                trailers: vec![],
            },
            40,
        );
        assert!(record.error);
    }
    #[test]
    fn session_payloads_preserve_order_and_are_bounded() {
        let request = Request::new(Protocol::WebSocket, "echo");
        let mut record = RunRecord::error(&request, "Local".into(), String::new());
        record_session_event(&mut record, &SessionEvent::Sent("{\"n\":1}".into()), 5);
        record_session_event(&mut record, &SessionEvent::Received("{\"n\":1}".into()), 10);
        assert!(record.messages[0].sent);
        assert!(!record.messages[1].sent);
        assert_eq!(
            record
                .messages
                .iter()
                .map(|m| m.elapsed_ms)
                .collect::<Vec<_>>(),
            vec![5, 10]
        );
        let saved = serde_json::to_string(&record).unwrap();
        let restored: RunRecord = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.messages[1].body, "{\"n\":1}");
        record_session_event(
            &mut record,
            &SessionEvent::Received("x".repeat(1024 * 1024)),
            20,
        );
        assert!(record.truncated);
        assert_eq!(record.messages.len(), 2);
    }
}
