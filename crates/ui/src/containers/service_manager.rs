//! Service management module for WorkspaceContent
//!
//! This module encapsulates service lifecycle management (create, open, close, rename)
//! and related state, reducing the complexity of WorkspaceContent.

use crate::containers::HttpService;
use crate::dialogs::ConfirmDialog;
use gpui::*;
#[allow(unused_imports)]
use models::Workspace;
use models::{ContentType, HttpConfig, ServiceType};
use std::collections::HashMap;
use uuid::Uuid;

/// Manages HTTP service instances and their lifecycle
pub struct ServiceManager {
    /// Cache of HTTP service entities by service ID
    http_services: HashMap<Uuid, Entity<HttpService>>,
    /// Current active service/content
    current_content: Option<ContentType>,
    /// Confirmation dialog for unsaved changes
    confirm_dialog: Option<Entity<ConfirmDialog>>,
    /// Whether the confirmation dialog is visible
    show_confirm_dialog: bool,
    /// Content pending close (waiting for user confirmation)
    pending_close_content: Option<ContentType>,
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceManager {
    /// Creates a new ServiceManager
    pub fn new() -> Self {
        Self {
            http_services: HashMap::new(),
            current_content: None,
            confirm_dialog: None,
            show_confirm_dialog: false,
            pending_close_content: None,
        }
    }

    /// Returns the current active content
    pub fn current_content(&self) -> Option<&ContentType> {
        self.current_content.as_ref()
    }

    /// Sets the current active content
    pub fn set_current_content(&mut self, content: Option<ContentType>) {
        self.current_content = content;
    }

    /// Returns a reference to the HTTP services map
    #[allow(unused)]
    pub fn http_services(&self) -> &HashMap<Uuid, Entity<HttpService>> {
        &self.http_services
    }

    /// Returns a mutable reference to the HTTP services map
    #[allow(unused)]
    pub fn http_services_mut(&mut self) -> &mut HashMap<Uuid, Entity<HttpService>> {
        &mut self.http_services
    }

    /// Gets an HTTP service by ID
    pub fn get_http_service(&self, id: &Uuid) -> Option<&Entity<HttpService>> {
        self.http_services.get(id)
    }

    /// Inserts an HTTP service
    #[allow(unused)]
    pub fn insert_http_service(&mut self, id: Uuid, service: Entity<HttpService>) {
        self.http_services.insert(id, service);
    }

    /// Removes an HTTP service by ID
    pub fn remove_http_service(&mut self, id: &Uuid) -> Option<Entity<HttpService>> {
        self.http_services.remove(id)
    }

    /// Clears all HTTP services
    pub fn clear_http_services(&mut self) {
        self.http_services.clear();
    }

    /// Checks if the confirmation dialog is visible
    pub fn is_confirm_dialog_visible(&self) -> bool {
        self.show_confirm_dialog
    }

    /// Returns a reference to the confirmation dialog if it exists
    pub fn confirm_dialog(&self) -> Option<&Entity<ConfirmDialog>> {
        self.confirm_dialog.as_ref()
    }

    /// Returns the pending close content
    #[allow(unused)]
    pub fn pending_close_content(&self) -> Option<&ContentType> {
        self.pending_close_content.as_ref()
    }

    /// Takes the pending close content, leaving None in its place
    pub fn take_pending_close_content(&mut self) -> Option<ContentType> {
        self.pending_close_content.take()
    }

    /// Sets up state for showing the confirmation dialog
    /// The dialog entity should be created and subscribed by the caller
    pub fn show_close_confirmation(&mut self, ct: ContentType, dialog: Entity<ConfirmDialog>) {
        self.confirm_dialog = Some(dialog);
        self.show_confirm_dialog = true;
        self.pending_close_content = Some(ct);
    }

    /// Hides the confirmation dialog
    pub fn hide_confirm_dialog(&mut self) {
        self.confirm_dialog = None;
        self.show_confirm_dialog = false;
        self.pending_close_content = None;
    }

    /// Checks if a service has unsaved changes
    pub fn has_unsaved_changes(&self, service_id: &Uuid, cx: &App) -> bool {
        if let Some(http_service) = self.http_services.get(service_id) {
            http_service.read(cx).is_modified()
        } else {
            false
        }
    }

    /// Saves changes for a service
    pub fn save_service_changes(&self, service_id: &Uuid, cx: &mut App) {
        if let Some(http_service) = self.http_services.get(service_id) {
            http_service.update(cx, |service, cx| {
                service.save_changes_no_window(cx);
            });
        }
    }

    /// Creates an HTTP service entity for the given service configuration
    pub fn create_http_service(
        &mut self,
        service_id: Uuid,
        config: HttpConfig,
        folder_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<HttpService> {
        let http_service = cx.new(|cx| HttpService::new(service_id, config, folder_id, window, cx));
        self.http_services.insert(service_id, http_service.clone());
        http_service
    }

    /// Handles content type switching and creates service instances as needed
    pub fn set_content(
        &mut self,
        _workspace: &Workspace,
        ct: &ContentType,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.current_content = Some(ct.clone());

        if let ContentType::Service(service) = ct {
            if let ServiceType::Http(config) = &service.service_type {
                if !self.http_services.contains_key(&service.id) {
                    self.create_http_service(
                        service.id,
                        config.clone(),
                        service.folder_id,
                        window,
                        cx,
                    );
                }
            }
        }
        // No HTTP service creation needed for ServiceFolder, SchemaFolder, or Schema types
    }
}
