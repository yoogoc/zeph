use anyhow::Result;
use models::{DataSource, Service, ServiceType, SyncMeta};
use rusqlite::{params, Connection};
use std::sync::Arc;
use uuid::Uuid;

use crate::{PathParameterStore, QueryParameterStore};

pub struct ServiceStore {
    conn: Arc<Connection>,
}

impl ServiceStore {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    /// Validate service name
    /// Returns an error if the name is empty, only whitespace, or exceeds 255 characters
    fn validate_name(&self, name: &str) -> Result<()> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            anyhow::bail!("Service name cannot be empty or only whitespace");
        }
        if name.len() > 255 {
            anyhow::bail!("Service name cannot exceed 255 characters");
        }
        Ok(())
    }

    /// Validate service description
    /// Returns an error if the description exceeds 1000 characters
    fn validate_description(&self, description: &str) -> Result<()> {
        if description.len() > 1000 {
            anyhow::bail!("Service description cannot exceed 1000 characters");
        }
        Ok(())
    }

    pub fn insert(&self, service: &Service) -> Result<()> {
        // Validate name before inserting
        self.validate_name(service.name.as_ref())?;

        // Validate description if present
        if let Some(ref desc) = service.description {
            self.validate_description(desc.as_ref())?;
        }
        let (service_type, config) = serialize_service_type(&service.service_type)?;

        self.conn.execute(
            "INSERT INTO services (id, workspace_id, folder_id, name, description, service_type, config, deprecated, sort_order, created_at, updated_at, sync_version, source, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                service.id.to_string(),
                service.workspace_id.to_string(),
                service.folder_id.map(|id| id.to_string()),
                service.name.as_ref(),
                service.description.as_ref().map(|id| id.to_string()),
                service_type,
                config,
                service.deprecated,
                service.sort_order,
                service.created_at,
                service.updated_at,
                service.sync.sync_version,
                source_to_str(&service.sync.source),
                service.sync.deleted,
            ],
        )?;

        // Save query and path parameters for HTTP services
        if let ServiceType::Http(ref http_config) = service.service_type {
            let query_param_store = QueryParameterStore::new(self.conn.clone());
            let path_param_store = PathParameterStore::new(self.conn.clone());

            // Save query parameters
            for (index, param) in http_config.query_params.iter().enumerate() {
                query_param_store.create(
                    service.id,
                    param.key.as_ref(),
                    param.value.as_ref(),
                    param.enabled,
                    param.description.as_ref().map(|s| s.as_ref()),
                    index as i32,
                )?;
            }

            // Save path parameters
            for param in &http_config.path_params {
                path_param_store.create(
                    service.id,
                    param.name.as_ref(),
                    param.value.as_ref(),
                    param.description.as_ref().map(|s| s.as_ref()),
                )?;
            }
        }

        Ok(())
    }

    pub fn update(&self, service: &Service) -> Result<()> {
        // Validate name before updating
        self.validate_name(service.name.as_ref())?;

        // Validate description if present
        if let Some(ref desc) = service.description {
            self.validate_description(desc.as_ref())?;
        }

        let (service_type, config) = serialize_service_type(&service.service_type)?;

        // Ensure updated_at is set to current time
        let updated_at = chrono::Utc::now().timestamp();

        self.conn.execute(
            "UPDATE services SET folder_id = ?2, name = ?3, description = ?4, service_type = ?5, config = ?6, deprecated = ?7, sort_order = ?8, updated_at = ?9, sync_version = ?10
             WHERE id = ?1 AND deleted = 0",
            params![
                service.id.to_string(),
                service.folder_id.map(|id| id.to_string()),
                service.name.as_ref(),
                service.description.as_ref().map(|id| id.to_string()),
                service_type,
                config,
                service.deprecated,
                service.sort_order,
                updated_at,
                service.sync.sync_version,
            ],
        )?;

        // Update query and path parameters for HTTP services
        if let ServiceType::Http(ref http_config) = service.service_type {
            let query_param_store = QueryParameterStore::new(self.conn.clone());
            let path_param_store = PathParameterStore::new(self.conn.clone());

            // Delete old parameters
            query_param_store.delete_by_service(service.id)?;
            path_param_store.delete_by_service(service.id)?;

            // Insert new query parameters
            for (index, param) in http_config.query_params.iter().enumerate() {
                query_param_store.create(
                    service.id,
                    param.key.as_ref(),
                    param.value.as_ref(),
                    param.enabled,
                    param.description.as_ref().map(|s| s.as_ref()),
                    index as i32,
                )?;
            }

            // Insert new path parameters
            for param in &http_config.path_params {
                path_param_store.create(
                    service.id,
                    param.name.as_ref(),
                    param.value.as_ref(),
                    param.description.as_ref().map(|s| s.as_ref()),
                )?;
            }
        }

        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE services SET deleted = 1, updated_at = ?2 WHERE id = ?1",
            params![id.to_string(), now],
        )?;
        Ok(())
    }

    pub fn get(&self, id: Uuid) -> Result<Option<Service>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, folder_id, name, description, service_type, config, deprecated, sort_order, created_at, updated_at, sync_version, source, deleted
             FROM services WHERE id = ?1 AND deleted = 0",
        )?;

        let mut rows = stmt.query(params![id.to_string()])?;
        if let Some(row) = rows.next()? {
            let mut service = row_to_service(row)?;

            // Load query and path parameters for HTTP services
            if let ServiceType::Http(ref mut http_config) = service.service_type {
                let query_param_store = QueryParameterStore::new(self.conn.clone());
                let path_param_store = PathParameterStore::new(self.conn.clone());

                http_config.query_params = query_param_store.list_by_service(id)?;
                http_config.path_params = path_param_store.list_by_service(id)?;
            }

            Ok(Some(service))
        } else {
            Ok(None)
        }
    }

    pub fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<Service>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, folder_id, name, description, service_type, config, deprecated, sort_order, created_at, updated_at, sync_version, source, deleted
             FROM services WHERE workspace_id = ?1 AND deleted = 0 ORDER BY sort_order, name",
        )?;

        let rows = stmt.query_map(params![workspace_id.to_string()], |row| {
            Ok(row_to_service(row).unwrap())
        })?;

        let mut services: Vec<Service> = rows.filter_map(|r| r.ok()).collect();

        // Load query and path parameters for all HTTP services
        let query_param_store = QueryParameterStore::new(self.conn.clone());
        let path_param_store = PathParameterStore::new(self.conn.clone());

        for service in &mut services {
            if let ServiceType::Http(ref mut http_config) = service.service_type {
                http_config.query_params = query_param_store
                    .list_by_service(service.id)
                    .unwrap_or_default();
                http_config.path_params = path_param_store
                    .list_by_service(service.id)
                    .unwrap_or_default();
            }
        }

        Ok(services)
    }
}

