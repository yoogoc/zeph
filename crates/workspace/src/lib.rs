use gpui::{App, AppContext, Context, Entity, EventEmitter, Global, SharedString};
use log::error;
use models::{ContentType, CreateWorkspace, Environment, EnvironmentVariable, Folder, HttpConfig, HttpMethod, Service, ServiceType, Workspace};
use std::sync::Arc;
use storage::{
    Database, EnvironmentStore, EnvironmentVariableStore, FolderStore, SchemaStore, ServiceStore,
    WorkspaceStore,
};
use uuid::Uuid;

mod openapi;
pub use openapi::{
    check_name_conflicts, generate_import_preview, import_openapi, import_openapi_with_options,
    import_openapi_with_options_and_existing_folders, import_openapi_with_validation,
    parse_openapi, resolve_conflicts, validate_import_options, validate_import_request,
    validate_openapi_content, ConflictResolution, ConflictType, ImportOptions, ImportPreview,
    ImportResult, NameConflict, OpenApiSpec, SchemaFolderStrategy, SchemaPreview, ServicePreview,
};

#[derive(Clone, Debug)]
pub enum WorkspaceEvent {
    Open(Workspace),
    Close,
    Reload,
}

#[derive(Clone, Debug)]
pub enum WorkspaceServiceEvent {
    Open(Workspace, ContentType),
    Switch {
        workspace: Workspace,
        old: ContentType,
        new: Option<ContentType>,
    },
}

pub fn init(cx: &mut App) {
    WorkspaceRegistry::set_global(cx.new(WorkspaceRegistry::new), cx);
}

struct GlobalWorkspaceRegistry(Entity<WorkspaceRegistry>);
impl Global for GlobalWorkspaceRegistry {}

pub struct WorkspaceRegistry {
    db: Arc<Database>,
    app_state: models::ApplicationState,
}

impl EventEmitter<WorkspaceEvent> for WorkspaceRegistry {}
impl EventEmitter<WorkspaceServiceEvent> for WorkspaceRegistry {}

