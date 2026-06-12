use crate::containers::{FolderDetailPage, HttpService};
use crate::state::{build_tree, ContentViewHandle, PlaceholderView};
use gpui::*;
use gpui_component::tree::TreeState;
use gpui_component::IconName;
use log::error;
use models::{
    Environment, EnvironmentVariable, Folder, FolderType, Header, Schema, SearchParameter, Service,
    ServiceType,
};
use register::GlobalStorage;
use uuid::Uuid;

/// A cached open content view (mirrors `ApplicationState::Tab`).
pub struct OpenView {
    pub view: Box<dyn ContentViewHandle>,
}

pub struct ServiceState {
    pub workspace_id: Uuid,
    pub services: Vec<Service>,
    pub folders: Vec<Folder>,
    pub schemas: Vec<Schema>,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<Uuid>,

    /// Sidebar tree UI state (items, selection, expansion).
    tree: Entity<TreeState>,

    /// Open content view cache and tab list (mirrors `ApplicationState.tabs`).
    pub open_views: Vec<OpenView>,
    /// Index of the active entry in `open_views` (mirrors `ApplicationState.active_idx`).
    pub active_view_ix: Option<usize>,
}

impl ServiceState {
    pub fn new(workspace_id: Uuid, cx: &mut Context<Self>) -> Self {
        let storage = GlobalStorage::global(cx);

        let folders = storage
            .folder
            .list_by_workspace(workspace_id)
            .unwrap_or_default();
        let services = storage
            .service
            .list_by_workspace(workspace_id)
            .unwrap_or_default();
        let schemas = storage
            .schema
            .list_by_workspace(workspace_id)
            .unwrap_or_default();
        let mut environments = storage
            .env
            .list_by_workspace(workspace_id)
            .unwrap_or_default();

        for env in &mut environments {
            env.variables = storage.var.list_by_environment(env.id).unwrap_or_default();
        }

        let active_environment_id = environments
            .iter()
            .find(|e| e.is_default)
            .or(environments.first())
            .map(|e| e.id);

        let tree = cx.new(|cx| TreeState::new(cx));
        let items = build_tree(&folders, &services, &schemas);
        tree.update(cx, |state, cx| {
            state.set_items(items, cx);
        });

        Self {
            workspace_id,
            services,
            folders,
            schemas,
            environments,
            active_environment_id,
            tree,
            open_views: vec![],
            active_view_ix: None,
        }
    }

    /// Accessor for the sidebar tree state.
    pub fn tree(&self) -> &Entity<TreeState> {
        &self.tree
    }

