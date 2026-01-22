use anyhow::Result;
use models::QueryParameter;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct QueryParameterStore<'a> {
    conn: &'a Connection,
}

impl<'a> QueryParameterStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new query parameter for a service
    pub fn create(
        &self,
        service_id: Uuid,
        key: &str,
        value: &str,
        enabled: bool,
        description: Option<&str>,
        sort_order: i32,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO query_parameters (id, service_id, key, value, enabled, description, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id.to_string(),
                service_id.to_string(),
                key,
                value,
                enabled as i32,
                description,
                sort_order,
                now,
                now,
            ],
        )?;

        Ok(id)
    }

    /// Update an existing query parameter
    pub fn update(
        &self,
        id: Uuid,
        key: &str,
        value: &str,
        enabled: bool,
        description: Option<&str>,
        sort_order: i32,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "UPDATE query_parameters SET key = ?2, value = ?3, enabled = ?4, description = ?5, sort_order = ?6, updated_at = ?7
             WHERE id = ?1",
            params![
                id.to_string(),
                key,
                value,
                enabled as i32,
                description,
                sort_order,
                now,
            ],
        )?;

        Ok(())
    }

    /// Delete a query parameter
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM query_parameters WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Delete all query parameters for a service
    pub fn delete_by_service(&self, service_id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM query_parameters WHERE service_id = ?1",
            params![service_id.to_string()],
        )?;
        Ok(())
    }

    /// List all query parameters for a service
    pub fn list_by_service(&self, service_id: Uuid) -> Result<Vec<QueryParameter>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, key, value, enabled, description, sort_order
             FROM query_parameters
             WHERE service_id = ?1
             ORDER BY sort_order, key",
        )?;

        let rows = stmt.query_map(params![service_id.to_string()], |row| {
            let enabled: i32 = row.get(3)?;
            let description: Option<String> = row.get(4)?;

            Ok(QueryParameter {
                key: row.get::<_, String>(1)?.into(),
                value: row.get::<_, String>(2)?.into(),
                enabled: enabled != 0,
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
    fn test_create_query_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = QueryParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        let param_id = store
            .create(service_id, "limit", "10", true, Some("Page limit"), 0)
            .expect("Failed to create query parameter");

        assert_ne!(param_id, Uuid::nil());

        // Verify it was created
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].key.as_ref(), "limit");
        assert_eq!(params[0].value.as_ref(), "10");
        assert!(params[0].enabled);
        assert_eq!(
            params[0].description.as_ref().unwrap().as_ref(),
            "Page limit"
        );
    }

    #[test]
    fn test_update_query_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = QueryParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        let param_id = store
            .create(service_id, "limit", "10", true, None, 0)
            .expect("Failed to create query parameter");

        // Update the parameter
        store
            .update(param_id, "limit", "20", false, Some("Updated limit"), 1)
            .expect("Failed to update query parameter");

        // Verify the update
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].key.as_ref(), "limit");
        assert_eq!(params[0].value.as_ref(), "20");
        assert!(!params[0].enabled);
        assert_eq!(
            params[0].description.as_ref().unwrap().as_ref(),
            "Updated limit"
        );
    }

    #[test]
    fn test_delete_query_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = QueryParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        let param_id = store
            .create(service_id, "limit", "10", true, None, 0)
            .expect("Failed to create query parameter");

        // Delete the parameter
        store
            .delete(param_id)
            .expect("Failed to delete query parameter");

        // Verify it was deleted
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_list_by_service_multiple_params() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = QueryParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        // Create multiple parameters
        store
            .create(service_id, "limit", "10", true, None, 0)
            .expect("Failed to create parameter");
        store
            .create(service_id, "page", "1", true, None, 1)
            .expect("Failed to create parameter");
        store
            .create(service_id, "sort", "asc", false, None, 2)
            .expect("Failed to create parameter");

        // List all parameters
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].key.as_ref(), "limit");
        assert_eq!(params[1].key.as_ref(), "page");
        assert_eq!(params[2].key.as_ref(), "sort");
        assert!(params[0].enabled);
        assert!(params[1].enabled);
        assert!(!params[2].enabled);
    }

    #[test]
    fn test_list_by_service_empty() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = QueryParameterStore::new(db.conn());
        let service_id = Uuid::new_v4();

        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_delete_by_service() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = QueryParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        // Create multiple parameters
        store
            .create(service_id, "limit", "10", true, None, 0)
            .expect("Failed to create parameter");
        store
            .create(service_id, "page", "1", true, None, 1)
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
    fn test_enabled_state_persistence() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = QueryParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        // Create enabled parameter
        store
            .create(service_id, "enabled_param", "value1", true, None, 0)
            .expect("Failed to create parameter");

        // Create disabled parameter
        store
            .create(service_id, "disabled_param", "value2", false, None, 1)
            .expect("Failed to create parameter");

        // Verify enabled states
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 2);
        assert!(params[0].enabled);
        assert!(!params[1].enabled);
    }

    #[test]
    fn test_sort_order_preserved() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = QueryParameterStore::new(db.conn());
        let service_id = create_test_service(&db);

        // Create parameters with specific sort order
        store
            .create(service_id, "third", "3", true, None, 2)
            .expect("Failed to create parameter");
        store
            .create(service_id, "first", "1", true, None, 0)
            .expect("Failed to create parameter");
        store
            .create(service_id, "second", "2", true, None, 1)
            .expect("Failed to create parameter");

        // Verify they are returned in sort order
        let params = store
            .list_by_service(service_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].key.as_ref(), "first");
        assert_eq!(params[1].key.as_ref(), "second");
        assert_eq!(params[2].key.as_ref(), "third");
    }
}
