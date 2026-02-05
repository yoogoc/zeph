use crate::containers::service_manager::ServiceManager;
use crate::containers::{FolderDetailPage, ServiceTabs, ServiceTabsEvent};
use crate::dialogs::{ConfirmDialog, ConfirmDialogEvent};
use crate::selectors::EnvironmentSelector;
use crate::ws_sidebar::{TreeId, WorkspaceSidebar as WsSidebar};
use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::{h_flex, v_flex, WindowExt};
use models::{ContentType, ServiceType, WorkspaceView};
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
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceContent {
    pub fn new(_workspace_id: Option<Uuid>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| WsSidebar::new(cx));
        let service_tabs = cx.new(|cx| ServiceTabs::new(cx));
        let env_selector = cx.new(|_| EnvironmentSelector::new());
        let folder_detail_page = cx.new(|cx| FolderDetailPage::new(cx));

        let workspace_registry = workspace::WorkspaceRegistry::global(cx);

        // Subscribe to WorkspaceEvent for http_services cache management
        // UI components (sidebar, service_tabs, env_selector, folder_detail) handle their own subscriptions
        let registry_sub = cx.subscribe_in(&workspace_registry, window, {
            move |this, _registry, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::Open(workspace) => {
                    // Clear http_services cache to release old workspace's service instances
                    this.service_manager.clear_http_services();

                    // Create HttpService for active_service if exists
                    if let Some(active_service) = &workspace.active_service {
                        this.service_manager
                            .ensure_http_service(active_service, window, cx);
                    }
                    cx.notify();
                }
                _ => cx.notify(),
            }
        });

        let registry_sub2 = cx.subscribe_in(&workspace_registry, window, {
            move |this, _registry, event: &WorkspaceServiceEvent, window, cx| {
                match event {
                    WorkspaceServiceEvent::Open(_workspace, service) => {
                        this.service_manager
                            .ensure_http_service(service, window, cx);
                        this.update_folder_detail_if_needed(service, cx);
                    }
                    WorkspaceServiceEvent::Switch {
                        workspace: _,
                        old,
                        new,
                    } => {
                        if let ContentType::Service(service) = old {
                            this.service_manager.remove_http_service(&service.id);
                        }
                        if let Some(new) = new {
                            this.service_manager.ensure_http_service(new, window, cx);
                            this.update_folder_detail_if_needed(new, cx);
                        }
                    }
                }
                cx.notify();
            }
        });

        let tabs_sub = cx.subscribe_in(&service_tabs, window, {
            let sidebar = sidebar.clone();
            move |this, _, event, window, cx| match event {
                ServiceTabsEvent::Select(ct) => {
                    sidebar.update(cx, |s, cx| {
                        s.set_ix(TreeId::from_content(ct), cx);
                    });
                    this.set_active_service(ct.clone(), cx);
                }
                ServiceTabsEvent::Close(ct) => this.close_service(ct.clone(), window, cx),
                ServiceTabsEvent::Rename(ct, new_name) => {
                    this.rename_service_from_tab(ct.clone(), new_name.clone(), cx);
                }
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
            _subscriptions: vec![registry_sub, registry_sub2, tabs_sub],
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
            let registry = workspace::WorkspaceRegistry::global(cx);
            registry.update(cx, |registry, cx| {
                registry.rename_service(service.id, new_name, cx);
            });
        }
    }

    fn close_service(&mut self, ct: ContentType, window: &mut Window, cx: &mut Context<Self>) {
        // Check if this is a Service with unsaved changes (Requirements 3.5)
        if let ContentType::Service(service) = &ct {
            if self.service_manager.has_unsaved_changes(&service.id, cx) {
                // Show confirmation dialog
                self.show_close_confirmation(ct.clone(), window, cx);
                return;
            }
        }

        // No unsaved changes, proceed with close
        self.do_close_service(ct, cx);
    }

    /// Shows the confirmation dialog for closing a service with unsaved changes
    /// Implements Requirements 3.5
    fn show_close_confirmation(
        &mut self,
        ct: ContentType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dialog_entity = cx.new(|_cx| ConfirmDialog::unsaved_changes());
        let ct_for_handler = ct.clone();

        cx.subscribe_in(
            &dialog_entity,
            window,
            move |this, _, event: &ConfirmDialogEvent, window, cx| {
                match event {
                    ConfirmDialogEvent::Confirm => {
                        // Save and then close
                        if let ContentType::Service(service) = &ct_for_handler {
                            this.service_manager.save_service_changes(&service.id, cx);
                        }
                        this.do_close_service(ct_for_handler.clone(), cx);
                    }
                    ConfirmDialogEvent::Discard => {
                        // Close without saving
                        this.do_close_service(ct_for_handler.clone(), cx);
                    }
                    ConfirmDialogEvent::Cancel => {
                        // Cancel the close operation - do nothing
                    }
                }
                window.close_dialog(cx);
                cx.notify();
            },
        )
        .detach();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog.title(shared_t!("service.confirm_close.title")).child(dialog_entity.clone())
        });
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

    fn render_service_content(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let registry = workspace::WorkspaceRegistry::global(cx);
        let active_service = registry
            .read(cx)
            .get_active_workspace()
            .and_then(|ws| ws.active_service.clone());

        match active_service.as_ref() {
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

        if let Some(_workspace) = workspace {
            h_resizable("workspace-content")
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
                                    .child(self.render_service_content(window, cx)),
                            ),
                    ),
                )
                .into_any_element()
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
