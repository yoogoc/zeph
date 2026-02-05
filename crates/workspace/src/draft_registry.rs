//! Draft Registry for HTTP service editing state with CRDT support.
//!
//! The DraftRegistry manages in-memory editing state for HTTP services,
//! providing undo/redo functionality and persistence to the database.
//! It only keeps drafts for the current workspace in memory.

use anyhow::Result;
use automerge::AutoCommit;
use autosurgeon::{hydrate, reconcile};
use models::{HttpDraftConfig, Service, ServiceType};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use storage::{Database, Draft, DraftStore};
use uuid::Uuid;

/// Event emitted when a draft changes
#[derive(Clone, Debug)]
pub enum DraftEvent {
    /// Draft content was updated
    Updated(Uuid),
    /// Undo was performed
    Undone(Uuid),
    /// Redo was performed
    Redone(Uuid),
    /// Draft was saved to service
    Saved(Uuid),
}

/// The DraftRegistry manages editing state for HTTP services.
///
/// It provides:
/// - CRDT-based state management using automerge
/// - Undo/redo functionality (checkpoints created on blur)
/// - Persistence to SQLite via DraftStore
/// - Workspace-scoped memory management
pub struct DraftRegistry {
    db: Arc<Database>,

    /// Current workspace ID (only this workspace's drafts are in memory)
    current_workspace_id: Option<Uuid>,

    /// Automerge documents for each service (only current workspace)
    docs: HashMap<Uuid, AutoCommit>,

    /// Services that have unsaved changes (dirty)
    dirty: HashSet<Uuid>,

    /// Undo snapshot stacks (binary automerge state)
    undo_snapshots: HashMap<Uuid, VecDeque<Vec<u8>>>,

    /// Redo snapshot stacks
    redo_snapshots: HashMap<Uuid, VecDeque<Vec<u8>>>,

    /// Base version for each draft (service.updated_at when draft was created)
    base_versions: HashMap<Uuid, i64>,

    /// Original configs for each draft (to compare for dirty state)
    original_configs: HashMap<Uuid, HttpDraftConfig>,
}

