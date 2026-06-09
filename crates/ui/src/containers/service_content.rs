//! Service content renderer for WorkspaceContent
//!
//! Renders the active content area (HTTP/gRPC/WebSocket service, schema, folder)
//! based on the current `ServiceState`.

use crate::containers::FolderDetailPage;
use crate::state::ServiceState;
use gpui::*;
use gpui_component::h_flex;
use models::{ContentType, ServiceType};

/// Renders the main content area for the currently active service/content.
///
/// Re-renders automatically when `service_state` changes: GPUI tracks the
/// entities `read` during `render` and invalidates this view on their `notify`.
pub struct ServiceContent {
    service_state: Entity<ServiceState>,
    folder_detail: Entity<FolderDetailPage>,
}

impl ServiceContent {
    pub fn new(service_state: Entity<ServiceState>, cx: &mut Context<Self>) -> Self {
        let folder_detail = cx.new(|cx| FolderDetailPage::new(service_state.clone(), cx));

        Self {
            service_state,
            folder_detail,
        }
    }
}

impl Render for ServiceContent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_service = self.service_state.read(cx).active_service.clone();

        match active_service.as_ref() {
            Some(content) => match content {
                ContentType::Service(service) => match &service.service_type {
                    ServiceType::Http(_) => self
                        .service_state
                        .read(cx)
                        .active_http_service()
                        .map(|service| service.into_any_element())
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
}
