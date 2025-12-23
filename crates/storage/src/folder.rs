use anyhow::Result;
use models::{DataSource, Folder, FolderType, SyncMeta};
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
            "INSERT INTO folders (id, workspace_id, parent_id, folder_type, name, description, sort_order, created_at, updated_at, sync_version, source, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                folder.id.to_string(),
                folder.workspace_id.to_string(),
                folder.parent_id.map(|id| id.to_string()),
                folder_type_to_str(&folder.folder_type),
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

    pub fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, parent_id, folder_type, name, description, sort_order, created_at, updated_at, sync_version, source, deleted
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
    let folder_type: String = row.get(3)?;
    let name: String = row.get(4)?;
    let description: Option<String> = row.get(5)?;
    let sort_order: i32 = row.get(6)?;
    let created_at: i64 = row.get(7)?;
    let updated_at: i64 = row.get(8)?;
    let sync_version: i64 = row.get(9)?;
    let source: String = row.get(10)?;
    let deleted: bool = row.get(11)?;

    Ok(Folder {
        id: Uuid::parse_str(&id)?,
        workspace_id: Uuid::parse_str(&workspace_id)?,
        parent_id: parent_id.and_then(|s| Uuid::parse_str(&s).ok()),
        folder_type: str_to_folder_type(&folder_type),
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