impl DraftRegistry {
    /// Create a new DraftRegistry
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            current_workspace_id: None,
            docs: HashMap::new(),
            dirty: HashSet::new(),
            undo_snapshots: HashMap::new(),
            redo_snapshots: HashMap::new(),
            base_versions: HashMap::new(),
            original_configs: HashMap::new(),
        }
    }

    // ========== Workspace Lifecycle ==========

    /// Switch to a different workspace
    ///
    /// This will:
    /// 1. Flush all dirty drafts to the database
    /// 2. Clear in-memory state
    /// 3. Load drafts for the new workspace
    pub fn switch_workspace(&mut self, new_workspace_id: Uuid) -> Result<()> {
        // Flush current workspace's dirty drafts
        self.flush_all()?;

        // Clear memory
        self.clear_memory();

        // Load new workspace's drafts
        self.load_workspace(new_workspace_id)?;

        self.current_workspace_id = Some(new_workspace_id);
        Ok(())
    }

    /// Unload the current workspace (e.g., when going to home)
    pub fn unload_workspace(&mut self) -> Result<()> {
        self.flush_all()?;
        self.clear_memory();
        self.current_workspace_id = None;
        Ok(())
    }

    /// Load all drafts for a workspace from the database
    fn load_workspace(&mut self, workspace_id: Uuid) -> Result<()> {
        let conn = self.db.conn();
        let draft_store = DraftStore::new(&conn);

        for draft in draft_store.list_by_workspace(workspace_id)? {
            if let Ok(doc) = AutoCommit::load(&draft.automerge_state) {
                self.docs.insert(draft.service_id, doc);
                self.base_versions
                    .insert(draft.service_id, draft.base_version);
            }
        }

        Ok(())
    }

    /// Flush all dirty drafts to the database
    pub fn flush_all(&mut self) -> Result<()> {
        let workspace_id = match self.current_workspace_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let conn = self.db.conn();
        let draft_store = DraftStore::new(&conn);
        let now = chrono::Utc::now().timestamp();

        for service_id in self.dirty.drain() {
            if let Some(doc) = self.docs.get_mut(&service_id) {
                let draft = Draft {
                    service_id,
                    workspace_id,
                    automerge_state: doc.save(),
                    base_version: *self.base_versions.get(&service_id).unwrap_or(&0),
                    created_at: now,
                    updated_at: now,
                };
                draft_store.upsert(&draft)?;
            }
        }

        Ok(())
    }

    /// Clear all in-memory state
    fn clear_memory(&mut self) {
        self.docs.clear();
        self.dirty.clear();
        self.undo_snapshots.clear();
        self.redo_snapshots.clear();
        self.base_versions.clear();
        self.original_configs.clear();
    }

    // ========== Draft Operations ==========

    /// Get or initialize a draft for a service
    ///
    /// If no draft exists, creates one from the service's current config.
    pub fn get_or_init(&mut self, service: &Service) -> Result<HttpDraftConfig> {
        let service_id = service.id;

        if !self.docs.contains_key(&service_id) {
            // Initialize from service config
            let mut doc = AutoCommit::new();
            if let ServiceType::Http(ref config) = service.service_type {
                let draft_config = HttpDraftConfig::from_http_config(config);
                reconcile(&mut doc, &draft_config)?;
                // Store original config for dirty comparison
                self.original_configs.insert(service_id, draft_config);
            }
            self.docs.insert(service_id, doc);
            self.base_versions.insert(service_id, service.updated_at);
        }

        let doc = self.docs.get(&service_id).unwrap();
        Ok(hydrate(doc)?)
    }

    /// Get the current draft config for a service (if it exists)
    pub fn get(&self, service_id: Uuid) -> Option<HttpDraftConfig> {
        self.docs.get(&service_id).and_then(|doc| hydrate(doc).ok())
    }

    /// Check if a draft exists for a service
    pub fn has_draft(&self, service_id: Uuid) -> bool {
        self.docs.contains_key(&service_id)
    }

    /// Update a draft with new config
    ///
    /// This performs a CRDT reconcile, which only updates changed fields.
    /// Marks as dirty only if the new config differs from the original config.
    /// If the user reverts changes back to the original, dirty is removed.
    pub fn update(&mut self, service_id: Uuid, config: &HttpDraftConfig) -> Result<()> {
        if let Some(doc) = self.docs.get_mut(&service_id) {
            // Perform CRDT reconcile
            reconcile(doc, config)?;

            // Compare with original config to determine dirty state
            if let Some(original) = self.original_configs.get(&service_id) {
                if original == config {
                    // Reverted to original - remove dirty flag
                    self.dirty.remove(&service_id);
                } else {
                    // Different from original - mark as dirty
                    self.dirty.insert(service_id);
                }
            }
        }
        Ok(())
    }

    /// Check if a draft has unsaved changes compared to the original
    pub fn has_changes(&self, service_id: Uuid, original: &HttpDraftConfig) -> bool {
        self.get(service_id)
            .map_or(false, |config| config != *original)
    }

    /// Check if a specific draft is dirty (has been modified since last save)
    pub fn is_dirty(&self, service_id: Uuid) -> bool {
        self.dirty.contains(&service_id)
    }

    /// Remove a draft (e.g., when closing a tab without saving)
    pub fn remove(&mut self, service_id: Uuid) -> Result<()> {
        self.docs.remove(&service_id);
        self.dirty.remove(&service_id);
        self.undo_snapshots.remove(&service_id);
        self.redo_snapshots.remove(&service_id);
        self.base_versions.remove(&service_id);
        self.original_configs.remove(&service_id);

        // Also remove from database
        let conn = self.db.conn();
        let draft_store = DraftStore::new(&conn);
        draft_store.delete(service_id)?;

        Ok(())
    }

    // ========== Undo/Redo ==========

    /// Create a checkpoint for undo (call this on blur events)
    pub fn checkpoint(&mut self, service_id: Uuid) {
        if let Some(doc) = self.docs.get_mut(&service_id) {
            let snapshot = doc.save();

            // Don't create duplicate checkpoints
            let stack = self.undo_snapshots.entry(service_id).or_default();
            if stack.back().map_or(true, |last| *last != snapshot) {
                stack.push_back(snapshot);
                // Clear redo stack when new checkpoint is created
                self.redo_snapshots.remove(&service_id);
            }
        }
    }

    /// Undo to the previous checkpoint
    ///
    /// The undo stack contains checkpoints: [state1, state2, ..., current_state]
    /// To undo, we pop the current state (move to redo) and restore from the previous.
    pub fn undo(&mut self, service_id: Uuid) -> Option<HttpDraftConfig> {
        let undo_stack = self.undo_snapshots.get_mut(&service_id)?;

        // Need at least 2 checkpoints: current state + state to restore to
        if undo_stack.len() < 2 {
            return None;
        }

        // Pop current state and move to redo stack
        let current_snapshot = undo_stack.pop_back()?;
        self.redo_snapshots
            .entry(service_id)
            .or_default()
            .push_back(current_snapshot);

        // Restore from the previous state (now the last item in undo stack)
        let previous_snapshot = undo_stack.back()?;
        let doc = AutoCommit::load(previous_snapshot).ok()?;
        let config: HttpDraftConfig = hydrate(&doc).ok()?;
        self.docs.insert(service_id, doc);
        self.dirty.insert(service_id);

        Some(config)
    }

    /// Redo to the next checkpoint
    pub fn redo(&mut self, service_id: Uuid) -> Option<HttpDraftConfig> {
        let redo_stack = self.redo_snapshots.get_mut(&service_id)?;
        let snapshot = redo_stack.pop_back()?;

        // Restore from snapshot first
        let doc = AutoCommit::load(&snapshot).ok()?;
        let config: HttpDraftConfig = hydrate(&doc).ok()?;
        self.docs.insert(service_id, doc);
        self.dirty.insert(service_id);

        // Push the restored state to undo stack
        self.undo_snapshots
            .entry(service_id)
            .or_default()
            .push_back(snapshot);

        Some(config)
    }

    /// Check if undo is available
    ///
    /// Requires at least 2 checkpoints: current state + state to restore to
    pub fn can_undo(&self, service_id: Uuid) -> bool {
        self.undo_snapshots
            .get(&service_id)
            .map_or(false, |s| s.len() >= 2)
    }

    /// Check if redo is available
    pub fn can_redo(&self, service_id: Uuid) -> bool {
        self.redo_snapshots
            .get(&service_id)
            .map_or(false, |s| !s.is_empty())
    }

    // ========== Save to Service ==========

    /// Save the draft to the service (commits changes)
    ///
    /// Returns the final HttpDraftConfig that should be persisted to the service.
    /// The caller is responsible for actually updating the service in the database.
    pub fn save_to_service(&mut self, service_id: Uuid) -> Result<HttpDraftConfig> {
        let doc = self
            .docs
            .get(&service_id)
            .ok_or_else(|| anyhow::anyhow!("Draft not found for service {}", service_id))?;

        let config: HttpDraftConfig = hydrate(doc)?;

        // Remove draft from database
        let conn = self.db.conn();
        let draft_store = DraftStore::new(&conn);
        draft_store.delete(service_id)?;

        // Clear in-memory state for this draft
        self.docs.remove(&service_id);
        self.dirty.remove(&service_id);
        self.undo_snapshots.remove(&service_id);
        self.redo_snapshots.remove(&service_id);
        self.base_versions.remove(&service_id);
        self.original_configs.remove(&service_id);

        Ok(config)
    }

    /// Discard draft changes and reload from service
    pub fn discard(&mut self, service: &Service) -> Result<HttpDraftConfig> {
        let service_id = service.id;

        // Remove from database
        let conn = self.db.conn();
        let draft_store = DraftStore::new(&conn);
        draft_store.delete(service_id)?;

        // Clear in-memory state
        self.docs.remove(&service_id);
        self.dirty.remove(&service_id);
        self.undo_snapshots.remove(&service_id);
        self.redo_snapshots.remove(&service_id);
        self.base_versions.remove(&service_id);
        self.original_configs.remove(&service_id);

        // Return original config as HttpDraftConfig
        if let ServiceType::Http(ref config) = service.service_type {
            Ok(HttpDraftConfig::from_http_config(config))
        } else {
            Err(anyhow::anyhow!("Service is not HTTP type"))
        }
    }

    // ========== CRDT Conflict Handling ==========

    /// Merge remote changes into a draft
    ///
    /// Used when sync brings in changes from another device.
    pub fn merge_remote(
        &mut self,
        service_id: Uuid,
        remote_state: &[u8],
    ) -> Result<HttpDraftConfig> {
        let doc = self
            .docs
            .get_mut(&service_id)
            .ok_or_else(|| anyhow::anyhow!("Draft not found for service {}", service_id))?;

        let mut remote_doc = AutoCommit::load(remote_state)?;
        doc.merge(&mut remote_doc)?;

        self.dirty.insert(service_id);
        Ok(hydrate(doc)?)
    }

    /// Check if a draft has a version conflict with the service
    ///
    /// Returns true if the service has been updated since the draft was created.
    pub fn has_conflict(&self, service_id: Uuid, service_updated_at: i64) -> bool {
        self.base_versions
            .get(&service_id)
            .map_or(false, |&base| base < service_updated_at)
    }

    // ========== Diagnostics ==========

    /// Get the number of drafts in memory
    pub fn draft_count(&self) -> usize {
        self.docs.len()
    }

    /// Get the number of dirty drafts
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }

    /// Get all service IDs that have drafts
    pub fn draft_service_ids(&self) -> Vec<Uuid> {
        self.docs.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::{HttpConfig, HttpMethod};
    use rusqlite;
    use storage::Database;

    fn setup() -> (Arc<Database>, Service) {
        let db = Arc::new(Database::open_in_memory().expect("Failed to create database"));

        // Create workspace and service in database
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
            let store = storage::ServiceStore::new(&conn);
            store.insert(&service).expect("Failed to insert service");
        }

        (db, service)
    }

    #[test]
    fn test_get_or_init_creates_draft() {
        let (db, service) = setup();
        let mut registry = DraftRegistry::new(db);
        registry.current_workspace_id = Some(service.workspace_id);

        let config = registry
            .get_or_init(&service)
            .expect("Failed to get_or_init");
        assert_eq!(config.method, HttpMethod::Get);
        assert!(registry.has_draft(service.id));
    }

    #[test]
    fn test_update_marks_dirty() {
        let (db, service) = setup();
        let mut registry = DraftRegistry::new(db);
        registry.current_workspace_id = Some(service.workspace_id);

        registry
            .get_or_init(&service)
            .expect("Failed to get_or_init");
        assert!(!registry.is_dirty(service.id));

        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Post;
        registry
            .update(service.id, &config)
            .expect("Failed to update");

        assert!(registry.is_dirty(service.id));
    }

    #[test]
    fn test_undo_redo() {
        let (db, service) = setup();
        let mut registry = DraftRegistry::new(db);
        registry.current_workspace_id = Some(service.workspace_id);

        // Initial state
        registry
            .get_or_init(&service)
            .expect("Failed to get_or_init");
        registry.checkpoint(service.id);

        // Make change
        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Post;
        registry
            .update(service.id, &config)
            .expect("Failed to update");
        registry.checkpoint(service.id);

        // Verify change
        let current = registry.get(service.id).unwrap();
        assert_eq!(current.method, HttpMethod::Post);

        // Undo
        assert!(registry.can_undo(service.id));
        let undone = registry.undo(service.id).expect("Failed to undo");
        assert_eq!(undone.method, HttpMethod::Get);

        // Redo
        assert!(registry.can_redo(service.id));
        let redone = registry.redo(service.id).expect("Failed to redo");
        assert_eq!(redone.method, HttpMethod::Post);
    }

    #[test]
    fn test_has_changes() {
        let (db, service) = setup();
        let mut registry = DraftRegistry::new(db);
        registry.current_workspace_id = Some(service.workspace_id);

        let original = HttpDraftConfig::default();
        registry
            .get_or_init(&service)
            .expect("Failed to get_or_init");

        // No changes yet
        assert!(!registry.has_changes(service.id, &original));

        // Make change
        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Post;
        registry
            .update(service.id, &config)
            .expect("Failed to update");

        // Now has changes
        assert!(registry.has_changes(service.id, &original));
    }

    #[test]
    fn test_save_to_service() {
        let (db, service) = setup();
        let mut registry = DraftRegistry::new(db);
        registry.current_workspace_id = Some(service.workspace_id);

        // Create and modify draft
        registry
            .get_or_init(&service)
            .expect("Failed to get_or_init");
        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Delete;
        registry
            .update(service.id, &config)
            .expect("Failed to update");

        // Save
        let saved_config = registry
            .save_to_service(service.id)
            .expect("Failed to save");
        assert_eq!(saved_config.method, HttpMethod::Delete);

        // Draft should be removed
        assert!(!registry.has_draft(service.id));
    }

    #[test]
    fn test_discard() {
        let (db, service) = setup();
        let mut registry = DraftRegistry::new(db);
        registry.current_workspace_id = Some(service.workspace_id);

        // Create and modify draft
        registry
            .get_or_init(&service)
            .expect("Failed to get_or_init");
        let mut config = HttpDraftConfig::default();
        config.method = HttpMethod::Delete;
        registry
            .update(service.id, &config)
            .expect("Failed to update");

        // Discard
        let original = registry.discard(&service).expect("Failed to discard");
        assert_eq!(original.method, HttpMethod::Get);

        // Draft should be removed
        assert!(!registry.has_draft(service.id));
    }
}
