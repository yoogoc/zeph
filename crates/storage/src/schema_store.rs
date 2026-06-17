use anyhow::Result;
use models::{DataSource, Schema, SchemaType, SyncMeta};
use rusqlite::{params, Connection};
use std::sync::Arc;
use uuid::Uuid;

pub struct SchemaStore {
    conn: Arc<Connection>,
}

impl SchemaStore {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    pub fn insert(&self, schema: &Schema) -> Result<()> {
        self.conn.execute(
            "INSERT INTO schemas (id, workspace_id, folder_id, name, description, schema_type, properties, required, sort_order, created_at, updated_at, sync_version, source, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                schema.id.to_string(),
                schema.workspace_id.to_string(),
                schema.folder_id.map(|id| id.to_string()),
                schema.name.as_ref(),
                schema.description.as_ref().map(|id| id.to_string()),
                serde_json::to_string(&schema.schema_type)?,
                serde_json::to_string(&schema.properties)?,
                serde_json::to_string(&schema.required)?,
                schema.sort_order,
                schema.created_at,
                schema.updated_at,
                schema.sync.sync_version,
                source_to_str(&schema.sync.source),
                schema.sync.deleted,
            ],
        )?;
        Ok(())
    }

    pub fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<Schema>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, folder_id, name, description, schema_type, properties, required, sort_order, created_at, updated_at, sync_version, source, deleted
             FROM schemas WHERE workspace_id = ?1 AND deleted = 0 ORDER BY sort_order, name",
        )?;

        let rows = stmt.query_map(params![workspace_id.to_string()], |row| {
            Ok(row_to_schema(row).unwrap())
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn update(&self, schema: &Schema) -> Result<()> {
        let updated_at = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE schemas SET folder_id = ?2, name = ?3, description = ?4, schema_type = ?5, properties = ?6, required = ?7, sort_order = ?8, updated_at = ?9, sync_version = ?10
             WHERE id = ?1 AND deleted = 0",
            params![
                schema.id.to_string(),
                schema.folder_id.map(|id| id.to_string()),
                schema.name.as_ref(),
                schema.description.as_ref().map(|s| s.to_string()),
                serde_json::to_string(&schema.schema_type)?,
                serde_json::to_string(&schema.properties)?,
                serde_json::to_string(&schema.required)?,
                schema.sort_order,
                updated_at,
                schema.sync.sync_version,
            ],
        )?;
        Ok(())
    }

    /// Soft-deletes a schema (sets `deleted = 1`).
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.conn.execute(
            "UPDATE schemas SET deleted = 1 WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }
}

fn row_to_schema(row: &rusqlite::Row) -> Result<Schema> {
    let id: String = row.get(0)?;
    let workspace_id: String = row.get(1)?;
    let folder_id: Option<String> = row.get(2)?;
    let name: String = row.get(3)?;
    let description: Option<String> = row.get(4)?;
    let schema_type: String = row.get(5)?;
    let properties: String = row.get(6)?;
    let required: String = row.get(7)?;
    let sort_order: i32 = row.get(8)?;
    let created_at: i64 = row.get(9)?;
    let updated_at: i64 = row.get(10)?;
    let sync_version: i64 = row.get(11)?;
    let source: String = row.get(12)?;
    let deleted: bool = row.get(13)?;

    Ok(Schema {
        id: Uuid::parse_str(&id)?,
        workspace_id: Uuid::parse_str(&workspace_id)?,
        folder_id: folder_id.and_then(|s| Uuid::parse_str(&s).ok()),
        name: name.into(),
        description: description.map(|id| id.into()),
        schema_type: serde_json::from_str(&schema_type).unwrap_or(SchemaType::Object),
        properties: serde_json::from_str(&properties).unwrap_or_default(),
        required: serde_json::from_str(&required).unwrap_or_default(),
        sort_order,
        created_at,
        updated_at,
        sync: SyncMeta {
            sync_version,
            source: str_to_source(&source),
            deleted,
        },
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
