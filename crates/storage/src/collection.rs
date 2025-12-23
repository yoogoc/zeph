use anyhow::Result;
use models::{Collection, DataSource, SyncMeta};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct CollectionStore<'a> {
    conn: &'a Connection,
}

impl<'a> CollectionStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, collection: &Collection) -> Result<()> {
        self.conn.execute(
            "INSERT INTO collections (id, workspace_id, name, description, version, openapi_version, created_at, updated_at, sync_version, source, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                collection.id.to_string(),
                collection.workspace_id.to_string(),
                collection.name.as_ref(),
                collection.description,
                collection.version,
                collection.openapi_version,
                collection.created_at,
                collection.updated_at,
                collection.sync.sync_version,
                source_to_str(&collection.sync.source),
                collection.sync.deleted,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, collection: &Collection) -> Result<()> {
        self.conn.execute(
            "UPDATE collections SET name = ?2, description = ?3, version = ?4, openapi_version = ?5, updated_at = ?6, sync_version = ?7
             WHERE id = ?1 AND deleted = 0",
            params![
                collection.id.to_string(),
                collection.name.as_ref(),
                collection.description,
                collection.version,
                collection.openapi_version,
                collection.updated_at,
                collection.sync.sync_version,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE collections SET deleted = 1, updated_at = ?2 WHERE id = ?1",
            params![id.to_string(), now],
        )?;
        Ok(())
    }

    pub fn get(&self, id: Uuid) -> Result<Option<Collection>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, name, description, version, openapi_version, created_at, updated_at, sync_version, source, deleted
             FROM collections WHERE id = ?1 AND deleted = 0",
        )?;
        
        let mut rows = stmt.query(params![id.to_string()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_collection(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<Collection>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, name, description, version, openapi_version, created_at, updated_at, sync_version, source, deleted
             FROM collections WHERE workspace_id = ?1 AND deleted = 0 ORDER BY created_at DESC",
        )?;
        
        let rows = stmt.query_map(params![workspace_id.to_string()], |row| {
            Ok(row_to_collection(row).unwrap())
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn row_to_collection(row: &rusqlite::Row) -> Result<Collection> {
    let id: String = row.get(0)?;
    let workspace_id: String = row.get(1)?;
    let name: String = row.get(2)?;
    let description: Option<String> = row.get(3)?;
    let version: Option<String> = row.get(4)?;
    let openapi_version: Option<String> = row.get(5)?;
    let created_at: i64 = row.get(6)?;
    let updated_at: i64 = row.get(7)?;
    let sync_version: i64 = row.get(8)?;
    let source: String = row.get(9)?;
    let deleted: bool = row.get(10)?;

    Ok(Collection {
        id: Uuid::parse_str(&id)?,
        workspace_id: Uuid::parse_str(&workspace_id)?,
        name: name.into(),
        description,
        version,
        openapi_version,
        created_at,
        updated_at,
        sync: SyncMeta {
            sync_version,
            source: str_to_source(&source),
            deleted,
        },
        environments: vec![],
        folders: vec![],
        services: vec![],
        opened_services: vec![],
        active_service: None,
        active_environment: None,
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
