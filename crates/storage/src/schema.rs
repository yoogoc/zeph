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
    name TEXT NOT NULL,
    description TEXT,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sync_version INTEGER DEFAULT 0,
    source TEXT DEFAULT 'local',
    deleted INTEGER DEFAULT 0,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id),
    FOREIGN KEY (parent_id) REFERENCES folders(id)
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
    deleted INTEGER DEFAULT 0,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id),
    FOREIGN KEY (folder_id) REFERENCES folders(id)
);

CREATE INDEX IF NOT EXISTS idx_folders_workspace ON folders(workspace_id);
CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id);
CREATE INDEX IF NOT EXISTS idx_services_workspace ON services(workspace_id);
CREATE INDEX IF NOT EXISTS idx_services_folder ON services(folder_id);
"#;
