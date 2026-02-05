use crate::containers::import_handler::{self, ImportHandler};
use crate::containers::service_manager::ServiceManager;
use crate::containers::{FolderDetailPage, FolderDetailPageEvent, ServiceTabs, ServiceTabsEvent};
use crate::dialogs::ConfirmDialogEvent;
use crate::selectors::EnvironmentSelector;
use crate::simple_import_dialog::ImportErrorType;
use crate::widgets::OverlayDialog;
use crate::ws_sidebar::{TreeId, WorkspaceSidebar as WsSidebar, WorkspaceSidebarEvent};
use crate::{ImportConfig, SimpleImportConfig, SimpleImportDialog, SimpleImportDialogEvent};
use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::{h_flex, v_flex};
use models::{ContentType, FolderType, ServiceType, Workspace, WorkspaceView};
use uuid::Uuid;
use workspace::{WorkspaceEvent, WorkspaceServiceEvent};

pub struct WorkspaceContent {
    workspace_id: Option<Uuid>,
    view: WorkspaceView,
    /// Service manager handles HTTP service lifecycle and state
    service_manager: ServiceManager,
    service_tabs: Entity<ServiceTabs>,
    sidebar: Entity<WsSidebar>,
    env_selector: Entity<EnvironmentSelector>,
    folder_detail: Entity<FolderDetailPage>,
    /// Import handler manages import dialog state
    import_handler: ImportHandler,
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceContent {
    pub fn new(_workspace_id: Option<Uuid>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| WsSidebar::new(cx));
        let service_tabs = cx.new(|_| ServiceTabs::new());
        let env_selector = cx.new(|_| EnvironmentSelector::new());
        let folder_detail_page = cx.new(|cx| FolderDetailPage::new(cx));

        let workspace_registry = workspace::WorkspaceRegistry::global(cx);

        let registry_sub = cx.subscribe(&workspace_registry, {
            let sidebar = sidebar.clone();
            let service_tabs = service_tabs.clone();
            let env_selector = env_selector.clone();
            let folder_detail = folder_detail_page.clone();
            move |this, registry, event: &WorkspaceEvent, cx| match event {
                WorkspaceEvent::Open(workspace) => {
                    // Sync current_service to target workspace's active_service
                    this.service_manager
                        .set_current_content(workspace.active_service.clone());
                    // Clear http_services cache to release old workspace's service instances
                    this.service_manager.clear_http_services();

                    sidebar.update(cx, |s, cx| {
                        s.set_workspace(workspace.clone(), cx);
                    });
                    service_tabs.update(cx, |s, cx| {
                        s.set_workspace(workspace.clone());
                        cx.notify();
                    });
                    env_selector.update(cx, |_, cx| {
                        cx.notify();
                    });
                    cx.notify();
                }
                _ => {
                    // Handle WorkspaceEvent::Reload and WorkspaceEvent::Close
                    // Update all UI components with the latest workspace data
                    // Implements Requirements 1.4, 2.4, 8.1, 8.2
                    sidebar.update(cx, |s, cx| {
                        if let Some(workspace) = registry.read(cx).get_active_workspace() {
                            s.set_workspace(workspace.clone(), cx);
                        }
                    });
                    service_tabs.update(cx, |s, cx| {
                        if let Some(workspace) = registry.read(cx).get_active_workspace() {
                            s.set_workspace(workspace.clone());
                        }
                        cx.notify();
                    });
                    env_selector.update(cx, |_, cx| {
                        cx.notify();
                    });
                    // Update folder detail page if a folder is currently being displayed
                    folder_detail.update(cx, |detail, cx| {
                        if let Some(folder_id) = detail.get_folder_id() {
                            if let Some(workspace) = registry.read(cx).get_active_workspace() {
                                // Find the updated folder in the workspace
                                if let Some(updated_folder) =
                                    workspace.folders.iter().find(|f| f.id == folder_id)
                                {
                                    detail.set_folder(Some(updated_folder.clone()), cx);
                                }
                            }
                        }
                    });
                    cx.notify()
                }
            }
        });

        let registry_sub2 = cx.subscribe_in(&workspace_registry, window, {
            let service_tabs = service_tabs.clone();
            move |this, _registry, event: &WorkspaceServiceEvent, window, cx| {
                match event {
                    WorkspaceServiceEvent::Open(workspace, service) => {
                        this.service_manager
                            .set_content(workspace, service, window, cx);
                        this.update_folder_detail_if_needed(service, cx);
                        service_tabs.update(cx, |s, cx| {
                            s.set_workspace(workspace.clone());
                            cx.notify();
                        });
                    }
                    WorkspaceServiceEvent::Switch {
                        workspace,
                        old,
                        new,
                    } => {
                        if let ContentType::Service(service) = old {
                            this.service_manager.remove_http_service(&service.id);
                        }
                        if let Some(new) = new {
                            this.service_manager.set_content(workspace, new, window, cx);
                            this.update_folder_detail_if_needed(new, cx);
                        } else {
                            this.service_manager.set_current_content(None);
                        }
                        service_tabs.update(cx, |s, cx| {
                            s.set_workspace(workspace.clone());
                            cx.notify();
                        });
                    }
                }
                cx.notify();
            }
        });

        let tabs_sub = cx.subscribe(&service_tabs, {
            let sidebar = sidebar.clone();
            move |this, _, event, cx| match event {
                ServiceTabsEvent::Select(ct) => {
                    sidebar.update(cx, |s, cx| {
                        s.set_ix(TreeId::from_content(ct), cx);
                    });
                    this.set_active_service(ct.clone(), cx);
                }
                ServiceTabsEvent::Close(ct) => this.close_service(ct.clone(), cx),
                ServiceTabsEvent::Rename(ct, new_name) => {
                    this.rename_service_from_tab(ct.clone(), new_name.clone(), cx);
                }
            }
        });

        let sidebar_sub = cx.subscribe_in(
            &sidebar,
            window,
            |this, sidebar_entity, event, window, cx| match event {
                WorkspaceSidebarEvent::OpenService(id) => this.open_service(*id, cx),
                WorkspaceSidebarEvent::OpenSchema(_id) => {}
                WorkspaceSidebarEvent::OpenFolderDetail(id) => this.open_folder_detail(*id, cx),
                WorkspaceSidebarEvent::CreateService(name, folder_id) => {
                    this.create_service(name.clone(), *folder_id, cx)
                }
                WorkspaceSidebarEvent::CreateSchema(name, folder_id) => {
                    this.create_schema(name.clone(), *folder_id, cx)
                }
                WorkspaceSidebarEvent::CreateFolder(name, parent_id, folder_type) => {
                    this.create_folder(name.clone(), *parent_id, folder_type.clone(), cx)
                }
                WorkspaceSidebarEvent::ImportOpenApi(config) => {
                    this.import_openapi(config.clone(), cx)
                }
                // New menu events - for now just log them, actual implementation will be added later
                WorkspaceSidebarEvent::DeleteService(id) => {
                    // TODO: Implement service deletion
                    eprintln!("Delete service requested: {}", id);
                }
                WorkspaceSidebarEvent::DeleteSchema(id) => {
                    // TODO: Implement schema deletion
                    eprintln!("Delete schema requested: {}", id);
                }
                WorkspaceSidebarEvent::DeleteFolder(id) => {
                    // TODO: Implement folder deletion
                    eprintln!("Delete folder requested: {}", id);
                }
                // Import dialog events
                // Implements Requirements 1.1, 1.2, 1.3, 1.4, 3.4
                WorkspaceSidebarEvent::ShowImportDialog(folder_id) => {
                    this.show_import_dialog(*folder_id, window, cx);
                }
                // Implements Requirements 3.4, 4.4, 4.5
                WorkspaceSidebarEvent::ExecuteImport(config) => {
                    this.execute_simple_import(config.clone(), cx);
                }
                // Rename events - show the rename dialog
                // Implements Requirements 1.1, 1.2, 2.1, 2.2
                WorkspaceSidebarEvent::RenameService(_id) => {
                    sidebar_entity.update(cx, |sidebar, cx| {
                        sidebar.handle_rename_event(event, window, cx);
                    });
                }
                WorkspaceSidebarEvent::RenameFolder(_id) => {
                    sidebar_entity.update(cx, |sidebar, cx| {
                        sidebar.handle_rename_event(event, window, cx);
                    });
                }
                // Confirm rename events - update the database
                // Implements Requirements 1.3, 2.3
                WorkspaceSidebarEvent::ConfirmRenameService(id, new_name) => {
                    this.rename_service(*id, new_name.clone(), cx);
                }
                WorkspaceSidebarEvent::ConfirmRenameFolder(id, new_name) => {
                    this.rename_folder(*id, new_name.clone(), cx);
                }
            },
        );

        let folder_detail_sub =
            cx.subscribe(&folder_detail_page, |this, _, event, cx| match event {
                FolderDetailPageEvent::NavigateBack => this.navigate_back_from_folder_detail(cx),
                FolderDetailPageEvent::NavigateToFolder(folder_id) => {
                    this.open_folder_detail(*folder_id, cx)
                }
                FolderDetailPageEvent::UpdateCommonHeaders(folder_id, headers) => {
                    this.update_folder_common_headers(*folder_id, headers.clone(), cx)
                }
                FolderDetailPageEvent::UpdateSearchParams(folder_id, params) => {
                    this.update_folder_search_params(*folder_id, params.clone(), cx)
                }
                FolderDetailPageEvent::UpdateFolderInfo(folder_id, name, description) => {
                    this.update_folder_info(*folder_id, name.clone(), description.clone(), cx)
                }
            });

        Self {
            workspace_id: None,
            view: WorkspaceView::Services,
            service_manager: ServiceManager::new(),
            service_tabs,
            sidebar,
            env_selector,
            folder_detail: folder_detail_page,
            import_handler: ImportHandler::new(),
            _subscriptions: vec![
                registry_sub,
                registry_sub2,
                tabs_sub,
                sidebar_sub,
                folder_detail_sub,
            ],
        }
    }

