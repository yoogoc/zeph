pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sync_version INTEGER DEFAULT 0,
    source TEXT DEFAULT 'local',
    deleted INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    parent_id TEXT,
    folder_type TEXT DEFAULT 'service',
    name TEXT NOT NULL,
    description TEXT,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sync_version INTEGER DEFAULT 0,
    source TEXT DEFAULT 'local',
    deleted INTEGER DEFAULT 0,
    common_headers TEXT DEFAULT '[]',
    common_search_params TEXT DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS services (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    folder_id TEXT,
    name TEXT NOT NULL,
    description TEXT,
    service_type TEXT NOT NULL,
    config TEXT NOT NULL,
    deprecated INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sync_version INTEGER DEFAULT 0,
    source TEXT DEFAULT 'local',
    deleted INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS schemas (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    folder_id TEXT,
    name TEXT NOT NULL,
    description TEXT,
    schema_type TEXT NOT NULL,
    properties TEXT NOT NULL,
    required TEXT NOT NULL,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sync_version INTEGER DEFAULT 0,
    source TEXT DEFAULT 'local',
    deleted INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_folders_workspace ON folders(workspace_id);
CREATE INDEX IF NOT EXISTS idx_services_workspace ON services(workspace_id);
CREATE INDEX IF NOT EXISTS idx_schemas_workspace ON schemas(workspace_id);

CREATE TABLE IF NOT EXISTS environments (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL,
    base_url TEXT DEFAULT '',
    is_default INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sync_version INTEGER DEFAULT 0,
    source TEXT DEFAULT 'local',
    deleted INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS environment_variables (
    id TEXT PRIMARY KEY,
    environment_id TEXT NOT NULL,
    name TEXT NOT NULL,
    var_type TEXT DEFAULT 'default',
    remote_value TEXT,
    local_value TEXT,
    description TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_environments_workspace ON environments(workspace_id);
CREATE INDEX IF NOT EXISTS idx_env_vars_environment ON environment_variables(environment_id);

CREATE TABLE IF NOT EXISTS query_parameters (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS path_parameters (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL,
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    description TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS folder_query_parameters (
    id TEXT PRIMARY KEY,
    folder_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_query_params_service ON query_parameters(service_id);
CREATE INDEX IF NOT EXISTS idx_path_params_service ON path_parameters(service_id);
CREATE INDEX IF NOT EXISTS idx_folder_query_params_folder ON folder_query_parameters(folder_id);

CREATE TABLE IF NOT EXISTS drafts (
    service_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    automerge_state BLOB NOT NULL,
    base_version INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_drafts_workspace ON drafts(workspace_id);
"#;

#[cfg(test)]
mod tests {
    use crate::db::Database;

    #[test]
    fn test_parameter_tables_created() {
        // Create an in-memory database
        let db = Database::open_in_memory().expect("Failed to create database");
        let conn = db.conn();

        // Verify query_parameters table exists
        let query_params_exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='query_parameters'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check query_parameters table");
        assert_eq!(
            query_params_exists, 1,
            "query_parameters table should exist"
        );

        // Verify path_parameters table exists
        let path_params_exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='path_parameters'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check path_parameters table");
        assert_eq!(path_params_exists, 1, "path_parameters table should exist");

        // Verify folder_query_parameters table exists
        let folder_query_params_exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='folder_query_parameters'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check folder_query_parameters table");
        assert_eq!(
            folder_query_params_exists, 1,
            "folder_query_parameters table should exist"
        );

        // Verify indexes exist
        let query_params_index_exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_query_params_service'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check query_params index");
        assert_eq!(
            query_params_index_exists, 1,
            "idx_query_params_service index should exist"
        );

        let path_params_index_exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_path_params_service'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check path_params index");
        assert_eq!(
            path_params_index_exists, 1,
            "idx_path_params_service index should exist"
        );

        let folder_query_params_index_exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_folder_query_params_folder'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check folder_query_params index");
        assert_eq!(
            folder_query_params_index_exists, 1,
            "idx_folder_query_params_folder index should exist"
        );
    }

    #[test]
    fn test_parameter_table_structure() {
        // Create an in-memory database
        let db = Database::open_in_memory().expect("Failed to create database");
        let conn = db.conn();

        // Test query_parameters table structure
        let query_params_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(query_parameters)")
            .expect("Failed to prepare statement")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("Failed to query columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to collect columns");

        assert!(query_params_columns.contains(&"id".to_string()));
        assert!(query_params_columns.contains(&"service_id".to_string()));
        assert!(query_params_columns.contains(&"key".to_string()));
        assert!(query_params_columns.contains(&"value".to_string()));
        assert!(query_params_columns.contains(&"enabled".to_string()));
        assert!(query_params_columns.contains(&"description".to_string()));
        assert!(query_params_columns.contains(&"sort_order".to_string()));
        assert!(query_params_columns.contains(&"created_at".to_string()));
        assert!(query_params_columns.contains(&"updated_at".to_string()));

        // Test path_parameters table structure
        let path_params_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(path_parameters)")
            .expect("Failed to prepare statement")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("Failed to query columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to collect columns");

        assert!(path_params_columns.contains(&"id".to_string()));
        assert!(path_params_columns.contains(&"service_id".to_string()));
        assert!(path_params_columns.contains(&"name".to_string()));
        assert!(path_params_columns.contains(&"value".to_string()));
        assert!(path_params_columns.contains(&"description".to_string()));
        assert!(path_params_columns.contains(&"created_at".to_string()));
        assert!(path_params_columns.contains(&"updated_at".to_string()));

        // Test folder_query_parameters table structure
        let folder_query_params_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(folder_query_parameters)")
            .expect("Failed to prepare statement")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("Failed to query columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to collect columns");

        assert!(folder_query_params_columns.contains(&"id".to_string()));
        assert!(folder_query_params_columns.contains(&"folder_id".to_string()));
        assert!(folder_query_params_columns.contains(&"key".to_string()));
        assert!(folder_query_params_columns.contains(&"value".to_string()));
        assert!(folder_query_params_columns.contains(&"enabled".to_string()));
        assert!(folder_query_params_columns.contains(&"description".to_string()));
        assert!(folder_query_params_columns.contains(&"sort_order".to_string()));
        assert!(folder_query_params_columns.contains(&"created_at".to_string()));
        assert!(folder_query_params_columns.contains(&"updated_at".to_string()));
    }
}
