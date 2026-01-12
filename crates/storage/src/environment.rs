use anyhow::Result;
use models::{DataSource, Environment, EnvironmentVariable, SyncMeta, VariableType};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub struct EnvironmentStore<'a> {
    conn: &'a Connection,
}

impl<'a> EnvironmentStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, env: &Environment) -> Result<()> {
        self.conn.execute(
            "INSERT INTO environments (id, workspace_id, name, base_url, is_default, sort_order, created_at, updated_at, sync_version, source, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                env.id.to_string(),
                env.workspace_id.to_string(),
                env.name.to_string(),
                env.base_url.to_string(),
                env.is_default,
                env.sort_order,
                env.created_at,
                env.updated_at,
                env.sync.sync_version,
                source_to_str(&env.sync.source),
                env.sync.deleted,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, env: &Environment) -> Result<()> {
        self.conn.execute(
            "UPDATE environments SET name = ?2, base_url = ?3, is_default = ?4, sort_order = ?5, updated_at = ?6
             WHERE id = ?1 AND deleted = 0",
            params![
                env.id.to_string(),
                env.name.to_string(),
                env.base_url.to_string(),
                env.is_default,
                env.sort_order,
                env.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "UPDATE environments SET deleted = 1, updated_at = ?2 WHERE id = ?1",
            params![id.to_string(), now],
        )?;
        Ok(())
    }

    pub fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<Environment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace_id, name, base_url, is_default, sort_order, created_at, updated_at, sync_version, source, deleted
             FROM environments WHERE workspace_id = ?1 AND deleted = 0 ORDER BY sort_order, name",
        )?;

        let rows = stmt.query_map(params![workspace_id.to_string()], |row| {
            Ok(row_to_environment(row).unwrap())
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn row_to_environment(row: &rusqlite::Row) -> Result<Environment> {
    let id: String = row.get(0)?;
    let workspace_id: String = row.get(1)?;
    let name: String = row.get(2)?;
    let base_url: String = row.get(3)?;
    let is_default: bool = row.get(4)?;
    let sort_order: i32 = row.get(5)?;
    let created_at: i64 = row.get(6)?;
    let updated_at: i64 = row.get(7)?;
    let sync_version: i64 = row.get(8)?;
    let source: String = row.get(9)?;
    let deleted: bool = row.get(10)?;

    Ok(Environment {
        id: Uuid::parse_str(&id)?,
        workspace_id: Uuid::parse_str(&workspace_id)?,
        name: name.into(),
        base_url: base_url.into(),
        is_default,
        sort_order,
        created_at,
        updated_at,
        sync: SyncMeta {
            sync_version,
            source: str_to_source(&source),
            deleted,
        },
        variables: vec![],
    })
}

pub struct EnvironmentVariableStore<'a> {
    conn: &'a Connection,
}

impl<'a> EnvironmentVariableStore<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, var: &EnvironmentVariable) -> Result<()> {
        self.conn.execute(
            "INSERT INTO environment_variables (id, environment_id, name, var_type, remote_value, local_value, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                var.id.to_string(),
                var.environment_id.to_string(),
                var.name.to_string(),
                var_type_to_str(&var.var_type),
                var.remote_value.as_ref().map(|x|x.to_string()),
                var.local_value.as_ref().map(|x|x.to_string()),
                var.description.as_ref().map(|x|x.to_string()),
                var.created_at,
                var.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, var: &EnvironmentVariable) -> Result<()> {
        self.conn.execute(
            "UPDATE environment_variables SET name = ?2, var_type = ?3, remote_value = ?4, local_value = ?5, description = ?6, updated_at = ?7
             WHERE id = ?1",
            params![
                var.id.to_string(),
                var.name.as_ref(),
                var_type_to_str(&var.var_type),
                var.remote_value.as_ref().map(|x|x.to_string()),
                var.local_value.as_ref().map(|x|x.to_string()),
                var.description.as_ref().map(|x|x.to_string()),
                var.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM environment_variables WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    pub fn list_by_environment(&self, environment_id: Uuid) -> Result<Vec<EnvironmentVariable>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, environment_id, name, var_type, remote_value, local_value, description, created_at, updated_at
             FROM environment_variables WHERE environment_id = ?1 ORDER BY name",
        )?;

        let rows = stmt.query_map(params![environment_id.to_string()], |row| {
            Ok(row_to_variable(row).unwrap())
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn row_to_variable(row: &rusqlite::Row) -> Result<EnvironmentVariable> {
    let id: String = row.get(0)?;
    let environment_id: String = row.get(1)?;
    let name: String = row.get(2)?;
    let var_type: String = row.get(3)?;
    let remote_value: Option<String> = row.get(4)?;
    let local_value: Option<String> = row.get(5)?;
    let description: Option<String> = row.get(6)?;
    let created_at: i64 = row.get(7)?;
    let updated_at: i64 = row.get(8)?;

    Ok(EnvironmentVariable {
        id: Uuid::parse_str(&id)?,
        environment_id: Uuid::parse_str(&environment_id)?,
        name: name.into(),
        var_type: str_to_var_type(&var_type),
        remote_value: remote_value.map(|x| x.into()),
        local_value: local_value.map(|x| x.into()),
        description: description.map(|x| x.into()),
        created_at,
        updated_at,
    })
}

fn var_type_to_str(t: &VariableType) -> &'static str {
    match t {
        VariableType::Default => "default",
        VariableType::Secret => "secret",
    }
}

fn str_to_var_type(s: &str) -> VariableType {
    match s {
        "secret" => VariableType::Secret,
        _ => VariableType::Default,
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
