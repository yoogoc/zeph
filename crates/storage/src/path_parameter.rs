use anyhow::Result;
use models::PathParameter;
use rusqlite::{params, Connection};
use std::sync::Arc;
use uuid::Uuid;

pub struct PathParameterStore {
    conn: Arc<Connection>,
}

impl PathParameterStore {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    /// Create a new path parameter for a service
    pub fn create(
        &self,
        service_id: Uuid,
        name: &str,
        value: &str,
        description: Option<&str>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO path_parameters (id, service_id, name, value, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id.to_string(),
                service_id.to_string(),
                name,
                value,
                description,
                now,
                now,
            ],
        )?;

        Ok(id)
    }

    /// Update an existing path parameter
    pub fn update(
        &self,
        id: Uuid,
        name: &str,
        value: &str,
        description: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "UPDATE path_parameters SET name = ?2, value = ?3, description = ?4, updated_at = ?5
             WHERE id = ?1",
            params![id.to_string(), name, value, description, now,],
        )?;

        Ok(())
    }

    /// Delete a path parameter
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM path_parameters WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Delete all path parameters for a service
    pub fn delete_by_service(&self, service_id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM path_parameters WHERE service_id = ?1",
            params![service_id.to_string()],
        )?;
        Ok(())
    }

    /// List all path parameters for a service
    pub fn list_by_service(&self, service_id: Uuid) -> Result<Vec<PathParameter>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, value, description
             FROM path_parameters
             WHERE service_id = ?1
             ORDER BY name",
        )?;

        let rows = stmt.query_map(params![service_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let description: Option<String> = row.get(3)?;

            Ok(PathParameter {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                name: row.get::<_, String>(1)?.into(),
                value: row.get::<_, String>(2)?.into(),
                description: description.map(|s| s.into()),
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, ServiceStore};
    use models::{HttpConfig, Service, ServiceType};

    fn create_test_service(db: &Database) -> Uuid {
        let workspace_id = Uuid::new_v4();
        let service = Service::new(
            workspace_id,
            "Test Service",
            ServiceType::Http(HttpConfig::default()),
        );
        let service_id = service.id;
        let service_store = ServiceStore::new(db.conn());
        service_store
            .insert(&service)
            .expect("Failed to insert service");
        service_id
    }

    #[test]
    fn test_create_path_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        let param_id = store
            .create(service_id, "userId", "123", Some("User identifier"))
            .expect("Failed to create path parameter");

        assert_ne!(param_id, Uuid::nil());

        // Verify it was created
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name.as_ref(), "userId");
        assert_eq!(params[0].value.as_ref(), "123");
        assert_eq!(
            params[0].description.as_ref().unwrap().as_ref(),
            "User identifier"
        );
    }

    #[test]
    fn test_update_path_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        let param_id = store
            .create(service_id, "userId", "123", None)
            .expect("Failed to create path parameter");

        // Update the parameter
        store
            .update(param_id, "userId", "456", Some("Updated user ID"))
            .expect("Failed to update path parameter");

        // Verify the update
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name.as_ref(), "userId");
        assert_eq!(params[0].value.as_ref(), "456");
        assert_eq!(
            params[0].description.as_ref().unwrap().as_ref(),
            "Updated user ID"
        );
    }

    #[test]
    fn test_delete_path_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        let param_id = store
            .create(service_id, "userId", "123", None)
            .expect("Failed to create path parameter");

        // Delete the parameter
        store
            .delete(param_id)
            .expect("Failed to delete path parameter");

        // Verify it was deleted
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_list_by_service_multiple_params() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        // Create multiple parameters
        store
            .create(service_id, "userId", "123", None)
            .expect("Failed to create parameter");
        store
            .create(service_id, "postId", "456", None)
            .expect("Failed to create parameter");
        store
            .create(service_id, "commentId", "789", None)
            .expect("Failed to create parameter");

        // List all parameters
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 3);
        // Should be sorted by name
        assert_eq!(params[0].name.as_ref(), "commentId");
        assert_eq!(params[1].name.as_ref(), "postId");
        assert_eq!(params[2].name.as_ref(), "userId");
    }

    #[test]
    fn test_list_by_service_empty() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = Uuid::new_v4();

        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_delete_by_service() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        // Create multiple parameters
        store
            .create(service_id, "userId", "123", None)
            .expect("Failed to create parameter");
        store
            .create(service_id, "postId", "456", None)
            .expect("Failed to create parameter");

        // Delete all parameters for the service
        store
            .delete_by_service(service_id)
            .expect("Failed to delete parameters");

        // Verify all were deleted
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_path_parameter_without_description() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        store
            .create(service_id, "id", "999", None)
            .expect("Failed to create parameter");

        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name.as_ref(), "id");
        assert_eq!(params[0].value.as_ref(), "999");
        assert!(params[0].description.is_none());
    }

    #[test]
    fn test_path_parameter_name_sorting() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        // Create parameters in non-alphabetical order
        store
            .create(service_id, "zebra", "z", None)
            .expect("Failed to create parameter");
        store
            .create(service_id, "alpha", "a", None)
            .expect("Failed to create parameter");
        store
            .create(service_id, "beta", "b", None)
            .expect("Failed to create parameter");

        // Verify they are returned in alphabetical order
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].name.as_ref(), "alpha");
        assert_eq!(params[1].name.as_ref(), "beta");
        assert_eq!(params[2].name.as_ref(), "zebra");
    }

    #[test]
    fn test_path_parameter_empty_value() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = PathParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        // Create parameter with empty value
        store
            .create(service_id, "emptyParam", "", None)
            .expect("Failed to create parameter");

        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name.as_ref(), "emptyParam");
        assert_eq!(params[0].value.as_ref(), "");
    }
}
