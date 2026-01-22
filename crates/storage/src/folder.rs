use anyhow::{anyhow, Context, Result};
use models::{DataSource, Folder, FolderType, Header, SearchParameter, SyncMeta};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::FolderQueryParameterStore;

pub struct FolderStore<'a> {
    conn: &'a Connection,
}

impl<'a> FolderStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, folder: &Folder) -> Result<()> {
        let common_headers_json = serde_json::to_string(&folder.common_headers)?;
        let common_search_params_json = serde_json::to_string(&folder.common_search_params)?;

        self.conn.execute(
            "INSERT INTO folders (id, workspace_id, parent_id, folder_type, name, description, sort_order, created_at, updated_at, sync_version, source, deleted, common_headers, common_search_params)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                folder.id.to_string(),
                folder.workspace_id.to_string(),
                folder.parent_id.map(|id| id.to_string()),
                folder_type_to_str(&folder.folder_type),
                folder.name.as_ref(),
                folder.description.as_ref().map(|id| id.to_string()),
                folder.sort_order,
                folder.created_at,
                folder.updated_at,
                folder.sync.sync_version,
                source_to_str(&folder.sync.source),
                folder.sync.deleted,
                common_headers_json,
                common_search_params_json,
            ],
        )?;

        // Save common query parameters to the folder_query_parameters table
        if !folder.common_search_params.is_empty() {
            let folder_query_param_store = FolderQueryParameterStore::new(self.conn);

            for (index, param) in folder.common_search_params.iter().enumerate() {
                folder_query_param_store.create(
                    folder.id,
                    param.key.as_ref(),
                    param.value.as_ref(),
                    true, // enabled by default
                    None, // no description
                    index as i32,
                )?;
            }
        }

        Ok(())
    }

    pub fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, parent_id, folder_type, name, description, sort_order, created_at, updated_at, sync_version, source, deleted, common_headers, common_search_params
             FROM folders WHERE workspace_id = ?1 AND deleted = 0 ORDER BY sort_order, name",
        )?;

        let rows = stmt.query_map(params![workspace_id.to_string()], |row| {
            Ok(row_to_folder(row).unwrap())
        })?;

        let mut folders: Vec<Folder> = rows.filter_map(|r| r.ok()).collect();

        // Load common query parameters from the folder_query_parameters table
        let folder_query_param_store = FolderQueryParameterStore::new(self.conn);

        for folder in &mut folders {
            // Load query parameters from the new table
            let query_params = folder_query_param_store
                .list_by_folder(folder.id)
                .unwrap_or_default();

            // Convert QueryParameter to SearchParameter for backward compatibility
            folder.common_search_params = query_params
                .iter()
                .map(|qp| SearchParameter {
                    key: qp.key.clone(),
                    value: qp.value.clone(),
                })
                .collect();
        }

        Ok(folders)
    }

    pub fn get_folder_with_common_settings(&self, folder_id: Uuid) -> Result<Option<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, parent_id, folder_type, name, description, sort_order, created_at, updated_at, sync_version, source, deleted, common_headers, common_search_params
             FROM folders WHERE id = ?1 AND deleted = 0",
        )?;

        let mut rows = stmt.query_map(params![folder_id.to_string()], |row| {
            Ok(row_to_folder(row).unwrap())
        })?;

        match rows.next() {
            Some(Ok(mut folder)) => {
                // Load common query parameters from the folder_query_parameters table
                let folder_query_param_store = FolderQueryParameterStore::new(self.conn);
                let query_params = folder_query_param_store
                    .list_by_folder(folder.id)
                    .unwrap_or_default();

                // Convert QueryParameter to SearchParameter for backward compatibility
                folder.common_search_params = query_params
                    .iter()
                    .map(|qp| SearchParameter {
                        key: qp.key.clone(),
                        value: qp.value.clone(),
                    })
                    .collect();

                Ok(Some(folder))
            }
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn update_common_headers(&self, folder_id: Uuid, headers: Vec<Header>) -> Result<()> {
        // Validate headers before saving
        self.validate_headers(&headers)
            .with_context(|| "Invalid headers provided")?;

        let headers_json = serde_json::to_string(&headers)
            .with_context(|| "Failed to serialize headers to JSON")?;
        let now = chrono::Utc::now().timestamp();

        let rows_affected = self.conn.execute(
            "UPDATE folders SET common_headers = ?1, updated_at = ?2 WHERE id = ?3 AND deleted = 0",
            params![headers_json, now, folder_id.to_string()],
        ).with_context(|| format!("Failed to update common headers for folder {}", folder_id))?;

        if rows_affected == 0 {
            return Err(anyhow!(
                "Folder not found or has been deleted: {}",
                folder_id
            ));
        }

        Ok(())
    }

    pub fn update_search_params(
        &self,
        folder_id: Uuid,
        params: Vec<SearchParameter>,
    ) -> Result<()> {
        // Validate parameters before saving
        self.validate_search_params(&params)
            .with_context(|| "Invalid search parameters provided")?;

        // Update the folder_query_parameters table
        let folder_query_param_store = FolderQueryParameterStore::new(self.conn);

        // Delete old parameters
        folder_query_param_store.delete_by_folder(folder_id)?;

        // Insert new parameters
        for (index, param) in params.iter().enumerate() {
            folder_query_param_store.create(
                folder_id,
                param.key.as_ref(),
                param.value.as_ref(),
                true, // enabled by default
                None, // no description
                index as i32,
            )?;
        }

        // Also update the JSON field for backward compatibility
        let params_json = serde_json::to_string(&params)
            .with_context(|| "Failed to serialize search parameters to JSON")?;
        let now = chrono::Utc::now().timestamp();

        let rows_affected = self.conn.execute(
            "UPDATE folders SET common_search_params = ?1, updated_at = ?2 WHERE id = ?3 AND deleted = 0",
            params![params_json, now, folder_id.to_string()],
        ).with_context(|| format!("Failed to update search parameters for folder {}", folder_id))?;

        if rows_affected == 0 {
            return Err(anyhow!(
                "Folder not found or has been deleted: {}",
                folder_id
            ));
        }

        Ok(())
    }

    /// Validate headers before database operations
    fn validate_headers(&self, headers: &[Header]) -> Result<()> {
        if headers.len() > 50 {
            return Err(anyhow!("Too many headers (max 50 allowed)"));
        }

        let mut seen_names = std::collections::HashSet::new();

        for header in headers {
            let name = header.key.trim();
            let value = header.value.trim();

            // Check for empty values
            if name.is_empty() {
                return Err(anyhow!("Header name cannot be empty"));
            }
            if value.is_empty() {
                return Err(anyhow!("Header value cannot be empty"));
            }

            // Check length limits
            if name.len() > 100 {
                return Err(anyhow!(
                    "Header name '{}' is too long (max 100 characters)",
                    name
                ));
            }
            if value.len() > 8192 {
                return Err(anyhow!(
                    "Header value for '{}' is too long (max 8192 characters)",
                    name
                ));
            }

            // Check for duplicates
            let name_lower = name.to_lowercase();
            if seen_names.contains(&name_lower) {
                return Err(anyhow!("Duplicate header name: '{}'", name));
            }
            seen_names.insert(name_lower.clone());

            // Validate header name format
            if !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(anyhow!("Invalid header name format: '{}'", name));
            }

            // Check for reserved headers
            if matches!(
                name_lower.as_str(),
                "host"
                    | "connection"
                    | "content-length"
                    | "transfer-encoding"
                    | "upgrade"
                    | "proxy-connection"
                    | "te"
                    | "trailer"
            ) {
                return Err(anyhow!("'{}' is a reserved header and cannot be set", name));
            }
        }

        Ok(())
    }

    /// Validate search parameters before database operations
    fn validate_search_params(&self, params: &[SearchParameter]) -> Result<()> {
        if params.len() > 50 {
            return Err(anyhow!("Too many search parameters (max 50 allowed)"));
        }

        let mut seen_names = std::collections::HashSet::new();

        for param in params {
            let name = param.key.trim();
            let value = param.value.trim();

            // Check for empty values
            if name.is_empty() {
                return Err(anyhow!("Parameter name cannot be empty"));
            }
            if value.is_empty() {
                return Err(anyhow!("Parameter value cannot be empty"));
            }

            // Check length limits
            if name.len() > 100 {
                return Err(anyhow!(
                    "Parameter name '{}' is too long (max 100 characters)",
                    name
                ));
            }
            if value.len() > 2048 {
                return Err(anyhow!(
                    "Parameter value for '{}' is too long (max 2048 characters)",
                    name
                ));
            }

            // Check for duplicates
            let name_lower = name.to_lowercase();
            if seen_names.contains(&name_lower) {
                return Err(anyhow!("Duplicate parameter name: '{}'", name));
            }
            seen_names.insert(name_lower);

            // Validate parameter name format
            if !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                return Err(anyhow!("Invalid parameter name format: '{}'", name));
            }

            // Check for newlines in values
            if value.contains('\n') || value.contains('\r') {
                return Err(anyhow!(
                    "Parameter value for '{}' cannot contain newline characters",
                    name
                ));
            }
        }

        Ok(())
    }

    /// Update folder basic information
    ///
    /// Updates one or more fields of a folder. Pass `None` to leave a field unchanged.
    /// Pass `Some(None)` to set a nullable field to NULL.
    ///
    /// # Arguments
    /// * `folder_id` - The UUID of the folder to update
    /// * `name` - Optional new name for the folder
    /// * `description` - Optional new description (Some(None) to clear, Some(Some(desc)) to set)
    /// * `parent_id` - Optional new parent_id (Some(None) to clear, Some(Some(id)) to set)
    ///
    /// # Returns
    /// The updated Folder object with all fields populated
    ///
    /// # Errors
    /// Returns an error if:
    /// - The folder_id does not exist or has been deleted
    /// - The name validation fails (empty, whitespace-only, or too long)
    /// - The description validation fails (too long)
    /// - Database operation fails
    pub fn update(
        &self,
        folder_id: Uuid,
        name: Option<&str>,
        description: Option<Option<&str>>,
        parent_id: Option<Option<Uuid>>,
    ) -> Result<Folder> {
        // Validate name if provided
        if let Some(name_str) = name {
            self.validate_name(name_str)
                .with_context(|| "Invalid folder name")?;
        }

        // Validate description if provided
        if let Some(Some(desc_str)) = description {
            self.validate_description(desc_str)
                .with_context(|| "Invalid folder description")?;
        }

        // Build dynamic UPDATE query based on which fields are provided
        let mut updates = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(name_str) = name {
            updates.push("name = ?");
            params_vec.push(Box::new(name_str.to_string()));
        }

        if let Some(desc_opt) = description {
            updates.push("description = ?");
            params_vec.push(Box::new(desc_opt.map(|s| s.to_string())));
        }

        if let Some(parent_opt) = parent_id {
            updates.push("parent_id = ?");
            params_vec.push(Box::new(parent_opt.map(|id| id.to_string())));
        }

        // Always update the updated_at timestamp
        let now = chrono::Utc::now().timestamp();
        updates.push("updated_at = ?");
        params_vec.push(Box::new(now));

        // If no fields to update, just return the current folder
        if updates.len() == 1 {
            // Only updated_at would be updated, so just fetch and return
            return self
                .get_folder_with_common_settings(folder_id)?
                .ok_or_else(|| anyhow!("Folder not found or has been deleted: {}", folder_id));
        }

        // Build the SQL query
        let sql = format!(
            "UPDATE folders SET {} WHERE id = ? AND deleted = 0",
            updates.join(", ")
        );

        // Add folder_id as the last parameter
        params_vec.push(Box::new(folder_id.to_string()));

        // Convert to references for rusqlite
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        // Execute the update
        let rows_affected = self
            .conn
            .execute(&sql, params_refs.as_slice())
            .with_context(|| format!("Failed to update folder {}", folder_id))?;

        if rows_affected == 0 {
            return Err(anyhow!(
                "Folder not found or has been deleted: {}",
                folder_id
            ));
        }

        // Fetch and return the updated folder
        self.get_folder_with_common_settings(folder_id)?
            .ok_or_else(|| anyhow!("Failed to retrieve updated folder: {}", folder_id))
    }

    /// Validate folder name
    fn validate_name(&self, name: &str) -> Result<()> {
        let trimmed = name.trim();

        // Check for empty or whitespace-only names
        if trimmed.is_empty() {
            return Err(anyhow!("Folder name cannot be empty or whitespace-only"));
        }

        // Check length limit (255 characters)
        if trimmed.len() > 255 {
            return Err(anyhow!(
                "Folder name is too long (max 255 characters, got {})",
                trimmed.len()
            ));
        }

        Ok(())
    }

    /// Validate folder description
    fn validate_description(&self, description: &str) -> Result<()> {
        // Check length limit (1000 characters)
        if description.len() > 1000 {
            return Err(anyhow!(
                "Folder description is too long (max 1000 characters, got {})",
                description.len()
            ));
        }

        Ok(())
    }
}

