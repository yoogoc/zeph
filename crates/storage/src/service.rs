use anyhow::Result;
use models::{DataSource, Service, ServiceType, SyncMeta};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct ServiceStore<'a> {
    conn: &'a Connection,
}

impl<'a> ServiceStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, service: &Service) -> Result<()> {
        let (service_type, config) = serialize_service_type(&service.service_type)?;
        
        self.conn.execute(
            "INSERT INTO services (id, collection_id, folder_id, name, description, service_type, config, deprecated, sort_order, created_at, updated_at, sync_version, source, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                service.id.to_string(),
                service.collection_id.to_string(),
                service.folder_id.map(|id| id.to_string()),
                service.name.as_ref(),
                service.description,
                service_type,
                config,
                service.deprecated,
                service.sort_order,
                service.created_at,
                service.updated_at,
                service.sync.sync_version,
                source_to_str(&service.sync.source),
                service.sync.deleted,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, service: &Service) -> Result<()> {
        let (service_type, config) = serialize_service_type(&service.service_type)?;
        
        self.conn.execute(
            "UPDATE services SET folder_id = ?2, name = ?3, description = ?4, service_type = ?5, config = ?6, deprecated = ?7, sort_order = ?8, updated_at = ?9, sync_version = ?10
             WHERE id = ?1 AND deleted = 0",
            params![
                service.id.to_string(),
                service.folder_id.map(|id| id.to_string()),
                service.name.as_ref(),
                service.description,
                service_type,
                config,
                service.deprecated,
                service.sort_order,
                service.updated_at,
                service.sync.sync_version,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE services SET deleted = 1, updated_at = ?2 WHERE id = ?1",
            params![id.to_string(), now],
        )?;
        Ok(())
    }

    pub fn get(&self, id: Uuid) -> Result<Option<Service>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, collection_id, folder_id, name, description, service_type, config, deprecated, sort_order, created_at, updated_at, sync_version, source, deleted
             FROM services WHERE id = ?1 AND deleted = 0",
        )?;
        
        let mut rows = stmt.query(params![id.to_string()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_service(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_by_collection(&self, collection_id: Uuid) -> Result<Vec<Service>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, collection_id, folder_id, name, description, service_type, config, deprecated, sort_order, created_at, updated_at, sync_version, source, deleted
             FROM services WHERE collection_id = ?1 AND deleted = 0 ORDER BY sort_order, name",
        )?;
        
        let rows = stmt.query_map(params![collection_id.to_string()], |row| {
            Ok(row_to_service(row).unwrap())
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn row_to_service(row: &rusqlite::Row) -> Result<Service> {
    let id: String = row.get(0)?;
    let collection_id: String = row.get(1)?;
    let folder_id: Option<String> = row.get(2)?;
    let name: String = row.get(3)?;
    let description: Option<String> = row.get(4)?;
    let service_type_str: String = row.get(5)?;
    let config: String = row.get(6)?;
    let deprecated: bool = row.get(7)?;
    let sort_order: i32 = row.get(8)?;
    let created_at: i64 = row.get(9)?;
    let updated_at: i64 = row.get(10)?;
    let sync_version: i64 = row.get(11)?;
    let source: String = row.get(12)?;
    let deleted: bool = row.get(13)?;

    let service_type = deserialize_service_type(&service_type_str, &config)?;

    Ok(Service {
        id: Uuid::parse_str(&id)?,
        collection_id: Uuid::parse_str(&collection_id)?,
        folder_id: folder_id.map(|s| Uuid::parse_str(&s).unwrap()),
        name: name.into(),
        description,
        service_type,
        deprecated,
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

fn serialize_service_type(st: &ServiceType) -> Result<(&'static str, String)> {
    match st {
        ServiceType::Http(config) => Ok(("http", serde_json::to_string(config)?)),
        ServiceType::Grpc(config) => Ok(("grpc", serde_json::to_string(config)?)),
        ServiceType::WebSocket(config) => Ok(("websocket", serde_json::to_string(config)?)),
    }
}

fn deserialize_service_type(type_str: &str, config: &str) -> Result<ServiceType> {
    match type_str {
        "http" => Ok(ServiceType::Http(serde_json::from_str(config)?)),
        "grpc" => Ok(ServiceType::Grpc(serde_json::from_str(config)?)),
        "websocket" => Ok(ServiceType::WebSocket(serde_json::from_str(config)?)),
        _ => anyhow::bail!("Unknown service type: {}", type_str),
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
