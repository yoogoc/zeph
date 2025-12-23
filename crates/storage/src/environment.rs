use anyhow::Result;
use models::{EnvVariable, Environment};
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
        let variables = serde_json::to_string(&env.variables)?;
        
        self.conn.execute(
            "INSERT INTO environments (id, collection_id, name, url, description, variables, is_default, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                env.id.to_string(),
                env.collection_id.to_string(),
                env.name.as_ref(),
                env.url,
                env.description,
                variables,
                env.is_default,
                env.sort_order,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, env: &Environment) -> Result<()> {
        let variables = serde_json::to_string(&env.variables)?;
        
        self.conn.execute(
            "UPDATE environments SET name = ?2, url = ?3, description = ?4, variables = ?5, is_default = ?6, sort_order = ?7
             WHERE id = ?1",
            params![
                env.id.to_string(),
                env.name.as_ref(),
                env.url,
                env.description,
                variables,
                env.is_default,
                env.sort_order,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.conn.execute("DELETE FROM environments WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    pub fn list_by_collection(&self, collection_id: Uuid) -> Result<Vec<Environment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, collection_id, name, url, description, variables, is_default, sort_order
             FROM environments WHERE collection_id = ?1 ORDER BY sort_order, name",
        )?;
        
        let rows = stmt.query_map(params![collection_id.to_string()], |row| {
            Ok(row_to_environment(row).unwrap())
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn row_to_environment(row: &rusqlite::Row) -> Result<Environment> {
    let id: String = row.get(0)?;
    let collection_id: String = row.get(1)?;
    let name: String = row.get(2)?;
    let url: String = row.get(3)?;
    let description: Option<String> = row.get(4)?;
    let variables_json: String = row.get(5)?;
    let is_default: bool = row.get(6)?;
    let sort_order: i32 = row.get(7)?;

    let variables: Vec<EnvVariable> = serde_json::from_str(&variables_json).unwrap_or_default();

    Ok(Environment {
        id: Uuid::parse_str(&id)?,
        collection_id: Uuid::parse_str(&collection_id)?,
        name: name.into(),
        url,
        description,
        variables,
        is_default,
        sort_order,
    })
}
