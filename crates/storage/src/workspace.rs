use anyhow::Result;
use models::{DataSource, SyncMeta, Workspace};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct WorkspaceStore<'a> {
    conn: &'a Connection,
}

impl<'a> WorkspaceStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, workspace: &Workspace) -> Result<()> {
        self.conn.execute(
            "INSERT INTO workspaces (id, name, description, created_at, updated_at, sync_version, source, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                workspace.id.to_string(),
                workspace.name.as_ref(),
                workspace.description,
                workspace.created_at,
                workspace.updated_at,
                workspace.sync.sync_version,
                source_to_str(&workspace.sync.source),
                workspace.sync.deleted,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, workspace: &Workspace) -> Result<()> {
        self.conn.execute(
            "UPDATE workspaces SET name = ?2, description = ?3, updated_at = ?4, sync_version = ?5
             WHERE id = ?1 AND deleted = 0",
            params![
                workspace.id.to_string(),
                workspace.name.as_ref(),
                workspace.description,
                workspace.updated_at,
                workspace.sync.sync_version,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE workspaces SET deleted = 1, updated_at = ?2 WHERE id = ?1",
            params![id.to_string(), now],
        )?;
        Ok(())
    }

    pub fn get(&self, id: Uuid) -> Result<Option<Workspace>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, created_at, updated_at, sync_version, source, deleted
             FROM workspaces WHERE id = ?1 AND deleted = 0",
        )?;
        
        let mut rows = stmt.query(params![id.to_string()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_workspace(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list(&self) -> Result<Vec<Workspace>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, created_at, updated_at, sync_version, source, deleted
             FROM workspaces WHERE deleted = 0 ORDER BY created_at DESC",
        )?;
        
        let rows = stmt.query_map([], |row| Ok(row_to_workspace(row).unwrap()))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn row_to_workspace(row: &rusqlite::Row) -> Result<Workspace> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let description: Option<String> = row.get(2)?;
    let created_at: i64 = row.get(3)?;
    let updated_at: i64 = row.get(4)?;
    let sync_version: i64 = row.get(5)?;
    let source: String = row.get(6)?;
    let deleted: bool = row.get(7)?;

    Ok(Workspace {
        id: Uuid::parse_str(&id)?,
        name: name.into(),
        description,
        created_at,
        updated_at,
        sync: SyncMeta {
            sync_version,
            source: str_to_source(&source),
            deleted,
        },
        collections: vec![],
        current_view: Default::default(),
    })
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
