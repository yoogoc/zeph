use gpui::{App, AppContext, Context, Entity, EventEmitter, Global, SharedString};
use models::{HttpConfig, HttpMethod, Service, ServiceType, Workspace};
use storage::{Database, FolderStore, ServiceStore, WorkspaceStore};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct WorkspaceEvent;

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

        if app_state.workspaces.is_empty() {
            let workspace = Workspace::new("My Workspace");
            store.insert(&workspace).ok();
            app_state.workspaces.push(workspace);
        }

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

    pub fn create_workspace(&mut self, name: SharedString, cx: &mut Context<Self>) {
        let workspace = Workspace::new(name);
        let store = WorkspaceStore::new(self.db.conn());
        store.insert(&workspace).ok();
        self.app_state.workspaces.push(workspace);
        cx.emit(WorkspaceEvent);
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
        cx.emit(WorkspaceEvent);
    }

    pub fn set_active_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        self.app_state.active_workspace_id = Some(workspace_id);
        self.app_state.current_view = models::ApplicationView::Workspace(workspace_id);
        cx.emit(WorkspaceEvent);
    }

    pub fn go_to_home(&mut self, cx: &mut Context<Self>) {
        self.app_state.current_view = models::ApplicationView::Home;
        self.app_state.active_workspace_id = None;
        cx.emit(WorkspaceEvent);
    }

    pub fn set_workspace_view(&mut self, workspace_id: Uuid, view: models::WorkspaceView, cx: &mut Context<Self>) {
        if let Some(workspace) = self.app_state.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            workspace.current_view = view;
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn remove_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if self.app_state.workspaces.len() <= 1 {
            return;
        }

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
            cx.emit(WorkspaceEvent);
        }
    }

    // === Content Loading ===

    fn load_workspace_content(&mut self, workspace_id: Uuid) {
        if let Some(workspace) = self.app_state.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            if workspace.services.is_empty() && workspace.folders.is_empty() {
                let folder_store = FolderStore::new(self.db.conn());
                let service_store = ServiceStore::new(self.db.conn());

                workspace.folders = folder_store.list_by_workspace(workspace_id).unwrap_or_default();
                workspace.services = service_store.list_by_workspace(workspace_id).unwrap_or_default();

                // Create sample services if empty
                if workspace.services.is_empty() {
                    self.create_sample_services(workspace_id);
                }
            }
        }
    }

    fn create_sample_services(&mut self, workspace_id: Uuid) {
        let samples = vec![
            ("Get Users", HttpMethod::Get, "https://api.example.com/users"),
            ("Create User", HttpMethod::Post, "https://api.example.com/users"),
            ("httpbin json", HttpMethod::Get, "https://httpbin.org/json"),
            ("httpbin stream", HttpMethod::Get, "https://httpbin.org/stream/5"),
        ];

        let service_store = ServiceStore::new(self.db.conn());

        if let Some(workspace) = self.app_state.workspaces.iter_mut().find(|w| w.id == workspace_id) {
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
                service_store.insert(&service).ok();
                workspace.services.push(service);
            }
        }
    }

    // === Service Operations ===

    pub fn open_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            if !workspace.opened_services.contains(&service_id) {
                workspace.opened_services.push(service_id);
            }
            workspace.active_service = Some(service_id);
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn close_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.opened_services.retain(|&id| id != service_id);
            if workspace.active_service == Some(service_id) {
                workspace.active_service = workspace.opened_services.first().copied();
            }
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn set_active_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.active_service = Some(service_id);
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn get_service(&self, service_id: Uuid) -> Option<&Service> {
        self.get_active_workspace()
            .and_then(|w| w.services.iter().find(|s| s.id == service_id))
    }
}
