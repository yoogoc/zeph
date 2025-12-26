use crate::{EnvironmentSelector, EnvironmentSelectorEvent, HttpService, ServiceTabs, ServiceTabsEvent, WorkspaceSidebar, WorkspaceSidebarEvent, ImportConfig};
use gpui::*;
use gpui_component::StyledExt;
use models::{ServiceType, Workspace, WorkspaceView};
use uuid::Uuid;

pub struct WorkspaceContent {
    workspace_id: Option<Uuid>,
    view: WorkspaceView,
    http_services: std::collections::HashMap<Uuid, Entity<HttpService>>,
    service_tabs: Option<Entity<ServiceTabs>>,
    sidebar: Option<Entity<WorkspaceSidebar>>,
    env_selector: Option<Entity<EnvironmentSelector>>,
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceContent {
    pub fn new(_workspace_id: Option<Uuid>, _view: WorkspaceView) -> Self {
        Self {
            workspace_id: None,
            view: WorkspaceView::Services,
            http_services: std::collections::HashMap::new(),
            service_tabs: None,
            sidebar: None,
            env_selector: None,
            _subscriptions: vec![],
        }
    }

    pub fn init(&mut self, cx: &mut Context<Self>) {
        let service_tabs = cx.new(|_| ServiceTabs::new());
        let sidebar = cx.new(|_| WorkspaceSidebar::new());
        let env_selector = cx.new(|_| EnvironmentSelector::new());

        let tabs_sub = cx.subscribe(&service_tabs, |this, _, event, cx| match event {
            ServiceTabsEvent::Select(id) => this.set_active_service(*id, cx),
            ServiceTabsEvent::Close(id) => this.close_service(*id, cx),
        });

        let sidebar_sub = cx.subscribe(&sidebar, |this, _, event, cx| match event {
            WorkspaceSidebarEvent::OpenService(id) => this.open_service(*id, cx),
            WorkspaceSidebarEvent::OpenSchema(_id) => {}
            WorkspaceSidebarEvent::CreateService(name, folder_id) => this.create_service(name.clone(), *folder_id, cx),
            WorkspaceSidebarEvent::CreateSchema(name, folder_id) => this.create_schema(name.clone(), *folder_id, cx),
            WorkspaceSidebarEvent::CreateFolder(name, parent_id, folder_type) => this.create_folder(name.clone(), *parent_id, folder_type.clone(), cx),
            WorkspaceSidebarEvent::ToggleFolder(id) => this.toggle_folder(*id, cx),
            WorkspaceSidebarEvent::ImportOpenApi(config) => this.import_openapi(config.clone(), cx),
            WorkspaceSidebarEvent::PreviewImport(config) => this.preview_import(config.clone(), cx),
            WorkspaceSidebarEvent::RequestImportPreview(config) => this.request_import_preview(config.clone(), cx),
            WorkspaceSidebarEvent::ResolveImportConflicts(config, resolutions) => this.resolve_import_conflicts(config.clone(), resolutions.clone(), cx),
        });

        let env_sub = cx.subscribe(&env_selector, |this, _, event, cx| match event {
            EnvironmentSelectorEvent::SelectEnvironment(id) => this.select_environment(*id, cx),
            EnvironmentSelectorEvent::CreateEnvironment(name) => this.create_environment(name.clone(), cx),
            EnvironmentSelectorEvent::UpdateEnvironment(env) => this.update_environment(env.clone(), cx),
            EnvironmentSelectorEvent::DeleteEnvironment(id) => this.delete_environment(*id, cx),
            EnvironmentSelectorEvent::AddVariable(env_id, var) => this.add_variable(*env_id, var.clone(), cx),
            EnvironmentSelectorEvent::UpdateVariable(var) => this.update_variable(var.clone(), cx),
            EnvironmentSelectorEvent::DeleteVariable(id) => this.delete_variable(*id, cx),
        });

        let registry = workspace::WorkspaceRegistry::global(cx);
        let registry_sub = cx.observe(&registry, |_, _, cx| cx.notify());

        self.service_tabs = Some(service_tabs);
        self.sidebar = Some(sidebar);
        self.env_selector = Some(env_selector);
        self._subscriptions = vec![tabs_sub, sidebar_sub, env_sub, registry_sub];
    }

    pub fn set_view(&mut self, view: WorkspaceView, workspace_id: Option<Uuid>, cx: &mut Context<Self>) {
        self.view = view;
        self.workspace_id = workspace_id;
        cx.notify();
    }

    // Environment operations
    fn select_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.set_active_environment(env_id, cx));
        
