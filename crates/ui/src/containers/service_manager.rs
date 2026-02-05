//! Service management module for WorkspaceContent
//!
//! This module manages HTTP service entity cache.

use crate::containers::HttpService;
use gpui::*;
use models::{ContentType, HttpConfig, ServiceType};
use std::collections::HashMap;
use uuid::Uuid;

/// Manages HTTP service instances and their lifecycle
pub struct ServiceManager {
    /// Cache of HTTP service entities by service ID
    http_services: HashMap<Uuid, Entity<HttpService>>,
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
        }
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

    /// Checks if a service has unsaved changes
    pub fn has_unsaved_changes(&self, service_id: &Uuid, cx: &App) -> bool {
        if let Some(http_service) = self.http_services.get(service_id) {
            http_service.read(cx).is_modified(cx)
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

    /// Ensures HTTP service entity exists for the given content (creates if needed)
    pub fn ensure_http_service(&mut self, ct: &ContentType, window: &mut Window, cx: &mut App) {
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
    }
}
