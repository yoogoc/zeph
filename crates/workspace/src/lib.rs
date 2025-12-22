use gpui::{App, AppContext, Context, Entity, EventEmitter, Global, SharedString};
use models::{ApplicationState, Workspace};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct WorkspaceEvent;

pub fn init(cx: &mut App) {
    WorkspaceRegistry::set_global(cx.new(WorkspaceRegistry::new), cx);
}

struct GlobalWorkspaceRegistry(Entity<WorkspaceRegistry>);
impl Global for GlobalWorkspaceRegistry {}

pub struct WorkspaceRegistry {
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
    
    fn new(_cx: &mut Context<Self>) -> Self {
        let mut app_state = ApplicationState::default();
        
        // Create a default workspace
        let mut default_workspace = Workspace::new("My Workspace".into());
        default_workspace.current_view = models::WorkspaceView::Collections; // Default to Collections
        app_state.active_workspace_id = Some(default_workspace.id);
        app_state.current_view = models::ApplicationView::Workspace(default_workspace.id);
        app_state.workspaces.push(default_workspace);
        
        Self { app_state }
    }
    
    pub fn get_state(&self) -> &ApplicationState {
        &self.app_state
    }
    
    pub fn get_active_workspace(&self) -> Option<&Workspace> {
        self.app_state.active_workspace_id
            .and_then(|id| self.app_state.workspaces.iter().find(|w| w.id == id))
    }
    
    pub fn create_workspace(&mut self, name: SharedString, cx: &mut Context<Self>) {
        let mut workspace = Workspace::new(name);
        workspace.current_view = models::WorkspaceView::Collections; // Default to Collections
        self.app_state.workspaces.push(workspace);
        cx.emit(WorkspaceEvent);
    }
    
    pub fn set_active_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        self.app_state.active_workspace_id = Some(workspace_id);
        self.app_state.current_view = models::ApplicationView::Workspace(workspace_id);
        cx.emit(WorkspaceEvent);
    }
    
    pub fn set_workspace_view(&mut self, workspace_id: uuid::Uuid, view: models::WorkspaceView, cx: &mut Context<Self>) {
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
        // Don't allow removing the last workspace
        if self.app_state.workspaces.len() <= 1 {
            return;
        }
        
        // Remove the workspace
        self.app_state.workspaces.retain(|w| w.id != workspace_id);
        
        // If the removed workspace was active, switch to another one or home
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
}
