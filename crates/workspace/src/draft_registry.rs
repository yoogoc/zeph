//! Per-service HTTP editing draft with CRDT support.
//!
//! `ServiceDraft` manages the in-memory editing state for a single HTTP service,
//! providing undo/redo and persistence to the database. Each open HTTP view owns
//! one `ServiceDraft`. Updates are persisted immediately via `DraftStore::upsert`.

use anyhow::Result;
use automerge::AutoCommit;
use autosurgeon::{hydrate, reconcile};
use models::{HttpDraftConfig, Service, ServiceType};
use std::collections::VecDeque;
use std::sync::Arc;
use storage::{Draft, DraftStore};
use uuid::Uuid;

/// In-memory editing draft for a single HTTP service.
///
/// Provides CRDT-based state (automerge), undo/redo checkpoints, and immediate
/// persistence to SQLite via `DraftStore`.
pub struct ServiceDraft {
    draft_store: Arc<DraftStore>,
    service_id: Uuid,
    workspace_id: Uuid,

    /// Automerge document for this service's draft.
    doc: AutoCommit,
    /// Whether the draft differs from the service's saved config.
    dirty: bool,
    /// Undo snapshot stack (binary automerge state).
    undo_snapshots: VecDeque<Vec<u8>>,
    /// Redo snapshot stack.
    redo_snapshots: VecDeque<Vec<u8>>,
    /// `service.updated_at` when the draft was created (for conflict detection).
    base_version: i64,
    /// Original config, to detect when edits revert back to the saved state.
    original_config: HttpDraftConfig,
}

impl ServiceDraft {
    /// Creates a draft for `service`, loading any persisted draft from the database.
    pub fn new(draft_store: Arc<DraftStore>, service: &Service) -> Self {
        let original_config = match &service.service_type {
            ServiceType::Http(config) => HttpDraftConfig::from_http_config(config),
            _ => HttpDraftConfig::default(),
        };

        // Reuse a persisted draft if present, else initialize from the service config.
        let (doc, dirty) = match draft_store.get(service.id) {
            Ok(Some(draft)) => match AutoCommit::load(&draft.automerge_state) {
                Ok(doc) => {
                    let dirty = hydrate(&doc)
                        .map(|cfg: HttpDraftConfig| cfg != original_config)
                        .unwrap_or(false);
                    (doc, dirty)
                }
                Err(_) => (Self::init_doc(&original_config), false),
            },
            _ => (Self::init_doc(&original_config), false),
        };

        Self {
            draft_store,
            service_id: service.id,
            workspace_id: service.workspace_id,
            doc,
            dirty,
            undo_snapshots: VecDeque::new(),
            redo_snapshots: VecDeque::new(),
            base_version: service.updated_at,
            original_config,
        }
    }

    fn init_doc(config: &HttpDraftConfig) -> AutoCommit {
        let mut doc = AutoCommit::new();
        let _ = reconcile(&mut doc, config);
        doc
    }

    /// The current draft config.
    pub fn config(&self) -> HttpDraftConfig {
        hydrate(&self.doc).unwrap_or_default()
    }

    /// Whether the draft has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Updates the draft with a new config (CRDT reconcile) and persists it.
    ///
    /// Marks dirty unless the new config matches the original saved config.
    pub fn update(&mut self, config: &HttpDraftConfig) -> Result<()> {
        reconcile(&mut self.doc, config)?;
        self.dirty = *config != self.original_config;
        self.persist()?;
        Ok(())
    }

    fn persist(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let draft = Draft {
            service_id: self.service_id,
            workspace_id: self.workspace_id,
            automerge_state: self.doc.clone().save(),
            base_version: self.base_version,
            created_at: now,
            updated_at: now,
        };
        self.draft_store.upsert(&draft)
    }

    /// Whether the service was updated since this draft was based on it.
    pub fn has_conflict(&self, service_updated_at: i64) -> bool {
        self.base_version < service_updated_at
    }

    // ========== Undo/Redo ==========

    /// Records a checkpoint for undo (e.g. on blur).
    pub fn checkpoint(&mut self) {
        let snapshot = self.doc.save();
        if self.undo_snapshots.back() != Some(&snapshot) {
            self.undo_snapshots.push_back(snapshot);
            self.redo_snapshots.clear();
        }
    }

    /// Undoes to the previous checkpoint, returning the restored config.
    pub fn undo(&mut self) -> Option<HttpDraftConfig> {
        // Need current state + a previous state to restore to.
        if self.undo_snapshots.len() < 2 {
            return None;
        }
        let current = self.undo_snapshots.pop_back()?;
        self.redo_snapshots.push_back(current);

        let previous = self.undo_snapshots.back()?;
        let doc = AutoCommit::load(previous).ok()?;
        let config = hydrate(&doc).ok()?;
        self.doc = doc;
        self.dirty = true;
        let _ = self.persist();
        Some(config)
    }

    /// Redoes to the next checkpoint, returning the restored config.
    pub fn redo(&mut self) -> Option<HttpDraftConfig> {
        let snapshot = self.redo_snapshots.pop_back()?;
        let doc = AutoCommit::load(&snapshot).ok()?;
        let config = hydrate(&doc).ok()?;
        self.doc = doc;
        self.dirty = true;
        self.undo_snapshots.push_back(snapshot);
        let _ = self.persist();
        Some(config)
    }

