use crate::schema;
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;

pub struct Database {
    conn: Arc<Connection>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Arc::new(Connection::open(path)?);
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Arc::new(Connection::open_in_memory()?);
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        self.conn.execute_batch(schema::SCHEMA)?;
        Ok(())
    }

    pub fn conn(&self) -> Arc<Connection> {
        self.conn.clone()
    }
}
