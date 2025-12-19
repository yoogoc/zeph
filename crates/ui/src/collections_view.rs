use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::{button::{Button, ButtonVariants}, *};
use models::{Collection, Service, ServiceType};
use uuid::Uuid;

pub struct CollectionsView {
    workspace_id: Option<Uuid>,
    collection_id: Option<Uuid>,
    search_query: String,
}

impl CollectionsView {
    pub fn new(workspace_id: Option<Uuid>, collection_id: Option<Uuid>) -> Self {
        Self {
            workspace_id,
            collection_id,
            search_query: String::new(),
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
    
    fn render_service_item(&mut self, service: &Service, collection: &Collection, cx: &mut Context<Self>) -> impl IntoElement {
        let service_id = service.id;
        let is_active = collection.active_service == Some(service_id);
        let method_color = match &service.service_type {
            ServiceType::Http(config) => match config.method {
                models::HttpMethod::Get => gpui::rgb(0x28a745),
                models::HttpMethod::Post => gpui::rgb(0x007bff),
                models::HttpMethod::Put => gpui::rgb(0xffc107),
                models::HttpMethod::Delete => gpui::rgb(0xdc3545),
                _ => gpui::rgb(0x6c757d),
            },
            _ => gpui::rgb(0x6c757d),
        };
        
        Button::new(format!("service_{}", service_id))
            .w_full()
            .justify_start()
            .when(is_active, |btn: Button| btn.primary())
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .bg(method_color)
                            .rounded_full()
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(service.name.clone())
                    )
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_service(service_id, cx);
            }))
    }
    
    fn render_sidebar(&mut self, collection: &Collection, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered_services: Vec<&Service> = collection.services.iter()
            .filter(|service| {
                self.search_query.is_empty() || 
                service.name.to_lowercase().contains(&self.search_query.to_lowercase())
            })
            .collect();
            
        div()
            .w(px(300.0))
            .h_full()
            .bg(gpui::rgb(0xf8f9fa))
            .border_r_1()
            .border_color(gpui::rgb(0xe0e0e0))
            .p_4()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .p_2()
                                    .bg(gpui::rgb(0xffffff))
                                    .border_1()
                                    .border_color(gpui::rgb(0xdee2e6))
                                    .rounded_md()
                                    .child("Search services...")
                            )
                            .child(
                                Button::new("add_service")
                                    .primary()
                                    .label("+ New")
                                    .on_click(|_, _, _| println!("Add service clicked"))
                            )
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .children(
                                filtered_services.into_iter().map(|service| {
                                    self.render_service_item(service, collection, cx)
                                })
                            )
                    )
            )
    }
    
    fn render_service_tabs(&mut self, collection: &Collection, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h_flex()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .bg(gpui::rgb(0xf8f9fa))
            .border_b_1()
            .border_color(gpui::rgb(0xe0e0e0))
            .children(
                collection.opened_services.iter().filter_map(|&service_id| {
                    collection.services.iter().find(|s| s.id == service_id).map(|service| {
                        let is_active = collection.active_service == Some(service_id);
                        
                        div()
                            .h_flex()
                            .items_center()
                            .gap_1()
                            .max_w(px(200.0)) // Limit maximum width
                            .child(
                                Button::new(format!("tab_{}", service_id))
                                    .px_3()
                                    .py_2()
                                    .flex_shrink()
                                    .min_w(px(80.0))
                                    .max_w(px(160.0))
                                    .label(service.name.clone())
                                    .when(is_active, |btn: Button| btn.primary())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_active_service(service_id, cx);
                                    }))
                            )
                            .child(
                                Button::new(format!("close_{}", service_id))
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .flex_shrink_0()
                                    .label("×")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.close_service(service_id, cx);
                                    }))
                            )
                    })
                })
            )
    }
    
    fn render_service_content(&self, collection: &Collection) -> impl IntoElement {
        if let Some(active_service_id) = collection.active_service {
            if let Some(service) = collection.services.iter().find(|s| s.id == active_service_id) {
                match &service.service_type {
                    ServiceType::Http(config) => {
                        div()
                            .flex_1()
                            .h_full()
                            .p_4()
                            .v_flex()
                            .gap_4()
                            .child(
                                div()
                                    .text_xl()
                                    .font_bold()
                                    .child(format!("{} {}", 
                                        match config.method {
                                            models::HttpMethod::Get => "GET",
                                            models::HttpMethod::Post => "POST",
                                            models::HttpMethod::Put => "PUT",
                                            models::HttpMethod::Delete => "DELETE",
                                            _ => "HTTP",
                                        },
                                        service.name
                                    ))
                            )
                            .child(
                                div()
                                    .p_4()
                                    .bg(gpui::rgb(0xf8f9fa))
                                    .border_1()
                                    .border_color(gpui::rgb(0xe0e0e0))
                                    .rounded_lg()
                                    .child(format!("URL: {}", config.url))
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .p_4()
                                    .bg(gpui::rgb(0xfafafa))
                                    .border_1()
                                    .border_color(gpui::rgb(0xe0e0e0))
                                    .rounded_lg()
                                    .child("HTTP request configuration will be displayed here")
                            )
                    }
                    ServiceType::Grpc(_) => {
                        div()
                            .flex_1()
                            .h_full()
                            .p_4()
                            .child("gRPC service configuration will be displayed here")
                    }
                    ServiceType::WebSocket(_) => {
                        div()
                            .flex_1()
                            .h_full()
                            .p_4()
                            .child("WebSocket service configuration will be displayed here")
                    }
                }
            } else {
                div()
                    .flex_1()
                    .h_full()
                    .p_4()
                    .child("Service not found")
            }
        } else {
            div()
                .flex_1()
                .h_full()
                .p_4()
                .items_center()
                .justify_center()
                .child("Select a service to view its configuration")
        }
    }
    
    fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(workspace_id) = self.workspace_id {
            let registry = collections::CollectionRegistry::global(cx);
            
            // Ensure workspace has collections
            registry.update(cx, |registry, cx| {
                registry.ensure_workspace_collections(workspace_id, cx);
            });
            
            // Get collections for this workspace
            let collections = registry.read(cx).get_collections(workspace_id);
            
            // If no collection is selected, use the first one
            let collection_id = self.collection_id.or_else(|| collections.first().map(|c| c.id));
            
            if let Some(collection_id) = collection_id {
                // Update our collection_id if it was None
                if self.collection_id.is_none() {
                    self.collection_id = Some(collection_id);
                }
                
                if let Some(collection) = collections.iter().find(|c| c.id == collection_id) {
                    div()
                        .h_flex()
                        .size_full()
                        .child(self.render_sidebar(collection, cx))
                        .child(
                            div()
                                .flex_1()
                                .h_full()
                                .v_flex()
                                .child(self.render_service_tabs(collection, cx))
                                .child(self.render_service_content(collection))
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

impl Default for CollectionsView {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl Render for CollectionsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Subscribe to registry updates to ensure UI refreshes when services are opened/closed
        if let (Some(workspace_id), Some(collection_id)) = (self.workspace_id, self.collection_id) {
            let registry = collections::CollectionRegistry::global(cx);
            let _collection = registry.read(cx).get_collection(workspace_id, collection_id);
        }
        
        self.render_content(cx)
    }
}
