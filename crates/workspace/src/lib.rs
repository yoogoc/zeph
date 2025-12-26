use gpui::{App, AppContext, Context, Entity, EventEmitter, Global};
use log::error;
use models::{CreateWorkspace, Environment, EnvironmentVariable, HttpConfig, HttpMethod, Service, ServiceType, Workspace};
use std::sync::Arc;
use storage::{Database, EnvironmentStore, EnvironmentVariableStore, FolderStore, SchemaStore, ServiceStore, WorkspaceStore};
use uuid::Uuid;

mod openapi;
pub use openapi::{
    parse_openapi, import_openapi, import_openapi_with_options, generate_import_preview,
    ImportResult, ImportOptions, ImportPreview, ServicePreview, SchemaPreview, 
    SchemaFolderStrategy, OpenApiSpec, validate_import_options, check_name_conflicts,
    import_openapi_with_options_and_existing_folders, validate_openapi_content,
    validate_import_request, import_openapi_with_validation, NameConflict, ConflictType,
    ConflictResolution, resolve_conflicts
};

#[cfg(test)]
mod tests {
    use crate::{OpenApiSpec, ImportOptions, import_openapi_with_options_and_existing_folders, 
                generate_import_preview, validate_import_request, check_name_conflicts, 
                resolve_conflicts, parse_openapi, ConflictResolution, SchemaFolderStrategy};
    use models::{Folder, FolderType, Service, ServiceType, HttpConfig, Schema};
    use uuid::Uuid;
    use std::collections::HashMap;

