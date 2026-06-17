use crate::containers::grpc_service::GrpcService;
use crate::containers::{FolderDetailPage, HttpService, SchemaDetailPage};
use crate::state::{build_tree, ContentViewHandle, EnvironmentState, PlaceholderView};
use gpui::*;
use gpui_component::tree::TreeState;
use gpui_component::IconName;
use log::error;
use models::{
    Folder, FolderType, Header, HttpConfig, Schema, SearchParameter, Service, ServiceType,
};
use register::GlobalStorage;
use std::collections::HashSet;
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

    /// Sidebar tree UI state (items, selection, expansion).
    tree: Entity<TreeState>,

    /// Open content view cache and tab list (mirrors `ApplicationState.tabs`).
    pub open_views: Vec<OpenView>,
    /// Index of the active entry in `open_views` (mirrors `ApplicationState.active_idx`).
    pub active_view_ix: Option<usize>,

    /// Environment state for this workspace (provides active env base_url).
    pub env_state: Entity<EnvironmentState>,

    /// Ids of services that exist only as open draft tabs (untitled requests):
    /// present in `services` (so lookups work) and open as a tab, but not yet
    /// persisted to the DB and hidden from the sidebar tree until first save.
    unsaved: HashSet<Uuid>,
}

impl ServiceState {
    pub fn new(
        workspace_id: Uuid,
        env_state: Entity<EnvironmentState>,
        cx: &mut Context<Self>,
    ) -> Self {
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
            tree,
            open_views: vec![],
            active_view_ix: None,
            env_state,
            unsaved: HashSet::new(),
        }
    }

    /// Whether `service_id` is an unsaved draft (untitled request not yet persisted).
    pub fn is_unsaved(&self, service_id: Uuid) -> bool {
        self.unsaved.contains(&service_id)
    }

    /// Creates a new untitled request of the given type as a draft tab: it is
    /// added to `services` (so lookups/draft work) and opened as a tab, but is
    /// NOT written to the DB and does NOT appear in the sidebar tree until first
    /// saved.
    pub fn create_untitled_request(
        &mut self,
        service_type: ServiceType,
        folder_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut service = Service::new(self.workspace_id, "Untitled", service_type);
        service.folder_id = folder_id;
        let id = service.id;

        self.services.push(service);
        self.unsaved.insert(id);
        // No DB write and no tree rebuild: the draft stays out of the tree.
        self.open_service(id, window, cx);
    }

    /// First-save of an untitled draft: writes the config into the in-memory
    /// service, inserts it into the DB, promotes it out of the unsaved set, and
    /// rebuilds the tree so it now appears as a normal service.
    pub fn commit_unsaved_service(
        &mut self,
        service_id: Uuid,
        new_config: HttpConfig,
        cx: &mut Context<Self>,
    ) {
        self.commit_unsaved_service_typed(service_id, ServiceType::Http(new_config), cx);
    }

    /// First-save of an untitled draft with an explicit service type (used by
    /// both HTTP and gRPC). See [`commit_unsaved_service`].
    pub fn commit_unsaved_service_typed(
        &mut self,
        service_id: Uuid,
        service_type: ServiceType,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.services.iter_mut().find(|s| s.id == service_id) else {
            error!("Unsaved service {} not found", service_id);
            return;
        };
        service.service_type = service_type;
        service.updated_at = chrono::Utc::now().timestamp();
        let service = service.clone();

        let storage = GlobalStorage::global(cx);
        if let Err(e) = storage.service.insert(&service) {
            error!("Failed to persist untitled service {}: {}", service_id, e);
            return;
        }

        self.unsaved.remove(&service_id);
        self.rebuild_tree(cx);
        cx.notify();
    }

    /// Writes a new HttpConfig to a service (DB + in-memory).
    pub fn update_service_config(
        &mut self,
        service_id: Uuid,
        new_config: HttpConfig,
        cx: &mut Context<Self>,
    ) {
        self.update_service_config_typed(service_id, ServiceType::Http(new_config), cx);
    }

    /// Writes an explicit service type to a service (DB + in-memory), used by
    /// both HTTP and gRPC save paths.
    pub fn update_service_config_typed(
        &mut self,
        service_id: Uuid,
        service_type: ServiceType,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.services.iter().find(|s| s.id == service_id).cloned() else {
            error!("Service {} not found", service_id);
            return;
        };
        let mut updated = service;
        updated.service_type = service_type;

        let storage = GlobalStorage::global(cx);
        if let Err(e) = storage.service.update(&updated) {
            error!("Failed to update service config {}: {}", service_id, e);
            return;
        }

        if let Some(s) = self.services.iter_mut().find(|s| s.id == service_id) {
            s.service_type = updated.service_type;
            s.updated_at = chrono::Utc::now().timestamp();
        }
        cx.notify();
    }

    /// Accessor for the sidebar tree state.
    pub fn tree(&self) -> &Entity<TreeState> {
        &self.tree
    }

    /// Rebuilds the sidebar tree from the current folders/services/schemas.
    /// Unsaved draft services are excluded — they exist only as open tabs.
    fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        let visible_services: Vec<Service> = self
            .services
            .iter()
            .filter(|s| !self.unsaved.contains(&s.id))
            .cloned()
            .collect();
        let items = build_tree(&self.folders, &visible_services, &self.schemas);
        self.tree.update(cx, |state, cx| {
            state.set_items(items, cx);
        });
    }

    /// Imports an OpenAPI spec into this workspace (DB + in-memory + tree).
    pub fn import_openapi(
        &mut self,
        content: &str,
        options: workspace::ImportOptions,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let spec = workspace::parse_openapi(content).map_err(|e| e.to_string())?;
        let result = workspace::import_openapi_with_options(
            self.workspace_id,
            &spec,
            options,
            &self.folders,
        )
        .map_err(|e| e.to_string())?;

        let storage = GlobalStorage::global(cx);
        for folder in &result.folders {
            if let Err(e) = storage.folder.insert(folder) {
                error!("Failed to insert folder {}: {}", folder.name, e);
            }
        }
        for service in &result.services {
            if let Err(e) = storage.service.insert(service) {
                error!("Failed to insert service {}: {}", service.name, e);
            }
        }
        for schema in &result.schemas {
            if let Err(e) = storage.schema.insert(schema) {
                error!("Failed to insert schema {}: {}", schema.name, e);
            }
        }

        self.folders.extend(result.folders);
        self.services.extend(result.services);
        self.schemas.extend(result.schemas);
        self.rebuild_tree(cx);
        cx.notify();
        Ok(())
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
        let closing_id = self.open_views.get(ix).map(|e| e.view.id(cx));
        self.remove_view(ix);

        // Closing an unsaved draft discards it entirely: it was never persisted,
        // so drop the in-memory placeholder and any draft-table record it wrote.
        if let Some(id) = closing_id {
            if self.unsaved.remove(&id) {
                self.services.retain(|s| s.id != id);
                if let Err(e) = GlobalStorage::global(cx).draft.delete(id) {
                    error!("Failed to delete draft for discarded request {}: {}", id, e);
                }
            }
        }
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
        // Unsaved drafts have no DB row yet: rename in memory and on the tab only.
        if self.unsaved.contains(&service_id) {
            if let Some(s) = self.services.iter_mut().find(|s| s.id == service_id) {
                s.name = new_name.clone();
            }
            if let Some(ix) = self
                .open_views
                .iter()
                .position(|e| e.view.id(cx) == service_id)
            {
                self.open_views[ix].view.set_title(new_name, cx);
            }
            cx.notify();
            return;
        }

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

    pub fn get_schema(&self, id: Uuid) -> Option<&Schema> {
        self.schemas.iter().find(|s| s.id == id)
    }

    /// Persists `schema` (by id) and updates the in-memory copy + tab title.
    pub fn update_schema(&mut self, schema: Schema, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        if let Err(e) = storage.schema.update(&schema) {
            error!("Failed to update schema {}: {}", schema.id, e);
            return;
        }
        let id = schema.id;
        let name = schema.name.clone();
        if let Some(existing) = self.schemas.iter_mut().find(|s| s.id == id) {
            *existing = schema;
        }
        if let Some(ix) = self.open_views.iter().position(|e| e.view.id(cx) == id) {
            self.open_views[ix].view.set_title(name, cx);
        }
        self.rebuild_tree(cx);
        cx.notify();
    }

    pub fn delete_schema(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        if let Err(e) = storage.schema.delete(id) {
            error!("Failed to delete schema {}: {}", id, e);
            return;
        }
        self.schemas.retain(|s| s.id != id);
        if let Some(ix) = self.open_views.iter().position(|e| e.view.id(cx) == id) {
            self.close_content(ix, cx);
        }
        self.rebuild_tree(cx);
        cx.notify();
    }

    pub fn open_schema_detail(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(schema) = self.get_schema(id).cloned() else {
            return;
        };
        // Collect the `$ref` candidate names here (excluding self), so the page
        // doesn't have to read `ServiceState` while it's still being updated.
        let schema_names: Vec<SharedString> = self
            .schemas
            .iter()
            .filter(|s| s.id != id)
            .map(|s| s.name.clone())
            .collect();
        self.open_view(id, window, cx, move |window, cx| {
            let service_state = cx.entity();
            Box::new(
                cx.new(|cx| SchemaDetailPage::new(service_state, schema, schema_names, window, cx)),
            )
        });
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

    // === Open helpers ===

    pub fn open_service(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.get_service(id).cloned() else {
            return;
        };
        // Named-schema `$ref` candidates for the design view's schema editor.
        // Collected here (not read inside the factory) to avoid re-entering
        // `ServiceState` while it is being updated.
        let schema_names: Vec<SharedString> = self.schemas.iter().map(|s| s.name.clone()).collect();
        self.open_view(id, window, cx, move |window, cx| {
            match &service.service_type {
                ServiceType::Http(_) => {
                    let service_state = cx.entity();
                    Box::new(cx.new(|cx| {
                        HttpService::new(service, service_state, schema_names, window, cx)
                    }))
                    // env_state is read from service_state inside HttpState.
                }
                ServiceType::Grpc(_) => {
                    let service_state = cx.entity();
                    Box::new(cx.new(|cx| GrpcService::new(service, service_state, window, cx)))
                }
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