fn row_to_folder(row: &rusqlite::Row) -> Result<Folder> {
    let id: String = row.get(0)?;
    let workspace_id: String = row.get(1)?;
    let parent_id: Option<String> = row.get(2)?;
    let folder_type: String = row.get(3)?;
    let name: String = row.get(4)?;
    let description: Option<String> = row.get(5)?;
    let sort_order: i32 = row.get(6)?;
    let created_at: i64 = row.get(7)?;
    let updated_at: i64 = row.get(8)?;
    let sync_version: i64 = row.get(9)?;
    let source: String = row.get(10)?;
    let deleted: bool = row.get(11)?;
    let common_headers_json: String = row.get(12).unwrap_or_else(|_| "[]".to_string());
    let common_search_params_json: String = row.get(13).unwrap_or_else(|_| "[]".to_string());

    let common_headers: Vec<Header> =
        serde_json::from_str(&common_headers_json).unwrap_or_else(|_| Vec::new());
    let common_search_params: Vec<SearchParameter> =
        serde_json::from_str(&common_search_params_json).unwrap_or_else(|_| Vec::new());

    Ok(Folder {
        id: Uuid::parse_str(&id)?,
        workspace_id: Uuid::parse_str(&workspace_id)?,
        parent_id: parent_id.and_then(|s| Uuid::parse_str(&s).ok()),
        folder_type: str_to_folder_type(&folder_type),
        name: name.into(),
        description: description.map(|x| x.into()),
        sort_order,
        created_at,
        updated_at,
        sync: SyncMeta {
            sync_version,
            source: str_to_source(&source),
            deleted,
        },
        common_headers,
        common_search_params,
        is_expanded: true,
    })
}