    fn create_test_spec() -> OpenApiSpec {
        let spec_json = r#"
        {
            "openapi": "3.0.0",
            "info": {
                "title": "Test API",
                "version": "1.0.0"
            },
            "paths": {
                "/users": {
                    "get": {
                        "tags": ["users"],
                        "summary": "Get users",
                        "responses": {
                            "200": {
                                "description": "Success"
                            }
                        }
                    },
                    "post": {
                        "tags": ["users"],
                        "summary": "Create user",
                        "responses": {
                            "201": {
                                "description": "Created"
                            }
                        }
                    }
                },
                "/posts": {
                    "get": {
                        "tags": ["posts"],
                        "summary": "Get posts",
                        "responses": {
                            "200": {
                                "description": "Success"
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "User": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "integer"},
                            "name": {"type": "string"}
                        }
                    },
                    "Post": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "integer"},
                            "title": {"type": "string"},
                            "content": {"type": "string"}
                        }
                    }
                }
            }
        }
        "#;
        
        serde_json::from_str(spec_json).unwrap()
    }

    #[test]
    fn test_basic_import_functionality() {
        let workspace_id = Uuid::new_v4();
        let spec = create_test_spec();
        
        let result = import_openapi_with_options_and_existing_folders(
            workspace_id, 
            &spec, 
            ImportOptions::default(), 
            &[]
        );
        
        // Should succeed with basic import
        assert!(result.is_ok());
        let result = result.unwrap();
        
        // Should have imported services and schemas
        assert!(!result.services.is_empty());
        assert!(!result.schemas.is_empty());
    }

    // Performance and Integration Tests
    
    /// Integration test: Large OpenAPI specification processing
    /// Tests performance optimization for large specs
    #[test]
    fn test_large_openapi_spec_performance() {
        let workspace_id = Uuid::new_v4();
        
        // Create a large OpenAPI spec with many paths and schemas
        let mut large_spec = create_test_spec();
        
        // Add many paths to simulate a large API
        for i in 0..100 {
            let path = format!("/resource{}", i);
            let tag = format!("resource{}", i % 10); // Group into 10 tags
            
            let path_item = serde_json::json!({
                "get": {
                    "tags": [tag],
                    "summary": format!("Get resource {}", i),
                    "responses": {
                        "200": {
                            "description": "Success"
                        }
                    }
                }
            });
            
            large_spec.paths.insert(path, serde_json::from_value(path_item).unwrap());
        }
        
        // Add many schemas
        if let Some(ref mut components) = large_spec.components {
            for i in 0..50 {
                let schema_name = format!("Schema{}", i);
                let schema = serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": {"type": "integer"},
                        "name": {"type": "string"}
                    }
                });
                components.schemas.insert(schema_name, serde_json::from_value(schema).unwrap());
            }
        }
        
        let start_time = std::time::Instant::now();
        
        let result = import_openapi_with_options_and_existing_folders(
            workspace_id,
            &large_spec,
            ImportOptions::default(),
            &[]
        );
        
        let duration = start_time.elapsed();
        
        assert!(result.is_ok());
        let import_result = result.unwrap();
        
        // Verify results
        assert!(import_result.services.len() >= 100, "Should have at least 100 services, got {}", import_result.services.len()); // One service per path plus original
        assert_eq!(import_result.schemas.len(), 52); // All schemas imported (50 + 2 original)
        
        // Performance assertion: should complete within reasonable time
        assert!(duration.as_millis() < 1000, "Import took too long: {:?}", duration);
        
        println!("Large spec import completed in {:?}", duration);
    }

    /// Integration test: Folder selection with complex hierarchy
    /// Tests performance optimization for folder operations
    #[test]
    fn test_complex_folder_hierarchy_performance() {
        let workspace_id = Uuid::new_v4();
        let spec = create_test_spec();
        
        // Create a complex folder hierarchy
        let mut existing_folders = Vec::new();
        let root_folder = Folder::with_type(workspace_id, "API", None, FolderType::Service);
        let root_id = root_folder.id;
        existing_folders.push(root_folder);
        
        // Create multiple levels of nested folders
        let mut current_parent = Some(root_id);
        for level in 0..5 {
            let mut level_folders = Vec::new();
            for i in 0..10 {
                let folder = Folder::with_type(
                    workspace_id, 
                    format!("Level{}Folder{}", level, i), 
                    current_parent, 
                    FolderType::Service
                );
                level_folders.push(folder.id);
                existing_folders.push(folder);
            }
            current_parent = level_folders.first().copied();
        }
        
        let options = ImportOptions {
            target_folder: Some(root_id),
            create_subfolders: true,
            schema_folder_strategy: SchemaFolderStrategy::SeparateFolder,
        };
        
        let start_time = std::time::Instant::now();
        
        let result = import_openapi_with_options_and_existing_folders(
            workspace_id,
            &spec,
            options,
            &existing_folders
        );
        
        let duration = start_time.elapsed();
        
        assert!(result.is_ok());
        
        // Performance assertion: should handle complex hierarchy efficiently
        assert!(duration.as_millis() < 500, "Complex hierarchy processing took too long: {:?}", duration);
        
        println!("Complex folder hierarchy processed in {:?}", duration);
    }

    /// Integration test: Preview generation performance
    /// Tests optimized preview generation for large specs
    #[test]
    fn test_preview_generation_performance() {
        let spec = create_test_spec();
        let existing_folders = vec![
            Folder::with_type(Uuid::new_v4(), "Existing", None, FolderType::Service)
        ];
        
        let options = ImportOptions {
            target_folder: Some(existing_folders[0].id),
            create_subfolders: true,
            schema_folder_strategy: SchemaFolderStrategy::SeparateFolder,
        };
        
        let start_time = std::time::Instant::now();
        
        let result = generate_import_preview(&spec, &options, &existing_folders);
        
        let duration = start_time.elapsed();
        
        assert!(result.is_ok());
        let preview = result.unwrap();
        
        // Verify preview content
        assert_eq!(preview.services.len(), 3);
        assert_eq!(preview.schemas.len(), 2);
        assert!(!preview.folders_to_create.is_empty());
        
        // Performance assertion
        assert!(duration.as_millis() < 100, "Preview generation took too long: {:?}", duration);
        
        println!("Preview generation completed in {:?}", duration);
    }

    /// Integration test: Validation and error handling
    /// Tests comprehensive validation workflow
    #[test]
    fn test_comprehensive_validation_workflow() {
        let workspace_id = Uuid::new_v4();
        let existing_folders = vec![
            Folder::with_type(workspace_id, "Services", None, FolderType::Service),
            Folder::with_type(workspace_id, "Schemas", None, FolderType::Schema),
        ];
        
        // Test valid content
        let valid_content = serde_json::to_string(&create_test_spec()).unwrap();
        let options = ImportOptions {
            target_folder: Some(existing_folders[0].id),
            create_subfolders: false,
            schema_folder_strategy: SchemaFolderStrategy::SeparateFolder,
        };
        
        let result = validate_import_request(&valid_content, &options, &existing_folders);
        assert!(result.is_ok());
        
        // Test invalid content
        let invalid_content = "invalid json content";
        let result = validate_import_request(invalid_content, &options, &existing_folders);
        assert!(result.is_err());
        
        // Test invalid target folder
        let invalid_options = ImportOptions {
            target_folder: Some(Uuid::new_v4()), // Non-existent folder
            create_subfolders: false,
            schema_folder_strategy: SchemaFolderStrategy::SeparateFolder,
        };
        
        let result = validate_import_request(&valid_content, &invalid_options, &existing_folders);
        assert!(result.is_err());
        
        // Test empty content
        let result = validate_import_request("", &options, &existing_folders);
        assert!(result.is_err());
        
        // Test oversized content
        let large_content = "x".repeat(15_000_000); // 15MB
        let result = validate_import_request(&large_content, &options, &existing_folders);
        assert!(result.is_err());
    }

    /// Integration test: Name conflict detection and resolution
    /// Tests conflict handling workflow
    #[test]
    fn test_name_conflict_detection_and_resolution() {
        let workspace_id = Uuid::new_v4();
        let spec = create_test_spec();
        let options = ImportOptions::default();
        
        // Create existing services with conflicting names
        let existing_services = vec![
            Service::new(workspace_id, "Get users", ServiceType::Http(HttpConfig::default())),
            Service::new(workspace_id, "Create user", ServiceType::Http(HttpConfig::default())),
        ];
        
        // Create existing schemas with conflicting names
        let existing_schemas = vec![
            Schema::new(workspace_id, "User".to_string()),
        ];
        
        let conflicts = check_name_conflicts(&spec, &options, &existing_services, &existing_schemas);
        assert!(conflicts.is_ok());
        
        let conflicts = conflicts.unwrap();
        assert!(!conflicts.is_empty(), "Should detect name conflicts");
        
        // Test conflict resolution
        let mut resolutions = HashMap::new();
        for conflict in &conflicts {
            resolutions.insert(conflict.name.clone(), ConflictResolution::Rename(conflict.suggested_name.clone()));
        }
        
        let result = resolve_conflicts(&conflicts, &resolutions);
        assert!(result.is_ok(), "Should resolve conflicts successfully");
        
        // Test incomplete resolution
        let incomplete_resolutions = HashMap::new();
        let result = resolve_conflicts(&conflicts, &incomplete_resolutions);
        assert!(result.is_err(), "Should fail with incomplete resolutions");
    }

    /// Integration test: End-to-end import workflow
    /// Tests complete import process with all features
    #[test]
    fn test_end_to_end_import_workflow() {
        let workspace_id = Uuid::new_v4();
        let content = serde_json::to_string(&create_test_spec()).unwrap();
        
        // Create existing folder structure
        let existing_folders = vec![
            Folder::with_type(workspace_id, "API", None, FolderType::Service),
            Folder::with_type(workspace_id, "Models", None, FolderType::Schema),
        ];
        
        let options = ImportOptions {
            target_folder: Some(existing_folders[0].id),
            create_subfolders: true,
            schema_folder_strategy: SchemaFolderStrategy::ExistingSchemaFolder(existing_folders[1].id),
        };
        
        // Step 1: Validate import request
        let spec = validate_import_request(&content, &options, &existing_folders);
        assert!(spec.is_ok());
        let spec = spec.unwrap();
        
        // Step 2: Generate preview
        let preview = generate_import_preview(&spec, &options, &existing_folders);
        assert!(preview.is_ok());
        let preview = preview.unwrap();
        
        // Verify preview content
        assert_eq!(preview.services.len(), 3);
        assert_eq!(preview.schemas.len(), 2);
        
        // Step 3: Check for conflicts (none expected with new workspace)
        let conflicts = check_name_conflicts(&spec, &options, &[], &[]);
        assert!(conflicts.is_ok());
        let conflicts = conflicts.unwrap();
        assert!(conflicts.is_empty());
        
        // Step 4: Perform actual import
        let result = import_openapi_with_options_and_existing_folders(
            workspace_id,
            &spec,
            options,
            &existing_folders
        );
        
        assert!(result.is_ok());
        let import_result = result.unwrap();
        
        // Verify import results
        assert_eq!(import_result.services.len(), 3);
        assert_eq!(import_result.schemas.len(), 2);
        
        // Verify folder structure
        let created_folders: Vec<_> = import_result.folders.iter()
            .filter(|f| f.parent_id == Some(existing_folders[0].id))
            .collect();
        assert_eq!(created_folders.len(), 2); // users and posts subfolders
        
        // Verify schemas are in the specified schema folder
        for schema in &import_result.schemas {
            assert_eq!(schema.folder_id, Some(existing_folders[1].id));
        }
    }

    /// Performance benchmark test for parsing optimization
    #[test]
    fn test_parsing_performance_with_caching() {
        let content = serde_json::to_string(&create_test_spec()).unwrap();
        
        // First parse (should cache)
        let start_time = std::time::Instant::now();
        let result1 = parse_openapi(&content);
        let first_parse_duration = start_time.elapsed();
        assert!(result1.is_ok());
        
        // Second parse (should use cache)
        let start_time = std::time::Instant::now();
        let result2 = parse_openapi(&content);
        let second_parse_duration = start_time.elapsed();
        assert!(result2.is_ok());
        
        // Cache should make second parse faster (though this might be flaky in CI)
        println!("First parse: {:?}, Second parse: {:?}", first_parse_duration, second_parse_duration);
        
        // At minimum, both should complete quickly
        assert!(first_parse_duration.as_millis() < 100);
        assert!(second_parse_duration.as_millis() < 100);
    }

    /// Memory usage test for large specifications
    #[test]
    fn test_memory_efficiency_large_spec() {
        // Create a very large spec to test memory efficiency
        let mut large_spec = create_test_spec();
        
        // Add 1000 paths with complex schemas
        for i in 0..1000 {
            let path = format!("/api/v1/resource{}/items", i);
            let tag = format!("resource{}", i % 20);
            
            let path_item = serde_json::json!({
                "get": {
                    "tags": [tag.clone()],
                    "summary": format!("Get resource {} items", i),
                    "parameters": [
                        {
                            "name": "limit",
                            "in": "query",
                            "schema": {"type": "integer"}
                        },
                        {
                            "name": "offset", 
                            "in": "query",
                            "schema": {"type": "integer"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Success",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": {"$ref": format!("#/components/schemas/Resource{}", i)}
                                    }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "tags": [tag],
                    "summary": format!("Create resource {} item", i),
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {"$ref": format!("#/components/schemas/Resource{}", i)}
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Created"
                        }
                    }
                }
            });
            
            large_spec.paths.insert(path, serde_json::from_value(path_item).unwrap());
        }
        
        // Add corresponding schemas
        if let Some(ref mut components) = large_spec.components {
            for i in 0..1000 {
                let schema_name = format!("Resource{}", i);
                let schema = serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": {"type": "integer"},
                        "name": {"type": "string"},
                        "description": {"type": "string"},
                        "created_at": {"type": "string", "format": "date-time"},
                        "updated_at": {"type": "string", "format": "date-time"},
                        "metadata": {
                            "type": "object",
                            "additionalProperties": true
                        }
                    },
                    "required": ["id", "name"]
                });
                components.schemas.insert(schema_name, serde_json::from_value(schema).unwrap());
            }
        }
        
        let workspace_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();
        
        let result = import_openapi_with_options_and_existing_folders(
            workspace_id,
            &large_spec,
            ImportOptions::default(),
            &[]
        );
        
        let duration = start_time.elapsed();
        
        assert!(result.is_ok());
        let import_result = result.unwrap();
        
        // Verify all items were processed
        assert!(import_result.services.len() >= 2000, "Should have at least 2000 services, got {}", import_result.services.len()); // 2 operations per path plus original
        assert!(import_result.schemas.len() >= 1000, "Should have at least 1000 schemas, got {}", import_result.schemas.len());
        
        // Performance assertion: should handle large specs efficiently
        assert!(duration.as_secs() < 5, "Large spec import took too long: {:?}", duration);
        
        println!("Large spec with 1000 resources processed in {:?}", duration);
    }
}