fn row_to_service(row: &rusqlite::Row) -> Result<Service> {
    let id: String = row.get(0)?;
    let workspace_id: String = row.get(1)?;
    let folder_id: Option<String> = row.get(2)?;
    let name: String = row.get(3)?;
    let description: Option<String> = row.get(4)?;
    let service_type_str: String = row.get(5)?;
    let config: String = row.get(6)?;
    let deprecated: bool = row.get(7)?;
    let sort_order: i32 = row.get(8)?;
    let created_at: i64 = row.get(9)?;
    let updated_at: i64 = row.get(10)?;
    let sync_version: i64 = row.get(11)?;
    let source: String = row.get(12)?;
    let deleted: bool = row.get(13)?;

    let service_type = deserialize_service_type(&service_type_str, &config)?;

    Ok(Service {
        id: Uuid::parse_str(&id)?,
        workspace_id: Uuid::parse_str(&workspace_id)?,
        folder_id: folder_id.map(|s| Uuid::parse_str(&s).unwrap()),
        name: name.into(),
        description: description.map(|id| id.into()),
        service_type,
        deprecated,
        sort_order,
        created_at,
        updated_at,
        sync: SyncMeta {
            sync_version,
            source: str_to_source(&source),
            deleted,
        },
    })
}

fn serialize_service_type(st: &ServiceType) -> Result<(&'static str, String)> {
    match st {
        ServiceType::Http(config) => Ok(("http", serde_json::to_string(config)?)),
        ServiceType::Grpc(config) => Ok(("grpc", serde_json::to_string(config)?)),
        ServiceType::WebSocket(config) => Ok(("websocket", serde_json::to_string(config)?)),
    }
}

fn deserialize_service_type(type_str: &str, config: &str) -> Result<ServiceType> {
    match type_str {
        "http" => Ok(ServiceType::Http(serde_json::from_str(config)?)),
        "grpc" => Ok(ServiceType::Grpc(serde_json::from_str(config)?)),
        "websocket" => Ok(ServiceType::WebSocket(serde_json::from_str(config)?)),
        _ => anyhow::bail!("Unknown service type: {}", type_str),
    }
}

fn source_to_str(source: &DataSource) -> &'static str {
    match source {
        DataSource::Local => "local",
        DataSource::Remote => "remote",
    }
}

