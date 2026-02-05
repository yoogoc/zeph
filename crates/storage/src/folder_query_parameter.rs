use anyhow::Result;
use models::QueryParameter;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct FolderQueryParameterStore<'a> {
    conn: &'a Connection,
}

impl<'a> FolderQueryParameterStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new query parameter for a folder
    pub fn create(
        &self,
        folder_id: Uuid,
        key: &str,
        value: &str,
        enabled: bool,
        description: Option<&str>,
        sort_order: i32,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO folder_query_parameters (id, folder_id, key, value, enabled, description, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id.to_string(),
                folder_id.to_string(),
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

    /// Update an existing folder query parameter
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
            "UPDATE folder_query_parameters SET key = ?2, value = ?3, enabled = ?4, description = ?5, sort_order = ?6, updated_at = ?7
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

    /// Delete a folder query parameter
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM folder_query_parameters WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Delete all query parameters for a folder
    pub fn delete_by_folder(&self, folder_id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM folder_query_parameters WHERE folder_id = ?1",
            params![folder_id.to_string()],
        )?;
        Ok(())
    }

    /// List all query parameters for a folder
    pub fn list_by_folder(&self, folder_id: Uuid) -> Result<Vec<QueryParameter>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, key, value, enabled, description, sort_order
             FROM folder_query_parameters
             WHERE folder_id = ?1
             ORDER BY sort_order, key",
        )?;

        let rows = stmt.query_map(params![folder_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let enabled: i32 = row.get(3)?;
            let description: Option<String> = row.get(4)?;

            Ok(QueryParameter {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
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
    use crate::{Database, FolderStore};
    use models::Folder;

    fn create_test_folder(db: &Database) -> Uuid {
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;
        let folder_store = FolderStore::new(db.conn());
        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");
        folder_id
    }

    #[test]
    fn test_create_folder_query_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = create_test_folder(&db);

        let param_id = store
            .create(folder_id, "api_version", "v1", true, Some("API version"), 0)
            .expect("Failed to create folder query parameter");

        assert_ne!(param_id, Uuid::nil());

        // Verify it was created
        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].key.as_ref(), "api_version");
        assert_eq!(params[0].value.as_ref(), "v1");
        assert!(params[0].enabled);
        assert_eq!(
            params[0].description.as_ref().unwrap().as_ref(),
            "API version"
        );
    }

    #[test]
    fn test_update_folder_query_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = create_test_folder(&db);

        let param_id = store
            .create(folder_id, "format", "json", true, None, 0)
            .expect("Failed to create folder query parameter");

        // Update the parameter
        store
            .update(param_id, "format", "xml", false, Some("Response format"), 1)
            .expect("Failed to update folder query parameter");

        // Verify the update
        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].key.as_ref(), "format");
        assert_eq!(params[0].value.as_ref(), "xml");
        assert!(!params[0].enabled);
        assert_eq!(
            params[0].description.as_ref().unwrap().as_ref(),
            "Response format"
        );
    }

    #[test]
    fn test_delete_folder_query_parameter() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = create_test_folder(&db);

        let param_id = store
            .create(folder_id, "limit", "10", true, None, 0)
            .expect("Failed to create folder query parameter");

        // Delete the parameter
        store
            .delete(param_id)
            .expect("Failed to delete folder query parameter");

        // Verify it was deleted
        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_list_by_folder_multiple_params() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = create_test_folder(&db);

        // Create multiple parameters
        store
            .create(folder_id, "api_key", "secret123", true, None, 0)
            .expect("Failed to create parameter");
        store
            .create(folder_id, "format", "json", true, None, 1)
            .expect("Failed to create parameter");
        store
            .create(folder_id, "version", "v2", false, None, 2)
            .expect("Failed to create parameter");

        // List all parameters
        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].key.as_ref(), "api_key");
        assert_eq!(params[1].key.as_ref(), "format");
        assert_eq!(params[2].key.as_ref(), "version");
        assert!(params[0].enabled);
        assert!(params[1].enabled);
        assert!(!params[2].enabled);
    }

    #[test]
    fn test_list_by_folder_empty() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = Uuid::new_v4();

        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_delete_by_folder() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = create_test_folder(&db);

        // Create multiple parameters
        store
            .create(folder_id, "param1", "value1", true, None, 0)
            .expect("Failed to create parameter");
        store
            .create(folder_id, "param2", "value2", true, None, 1)
            .expect("Failed to create parameter");

        // Delete all parameters for the folder
        store
            .delete_by_folder(folder_id)
            .expect("Failed to delete parameters");

        // Verify all were deleted
        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_enabled_state_persistence() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = create_test_folder(&db);

        // Create enabled parameter
        store
            .create(folder_id, "enabled_param", "value1", true, None, 0)
            .expect("Failed to create parameter");

        // Create disabled parameter
        store
            .create(folder_id, "disabled_param", "value2", false, None, 1)
            .expect("Failed to create parameter");

        // Verify enabled states
        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 2);
        assert!(params[0].enabled);
        assert!(!params[1].enabled);
    }

    #[test]
    fn test_sort_order_preserved() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = create_test_folder(&db);

        // Create parameters with specific sort order
        store
            .create(folder_id, "third", "3", true, None, 2)
            .expect("Failed to create parameter");
        store
            .create(folder_id, "first", "1", true, None, 0)
            .expect("Failed to create parameter");
        store
            .create(folder_id, "second", "2", true, None, 1)
            .expect("Failed to create parameter");

        // Verify they are returned in sort order
        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].key.as_ref(), "first");
        assert_eq!(params[1].key.as_ref(), "second");
        assert_eq!(params[2].key.as_ref(), "third");
    }

    #[test]
    fn test_multiple_folders_isolation() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder1_id = create_test_folder(&db);
        let folder2_id = create_test_folder(&db);

        // Create parameters for folder 1
        store
            .create(folder1_id, "folder1_param", "value1", true, None, 0)
            .expect("Failed to create parameter");

        // Create parameters for folder 2
        store
            .create(folder2_id, "folder2_param", "value2", true, None, 0)
            .expect("Failed to create parameter");

        // Verify folder 1 only has its own parameters
        let folder1_params = store
            .list_by_folder(folder1_id)
            .expect("Failed to list parameters");
        assert_eq!(folder1_params.len(), 1);
        assert_eq!(folder1_params[0].key.as_ref(), "folder1_param");

        // Verify folder 2 only has its own parameters
        let folder2_params = store
            .list_by_folder(folder2_id)
            .expect("Failed to list parameters");
        assert_eq!(folder2_params.len(), 1);
        assert_eq!(folder2_params[0].key.as_ref(), "folder2_param");
    }

    #[test]
    fn test_folder_query_parameter_without_description() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = FolderQueryParameterStore::new(db.conn());
        let folder_id = create_test_folder(&db);

        store
            .create(folder_id, "no_desc", "value", true, None, 0)
            .expect("Failed to create parameter");

        let params = store
            .list_by_folder(folder_id)
            .expect("Failed to list parameters");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].key.as_ref(), "no_desc");
        assert_eq!(params[0].value.as_ref(), "value");
        assert!(params[0].description.is_none());
    }
}
