use anyhow::Result;
use models::{DataSource, Folder, SyncMeta};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct FolderStore<'a> {
    conn: &'a Connection,
}

impl<'a> FolderStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, folder: &Folder) -> Result<()> {
        self.conn.execute(
            "INSERT INTO folders (id, workspace_id, parent_id, name, description, sort_order, created_at, updated_at, sync_version, source, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                folder.id.to_string(),
                folder.workspace_id.to_string(),
                folder.parent_id.map(|id| id.to_string()),
                folder.name.as_ref(),
                folder.description,
                folder.sort_order,
                folder.created_at,
                folder.updated_at,
                folder.sync.sync_version,
                source_to_str(&folder.sync.source),
                folder.sync.deleted,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, folder: &Folder) -> Result<()> {
        self.conn.execute(
            "UPDATE folders SET parent_id = ?2, name = ?3, description = ?4, sort_order = ?5, updated_at = ?6, sync_version = ?7
             WHERE id = ?1 AND deleted = 0",
            params![
                folder.id.to_string(),
                folder.parent_id.map(|id| id.to_string()),
                folder.name.as_ref(),
                folder.description,
                folder.sort_order,
                folder.updated_at,
                folder.sync.sync_version,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE folders SET deleted = 1, updated_at = ?2 WHERE id = ?1",
            params![id.to_string(), now],
        )?;
        Ok(())
    }

    pub fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, parent_id, name, description, sort_order, created_at, updated_at, sync_version, source, deleted
             FROM folders WHERE workspace_id = ?1 AND deleted = 0 ORDER BY sort_order, name",
        )?;

        let rows = stmt.query_map(params![workspace_id.to_string()], |row| {
            Ok(row_to_folder(row).unwrap())
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn row_to_folder(row: &rusqlite::Row) -> Result<Folder> {
    let id: String = row.get(0)?;
    let workspace_id: String = row.get(1)?;
    let parent_id: Option<String> = row.get(2)?;
    let name: String = row.get(3)?;
    let description: Option<String> = row.get(4)?;
    let sort_order: i32 = row.get(5)?;
    let created_at: i64 = row.get(6)?;
    let updated_at: i64 = row.get(7)?;
    let sync_version: i64 = row.get(8)?;
    let source: String = row.get(9)?;
    let deleted: bool = row.get(10)?;

    Ok(Folder {
        id: Uuid::parse_str(&id)?,
        workspace_id: Uuid::parse_str(&workspace_id)?,
        parent_id: parent_id.map(|s| Uuid::parse_str(&s).unwrap()),
        name: name.into(),
        description,
        sort_order,
        created_at,
        updated_at,
        sync: SyncMeta {
            sync_version,
            source: str_to_source(&source),
            deleted,
        },
        is_expanded: true,
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