        // Update base_url for all existing HttpService instances
        self.update_http_services_base_url(cx);
    }

    fn create_environment(&mut self, name: String, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.create_environment(name, cx));
    }

    fn update_environment(&mut self, env: models::Environment, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.update_environment(env, cx));
        
        // Update base_url for all existing HttpService instances if the active environment was updated
        self.update_http_services_base_url(cx);
    }

    fn delete_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.delete_environment(env_id, cx));
        
        // Update base_url for all existing HttpService instances (in case the deleted env was active)
        self.update_http_services_base_url(cx);
    }

    fn add_variable(&mut self, env_id: Uuid, var: models::EnvironmentVariable, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.add_environment_variable(env_id, var, cx));
    }

    fn update_variable(&mut self, var: models::EnvironmentVariable, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.update_environment_variable(var, cx));
    }

    fn delete_variable(&mut self, var_id: Uuid, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.delete_environment_variable(var_id, cx));
    }

    /// Updates the base_url for all existing HttpService instances when the environment changes
    fn update_http_services_base_url(&mut self, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        let workspace = registry.read(cx).get_active_workspace().cloned();
        
        if let Some(workspace) = workspace {
            // Get base_url from active environment
            let base_url = workspace.active_environment_id
                .and_then(|env_id| workspace.environments.iter().find(|e| e.id == env_id))
                .map(|env| env.base_url.clone())
                .unwrap_or_default();
            
            // Update all existing HttpService instances
            for http_service in self.http_services.values() {
                http_service.update(cx, |service, cx| {
                    service.set_base_url(base_url.clone(), cx);
                });
            }
        }
    }

    fn import_openapi(&mut self, config: ImportConfig, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            let options = workspace::ImportOptions {
                target_folder: config.target_folder,
                create_subfolders: config.create_subfolders,
                schema_folder_strategy: config.schema_folder_strategy,
            };
            if let Err(e) = registry.import_openapi_with_options(&config.content, options, cx) {
                eprintln!("Failed to import OpenAPI: {}", e);
            }
        });
    }

    fn preview_import(&mut self, config: ImportConfig, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, _cx| {
            let options = workspace::ImportOptions {
                target_folder: config.target_folder,
                create_subfolders: config.create_subfolders,
                schema_folder_strategy: config.schema_folder_strategy,
            };
            if let Ok(preview) = registry.generate_openapi_import_preview(&config.content, options) {
                // In a real implementation, we would send this preview back to the dialog
                // For now, we'll just log it
                println!("Import preview: {} services, {} schemas", preview.services.len(), preview.schemas.len());
            }
        });
    }

    /// Handle request for import preview with enhanced error handling
    /// Implements Requirements 6.1, 6.2 for error handling in UI
    fn request_import_preview(&mut self, config: ImportConfig, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, _cx| {
            let options = workspace::ImportOptions {
                target_folder: config.target_folder,
                create_subfolders: config.create_subfolders,
                schema_folder_strategy: config.schema_folder_strategy,
            };
            
            // Validate content first
            match workspace::validate_openapi_content(&config.content) {
                Ok(_) => {
                    // Generate preview
                    match registry.generate_openapi_import_preview(&config.content, options) {
                        Ok(preview) => {
                            // TODO: Send preview back to dialog
                            println!("Import preview generated: {} services, {} schemas", 
                                preview.services.len(), preview.schemas.len());
                        }
                        Err(e) => {
                            // TODO: Send error back to dialog
                            eprintln!("Failed to generate preview: {}", e);
                        }
                    }
                }
                Err(e) => {
                    // TODO: Send validation error back to dialog
                    eprintln!("Content validation failed: {}", e);
                }
            }
        });
    }

    /// Handle conflict resolution and import with enhanced validation
    /// Implements Requirements 6.4 for conflict resolution
    fn resolve_import_conflicts(&mut self, config: ImportConfig, resolutions: std::collections::HashMap<String, workspace::ConflictResolution>, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, _cx| {
            let options = workspace::ImportOptions {
                target_folder: config.target_folder,
                create_subfolders: config.create_subfolders,
                schema_folder_strategy: config.schema_folder_strategy,
            };
            
            // Get current workspace data for conflict checking
            if let Some(workspace) = registry.get_active_workspace() {
                let existing_folders = &workspace.folders;
                let existing_services = &workspace.services;
                let existing_schemas = &workspace.schemas;
                
                // Use the enhanced import function with conflict resolution
                match workspace::import_openapi_with_validation(
                    workspace.id,
                    &config.content,
                    options,
                    existing_folders,
                    existing_services,
                    existing_schemas,
                    Some(&resolutions),
                ) {
                    Ok(result) => {
                        println!("Import completed with conflict resolution: {} folders, {} services, {} schemas", 
                            result.folders.len(), result.services.len(), result.schemas.len());
                        // TODO: Update UI with imported data
                    }
                    Err(e) => {
                        eprintln!("Failed to import with conflict resolution: {}", e);
                        // TODO: Send error back to dialog
                    }
                }
            }
        });
    }

    fn create_service(&mut self, name: String, folder_id: Option<Uuid>, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.create_service(name, folder_id, cx);
        });
    }

    fn create_schema(&mut self, name: String, folder_id: Option<Uuid>, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.create_schema(name, folder_id, cx);
        });
    }

    fn create_folder(&mut self, name: String, parent_id: Option<Uuid>, folder_type: models::FolderType, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.create_folder(name, parent_id, folder_type, cx);
        });
    }

    fn open_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.open_service(service_id, cx);
        });
        cx.notify();
    }

    fn close_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.close_service(service_id, cx);
        });
        self.http_services.remove(&service_id);
        cx.notify();
    }

    fn set_active_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.set_active_service(service_id, cx);
        });
        cx.notify();
    }

    fn toggle_folder(&mut self, folder_id: Uuid, cx: &mut Context<Self>) {
        if let Some(sidebar) = &self.sidebar {
            sidebar.update(cx, |sidebar, _| {
                if sidebar.expanded_folders.contains(&folder_id) {
                    sidebar.expanded_folders.remove(&folder_id);
                } else {
                    sidebar.expanded_folders.insert(folder_id);
                }
            });
        }
        cx.notify();
    }

    fn render_service_content(
        &mut self,
        workspace: &Workspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Get base_url from active environment
        let base_url = workspace.active_environment_id
            .and_then(|env_id| workspace.environments.iter().find(|e| e.id == env_id))
            .map(|env| env.base_url.clone());

        if let Some(active_service_id) = workspace.active_service {
            if let Some(service) = workspace.services.iter().find(|s| s.id == active_service_id) {
                match &service.service_type {
                    ServiceType::Http(config) => {
                        if !self.http_services.contains_key(&service.id) {
                            let http_service =
                                cx.new(|cx| HttpService::new(config.clone(), base_url.clone(), window, cx));
                            self.http_services.insert(service.id, http_service);
                        }
                        self.http_services
                            .get(&service.id)
                            .unwrap()
                            .clone()
                            .into_any_element()
                    }
                    ServiceType::Grpc(_) => div()
                        .flex_1()
                        .h_full()
                        .p_4()
                        .child("gRPC service - coming soon")
                        .into_any_element(),
                    ServiceType::WebSocket(_) => div()
                        .flex_1()
                        .h_full()
                        .p_4()
                        .child("WebSocket service - coming soon")
                        .into_any_element(),
                }
            } else {
                div().flex_1().h_full().p_4().child("Service not found").into_any_element()
            }
        } else {
            div()
                .flex_1()
                .h_full()
                .p_4()
                .items_center()
                .justify_center()
                .child("Select a service to get started")
                .into_any_element()
        }
    }

    fn render_services(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = workspace::WorkspaceRegistry::global(cx);
        let workspace = registry.read(cx).get_active_workspace().cloned();

        if let Some(workspace) = workspace {
            if let Some(sidebar) = &self.sidebar {
                sidebar.update(cx, |s, _| s.set_workspace(workspace.clone()));
            }
            if let Some(tabs) = &self.service_tabs {
                tabs.update(cx, |t, _| t.set_workspace(workspace.clone()));
            }
            if let Some(env_sel) = &self.env_selector {
                env_sel.update(cx, |e, _| e.set_workspace(workspace.clone()));
            }

            div()
                .h_flex()
                .size_full()
                .children(self.sidebar.clone())
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .v_flex()
                        .child(
                            // Tabs row with environment selector
                            div()
                                .h_flex()
                                .items_center()
                                .justify_between()
                                .border_b_1()
                                .border_color(gpui::rgb(0xe0e0e0))
                                .child(
                                    div().flex_1().children(self.service_tabs.clone())
                                )
                                .child(
                                    div().px_2().children(self.env_selector.clone())
                                )
                        )
                        .child(self.render_service_content(&workspace, window, cx)),
                )
        } else {
            div().p_4().child("No workspace selected")
        }
    }
}

impl Render for WorkspaceContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.view {
            WorkspaceView::Services => self.render_services(window, cx).into_any_element(),
            WorkspaceView::Docs => div().p_4().child("Documentation - coming soon").into_any_element(),
            WorkspaceView::History => div().p_4().child("History - coming soon").into_any_element(),
            WorkspaceView::Settings => div().p_4().child("Settings - coming soon").into_any_element(),
        }
    }
}
