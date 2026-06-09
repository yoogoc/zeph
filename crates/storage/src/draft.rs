//! Draft storage for HTTP service editing state with CRDT support.
//!
//! Drafts store the automerge state of services being edited, allowing
//! users to switch tabs without losing unsaved changes.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Arc;
use uuid::Uuid;

/// A draft representing unsaved editing state for a service
#[derive(Debug, Clone)]
pub struct Draft {
    pub service_id: Uuid,
    pub workspace_id: Uuid,
    /// Serialized automerge document state
    pub automerge_state: Vec<u8>,
    /// The service.updated_at value this draft was based on
    pub base_version: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct DraftStore {
    conn: Arc<Connection>,
}

impl DraftStore {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    /// Insert or update a draft (upsert)
    pub fn upsert(&self, draft: &Draft) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO drafts (service_id, workspace_id, automerge_state, base_version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(service_id) DO UPDATE SET
                automerge_state = excluded.automerge_state,
                base_version = excluded.base_version,
                updated_at = ?6",
            params![
                draft.service_id.to_string(),
                draft.workspace_id.to_string(),
                &draft.automerge_state,
                draft.base_version,
                draft.created_at,
                now,
            ],
        )?;

        Ok(())
    }

    /// Get a draft by service ID
    pub fn get(&self, service_id: Uuid) -> Result<Option<Draft>> {
        let mut stmt = self.conn.prepare(
            "SELECT service_id, workspace_id, automerge_state, base_version, created_at, updated_at
             FROM drafts
             WHERE service_id = ?1",
        )?;

        let draft = stmt
            .query_row(params![service_id.to_string()], |row| {
                Ok(Draft {
                    service_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
                    workspace_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    automerge_state: row.get(2)?,
                    base_version: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .optional()?;

        Ok(draft)
    }

    /// Delete a draft by service ID
    pub fn delete(&self, service_id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM drafts WHERE service_id = ?1",
            params![service_id.to_string()],
        )?;
        Ok(())
    }

    /// List all drafts for a workspace
    pub fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<Draft>> {
        let mut stmt = self.conn.prepare(
            "SELECT service_id, workspace_id, automerge_state, base_version, created_at, updated_at
             FROM drafts
             WHERE workspace_id = ?1
             ORDER BY updated_at DESC",
        )?;

        let drafts = stmt
            .query_map(params![workspace_id.to_string()], |row| {
                Ok(Draft {
                    service_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
                    workspace_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    automerge_state: row.get(2)?,
                    base_version: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(drafts)
    }

    /// Delete all drafts for a workspace
    pub fn delete_by_workspace(&self, workspace_id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM drafts WHERE workspace_id = ?1",
            params![workspace_id.to_string()],
        )?;
        Ok(())
    }

    /// Check if a draft exists for a service
    pub fn exists(&self, service_id: Uuid) -> Result<bool> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM drafts WHERE service_id = ?1",
            params![service_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get the count of drafts for a workspace
    pub fn count_by_workspace(&self, workspace_id: Uuid) -> Result<i32> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM drafts WHERE workspace_id = ?1",
            params![workspace_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::{ServiceStore, WorkspaceStore};
    use models::{HttpConfig, Service, ServiceType, Workspace};

    fn setup_db() -> Database {
        Database::open_in_memory().expect("Failed to create database")
    }

    /// Helper to create a workspace and service for draft tests
    fn create_workspace_and_service(db: &Database) -> (Uuid, Uuid) {
        let conn = db.conn();
        let workspace_store = WorkspaceStore::new(conn.clone());
        let service_store = ServiceStore::new(conn.clone());

        let workspace = Workspace::new("Test Workspace");
        let workspace_id = workspace.id;
        workspace_store
            .insert(&workspace)
            .expect("Failed to insert workspace");

        let service = Service::new(
            workspace_id,
            "Test Service",
            ServiceType::Http(HttpConfig::default()),
        );
        let service_id = service.id;
        service_store
            .insert(&service)
            .expect("Failed to insert service");

        (workspace_id, service_id)
    }

    #[test]
    fn test_draft_upsert_and_get() {
        let db = setup_db();
        let (workspace_id, service_id) = create_workspace_and_service(&db);
        let conn = db.conn();
        let store = DraftStore::new(conn);

        let now = chrono::Utc::now().timestamp();

        let draft = Draft {
            service_id,
            workspace_id,
            automerge_state: vec![1, 2, 3, 4, 5],
            base_version: 100,
            created_at: now,
            updated_at: now,
        };

        // Insert
        store.upsert(&draft).expect("Failed to insert draft");

        // Get
        let retrieved = store.get(service_id).expect("Failed to get draft");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.service_id, service_id);
        assert_eq!(retrieved.workspace_id, workspace_id);
        assert_eq!(retrieved.automerge_state, vec![1, 2, 3, 4, 5]);
        assert_eq!(retrieved.base_version, 100);

        // Update
        let updated_draft = Draft {
            service_id,
            workspace_id,
            automerge_state: vec![6, 7, 8, 9, 10],
            base_version: 200,
            created_at: now,
            updated_at: now,
        };
        store
            .upsert(&updated_draft)
            .expect("Failed to update draft");

        let retrieved = store.get(service_id).expect("Failed to get draft");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.automerge_state, vec![6, 7, 8, 9, 10]);
        assert_eq!(retrieved.base_version, 200);
    }

    #[test]
    fn test_draft_delete() {
        let db = setup_db();
        let (workspace_id, service_id) = create_workspace_and_service(&db);
        let conn = db.conn();
        let store = DraftStore::new(conn);

        let now = chrono::Utc::now().timestamp();

        let draft = Draft {
            service_id,
            workspace_id,
            automerge_state: vec![1, 2, 3],
            base_version: 100,
            created_at: now,
            updated_at: now,
        };

        store.upsert(&draft).expect("Failed to insert draft");
        assert!(store.exists(service_id).expect("Failed to check exists"));

        store.delete(service_id).expect("Failed to delete draft");
        assert!(!store.exists(service_id).expect("Failed to check exists"));
    }

    #[test]
    fn test_draft_list_by_workspace() {
        let db = setup_db();
        let conn = db.conn();
        let workspace_store = WorkspaceStore::new(conn.clone());
        let service_store = ServiceStore::new(conn.clone());
        let store = DraftStore::new(conn.clone());

        let workspace = Workspace::new("Test Workspace");
        let workspace_id = workspace.id;
        workspace_store
            .insert(&workspace)
            .expect("Failed to insert workspace");

        let now = chrono::Utc::now().timestamp();

        // Create 3 services and drafts
        for i in 0..3 {
            let service = Service::new(
                workspace_id,
                format!("Test Service {}", i),
                ServiceType::Http(HttpConfig::default()),
            );
            let service_id = service.id;
            service_store
                .insert(&service)
                .expect("Failed to insert service");
            let draft = Draft {
                service_id,
                workspace_id,
                automerge_state: vec![i as u8],
                base_version: i as i64,
                created_at: now,
                updated_at: now + i as i64,
            };
            store.upsert(&draft).expect("Failed to insert draft");
        }

        let drafts = store
            .list_by_workspace(workspace_id)
            .expect("Failed to list drafts");
        assert_eq!(drafts.len(), 3);

        // Should be ordered by updated_at DESC
        assert!(drafts[0].updated_at >= drafts[1].updated_at);
        assert!(drafts[1].updated_at >= drafts[2].updated_at);
    }

    #[test]
    fn test_draft_delete_by_workspace() {
        let db = setup_db();
        let conn = db.conn();
        let workspace_store = WorkspaceStore::new(conn.clone());
        let service_store = ServiceStore::new(conn.clone());
        let store = DraftStore::new(conn.clone());

        let workspace = Workspace::new("Test Workspace");
        let workspace_id = workspace.id;
        workspace_store
            .insert(&workspace)
            .expect("Failed to insert workspace");

        let now = chrono::Utc::now().timestamp();

        // Create 3 services and drafts
        for i in 0..3 {
            let service = Service::new(
                workspace_id,
                format!("Test Service {}", i),
                ServiceType::Http(HttpConfig::default()),
            );
            let service_id = service.id;
            service_store
                .insert(&service)
                .expect("Failed to insert service");
            let draft = Draft {
                service_id,
                workspace_id,
                automerge_state: vec![1, 2, 3],
                base_version: 100,
                created_at: now,
                updated_at: now,
            };
            store.upsert(&draft).expect("Failed to insert draft");
        }

        assert_eq!(
            store
                .count_by_workspace(workspace_id)
                .expect("Failed to count"),
            3
        );

        store
            .delete_by_workspace(workspace_id)
            .expect("Failed to delete");
        assert_eq!(
            store
                .count_by_workspace(workspace_id)
                .expect("Failed to count"),
            0
        );
    }

    #[test]
    fn test_draft_get_nonexistent() {
        let db = setup_db();
        let conn = db.conn();
        let store = DraftStore::new(conn);

        let result = store.get(Uuid::new_v4()).expect("Failed to get draft");
        assert!(result.is_none());
    }
}