fn folder_type_to_str(t: &FolderType) -> &'static str {
    match t {
        FolderType::Service => "service",
        FolderType::Schema => "schema",
    }
}

fn str_to_folder_type(s: &str) -> FolderType {
    match s {
        "schema" => FolderType::Schema,
        _ => FolderType::Service,
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
    use models::{Header, SearchParameter};

    #[test]
    fn test_folder_serialization_with_common_settings() {
        let workspace_id = Uuid::new_v4();
        let mut folder = Folder::new(workspace_id, "Test Folder", None);

        // Add some common headers and search parameters
        folder.common_headers = vec![
            Header {
                key: "Authorization".into(),
                value: "Bearer token".into(),
            },
            Header {
                key: "Content-Type".into(),
                value: "application/json".into(),
            },
        ];

        folder.common_search_params = vec![
            SearchParameter::new("api_version", "v1"),
            SearchParameter::new("format", "json"),
        ];

        // Test serialization
        let serialized = serde_json::to_string(&folder).expect("Failed to serialize folder");
        let deserialized: Folder =
            serde_json::from_str(&serialized).expect("Failed to deserialize folder");

        // Verify the data is preserved
        assert_eq!(folder.id, deserialized.id);
        assert_eq!(folder.workspace_id, deserialized.workspace_id);
        assert_eq!(folder.name, deserialized.name);
        assert_eq!(
            folder.common_headers.len(),
            deserialized.common_headers.len()
        );
        assert_eq!(
            folder.common_search_params.len(),
            deserialized.common_search_params.len()
        );

        // Verify header content
        assert_eq!(
            folder.common_headers[0].key,
            deserialized.common_headers[0].key
        );
        assert_eq!(
            folder.common_headers[0].value,
            deserialized.common_headers[0].value
        );

        // Verify search parameter content
        assert_eq!(
            folder.common_search_params[0].key,
            deserialized.common_search_params[0].key
        );
        assert_eq!(
            folder.common_search_params[0].value,
            deserialized.common_search_params[0].value
        );
    }

    #[test]
    fn test_folder_default_initialization() {
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);

        // Verify new folders have empty collections
        assert!(folder.common_headers.is_empty());
        assert!(folder.common_search_params.is_empty());
    }

    #[test]
    fn test_folder_storage_operations() {
        // Create in-memory database for testing
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());

        // Create a test folder
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        // Insert the folder
        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Test updating common headers
        let headers = vec![
            Header {
                key: "Authorization".into(),
                value: "Bearer test-token".into(),
            },
            Header {
                key: "Content-Type".into(),
                value: "application/json".into(),
            },
        ];

        folder_store
            .update_common_headers(folder_id, headers.clone())
            .expect("Failed to update common headers");

        // Test updating search parameters
        let params = vec![
            SearchParameter::new("version", "v1"),
            SearchParameter::new("format", "json"),
        ];

        folder_store
            .update_search_params(folder_id, params.clone())
            .expect("Failed to update search parameters");

        // Retrieve folder with common settings
        let retrieved_folder = folder_store
            .get_folder_with_common_settings(folder_id)
            .expect("Failed to retrieve folder")
            .expect("Folder not found");

        // Verify the common headers were stored and retrieved correctly
        assert_eq!(retrieved_folder.common_headers.len(), 2);
        assert_eq!(retrieved_folder.common_headers[0].key, "Authorization");
        assert_eq!(
            retrieved_folder.common_headers[0].value,
            "Bearer test-token"
        );
        assert_eq!(retrieved_folder.common_headers[1].key, "Content-Type");
        assert_eq!(retrieved_folder.common_headers[1].value, "application/json");

        // Verify the search parameters were stored and retrieved correctly
        assert_eq!(retrieved_folder.common_search_params.len(), 2);
        assert_eq!(retrieved_folder.common_search_params[0].key, "version");
        assert_eq!(retrieved_folder.common_search_params[0].value, "v1");
        assert_eq!(retrieved_folder.common_search_params[1].key, "format");
        assert_eq!(retrieved_folder.common_search_params[1].value, "json");
    }

    #[test]
    fn test_header_validation_errors() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Test empty header name
        let invalid_headers = vec![Header {
            key: "".into(),
            value: "some-value".into(),
        }];
        assert!(folder_store
            .update_common_headers(folder_id, invalid_headers)
            .is_err());

        // Test empty header value
        let invalid_headers = vec![Header {
            key: "Some-Header".into(),
            value: "".into(),
        }];
        assert!(folder_store
            .update_common_headers(folder_id, invalid_headers)
            .is_err());

        // Test duplicate header names
        let invalid_headers = vec![
            Header {
                key: "Authorization".into(),
                value: "Bearer token1".into(),
            },
            Header {
                key: "authorization".into(), // Same name, different case
                value: "Bearer token2".into(),
            },
        ];
        assert!(folder_store
            .update_common_headers(folder_id, invalid_headers)
            .is_err());

        // Test reserved header name
        let invalid_headers = vec![Header {
            key: "Host".into(),
            value: "example.com".into(),
        }];
        assert!(folder_store
            .update_common_headers(folder_id, invalid_headers)
            .is_err());

        // Test too many headers
        let mut too_many_headers = Vec::new();
        for i in 0..51 {
            too_many_headers.push(Header {
                key: format!("Header-{}", i).into(),
                value: "value".into(),
            });
        }
        assert!(folder_store
            .update_common_headers(folder_id, too_many_headers)
            .is_err());
    }

    #[test]
    fn test_search_param_validation_errors() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Test empty parameter name
        let invalid_params = vec![SearchParameter::new("", "some-value")];
        assert!(folder_store
            .update_search_params(folder_id, invalid_params)
            .is_err());

        // Test empty parameter value
        let invalid_params = vec![SearchParameter::new("param", "")];
        assert!(folder_store
            .update_search_params(folder_id, invalid_params)
            .is_err());

        // Test duplicate parameter names
        let invalid_params = vec![
            SearchParameter::new("limit", "10"),
            SearchParameter::new("LIMIT", "20"), // Same name, different case
        ];
        assert!(folder_store
            .update_search_params(folder_id, invalid_params)
            .is_err());

        // Test parameter value with newlines
        let invalid_params = vec![SearchParameter::new("query", "line1\nline2")];
        assert!(folder_store
            .update_search_params(folder_id, invalid_params)
            .is_err());

        // Test too many parameters
        let mut too_many_params = Vec::new();
        for i in 0..51 {
            too_many_params.push(SearchParameter::new(format!("param{}", i), "value"));
        }
        assert!(folder_store
            .update_search_params(folder_id, too_many_params)
            .is_err());
    }

    #[test]
    fn test_update_nonexistent_folder() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let nonexistent_id = Uuid::new_v4();

        let headers = vec![Header {
            key: "Authorization".into(),
            value: "Bearer token".into(),
        }];

        // Should fail when trying to update a non-existent folder
        assert!(folder_store
            .update_common_headers(nonexistent_id, headers)
            .is_err());

        let params = vec![SearchParameter::new("limit", "10")];

        // Should fail when trying to update a non-existent folder
        assert!(folder_store
            .update_search_params(nonexistent_id, params)
            .is_err());
    }

    #[test]
    fn test_json_serialization_in_database() {
        // Create in-memory database for testing
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());

        // Create a folder with common settings
        let workspace_id = Uuid::new_v4();
        let mut folder = Folder::new(workspace_id, "Test Folder", None);

        folder.common_headers = vec![Header {
            key: "X-API-Key".into(),
            value: "secret-key".into(),
        }];

        folder.common_search_params = vec![SearchParameter::new("limit", "100")];

        // Insert folder with common settings
        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Retrieve and verify JSON serialization worked correctly
        let retrieved_folder = folder_store
            .get_folder_with_common_settings(folder.id)
            .expect("Failed to retrieve folder")
            .expect("Folder not found");

        assert_eq!(retrieved_folder.common_headers.len(), 1);
        assert_eq!(retrieved_folder.common_headers[0].key, "X-API-Key");
        assert_eq!(retrieved_folder.common_headers[0].value, "secret-key");

        assert_eq!(retrieved_folder.common_search_params.len(), 1);
        assert_eq!(retrieved_folder.common_search_params[0].key, "limit");
        assert_eq!(retrieved_folder.common_search_params[0].value, "100");
    }

    #[test]
    fn test_update_folder_name() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Original Name", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Update only the name
        let updated_folder = folder_store
            .update(folder_id, Some("New Name"), None, None)
            .expect("Failed to update folder name");

        assert_eq!(updated_folder.name.as_ref(), "New Name");
        assert_eq!(updated_folder.id, folder_id);
        assert_eq!(updated_folder.workspace_id, workspace_id);
    }

    #[test]
    fn test_update_folder_description() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Update only the description
        let updated_folder = folder_store
            .update(folder_id, None, Some(Some("New description")), None)
            .expect("Failed to update folder description");

        assert_eq!(
            updated_folder.description.as_ref().map(|s| s.as_ref()),
            Some("New description")
        );
        assert_eq!(updated_folder.name.as_ref(), "Test Folder");
    }

    #[test]
    fn test_update_folder_clear_description() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let mut folder = Folder::new(workspace_id, "Test Folder", None);
        folder.description = Some("Original description".into());
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Clear the description
        let updated_folder = folder_store
            .update(folder_id, None, Some(None), None)
            .expect("Failed to clear folder description");

        assert!(updated_folder.description.is_none());
        assert_eq!(updated_folder.name.as_ref(), "Test Folder");
    }

    #[test]
    fn test_update_folder_parent_id() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let parent_folder = Folder::new(workspace_id, "Parent Folder", None);
        let parent_id = parent_folder.id;
        let child_folder = Folder::new(workspace_id, "Child Folder", None);
        let child_id = child_folder.id;

        folder_store
            .insert(&parent_folder)
            .expect("Failed to insert parent folder");
        folder_store
            .insert(&child_folder)
            .expect("Failed to insert child folder");

        // Update parent_id
        let updated_folder = folder_store
            .update(child_id, None, None, Some(Some(parent_id)))
            .expect("Failed to update folder parent_id");

        assert_eq!(updated_folder.parent_id, Some(parent_id));
        assert_eq!(updated_folder.name.as_ref(), "Child Folder");
    }

    #[test]
    fn test_update_folder_multiple_fields() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Original Name", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Update multiple fields at once
        let updated_folder = folder_store
            .update(
                folder_id,
                Some("Updated Name"),
                Some(Some("Updated description")),
                None,
            )
            .expect("Failed to update folder");

        assert_eq!(updated_folder.name.as_ref(), "Updated Name");
        assert_eq!(
            updated_folder.description.as_ref().map(|s| s.as_ref()),
            Some("Updated description")
        );
    }

    #[test]
    fn test_update_folder_empty_name_fails() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Try to update with empty name
        let result = folder_store.update(folder_id, Some(""), None, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("empty")
                || err_msg.contains("whitespace")
                || err_msg.contains("Invalid folder name")
        );
    }

    #[test]
    fn test_update_folder_whitespace_name_fails() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Try to update with whitespace-only name
        let result = folder_store.update(folder_id, Some("   "), None, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("empty")
                || err_msg.contains("whitespace")
                || err_msg.contains("Invalid folder name")
        );
    }

    #[test]
    fn test_update_folder_name_too_long_fails() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Try to update with name longer than 255 characters
        let long_name = "a".repeat(256);
        let result = folder_store.update(folder_id, Some(&long_name), None, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too long") || err_msg.contains("Invalid folder name"));
    }

    #[test]
    fn test_update_folder_description_too_long_fails() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Try to update with description longer than 1000 characters
        let long_desc = "a".repeat(1001);
        let result = folder_store.update(folder_id, None, Some(Some(&long_desc)), None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too long") || err_msg.contains("Invalid folder description"));
    }

    #[test]
    fn test_update_nonexistent_folder_fails() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let nonexistent_id = Uuid::new_v4();

        // Try to update a folder that doesn't exist
        let result = folder_store.update(nonexistent_id, Some("New Name"), None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not found or has been deleted"));
    }

    #[test]
    fn test_update_folder_preserves_other_fields() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let mut folder = Folder::new(workspace_id, "Test Folder", None);

        // Set up common headers and search params
        folder.common_headers = vec![Header {
            key: "Authorization".into(),
            value: "Bearer token".into(),
        }];
        folder.common_search_params = vec![SearchParameter::new("limit", "10")];

        let folder_id = folder.id;
        let original_created_at = folder.created_at;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Sleep briefly to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Update only the name
        let updated_folder = folder_store
            .update(folder_id, Some("New Name"), None, None)
            .expect("Failed to update folder");

        // Verify other fields are preserved
        assert_eq!(updated_folder.name.as_ref(), "New Name");
        assert_eq!(updated_folder.workspace_id, workspace_id);
        assert_eq!(updated_folder.created_at, original_created_at);
        assert_eq!(updated_folder.common_headers.len(), 1);
        assert_eq!(updated_folder.common_search_params.len(), 1);
        // updated_at should be different (or at least not less than created_at)
        assert!(updated_folder.updated_at >= original_created_at);
    }

    #[test]
    fn test_update_folder_name_with_leading_trailing_spaces() {
        let db = Database::open_in_memory().expect("Failed to create test database");
        let folder_store = FolderStore::new(db.conn());
        let workspace_id = Uuid::new_v4();
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Update with name that has leading/trailing spaces (should be accepted)
        let updated_folder = folder_store
            .update(folder_id, Some("  Valid Name  "), None, None)
            .expect("Failed to update folder");

        // The name should be stored as-is (validation only checks trimmed version)
        assert_eq!(updated_folder.name.as_ref(), "  Valid Name  ");
    }
}