fn str_to_source(s: &str) -> DataSource {
    match s {
        "remote" => DataSource::Remote,
        _ => DataSource::Local,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use models::{HttpConfig, ServiceType};

    #[test]
    fn test_validate_name_empty_string() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());

        let result = store.validate_name("");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be empty or only whitespace"));
    }

    #[test]
    fn test_validate_name_only_whitespace() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());

        let result = store.validate_name("   ");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be empty or only whitespace"));
    }

    #[test]
    fn test_validate_name_tabs_and_newlines() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());

        let result = store.validate_name("\t\n  \t");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be empty or only whitespace"));
    }

    #[test]
    fn test_validate_name_exceeds_255_chars() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());

        let long_name = "a".repeat(256);
        let result = store.validate_name(&long_name);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot exceed 255 characters"));
    }

    #[test]
    fn test_validate_name_exactly_255_chars() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());

        let name = "a".repeat(255);
        let result = store.validate_name(&name);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_name_valid() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());

        let result = store.validate_name("Valid Service Name");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_description_exceeds_1000_chars() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());

        let long_desc = "a".repeat(1001);
        let result = store.validate_description(&long_desc);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot exceed 1000 characters"));
    }

    #[test]
    fn test_validate_description_exactly_1000_chars() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());

        let desc = "a".repeat(1000);
        let result = store.validate_description(&desc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_insert_service_with_empty_name_fails() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        let mut service = Service::new(
            workspace_id,
            "Valid Name",
            ServiceType::Http(HttpConfig::default()),
        );
        service.name = "".into();

        let result = store.insert(&service);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be empty or only whitespace"));
    }

    #[test]
    fn test_insert_service_with_whitespace_name_fails() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        let mut service = Service::new(
            workspace_id,
            "Valid Name",
            ServiceType::Http(HttpConfig::default()),
        );
        service.name = "   ".into();

        let result = store.insert(&service);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be empty or only whitespace"));
    }

    #[test]
    fn test_insert_service_with_long_name_fails() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        let long_name = "a".repeat(256);
        let mut service = Service::new(
            workspace_id,
            "Valid Name",
            ServiceType::Http(HttpConfig::default()),
        );
        service.name = long_name.into();

        let result = store.insert(&service);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot exceed 255 characters"));
    }

    #[test]
    fn test_update_service_with_empty_name_fails() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a valid service first
        let service = Service::new(
            workspace_id,
            "Valid Name",
            ServiceType::Http(HttpConfig::default()),
        );
        store.insert(&service).expect("Failed to insert service");

        // Try to update with empty name
        let mut updated_service = service.clone();
        updated_service.name = "".into();

        let result = store.update(&updated_service);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be empty or only whitespace"));
    }

    #[test]
    fn test_update_service_with_whitespace_name_fails() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a valid service first
        let service = Service::new(
            workspace_id,
            "Valid Name",
            ServiceType::Http(HttpConfig::default()),
        );
        store.insert(&service).expect("Failed to insert service");

        // Try to update with whitespace name
        let mut updated_service = service.clone();
        updated_service.name = "   \t\n  ".into();

        let result = store.update(&updated_service);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be empty or only whitespace"));
    }

    #[test]
    fn test_update_service_name_success() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a service
        let service = Service::new(
            workspace_id,
            "Original Name",
            ServiceType::Http(HttpConfig::default()),
        );
        let service_id = service.id;
        store.insert(&service).expect("Failed to insert service");

        // Update the name
        let mut updated_service = service.clone();
        updated_service.name = "New Name".into();

        let result = store.update(&updated_service);
        assert!(result.is_ok());

        // Verify the name was updated
        let loaded_service = store
            .get(service_id)
            .expect("Failed to get service")
            .expect("Service not found");
        assert_eq!(loaded_service.name.as_ref(), "New Name");
    }

    #[test]
    fn test_update_service_updates_timestamp() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a service with a specific timestamp
        let mut service = Service::new(
            workspace_id,
            "Test Service",
            ServiceType::Http(HttpConfig::default()),
        );
        let service_id = service.id;
        // Set an old timestamp
        service.updated_at = chrono::Utc::now().timestamp() - 100;
        let original_updated_at = service.updated_at;
        store.insert(&service).expect("Failed to insert service");

        // Update the service
        let mut updated_service = service.clone();
        updated_service.name = "Updated Name".into();

        store
            .update(&updated_service)
            .expect("Failed to update service");

        // Verify the timestamp was updated to current time
        let loaded_service = store
            .get(service_id)
            .expect("Failed to get service")
            .expect("Service not found");
        assert!(loaded_service.updated_at > original_updated_at);
    }

    #[test]
    fn test_insert_service_with_long_description_fails() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        let long_desc = "a".repeat(1001);
        let mut service = Service::new(
            workspace_id,
            "Valid Name",
            ServiceType::Http(HttpConfig::default()),
        );
        service.description = Some(long_desc.into());

        let result = store.insert(&service);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot exceed 1000 characters"));
    }

    #[test]
    fn test_update_service_with_long_description_fails() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a valid service first
        let service = Service::new(
            workspace_id,
            "Valid Name",
            ServiceType::Http(HttpConfig::default()),
        );
        store.insert(&service).expect("Failed to insert service");

        // Try to update with long description
        let long_desc = "a".repeat(1001);
        let mut updated_service = service.clone();
        updated_service.description = Some(long_desc.into());

        let result = store.update(&updated_service);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot exceed 1000 characters"));
    }
}
