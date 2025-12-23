use crate::{CollectionSidebar, CollectionSidebarEvent, HttpService, ServiceTabs, ServiceTabsEvent};
use gpui::*;
use gpui_component::*;
use models::{Collection, ServiceType};
use uuid::Uuid;

pub struct CollectionsView {
    workspace_id: Option<Uuid>,
    collection_id: Option<Uuid>,
    http_services: std::collections::HashMap<Uuid, Entity<HttpService>>,
    service_tabs: Entity<ServiceTabs>,
    sidebar: Entity<CollectionSidebar>,
    _subscriptions: Vec<Subscription>,
}

impl CollectionsView {
    pub fn new(workspace_id: Option<Uuid>, collection_id: Option<Uuid>, cx: &mut Context<Self>) -> Self {
        let service_tabs = cx.new(|_| ServiceTabs::new());
        let sidebar = cx.new(|_| CollectionSidebar::new());

        let tabs_sub = cx.subscribe(&service_tabs, |this, _, event, cx| match event {
            ServiceTabsEvent::Select(id) => this.set_active_service(*id, cx),
            ServiceTabsEvent::Close(id) => this.close_service(*id, cx),
        });

        let sidebar_sub = cx.subscribe(&sidebar, |this, _, event, cx| match event {
            CollectionSidebarEvent::OpenService(id) => this.open_service(*id, cx),
            CollectionSidebarEvent::CreateService(_folder_id) => {}
            CollectionSidebarEvent::CreateFolder(_parent_id) => {}
            CollectionSidebarEvent::ToggleFolder(id) => this.toggle_folder(*id, cx),
        });

        let registry = collections::CollectionRegistry::global(cx);
        let registry_sub = cx.observe(&registry, |_, _, cx| cx.notify());

        Self {
            workspace_id,
            collection_id,
            http_services: std::collections::HashMap::new(),
            service_tabs,
            sidebar,
            _subscriptions: vec![tabs_sub, sidebar_sub, registry_sub],
        }
    }

    fn open_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let (Some(workspace_id), Some(collection_id)) = (self.workspace_id, self.collection_id) {
            let registry = collections::CollectionRegistry::global(cx);
            registry.update(cx, |registry, cx| {
                registry.open_service(workspace_id, collection_id, service_id, cx);
            });
            cx.notify();
        }
    }

    fn close_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let (Some(workspace_id), Some(collection_id)) = (self.workspace_id, self.collection_id) {
            let registry = collections::CollectionRegistry::global(cx);
            registry.update(cx, |registry, cx| {
                registry.close_service(workspace_id, collection_id, service_id, cx);
            });
            self.http_services.remove(&service_id);
            cx.notify();
        }
    }

    fn set_active_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let (Some(workspace_id), Some(collection_id)) = (self.workspace_id, self.collection_id) {
            let registry = collections::CollectionRegistry::global(cx);
            registry.update(cx, |registry, cx| {
                registry.set_active_service(workspace_id, collection_id, service_id, cx);
            });
            cx.notify();
        }
    }

    fn toggle_folder(&mut self, folder_id: Uuid, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |sidebar, _| {
            if sidebar.expanded_folders.contains(&folder_id) {
                sidebar.expanded_folders.remove(&folder_id);
            } else {
                sidebar.expanded_folders.insert(folder_id);
            }
        });
        cx.notify();
    }

    fn render_service_content(
        &mut self,
        collection: &Collection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if let Some(active_service_id) = collection.active_service {
            if let Some(service) = collection
                .services
                .iter()
                .find(|s| s.id == active_service_id)
            {
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
                        .child("gRPC service configuration will be displayed here")
                        .into_any_element(),
                    ServiceType::WebSocket(_) => div()
                        .flex_1()
                        .h_full()
                        .p_4()
                        .child("WebSocket service configuration will be displayed here")
                        .into_any_element(),
                }
            } else {
                div()
                    .flex_1()
                    .h_full()
                    .p_4()
                    .child("Service not found")
                    .into_any_element()
            }
        } else {
            div()
                .flex_1()
                .h_full()
                .p_4()
                .items_center()
                .justify_center()
                .child("Select a service to view its configuration")
                .into_any_element()
        }
    }

    fn render_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(workspace_id) = self.workspace_id {
            let registry = collections::CollectionRegistry::global(cx);

            // Ensure workspace has collections
            registry.update(cx, |registry, cx| {
                registry.ensure_workspace_collections(workspace_id, cx);
            });

            // Get collections for this workspace
            let collections = registry.read(cx).get_collections(workspace_id);

            // If no collection is selected, use the first one
            let collection_id = self
                .collection_id
                .or_else(|| collections.first().map(|c| c.id));

            if let Some(collection_id) = collection_id {
                // Update our collection_id if it was None
                if self.collection_id.is_none() {
                    self.collection_id = Some(collection_id);
                }

                if let Some(collection) = collections.iter().find(|c| c.id == collection_id) {
                    self.sidebar.update(cx, |sidebar, _| {
                        sidebar.set_collection(collection.clone());
                    });
                    self.service_tabs.update(cx, |tabs, _| {
                        tabs.set_collection(collection.clone());
                    });

                    div()
                        .h_flex()
                        .size_full()
                        .child(self.sidebar.clone())
                        .child(
                            div()
                                .flex_1()
                                .h_full()
                                .v_flex()
                                .child(self.service_tabs.clone())
                                .child(self.render_service_content(collection, window, cx))
                        )
                } else {
                    div().p_4().child("Collection not found")
                }
            } else {
                div().p_4().child("No collections available")
            }
        } else {
            div().p_4().child("No workspace selected")
        }
    }
}

impl Render for CollectionsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_content(window, cx)
    }
}
