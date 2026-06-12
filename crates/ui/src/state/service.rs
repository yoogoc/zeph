use crate::containers::HttpService;
use crate::state::build_tree;
use gpui::*;
use gpui_component::tree::TreeState;
use log::error;
use models::{
    ContentType, Environment, EnvironmentVariable, Folder, FolderType, Header, Schema,
    SearchParameter, Service, ServiceType,
};
use register::GlobalStorage;
use uuid::Uuid;

/// A cached HttpService view entity (mirrors `ApplicationState::Tab`).
pub struct HttpEntry {
    pub service_id: Uuid,
    pub entity: Entity<HttpService>,
}

pub struct ServiceState {
    pub workspace_id: Uuid,
    pub services: Vec<Service>,
    pub folders: Vec<Folder>,
    pub schemas: Vec<Schema>,
    pub environments: Vec<Environment>,
    pub opened_services: Vec<ContentType>,
    pub active_service: Option<ContentType>,
    pub active_environment_id: Option<Uuid>,

    /// Sidebar tree UI state (items, selection, expansion).
    tree: Entity<TreeState>,

    /// HttpService view cache (mirrors `ApplicationState.tabs`).
    http_services: Vec<HttpEntry>,
    /// Index of the active entry in `http_services` (mirrors `ApplicationState.active_idx`).
    http_active_ix: Option<usize>,
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
            opened_services: vec![],
            active_service: None,
            active_environment_id,
            tree,
            http_services: vec![],
            http_active_ix: None,
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

    pub fn open_content(&mut self, ct: ContentType, window: &mut Window, cx: &mut Context<Self>) {
        if let ContentType::Service(service) = &ct {
            self.ensure_http_service(service, window, cx);
        }
        if !self.opened_services.contains(&ct) {
            self.opened_services.push(ct.clone());
        }
        self.active_service = Some(ct);
        cx.notify();
    }

    pub fn close_content(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        self.opened_services.retain(|id| id.ne(&ct));
        if let ContentType::Service(service) = &ct {
            self.remove_http_service(service.id);
        }
        if self.active_service == Some(ct) {
            self.active_service = self.opened_services.first().cloned();
            self.sync_http_active_ix();
        }
        cx.notify();
    }

    pub fn set_active_content(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        self.active_service = Some(ct);
        self.sync_http_active_ix();
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

                    let updated = self.services.iter().find(|s| s.id == service_id).cloned();
                    if let Some(updated_ref) = updated {
                        for content in self.opened_services.iter_mut() {
                            if let ContentType::Service(s) = content {
                                if s.id == service_id {
                                    *content = ContentType::Service(updated_ref.clone());
                                }
                            }
                        }
                        if let Some(ContentType::Service(active)) = &self.active_service {
                            if active.id == service_id {
                                self.active_service =
                                    Some(ContentType::Service(updated_ref.clone()));
                            }
                        }
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
        let was_active = matches!(
            &self.active_service,
            Some(ContentType::Service(s)) if s.id == service_id
        );
        self.opened_services
            .retain(|content| !matches!(content, ContentType::Service(s) if s.id == service_id));
        if was_active {
            self.active_service = self.opened_services.first().cloned();
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
                self.rebuild_tree(cx);
                cx.notify();
            }
            Err(e) => error!("Failed to rename folder {}: {}", folder_id, e),
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
        if let Some(service) = self.get_service(id).cloned() {
            self.open_content(ContentType::Service(service), window, cx);
        }
    }

    pub fn open_folder_detail(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(folder) = self.get_folder(id).cloned() {
            if folder.folder_type == FolderType::Service {
                self.open_content(ContentType::ServiceFolder(folder), window, cx);
            } else {
                self.open_content(ContentType::SchemaFolder(folder), window, cx);
            }
        }
    }

    // === HttpService view cache (mirrors ApplicationState.tabs) ===

    /// Returns the currently active HttpService entity (mirrors `tabs[active_idx].view`).
    pub fn active_http_service(&self) -> Option<Entity<HttpService>> {
        self.http_active_ix
            .and_then(|ix| self.http_services.get(ix))
            .map(|entry| entry.entity.clone())
    }

    /// Looks up a cached HttpService entity by service id.
    pub fn http_service_by_id(&self, service_id: Uuid) -> Option<&Entity<HttpService>> {
        self.http_services
            .iter()
            .find(|e| e.service_id == service_id)
            .map(|e| &e.entity)
    }

    /// Ensures a cached HttpService exists for the given service and makes it active.
    /// Mirrors `ApplicationState::open_workspace` (reuse existing, else create a view).
    pub fn ensure_http_service(
        &mut self,
        service: &Service,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ServiceType::Http(config) = &service.service_type else {
            return;
        };

        if let Some(pos) = self
            .http_services
            .iter()
            .position(|e| e.service_id == service.id)
        {
            self.http_active_ix = Some(pos);
            return;
        }

        let config = config.clone();
        let folder_id = service.folder_id;
        let service_id = service.id;
        let entity = cx.new(|cx| HttpService::new(service_id, config, folder_id, window, cx));
        self.http_services.push(HttpEntry { service_id, entity });
        self.http_active_ix = Some(self.http_services.len() - 1);
    }

    /// Removes a cached HttpService entry, fixing up the active index (mirrors `close_tab`).
    pub fn remove_http_service(&mut self, service_id: Uuid) {
        let Some(pos) = self
            .http_services
            .iter()
            .position(|e| e.service_id == service_id)
        else {
            return;
        };
        self.http_services.remove(pos);

        match self.http_active_ix {
            Some(ix) if ix == pos => {
                self.http_active_ix = if self.http_services.is_empty() {
                    None
                } else {
                    Some(pos.min(self.http_services.len() - 1))
                };
            }
            Some(ix) if ix > pos => self.http_active_ix = Some(ix - 1),
            other => self.http_active_ix = other,
        }
    }

    /// Checks whether a cached HttpService has unsaved changes.
    pub fn http_service_is_modified(&self, service_id: Uuid, cx: &App) -> bool {
        self.http_service_by_id(service_id)
            .map(|entity| entity.read(cx).is_modified(cx))
            .unwrap_or(false)
    }

    /// Saves changes for a cached HttpService.
    pub fn save_http_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(entity) = self.http_service_by_id(service_id).cloned() {
            entity.update(cx, |service, cx| {
                service.save_changes_no_window(cx);
            });
        }
    }

    /// Syncs `http_active_ix` to follow the current `active_service`.
    fn sync_http_active_ix(&mut self) {
        self.http_active_ix = match &self.active_service {
            Some(ContentType::Service(service)) => self
                .http_services
                .iter()
                .position(|e| e.service_id == service.id),
            _ => None,
        };
    }
}
