use gpui::{App, AppContext, Context, Entity, EventEmitter, Global, SharedString};
use models::{ApplicationState, Workspace};
use storage::{Database, WorkspaceStore};
use uuid::Uuid;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct WorkspaceEvent;

pub fn init(cx: &mut App) {
    WorkspaceRegistry::set_global(cx.new(WorkspaceRegistry::new), cx);
}

struct GlobalWorkspaceRegistry(Entity<WorkspaceRegistry>);
impl Global for GlobalWorkspaceRegistry {}

pub struct WorkspaceRegistry {
    db: Arc<Database>,
    app_state: ApplicationState,
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

        let mut app_state = ApplicationState::default();
        
        // Load workspaces from database
        let store = WorkspaceStore::new(db.conn());
        app_state.workspaces = store.list().unwrap_or_default();

        // Create default workspace if none exist
        if app_state.workspaces.is_empty() {
            let workspace = Workspace::new("My Workspace");
            store.insert(&workspace).ok();
            app_state.workspaces.push(workspace);
        }

        // Set the first workspace as active
        if let Some(first) = app_state.workspaces.first() {
            app_state.active_workspace_id = Some(first.id);
            app_state.current_view = models::ApplicationView::Workspace(first.id);
        }
        
        Self { db, app_state }
    }
    
    pub fn get_state(&self) -> &ApplicationState {
        &self.app_state
    }
    
    pub fn get_active_workspace(&self) -> Option<&Workspace> {
        self.app_state.active_workspace_id
            .and_then(|id| self.app_state.workspaces.iter().find(|w| w.id == id))
    }
    
    pub fn create_workspace(&mut self, name: SharedString, cx: &mut Context<Self>) {
        let workspace = Workspace::new(name);
        
        let store = WorkspaceStore::new(self.db.conn());
        store.insert(&workspace).ok();
        
        self.app_state.workspaces.push(workspace);
        cx.emit(WorkspaceEvent);
    }
    
    pub fn set_active_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        self.app_state.active_workspace_id = Some(workspace_id);
        self.app_state.current_view = models::ApplicationView::Workspace(workspace_id);
        cx.emit(WorkspaceEvent);
    }
    
    pub fn set_workspace_view(&mut self, workspace_id: Uuid, view: models::WorkspaceView, cx: &mut Context<Self>) {
        if let Some(workspace) = self.app_state.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            workspace.current_view = view;
            cx.emit(WorkspaceEvent);
        }
    }
    
    pub fn go_to_home(&mut self, cx: &mut Context<Self>) {
        self.app_state.current_view = models::ApplicationView::Home;
        self.app_state.active_workspace_id = None;
        cx.emit(WorkspaceEvent);
    }
    
    pub fn remove_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if self.app_state.workspaces.len() <= 1 {
            return;
        }

        let store = WorkspaceStore::new(self.db.conn());
        store.delete(workspace_id).ok();
        
        self.app_state.workspaces.retain(|w| w.id != workspace_id);
        
        if self.app_state.active_workspace_id == Some(workspace_id) {
            if let Some(first_workspace) = self.app_state.workspaces.first() {
                self.set_active_workspace(first_workspace.id, cx);
            } else {
                self.go_to_home(cx);
            }
        } else {
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn update_workspace(&mut self, workspace_id: Uuid, name: SharedString, cx: &mut Context<Self>) {
        if let Some(workspace) = self.app_state.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            workspace.name = name;
            workspace.updated_at = chrono::Utc::now().timestamp();
            
            let store = WorkspaceStore::new(self.db.conn());
            store.update(workspace).ok();
            
            cx.emit(WorkspaceEvent);
        }
    }
}