impl WorkspaceRegistry {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalWorkspaceRegistry>().0.clone()
    }

    fn set_global(registry: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalWorkspaceRegistry(registry));
    }

    fn db_path() -> std::path::PathBuf {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("zeph");
        std::fs::create_dir_all(&data_dir).ok();
        data_dir.join("zeph.db")
    }

    fn new(_cx: &mut Context<Self>) -> Self {
        let db_path = Self::db_path();
        let db = Database::open(&db_path).expect("Failed to open database");
        let db = Arc::new(db);

        let mut app_state = models::ApplicationState::default();

        let store = WorkspaceStore::new(db.conn());
        app_state.workspaces = store.list().unwrap_or_default();

        Self { db, app_state }
    }

    pub fn get_state(&self) -> &models::ApplicationState {
        &self.app_state
    }

    pub fn get_opened_workspaces(&self) -> Vec<&Workspace> {
        self.app_state
            .opened_workspaces
            .iter()
            .filter_map(|id| self.app_state.workspaces.iter().find(|w| w.id == *id))
            .collect()
    }

    pub fn get_active_workspace(&self) -> Option<&Workspace> {
        self.app_state
            .active_workspace_id
            .and_then(|id| self.app_state.workspaces.iter().find(|w| w.id == id))
    }

    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.app_state
            .active_workspace_id
            .and_then(|id| self.app_state.workspaces.iter_mut().find(|w| w.id == id))
    }

    // === Workspace CRUD ===

    pub fn create_workspace(&mut self, param: CreateWorkspace, cx: &mut Context<Self>) {
        let workspace = Workspace::new(param.name);
        let store = WorkspaceStore::new(self.db.conn());
        store.insert(&workspace).ok();
        if param.create_sample_services {
            self.create_sample_services(workspace.id);
        }
        self.app_state.workspaces.push(workspace.clone());
        cx.emit(WorkspaceEvent::Open(workspace.clone()));
    }

    pub fn open_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if !self.app_state.opened_workspaces.contains(&workspace_id) {
            self.app_state.opened_workspaces.push(workspace_id);
        }
        self.load_workspace_content(workspace_id);
        self.set_active_workspace(workspace_id, cx);
    }

    pub fn close_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        self.app_state
            .opened_workspaces
            .retain(|&id| id != workspace_id);

        if self.app_state.active_workspace_id == Some(workspace_id) {
            if let Some(&next_id) = self.app_state.opened_workspaces.last() {
                self.app_state.active_workspace_id = Some(next_id);
                self.app_state.current_view = models::ApplicationView::Workspace(next_id);
            } else {
                self.go_to_home(cx);
                return;
            }
        }
        cx.emit(WorkspaceEvent::Close);
    }

    pub fn set_active_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        self.app_state.active_workspace_id = Some(workspace_id);
        self.app_state.current_view = models::ApplicationView::Workspace(workspace_id);
        let workspace = self
            .app_state
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .unwrap();
        cx.emit(WorkspaceEvent::Open(workspace.clone()));
    }

    pub fn go_to_home(&mut self, cx: &mut Context<Self>) {
        self.app_state.current_view = models::ApplicationView::Home;
        self.app_state.active_workspace_id = None;
        cx.emit(WorkspaceEvent::Close);
    }

    pub fn set_workspace_view(
        &mut self,
        workspace_id: Uuid,
        view: models::WorkspaceView,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self
            .app_state
            .workspaces
            .iter_mut()
            .find(|w| w.id == workspace_id)
        {
            workspace.current_view = view;
            cx.emit(WorkspaceEvent::Open(workspace.clone()));
        }
    }

    pub fn remove_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        let store = WorkspaceStore::new(self.db.conn());
        store.delete(workspace_id).ok();

        self.app_state.workspaces.retain(|w| w.id != workspace_id);
        self.app_state
            .opened_workspaces
            .retain(|&id| id != workspace_id);

        if self.app_state.active_workspace_id == Some(workspace_id) {
            if let Some(first) = self.app_state.workspaces.first() {
                self.set_active_workspace(first.id, cx);
            } else {
                self.go_to_home(cx);
            }
        } else {
            cx.emit(WorkspaceEvent::Close);
        }
    }

    // === Content Loading ===

    fn load_workspace_content(&mut self, workspace_id: Uuid) {
        if let Some(workspace) = self
            .app_state
            .workspaces
            .iter_mut()
            .find(|w| w.id == workspace_id)
        {
            if workspace.services.is_empty() && workspace.folders.is_empty() {
                let folder_store = FolderStore::new(self.db.conn());
                let service_store = ServiceStore::new(self.db.conn());
                let schema_store = SchemaStore::new(self.db.conn());
                let env_store = EnvironmentStore::new(self.db.conn());
                let var_store = EnvironmentVariableStore::new(self.db.conn());

                workspace.folders = folder_store
                    .list_by_workspace(workspace_id)
                    .unwrap_or_default();
                workspace.services = service_store
                    .list_by_workspace(workspace_id)
                    .unwrap_or_default();
                workspace.schemas = schema_store
                    .list_by_workspace(workspace_id)
                    .unwrap_or_default();
                workspace.environments = env_store
                    .list_by_workspace(workspace_id)
                    .unwrap_or_default();

                // Load variables for each environment
                for env in &mut workspace.environments {
                    env.variables = var_store.list_by_environment(env.id).unwrap_or_default();
                }

                // Set default environment as active
                if workspace.active_environment_id.is_none() {
                    workspace.active_environment_id = workspace
                        .environments
                        .iter()
                        .find(|e| e.is_default)
                        .or(workspace.environments.first())
                        .map(|e| e.id);
                }
            }
        }
    }

    fn create_sample_services(&mut self, workspace_id: Uuid) {
        let samples = vec![
            (
                "Get Users",
                HttpMethod::Get,
                "https://api.example.com/users",
            ),
            (
                "Create User",
                HttpMethod::Post,
                "https://api.example.com/users",
            ),
            ("httpbin json", HttpMethod::Get, "https://httpbin.org/json"),
            (
                "httpbin stream",
                HttpMethod::Get,
                "https://httpbin.org/stream/5",
            ),
        ];

        let service_store = ServiceStore::new(self.db.conn());

        for (name, method, path) in samples {
            let service = Service::new(
                workspace_id,
                name,
                ServiceType::Http(HttpConfig {
                    method,
                    path: path.into(),
                    ..Default::default()
                }),
            );
            if let Err(err) = service_store.insert(&service) {
                error!("Failed to create service: {}", err);
            }
        }
    }

    // === Service Operations ===

    pub fn create_service(
        &mut self,
        name: SharedString,
        folder_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return,
        };

        let mut service = Service::new(
            workspace_id,
            &name,
            ServiceType::Http(HttpConfig::default()),
        );
        service.folder_id = folder_id;

        let service_store = ServiceStore::new(self.db.conn());
        service_store.insert(&service).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.services.push(service);
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn create_schema(
        &mut self,
        name: SharedString,
        folder_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return,
        };

        let mut schema = models::Schema::new(workspace_id, &name);
        schema.folder_id = folder_id;

        let schema_store = SchemaStore::new(self.db.conn());
        schema_store.insert(&schema).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.schemas.push(schema);
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn create_folder(
        &mut self,
        name: SharedString,
        parent_id: Option<Uuid>,
        folder_type: models::FolderType,
        cx: &mut Context<Self>,
    ) {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return,
        };

        let folder = models::Folder::with_type(workspace_id, &name, parent_id, folder_type);

        let folder_store = FolderStore::new(self.db.conn());
        match folder_store.insert(&folder) {
            Ok(_) => {
                println!("debug");
            }
            Err(err) => {
                error!("{err}");
            }
        }

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.folders.push(folder);
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn open_content(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            if !workspace.opened_services.contains(&ct) {
                workspace.opened_services.push(ct.clone());
            }
            workspace.active_service = Some(ct.clone());
            cx.emit(WorkspaceServiceEvent::Open(workspace.clone(), ct.clone()));
        }
    }

    pub fn close_content(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.opened_services.retain(|id| id.ne(&ct));
            if workspace.active_service == Some(ct.clone()) {
                workspace.active_service = workspace.opened_services.first().cloned();
            }
            cx.emit(WorkspaceServiceEvent::Switch {
                workspace: workspace.clone(),
                old: ct.clone(),
                new: workspace.active_service.clone(),
            });
        }
    }

    pub fn set_active_content(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.active_service = Some(ct.clone());
            cx.emit(WorkspaceServiceEvent::Open(workspace.clone(), ct.clone()));
        }
    }

    pub fn get_service(&self, service_id: Uuid) -> Option<&Service> {
        self.get_active_workspace()
            .and_then(|w| w.services.iter().find(|s| s.id == service_id))
    }

    pub fn get_folder(&self, folder_id: Uuid) -> Option<&Folder> {
        self.get_active_workspace()
            .and_then(|w| w.folders.iter().find(|s| s.id == folder_id))
    }

    // === OpenAPI Import ===

    pub fn import_openapi(&mut self, content: &str, cx: &mut Context<Self>) -> anyhow::Result<()> {
        self.import_openapi_with_options(content, openapi::ImportOptions::default(), cx)
    }

    pub fn import_openapi_with_options(
        &mut self,
        content: &str,
        options: openapi::ImportOptions,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return Err(anyhow::anyhow!("No active workspace")),
        };

        let workspace = self
            .get_state()
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .unwrap();

        let spec = openapi::parse_openapi(content)?;
        let result =
            openapi::import_openapi_with_options(workspace_id, &spec, options, &workspace.folders)?;

        log::info!(
            "Importing {} folders, {} services, {} schemas",
            result.folders.len(),
            result.services.len(),
            result.schemas.len()
        );

        let folder_store = FolderStore::new(self.db.conn());
        let service_store = ServiceStore::new(self.db.conn());
        let schema_store = SchemaStore::new(self.db.conn());

        for folder in &result.folders {
            if let Err(e) = folder_store.insert(folder) {
                error!("Failed to insert folder {}: {}", folder.name, e);
            }
        }
        for service in &result.services {
            if let Err(e) = service_store.insert(service) {
                error!("Failed to insert service {}: {}", service.name, e);
            }
        }
        for schema in &result.schemas {
            if let Err(e) = schema_store.insert(schema) {
                error!("Failed to insert schema {}: {}", schema.name, e);
            }
        }

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.folders.extend(result.folders);
            workspace.services.extend(result.services);
            workspace.schemas.extend(result.schemas);
        }

        cx.emit(WorkspaceEvent::Reload);
        Ok(())
    }

    pub fn generate_openapi_import_preview(
        &self,
        content: &str,
        options: openapi::ImportOptions,
    ) -> anyhow::Result<openapi::ImportPreview> {
        let workspace = self
            .get_active_workspace()
            .ok_or_else(|| anyhow::anyhow!("No active workspace"))?;

        let spec = openapi::parse_openapi(content)?;
        openapi::generate_import_preview(&spec, &options, &workspace.folders)
    }

    // === Environment Operations ===

    pub fn create_environment(&mut self, name: String, cx: &mut Context<Self>) {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return,
        };

        let env = Environment::new(workspace_id, &name);
        let env_store = EnvironmentStore::new(self.db.conn());
        env_store.insert(&env).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            let is_first = workspace.environments.is_empty();
            workspace.environments.push(env.clone());
            if is_first {
                workspace.active_environment_id = Some(env.id);
            }
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn update_environment(&mut self, env: Environment, cx: &mut Context<Self>) {
        let env_store = EnvironmentStore::new(self.db.conn());
        env_store.update(&env).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            if let Some(existing) = workspace.environments.iter_mut().find(|e| e.id == env.id) {
                existing.name = env.name;
                existing.base_url = env.base_url;
                existing.is_default = env.is_default;
            }
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn delete_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        let env_store = EnvironmentStore::new(self.db.conn());
        env_store.delete(env_id).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.environments.retain(|e| e.id != env_id);
            if workspace.active_environment_id == Some(env_id) {
                workspace.active_environment_id = workspace.environments.first().map(|e| e.id);
            }
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn set_active_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.active_environment_id = Some(env_id);
            cx.emit(WorkspaceEvent::Reload);
        }
    }

    pub fn get_active_environment(&self) -> Option<&Environment> {
        self.get_active_workspace().and_then(|w| {
            w.active_environment_id
                .and_then(|id| w.environments.iter().find(|e| e.id == id))
        })
    }

    // === Environment Variable Operations ===

    pub fn add_environment_variable(
        &mut self,
        env_id: Uuid,
        var: EnvironmentVariable,
        cx: &mut Context<Self>,
    ) {
        let var_store = EnvironmentVariableStore::new(self.db.conn());
        var_store.insert(&var).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            if let Some(env) = workspace.environments.iter_mut().find(|e| e.id == env_id) {
                env.variables.push(var);
            }
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn update_environment_variable(
        &mut self,
        var: EnvironmentVariable,
        cx: &mut Context<Self>,
    ) {
        let var_store = EnvironmentVariableStore::new(self.db.conn());
        var_store.update(&var).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            for env in &mut workspace.environments {
                if let Some(existing) = env.variables.iter_mut().find(|v| v.id == var.id) {
                    *existing = var.clone();
                    break;
                }
            }
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn delete_environment_variable(&mut self, var_id: Uuid, cx: &mut Context<Self>) {
        let var_store = EnvironmentVariableStore::new(self.db.conn());
        var_store.delete(var_id).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            for env in &mut workspace.environments {
                env.variables.retain(|v| v.id != var_id);
            }
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    // === Folder Operations ===

    pub fn update_folder_common_headers(
        &mut self,
        folder_id: Uuid,
        headers: Vec<models::Header>,
        cx: &mut Context<Self>,
    ) {
        let folder_store = FolderStore::new(self.db.conn());
        if let Err(e) = folder_store.update_common_headers(folder_id, headers.clone()) {
            error!("Failed to update folder common headers: {}", e);
            return;
        }

        // Update the in-memory workspace data
        if let Some(workspace) = self.get_active_workspace_mut() {
            if let Some(folder) = workspace.folders.iter_mut().find(|f| f.id == folder_id) {
                folder.common_headers = headers;
            }
        }
        cx.emit(WorkspaceEvent::Reload);
    }

    pub fn update_folder_search_params(
        &mut self,
        folder_id: Uuid,
        params: Vec<models::SearchParameter>,
        cx: &mut Context<Self>,
    ) {
        let folder_store = FolderStore::new(self.db.conn());
        if let Err(e) = folder_store.update_search_params(folder_id, params.clone()) {
            error!("Failed to update folder search parameters: {}", e);
            return;
        }

        // Update the in-memory workspace data
        if let Some(workspace) = self.get_active_workspace_mut() {
            if let Some(folder) = workspace.folders.iter_mut().find(|f| f.id == folder_id) {
                folder.common_search_params = params;
            }
        }
        cx.emit(WorkspaceEvent::Reload);
    }
}