    /// Rebuilds the sidebar tree from the current folders/services/schemas.
    fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        let items = build_tree(&self.folders, &self.services, &self.schemas);
        self.tree.update(cx, |state, cx| {
            state.set_items(items, cx);
        });
    }

    // === Content Operations ===

    /// The currently active opened content, if any.
    pub fn active_content(&self) -> Option<&OpenView> {
        self.active_view_ix.and_then(|ix| self.open_views.get(ix))
    }

    /// All opened contents (tab list).
    pub fn open_views(&self) -> &[OpenView] {
        &self.open_views
    }

    pub fn close_content(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.remove_view(ix);
        cx.notify();
    }

    pub fn set_active_content(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.active_view_ix = Some(ix);
        cx.notify();
    }

    // === Service CRUD ===

    pub fn get_service(&self, service_id: Uuid) -> Option<&Service> {
        self.services.iter().find(|s| s.id == service_id)
    }

    pub fn get_folder(&self, folder_id: Uuid) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == folder_id)
    }

    pub fn create_service(
        &mut self,
        name: SharedString,
        folder_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        let mut service = Service::new(
            self.workspace_id,
            &name,
            ServiceType::Http(models::HttpConfig::default()),
        );
        service.folder_id = folder_id;

        let storage = GlobalStorage::global(cx);
        storage.service.insert(&service).ok();

        self.services.push(service);
        self.rebuild_tree(cx);
        cx.notify();
    }

    pub fn rename_service(
        &mut self,
        service_id: Uuid,
        new_name: SharedString,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);

        if let Some(service) = self.services.iter().find(|s| s.id == service_id).cloned() {
            let mut updated_service = service.clone();
            updated_service.name = new_name.clone();

            match storage.service.update(&updated_service) {
                Ok(_) => {
                    if let Some(s) = self.services.iter_mut().find(|s| s.id == service_id) {
                        s.name = new_name.clone();
                        s.updated_at = chrono::Utc::now().timestamp();
                    }

                    if let Some(ix) = self
                        .open_views
                        .iter()
                        .position(|e| e.view.id(cx) == service_id)
                    {
                        self.open_views[ix].view.set_title(new_name.clone(), cx);
                    }

                    self.rebuild_tree(cx);
                    cx.notify();
                }
                Err(e) => error!("Failed to rename service {}: {}", service_id, e),
            }
        }
    }

    pub fn delete_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        if let Err(e) = storage.service.delete(service_id) {
            error!("Failed to delete service {}: {}", service_id, e);
            return;
        }

        self.services.retain(|s| s.id != service_id);

        // Close any opened tab referencing the deleted service.
        if let Some(ix) = self
            .open_views
            .iter()
            .position(|v| v.view.id(cx) == service_id)
        {
            self.remove_view(ix);
        }

        self.rebuild_tree(cx);
        cx.notify();
    }

    pub fn create_schema(
        &mut self,
        name: SharedString,
        folder_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        let mut schema = Schema::new(self.workspace_id, &name);
        schema.folder_id = folder_id;

        let storage = GlobalStorage::global(cx);
        storage.schema.insert(&schema).ok();

        self.schemas.push(schema);
        self.rebuild_tree(cx);
        cx.notify();
    }

    pub fn create_folder(
        &mut self,
        name: SharedString,
        parent_id: Option<Uuid>,
        folder_type: FolderType,
        cx: &mut Context<Self>,
    ) {
        let folder = Folder::with_type(self.workspace_id, &name, parent_id, folder_type);

        let storage = GlobalStorage::global(cx);
        if let Err(e) = storage.folder.insert(&folder) {
            error!("Failed to create folder: {}", e);
        }

        self.folders.push(folder);
        self.rebuild_tree(cx);
        cx.notify();
    }

    pub fn rename_folder(
        &mut self,
        folder_id: Uuid,
        new_name: SharedString,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);

        match storage
            .folder
            .update(folder_id, Some(new_name.as_ref()), None, None)
        {
            Ok(updated_folder) => {
                if let Some(folder) = self.folders.iter_mut().find(|f| f.id == folder_id) {
                    folder.name = updated_folder.name.clone();
                    folder.updated_at = updated_folder.updated_at;
                }
                self.refresh_open_folder_view(folder_id, cx);
                self.rebuild_tree(cx);
                cx.notify();
            }
            Err(e) => error!("Failed to rename folder {}: {}", folder_id, e),
        }
    }

    /// Re-syncs the open view (tab title) for a folder after its data changed.
    fn refresh_open_folder_view(&mut self, folder_id: Uuid, cx: &mut Context<Self>) {
        let Some(name) = self
            .folders
            .iter()
            .find(|f| f.id == folder_id)
            .map(|f| f.name.clone())
        else {
            return;
        };
        if let Some(ix) = self
            .open_views
            .iter()
            .position(|e| e.view.id(cx) == folder_id)
        {
            self.open_views[ix].view.set_title(name, cx);
        }
    }

    // === Folder Detail Operations ===

    pub fn update_folder_common_headers(
        &mut self,
        folder_id: Uuid,
        headers: Vec<Header>,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);
        if let Err(e) = storage
            .folder
            .update_common_headers(folder_id, headers.clone())
        {
            error!("Failed to update folder common headers: {}", e);
            return;
        }

        if let Some(folder) = self.folders.iter_mut().find(|f| f.id == folder_id) {
            folder.common_headers = headers;
        }
        cx.notify();
    }

    pub fn update_folder_search_params(
        &mut self,
        folder_id: Uuid,
        params: Vec<SearchParameter>,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);
        if let Err(e) = storage
            .folder
            .update_search_params(folder_id, params.clone())
        {
            error!("Failed to update folder search parameters: {}", e);
            return;
        }

        if let Some(folder) = self.folders.iter_mut().find(|f| f.id == folder_id) {
            folder.common_search_params = params;
        }
        cx.notify();
    }

    pub fn update_folder_info(
        &mut self,
        folder_id: Uuid,
        name: String,
        description: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);

        match storage
            .folder
            .update(folder_id, Some(&name), Some(description.as_deref()), None)
        {
            Ok(updated_folder) => {
                if let Some(folder) = self.folders.iter_mut().find(|f| f.id == folder_id) {
                    folder.name = updated_folder.name;
                    folder.description = updated_folder.description;
                    folder.updated_at = updated_folder.updated_at;
                }
                self.refresh_open_folder_view(folder_id, cx);
                self.rebuild_tree(cx);
                cx.notify();
            }
            Err(e) => error!("Failed to update folder info: {}", e),
        }
    }

    // === Environment Operations ===

    pub fn create_environment(&mut self, name: String, cx: &mut Context<Self>) {
        let env = Environment::new(self.workspace_id, &name);

        let storage = GlobalStorage::global(cx);
        storage.env.insert(&env).ok();

        let is_first = self.environments.is_empty();
        self.environments.push(env.clone());
        if is_first {
            self.active_environment_id = Some(env.id);
        }
        cx.notify();
    }

    pub fn update_environment(&mut self, env: Environment, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        storage.env.update(&env).ok();

        if let Some(existing) = self.environments.iter_mut().find(|e| e.id == env.id) {
            existing.name = env.name;
            existing.base_url = env.base_url;
            existing.is_default = env.is_default;
        }
        cx.notify();
    }

    pub fn delete_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        storage.env.delete(env_id).ok();

        self.environments.retain(|e| e.id != env_id);
        if self.active_environment_id == Some(env_id) {
            self.active_environment_id = self.environments.first().map(|e| e.id);
        }
        cx.notify();
    }

    pub fn set_active_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        self.active_environment_id = Some(env_id);
        cx.notify();
    }

    pub fn get_active_environment(&self) -> Option<&Environment> {
        self.active_environment_id
            .and_then(|id| self.environments.iter().find(|e| e.id == id))
    }

    // === Environment Variable Operations ===

    pub fn add_environment_variable(
        &mut self,
        env_id: Uuid,
        var: EnvironmentVariable,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);
        storage.var.insert(&var).ok();

        if let Some(env) = self.environments.iter_mut().find(|e| e.id == env_id) {
            env.variables.push(var);
        }
        cx.notify();
    }

    pub fn update_environment_variable(
        &mut self,
        var: EnvironmentVariable,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);
        storage.var.update(&var).ok();

        for env in &mut self.environments {
            if let Some(existing) = env.variables.iter_mut().find(|v| v.id == var.id) {
                *existing = var.clone();
                break;
            }
        }
        cx.notify();
    }

    pub fn delete_environment_variable(&mut self, var_id: Uuid, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        storage.var.delete(var_id).ok();

        for env in &mut self.environments {
            env.variables.retain(|v| v.id != var_id);
        }
        cx.notify();
    }

    // === Open helpers ===

    pub fn open_service(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.get_service(id).cloned() else {
            return;
        };
        self.open_view(id, window, cx, move |window, cx| {
            match &service.service_type {
                ServiceType::Http(_) => {
                    Box::new(cx.new(|cx| HttpService::new(service, window, cx)))
                }
                ServiceType::Grpc(_) => placeholder(
                    cx,
                    service.name.clone(),
                    "Grpc Page Coming soon...",
                    IconName::Globe,
                ),
                ServiceType::WebSocket(_) => placeholder(
                    cx,
                    service.name.clone(),
                    "WebSocket Page Coming soon...",
                    IconName::Globe,
                ),
            }
        });
    }

    pub fn open_folder_detail(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.get_folder(id).cloned() else {
            return;
        };
        self.open_view(id, window, cx, move |_window, cx| {
            let service_state = cx.entity();
            Box::new(cx.new(|cx| FolderDetailPage::new(service_state, folder, cx)))
        });
    }

    // === Open content view cache (mirrors ApplicationState.tabs) ===

    /// The view rendered for the currently active content (mirrors `tabs[active_idx].view`).
    pub fn active_view(&self) -> Option<AnyView> {
        self.active_view_ix
            .and_then(|ix| self.open_views.get(ix))
            .map(|entry| entry.view.view())
    }

    /// Looks up a cached view handle by content id.
    fn view_by_id(&self, id: Uuid, cx: &App) -> Option<&dyn ContentViewHandle> {
        self.open_views
            .iter()
            .find(|e| e.view.id(cx) == id)
            .map(|e| e.view.as_ref())
    }

    /// Ensures a cached view for `id` exists (building it via `factory` if absent)
    /// and makes it active. Mirrors `ApplicationState::open_workspace`.
    fn open_view(
        &mut self,
        id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
        factory: impl FnOnce(&mut Window, &mut Context<Self>) -> Box<dyn ContentViewHandle>,
    ) {
        if let Some(pos) = self.open_views.iter().position(|e| e.view.id(cx) == id) {
            self.active_view_ix = Some(pos);
        } else {
            let view = factory(window, cx);
            self.open_views.push(OpenView { view });
            self.active_view_ix = Some(self.open_views.len() - 1);
        }
        cx.notify();
    }

    /// Removes a cached view, fixing up the active index (mirrors `close_tab`).
    fn remove_view(&mut self, pos: usize) {
        self.open_views.remove(pos);
        match self.active_view_ix {
            Some(ix) if ix == pos => {
                self.active_view_ix = if self.open_views.is_empty() {
                    None
                } else {
                    Some(pos.min(self.open_views.len() - 1))
                };
            }
            Some(ix) if ix > pos => self.active_view_ix = Some(ix - 1),
            other => self.active_view_ix = other,
        }
    }

    /// Checks whether the cached view for a content id has unsaved changes.
    pub fn view_is_modified(&self, id: Uuid, cx: &App) -> bool {
        self.view_by_id(id, cx)
            .map(|view| view.is_modified(cx))
            .unwrap_or(false)
    }

    /// Saves changes for the cached view of a content id.
    pub fn save_view(&mut self, id: Uuid, cx: &mut App) {
        if let Some(ix) = self.open_views.iter().position(|e| e.view.id(cx) == id) {
            self.open_views[ix].view.save(cx);
        }
    }
}

/// Creates a boxed placeholder content view handle.
fn placeholder(
    cx: &mut Context<ServiceState>,
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    icon: IconName,
) -> Box<dyn ContentViewHandle> {
    Box::new(cx.new(|_| PlaceholderView::new(title, body, icon)))
}