    /// Updates the folder detail page if the content is a folder
    fn update_folder_detail_if_needed(&self, ct: &ContentType, cx: &mut Context<Self>) {
        if let ContentType::ServiceFolder(folder) = ct {
            self.folder_detail.update(cx, |s, cx| {
                s.set_folder(Some(folder.clone()), cx);
            });
        }
    }

    pub fn set_view(
        &mut self,
        view: WorkspaceView,
        workspace_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        self.view = view;
        self.workspace_id = workspace_id;
        cx.notify();
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

    /// Show the simplified import dialog
    /// Implements Requirements 1.1, 1.2, 1.3, 1.4, 3.4
    fn show_import_dialog(
        &mut self,
        folder_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Get folder name if folder_id is provided
        let folder_name = if let Some(id) = folder_id {
            let registry = workspace::WorkspaceRegistry::global(cx);
            registry
                .read(cx)
                .get_folder(id)
                .map(|f| SharedString::from(f.name.clone()))
        } else {
            None
        };

        // Create the import dialog
        let dialog = cx.new(|cx| {
            let mut dialog = SimpleImportDialog::new(window, cx);
            dialog.set_target_folder(folder_id, folder_name);
            dialog
        });

        // Subscribe to dialog events
        let dialog_sub = cx.subscribe(&dialog, |this, _, event: &SimpleImportDialogEvent, cx| {
            match event {
                SimpleImportDialogEvent::Import(config) => {
                    // Execute the import
                    this.execute_simple_import(config.clone(), cx);
                    // Close the dialog
                    this.import_handler.hide();
                    cx.notify();
                }
                SimpleImportDialogEvent::Cancel => {
                    // Close the dialog
                    this.import_handler.hide();
                    cx.notify();
                }
            }
        });

        // Store the subscription and show dialog
        self._subscriptions.push(dialog_sub);
        self.import_handler.show(dialog);
        cx.notify();
    }

    /// Execute import using SimpleImportConfig
    /// Implements Requirements 3.4, 4.4, 4.5, 5.1, 5.2
    fn execute_simple_import(&mut self, config: SimpleImportConfig, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            match import_handler::execute_import(&config, registry, cx) {
                Ok(_) => {
                    // Import successful - dialog will be closed by the event handler
                    // Implements Requirement 4.4 - dialog auto-closes on success
                    println!("OpenAPI import completed successfully");
                }
                Err(e) => {
                    // Import failed - show error in dialog
                    // Implements Requirement 5.3 - show error details and keep dialog open
                    eprintln!("Failed to import OpenAPI: {}", e);
                    self.import_handler
                        .set_error(ImportErrorType::ImportError(format!("导入失败: {}", e)), cx);
                }
            }
        });
    }

    fn create_service(
        &mut self,
        name: SharedString,
        folder_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.create_service(name, folder_id, cx);
        });
    }

    fn create_schema(
        &mut self,
        name: SharedString,
        folder_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.create_schema(name, folder_id, cx);
        });
    }

    fn create_folder(
        &mut self,
        name: SharedString,
        parent_id: Option<Uuid>,
        folder_type: FolderType,
        cx: &mut Context<Self>,
    ) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.create_folder(name, parent_id, folder_type, cx);
        });
    }

    /// Rename a service
    /// Implements Requirements 1.3
    fn rename_service(&mut self, service_id: Uuid, new_name: SharedString, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.rename_service(service_id, new_name, cx);
        });
    }

    /// Rename a folder
    /// Implements Requirements 2.3
    fn rename_folder(&mut self, folder_id: Uuid, new_name: SharedString, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.rename_folder(folder_id, new_name, cx);
        });
    }

    /// Rename a service from tab editing
    /// Implements Requirements 1.7: Tab title editing saves to database
    fn rename_service_from_tab(
        &mut self,
        content_type: ContentType,
        new_name: SharedString,
        cx: &mut Context<Self>,
    ) {
        // Only handle Service content type
        if let ContentType::Service(service) = content_type {
            self.rename_service(service.id, new_name, cx);
        }
    }

    fn open_service(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            if let Some(service) = registry.get_service(id) {
                registry.open_content(ContentType::Service(service.clone()), cx);
                cx.notify();
            }
        });
    }

    fn close_service(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        // Check if this is a Service with unsaved changes (Requirements 3.5)
        if let ContentType::Service(service) = &ct {
            if self.service_manager.has_unsaved_changes(&service.id, cx) {
                // Show confirmation dialog
                self.show_close_confirmation(ct.clone(), cx);
                return;
            }
        }

        // No unsaved changes, proceed with close
        self.do_close_service(ct, cx);
    }

    /// Shows the confirmation dialog for closing a service with unsaved changes
    /// Implements Requirements 3.5
    fn show_close_confirmation(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        use crate::dialogs::ConfirmDialog;

        let dialog = cx.new(|_cx| ConfirmDialog::unsaved_changes());

        // Subscribe to dialog events
        let dialog_sub = cx.subscribe(&dialog, move |this, _, event: &ConfirmDialogEvent, cx| {
            match event {
                ConfirmDialogEvent::Confirm => {
                    // Save and then close
                    if let Some(pending_ct) = this.service_manager.take_pending_close_content() {
                        if let ContentType::Service(service) = &pending_ct {
                            this.service_manager.save_service_changes(&service.id, cx);
                        }
                        this.do_close_service(pending_ct, cx);
                    }
                    this.service_manager.hide_confirm_dialog();
                    cx.notify();
                }
                ConfirmDialogEvent::Discard => {
                    // Close without saving
                    if let Some(pending_ct) = this.service_manager.take_pending_close_content() {
                        this.do_close_service(pending_ct, cx);
                    }
                    this.service_manager.hide_confirm_dialog();
                    cx.notify();
                }
                ConfirmDialogEvent::Cancel => {
                    // Cancel the close operation
                    this.service_manager.hide_confirm_dialog();
                    cx.notify();
                }
            }
        });

        self._subscriptions.push(dialog_sub);
        // Set up the confirmation dialog state in service manager
        self.service_manager.show_close_confirmation(ct, dialog);
        cx.notify();
    }

    /// Actually closes the service (without checking for unsaved changes)
    fn do_close_service(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, {
            let ct = ct.clone();
            |registry, cx| {
                registry.close_content(ct, cx);
            }
        });
        if let ContentType::Service(service) = &ct {
            if let ServiceType::Http(_) = service.service_type {
                self.service_manager.remove_http_service(&service.id);
            }
        }
        cx.notify();
    }

    fn set_active_service(&mut self, ct: ContentType, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.set_active_content(ct, cx);
        });
        cx.notify();
    }

    fn open_folder_detail(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            if let Some(folder) = registry.get_folder(id) {
                if folder.folder_type == FolderType::Service {
                    registry.open_content(ContentType::ServiceFolder(folder.clone()), cx);
                } else {
                    registry.open_content(ContentType::SchemaFolder(folder.clone()), cx);
                }
                cx.notify();
            }
        });
    }

    fn navigate_back_from_folder_detail(&mut self, cx: &mut Context<Self>) {
        // Navigate back to services view
        self.view = WorkspaceView::Services;
        cx.notify();
    }

    fn update_folder_common_headers(
        &mut self,
        folder_id: Uuid,
        headers: Vec<models::Header>,
        cx: &mut Context<Self>,
    ) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.update_folder_common_headers(folder_id, headers, cx);
        });
    }

    fn update_folder_search_params(
        &mut self,
        folder_id: Uuid,
        params: Vec<models::SearchParameter>,
        cx: &mut Context<Self>,
    ) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.update_folder_search_params(folder_id, params, cx);
        });
    }

    fn update_folder_info(
        &mut self,
        folder_id: Uuid,
        name: String,
        description: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.update_folder_info(folder_id, name, description, cx);
        });
    }

    fn render_service_content(
        &mut self,
        _workspace: &Workspace,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match self.service_manager.current_content() {
            Some(content) => match content {
                ContentType::Service(service) => match &service.service_type {
                    ServiceType::Http(_) => self
                        .service_manager
                        .get_http_service(&service.id)
                        .map(|service| service.clone().into_any_element())
                        .unwrap_or(div().into_any_element()),
                    ServiceType::Grpc(_) => h_flex()
                        .size_full()
                        .p_4()
                        .child("Grpc Page Coming soon...")
                        .into_any_element(),
                    ServiceType::WebSocket(_) => h_flex()
                        .size_full()
                        .p_4()
                        .child("WebSocket Page Coming soon...")
                        .into_any_element(),
                },
                ContentType::Schema(_) => h_flex()
                    .size_full()
                    .p_4()
                    .child("Schema Page Coming soon...")
                    .into_any_element(),
                ContentType::ServiceFolder(_) => h_flex()
                    .size_full()
                    .py_2()
                    .px_4()
                    .child(self.folder_detail.clone())
                    .into_any_element(),
                ContentType::SchemaFolder(_) => h_flex()
                    .size_full()
                    .p_4()
                    .child("Schema Folder Page Coming soon...")
                    .into_any_element(),
            },
            None => h_flex()
                .size_full()
                .p_4()
                .items_center()
                .justify_center()
                .child("Select a service to get started")
                .into_any_element(),
        }
    }

    fn render_services(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = workspace::WorkspaceRegistry::global(cx);
        let workspace = registry.read(cx).get_active_workspace().cloned();

        if let Some(workspace) = workspace {
            let main_content = h_resizable("workspace-content")
                .child(resizable_panel().size(px(200.)).child(self.sidebar.clone()))
                .child(
                    resizable_panel().child(
                        div()
                            .size_full()
                            .pt_1()
                            .pb_2()
                            .pr_2()
                            .bg(rgb(0xf5f5f5))
                            .child(
                                v_flex()
                                    .rounded_2xl()
                                    .bg(white())
                                    .flex_1()
                                    .h_full()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .justify_between()
                                            .border_b_1()
                                            .border_color(rgb(0xe0e0e0))
                                            .child(div().flex_1().child(self.service_tabs.clone()))
                                            .child(div().p_2().child(self.env_selector.clone())),
                                    )
                                    .child(self.render_service_content(&workspace, window, cx)),
                            ),
                    ),
                );

            // Determine which dialog overlay to show (if any)
            let overlay_content: Option<AnyElement> =
                if self.service_manager.is_confirm_dialog_visible() {
                    self.service_manager
                        .confirm_dialog()
                        .map(|d| d.clone().into_any_element())
                } else if self.import_handler.is_visible() {
                    self.import_handler
                        .dialog()
                        .map(|d| d.clone().into_any_element())
                } else {
                    None
                };

            // Render with OverlayDialog if a dialog is visible
            if let Some(dialog_content) = overlay_content {
                div()
                    .size_full()
                    .relative()
                    .child(main_content)
                    .child(OverlayDialog::new(dialog_content).dismissable(false))
                    .into_any_element()
            } else {
                main_content.into_any_element()
            }
        } else {
            div()
                .p_4()
                .child(shared_t!("workspaces.no_selected"))
                .into_any_element()
        }
    }
}

impl Render for WorkspaceContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.view {
            WorkspaceView::Services => self.render_services(window, cx).into_any_element(),
            WorkspaceView::Docs => div()
                .p_4()
                .child("Documentation - coming soon")
                .into_any_element(),
            WorkspaceView::History => div()
                .p_4()
                .child("History - coming soon")
                .into_any_element(),
            WorkspaceView::Settings => div()
                .p_4()
                .child("Settings - coming soon")
                .into_any_element(),
        }
    }
}
