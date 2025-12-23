use crate::{HttpService, ServiceTabs, ServiceTabsEvent, WorkspaceSidebar, WorkspaceSidebarEvent};
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
            _subscriptions: vec![],
        }
    }

    pub fn init(&mut self, cx: &mut Context<Self>) {
        let service_tabs = cx.new(|_| ServiceTabs::new());
        let sidebar = cx.new(|_| WorkspaceSidebar::new());

        let tabs_sub = cx.subscribe(&service_tabs, |this, _, event, cx| match event {
            ServiceTabsEvent::Select(id) => this.set_active_service(*id, cx),
            ServiceTabsEvent::Close(id) => this.close_service(*id, cx),
        });

        let sidebar_sub = cx.subscribe(&sidebar, |this, _, event, cx| match event {
            WorkspaceSidebarEvent::OpenService(id) => this.open_service(*id, cx),
            WorkspaceSidebarEvent::CreateService(name, folder_id) => this.create_service(name.clone(), *folder_id, cx),
            WorkspaceSidebarEvent::CreateFolder(name, parent_id) => this.create_folder(name.clone(), *parent_id, cx),
            WorkspaceSidebarEvent::ToggleFolder(id) => this.toggle_folder(*id, cx),
        });

        let registry = workspace::WorkspaceRegistry::global(cx);
        let registry_sub = cx.observe(&registry, |_, _, cx| cx.notify());

        self.service_tabs = Some(service_tabs);
        self.sidebar = Some(sidebar);
        self._subscriptions = vec![tabs_sub, sidebar_sub, registry_sub];
    }

    pub fn set_view(&mut self, view: WorkspaceView, workspace_id: Option<Uuid>, cx: &mut Context<Self>) {
        self.view = view;
        self.workspace_id = workspace_id;
        cx.notify();
    }

    fn create_service(&mut self, name: String, folder_id: Option<Uuid>, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.create_service(name, folder_id, cx);
        });
    }

    fn create_folder(&mut self, name: String, parent_id: Option<Uuid>, cx: &mut Context<Self>) {
        let registry = workspace::WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            registry.create_folder(name, parent_id, cx);
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
        if let Some(active_service_id) = workspace.active_service {
            if let Some(service) = workspace.services.iter().find(|s| s.id == active_service_id) {
                match &service.service_type {
                    ServiceType::Http(config) => {
                        if !self.http_services.contains_key(&service.id) {
                            let http_service =
                                cx.new(|cx| HttpService::new(config.clone(), window, cx));
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

            div()
                .h_flex()
                .size_full()
                .children(self.sidebar.clone())
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .v_flex()
                        .children(self.service_tabs.clone())
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