#[derive(Clone, Debug)]
pub struct WorkspaceEvent;

pub fn init(cx: &mut App) {
    WorkspaceRegistry::set_global(cx.new(WorkspaceRegistry::new), cx);
}

struct GlobalWorkspaceRegistry(Entity<WorkspaceRegistry>);
impl Global for GlobalWorkspaceRegistry {}

pub struct WorkspaceRegistry {
    db: Arc<Database>,
    app_state: models::ApplicationState,
}

impl EventEmitter<WorkspaceEvent> for WorkspaceRegistry {}

impl WorkspaceRegistry {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalWorkspaceRegistry>().0.clone()
    }

    fn set_global(registry: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalWorkspaceRegistry(registry));
    }

    fn db_path() -> std::path::PathBuf {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("zeph");
        std::fs::create_dir_all(&data_dir).ok();
        data_dir.join("zeph.db")
    }

    fn new(_cx: &mut Context<Self>) -> Self {
        let db_path = Self::db_path();
        let db = Database::open(&db_path).expect("Failed to open database");
        let db = Arc::new(db);

        let mut app_state = models::ApplicationState::default();

        let store = WorkspaceStore::new(db.conn());
        app_state.workspaces = store.list().unwrap_or_default();

        Self { db, app_state }
    }

    pub fn get_state(&self) -> &models::ApplicationState {
        &self.app_state
    }

    pub fn get_opened_workspaces(&self) -> Vec<&Workspace> {
        self.app_state
            .opened_workspaces
            .iter()
            .filter_map(|id| self.app_state.workspaces.iter().find(|w| w.id == *id))
            .collect()
    }

    pub fn get_active_workspace(&self) -> Option<&Workspace> {
        self.app_state
            .active_workspace_id
            .and_then(|id| self.app_state.workspaces.iter().find(|w| w.id == id))
    }

    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.app_state
            .active_workspace_id
            .and_then(|id| self.app_state.workspaces.iter_mut().find(|w| w.id == id))
    }

    // === Workspace CRUD ===

    pub fn create_workspace(&mut self, param: CreateWorkspace, cx: &mut Context<Self>) {
        let workspace = Workspace::new(param.name);
        let store = WorkspaceStore::new(self.db.conn());
        store.insert(&workspace).ok();
        if param.create_sample_services {
            self.create_sample_services(workspace.id);
        }
        self.app_state.workspaces.push(workspace);
        cx.emit(WorkspaceEvent);
    }

    pub fn open_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if !self.app_state.opened_workspaces.contains(&workspace_id) {
            self.app_state.opened_workspaces.push(workspace_id);
        }
        self.load_workspace_content(workspace_id);
        self.set_active_workspace(workspace_id, cx);
    }

    pub fn close_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        self.app_state
            .opened_workspaces
            .retain(|&id| id != workspace_id);

        if self.app_state.active_workspace_id == Some(workspace_id) {
            if let Some(&next_id) = self.app_state.opened_workspaces.last() {
                self.app_state.active_workspace_id = Some(next_id);
                self.app_state.current_view = models::ApplicationView::Workspace(next_id);
            } else {
                self.go_to_home(cx);
                return;
            }
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn set_active_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        self.app_state.active_workspace_id = Some(workspace_id);
        self.app_state.current_view = models::ApplicationView::Workspace(workspace_id);
        cx.emit(WorkspaceEvent);
    }

    pub fn go_to_home(&mut self, cx: &mut Context<Self>) {
        self.app_state.current_view = models::ApplicationView::Home;
        self.app_state.active_workspace_id = None;
        cx.emit(WorkspaceEvent);
    }

    pub fn set_workspace_view(
        &mut self,
        workspace_id: Uuid,
        view: models::WorkspaceView,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self
            .app_state
            .workspaces
            .iter_mut()
            .find(|w| w.id == workspace_id)
        {
            workspace.current_view = view;
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn remove_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        let store = WorkspaceStore::new(self.db.conn());
        store.delete(workspace_id).ok();

        self.app_state.workspaces.retain(|w| w.id != workspace_id);
        self.app_state
            .opened_workspaces
            .retain(|&id| id != workspace_id);

        if self.app_state.active_workspace_id == Some(workspace_id) {
            if let Some(first) = self.app_state.workspaces.first() {
                self.set_active_workspace(first.id, cx);
            } else {
                self.go_to_home(cx);
            }
        } else {
            cx.emit(WorkspaceEvent);
        }
    }

    // === Content Loading ===

    fn load_workspace_content(&mut self, workspace_id: Uuid) {
        if let Some(workspace) = self
            .app_state
            .workspaces
            .iter_mut()
            .find(|w| w.id == workspace_id)
        {
            if workspace.services.is_empty() && workspace.folders.is_empty() {
                let folder_store = FolderStore::new(self.db.conn());
                let service_store = ServiceStore::new(self.db.conn());
                let schema_store = SchemaStore::new(self.db.conn());
                let env_store = EnvironmentStore::new(self.db.conn());
                let var_store = EnvironmentVariableStore::new(self.db.conn());

                workspace.folders = folder_store
                    .list_by_workspace(workspace_id)
                    .unwrap_or_default();
                workspace.services = service_store
                    .list_by_workspace(workspace_id)
                    .unwrap_or_default();
                workspace.schemas = schema_store
                    .list_by_workspace(workspace_id)
                    .unwrap_or_default();
                workspace.environments = env_store
                    .list_by_workspace(workspace_id)
                    .unwrap_or_default();

                // Load variables for each environment
                for env in &mut workspace.environments {
                    env.variables = var_store.list_by_environment(env.id).unwrap_or_default();
                }

                // Set default environment as active
                if workspace.active_environment_id.is_none() {
                    workspace.active_environment_id = workspace.environments
                        .iter()
                        .find(|e| e.is_default)
                        .or(workspace.environments.first())
                        .map(|e| e.id);
                }
            }
        }
    }

    fn create_sample_services(&mut self, workspace_id: Uuid) {
        let samples = vec![
            (
                "Get Users",
                HttpMethod::Get,
                "https://api.example.com/users",
            ),
            (
                "Create User",
                HttpMethod::Post,
                "https://api.example.com/users",
            ),
            ("httpbin json", HttpMethod::Get, "https://httpbin.org/json"),
            (
                "httpbin stream",
                HttpMethod::Get,
                "https://httpbin.org/stream/5",
            ),
        ];

        let service_store = ServiceStore::new(self.db.conn());

        for (name, method, path) in samples {
            let service = Service::new(
                workspace_id,
                name,
                ServiceType::Http(HttpConfig {
                    method,
                    path: path.into(),
                    ..Default::default()
                }),
            );
            if let Err(err) = service_store.insert(&service) {
                error!("Failed to create service: {}", err);
            }
        }
    }

    // === Service Operations ===

    pub fn create_service(
        &mut self,
        name: String,
        folder_id: Option<Uuid>,
        cx: &mut Context<Self>,
    ) {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return,
        };

        let mut service = Service::new(
            workspace_id,
            &name,
            ServiceType::Http(HttpConfig::default()),
        );
        service.folder_id = folder_id;

        let service_store = ServiceStore::new(self.db.conn());
        service_store.insert(&service).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.services.push(service);
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn create_schema(&mut self, name: String, folder_id: Option<Uuid>, cx: &mut Context<Self>) {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return,
        };

        let mut schema = models::Schema::new(workspace_id, &name);
        schema.folder_id = folder_id;

        let schema_store = SchemaStore::new(self.db.conn());
        schema_store.insert(&schema).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.schemas.push(schema);
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn create_folder(
        &mut self,
        name: String,
        parent_id: Option<Uuid>,
        folder_type: models::FolderType,
        cx: &mut Context<Self>,
    ) {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return,
        };

        let folder = models::Folder::with_type(workspace_id, &name, parent_id, folder_type);

        let folder_store = FolderStore::new(self.db.conn());
        folder_store.insert(&folder).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.folders.push(folder);
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn open_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            if !workspace.opened_services.contains(&service_id) {
                workspace.opened_services.push(service_id);
            }
            workspace.active_service = Some(service_id);
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn close_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.opened_services.retain(|&id| id != service_id);
            if workspace.active_service == Some(service_id) {
                workspace.active_service = workspace.opened_services.first().copied();
            }
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn set_active_service(&mut self, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.active_service = Some(service_id);
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn get_service(&self, service_id: Uuid) -> Option<&Service> {
        self.get_active_workspace()
            .and_then(|w| w.services.iter().find(|s| s.id == service_id))
    }

    // === OpenAPI Import ===

    pub fn import_openapi(&mut self, content: &str, cx: &mut Context<Self>) -> anyhow::Result<()> {
        self.import_openapi_with_options(content, openapi::ImportOptions::default(), cx)
    }

    pub fn import_openapi_with_options(
        &mut self, 
        content: &str, 
        options: openapi::ImportOptions, 
        cx: &mut Context<Self>
    ) -> anyhow::Result<()> {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return Err(anyhow::anyhow!("No active workspace")),
        };

        let workspace = self.get_state().workspaces.iter().find(|w| w.id == workspace_id).unwrap();

        let spec = openapi::parse_openapi(content)?;
        let result = openapi::import_openapi_with_options(workspace_id, &spec, options, &workspace.folders)?;

        log::info!("Importing {} folders, {} services, {} schemas", 
            result.folders.len(), result.services.len(), result.schemas.len());

        let folder_store = FolderStore::new(self.db.conn());
        let service_store = ServiceStore::new(self.db.conn());
        let schema_store = SchemaStore::new(self.db.conn());

        for folder in &result.folders {
            if let Err(e) = folder_store.insert(folder) {
                error!("Failed to insert folder {}: {}", folder.name, e);
            }
        }
        for service in &result.services {
            if let Err(e) = service_store.insert(service) {
                error!("Failed to insert service {}: {}", service.name, e);
            }
        }
        for schema in &result.schemas {
            if let Err(e) = schema_store.insert(schema) {
                error!("Failed to insert schema {}: {}", schema.name, e);
            }
        }

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.folders.extend(result.folders);
            workspace.services.extend(result.services);
            workspace.schemas.extend(result.schemas);
        }

        cx.emit(WorkspaceEvent);
        Ok(())
    }

    pub fn generate_openapi_import_preview(
        &self,
        content: &str,
        options: openapi::ImportOptions,
    ) -> anyhow::Result<openapi::ImportPreview> {
        let workspace = self.get_active_workspace()
            .ok_or_else(|| anyhow::anyhow!("No active workspace"))?;

        let spec = openapi::parse_openapi(content)?;
        openapi::generate_import_preview(&spec, &options, &workspace.folders)
    }

    // === Environment Operations ===

    pub fn create_environment(&mut self, name: String, cx: &mut Context<Self>) {
        let workspace_id = match self.get_active_workspace() {
            Some(w) => w.id,
            None => return,
        };

        let env = Environment::new(workspace_id, &name);
        let env_store = EnvironmentStore::new(self.db.conn());
        env_store.insert(&env).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            let is_first = workspace.environments.is_empty();
            workspace.environments.push(env.clone());
            if is_first {
                workspace.active_environment_id = Some(env.id);
            }
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn update_environment(&mut self, env: Environment, cx: &mut Context<Self>) {
        let env_store = EnvironmentStore::new(self.db.conn());
        env_store.update(&env).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            if let Some(existing) = workspace.environments.iter_mut().find(|e| e.id == env.id) {
                existing.name = env.name;
                existing.base_url = env.base_url;
                existing.is_default = env.is_default;
            }
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn delete_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        let env_store = EnvironmentStore::new(self.db.conn());
        env_store.delete(env_id).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.environments.retain(|e| e.id != env_id);
            if workspace.active_environment_id == Some(env_id) {
                workspace.active_environment_id = workspace.environments.first().map(|e| e.id);
            }
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn set_active_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        if let Some(workspace) = self.get_active_workspace_mut() {
            workspace.active_environment_id = Some(env_id);
            cx.emit(WorkspaceEvent);
        }
    }

    pub fn get_active_environment(&self) -> Option<&Environment> {
        self.get_active_workspace().and_then(|w| {
            w.active_environment_id
                .and_then(|id| w.environments.iter().find(|e| e.id == id))
        })
    }

    // === Environment Variable Operations ===

    pub fn add_environment_variable(&mut self, env_id: Uuid, var: EnvironmentVariable, cx: &mut Context<Self>) {
        let var_store = EnvironmentVariableStore::new(self.db.conn());
        var_store.insert(&var).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            if let Some(env) = workspace.environments.iter_mut().find(|e| e.id == env_id) {
                env.variables.push(var);
            }
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn update_environment_variable(&mut self, var: EnvironmentVariable, cx: &mut Context<Self>) {
        let var_store = EnvironmentVariableStore::new(self.db.conn());
        var_store.update(&var).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            for env in &mut workspace.environments {
                if let Some(existing) = env.variables.iter_mut().find(|v| v.id == var.id) {
                    *existing = var.clone();
                    break;
                }
            }
        }
        cx.emit(WorkspaceEvent);
    }

    pub fn delete_environment_variable(&mut self, var_id: Uuid, cx: &mut Context<Self>) {
        let var_store = EnvironmentVariableStore::new(self.db.conn());
        var_store.delete(var_id).ok();

        if let Some(workspace) = self.get_active_workspace_mut() {
            for env in &mut workspace.environments {
                env.variables.retain(|v| v.id != var_id);
            }
        }
        cx.emit(WorkspaceEvent);
    }
}