    /// Whether undo is available (needs current + previous checkpoints).
    pub fn can_undo(&self) -> bool {
        self.undo_snapshots.len() >= 2
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_snapshots.is_empty()
    }

    // ========== Save / Discard ==========

    /// Commits the draft: returns the config to persist to the service and clears
    /// the in-memory + database draft.
    pub fn save_to_service(&mut self) -> Result<HttpDraftConfig> {
        let config: HttpDraftConfig = hydrate(&self.doc)?;
        self.draft_store.delete(self.service_id)?;
        self.dirty = false;
        self.undo_snapshots.clear();
        self.redo_snapshots.clear();
        self.original_config = config.clone();
        Ok(config)
    }

    /// Discards draft changes and resets to the service's saved config.
    pub fn discard(&mut self) -> Result<HttpDraftConfig> {
        self.draft_store.delete(self.service_id)?;
        self.doc = Self::init_doc(&self.original_config);
        self.dirty = false;
        self.undo_snapshots.clear();
        self.redo_snapshots.clear();
        Ok(self.original_config.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{HttpConfig, HttpMethod};
    use rusqlite;
    use storage::Database;

    fn setup() -> (Arc<DraftStore>, Service) {
        let db = Arc::new(Database::open_in_memory().expect("Failed to create database"));

        let workspace_id = Uuid::new_v4();
        {
            let conn = db.conn();
            conn.execute(
                "INSERT INTO workspaces (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    workspace_id.to_string(),
                    "Test Workspace",
                    chrono::Utc::now().timestamp(),
                    chrono::Utc::now().timestamp()
                ],
            )
            .expect("Failed to insert workspace");
        }

        let service = Service::new(
            workspace_id,
            "Test Service",
            ServiceType::Http(HttpConfig::default()),
        );
        {
            let conn = db.conn();
            let store = storage::ServiceStore::new(conn);
            store.insert(&service).expect("Failed to insert service");
        }

        let draft_store = Arc::new(DraftStore::new(db.conn()));
        (draft_store, service)
    }

    #[test]
    fn test_new_initializes_from_service() {
        let (store, service) = setup();
        let draft = ServiceDraft::new(store, &service);
        assert_eq!(draft.config().method, HttpMethod::Get);
        assert!(!draft.is_dirty());
    }

    #[test]
    fn test_update_marks_dirty() {
        let (store, service) = setup();
        let mut draft = ServiceDraft::new(store, &service);
        assert!(!draft.is_dirty());

        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Post;
        draft.update(&config).expect("Failed to update");

        assert!(draft.is_dirty());
    }

    #[test]
    fn test_update_revert_clears_dirty() {
        let (store, service) = setup();
        let mut draft = ServiceDraft::new(store, &service);

        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Post;
        draft.update(&config).expect("Failed to update");
        assert!(draft.is_dirty());

        // Revert to original
        draft
            .update(&HttpDraftConfig::default())
            .expect("Failed to update");
        assert!(!draft.is_dirty());
    }

    #[test]
    fn test_undo_redo() {
        let (store, service) = setup();
        let mut draft = ServiceDraft::new(store, &service);
        draft.checkpoint();

        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Post;
        draft.update(&config).expect("Failed to update");
        draft.checkpoint();

        assert_eq!(draft.config().method, HttpMethod::Post);

        assert!(draft.can_undo());
        let undone = draft.undo().expect("Failed to undo");
        assert_eq!(undone.method, HttpMethod::Get);

        assert!(draft.can_redo());
        let redone = draft.redo().expect("Failed to redo");
        assert_eq!(redone.method, HttpMethod::Post);
    }

    #[test]
    fn test_save_to_service() {
        let (store, service) = setup();
        let mut draft = ServiceDraft::new(store, &service);

        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Delete;
        draft.update(&config).expect("Failed to update");

        let saved = draft.save_to_service().expect("Failed to save");
        assert_eq!(saved.method, HttpMethod::Delete);
        assert!(!draft.is_dirty());
    }

    #[test]
    fn test_discard() {
        let (store, service) = setup();
        let mut draft = ServiceDraft::new(store, &service);

        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Delete;
        draft.update(&config).expect("Failed to update");

        let original = draft.discard().expect("Failed to discard");
        assert_eq!(original.method, HttpMethod::Get);
        assert!(!draft.is_dirty());
        assert_eq!(draft.config().method, HttpMethod::Get);
    }

    #[test]
    fn test_persisted_draft_reloads() {
        let (store, service) = setup();
        {
            let mut draft = ServiceDraft::new(store.clone(), &service);
            let mut config = HttpDraftConfig::default();
            config.method = HttpMethod::Patch;
            draft.update(&config).expect("Failed to update");
        }
        // New draft for the same service should reload the persisted state.
        let draft = ServiceDraft::new(store, &service);
        assert_eq!(draft.config().method, HttpMethod::Patch);
        assert!(draft.is_dirty());
    }
}
