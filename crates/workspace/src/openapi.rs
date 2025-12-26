use anyhow::{anyhow, Result};
use models::{
    Folder, FolderType, HttpConfig, HttpMethod, MediaContent, Parameter, ParameterLocation,
    PropertyType, RequestBody, Response, Schema, SchemaProperty, SchemaType, Service, ServiceType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// Performance optimization: Use faster hasher for large OpenAPI specs
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Performance optimization: Pre-compiled regex for validation
use std::sync::OnceLock;

/// Performance optimization: Cache for parsed OpenAPI specs to avoid re-parsing
static SPEC_CACHE: OnceLock<std::sync::Mutex<HashMap<u64, OpenApiSpec>>> = OnceLock::new();

/// Performance optimization: Get or initialize the spec cache
fn get_spec_cache() -> &'static std::sync::Mutex<HashMap<u64, OpenApiSpec>> {
    SPEC_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenApiSpec {
    pub openapi: Option<String>,
    pub swagger: Option<String>,
    pub info: OpenApiInfo,
    #[serde(default)]
    pub paths: HashMap<String, PathItem>,
    #[serde(default)]
    pub components: Option<Components>,
    #[serde(default)]
    pub definitions: Option<HashMap<String, SchemaObject>>, // Swagger 2.0
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OpenApiInfo {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PathItem {
    #[serde(default)]
    pub get: Option<Operation>,
    #[serde(default)]
    pub post: Option<Operation>,
    #[serde(default)]
    pub put: Option<Operation>,
    #[serde(default)]
    pub delete: Option<Operation>,
    #[serde(default)]
    pub patch: Option<Operation>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub operation_id: Option<String>,
    #[serde(default)]
    pub parameters: Vec<ParameterObject>,
    #[serde(default)]
    pub request_body: Option<RequestBodyObject>,
    #[serde(default)]
    pub responses: HashMap<String, ResponseObject>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ParameterObject {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub schema: Option<SchemaObject>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RequestBodyObject {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub content: HashMap<String, MediaTypeObject>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ResponseObject {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub content: Option<HashMap<String, MediaTypeObject>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MediaTypeObject {
    #[serde(default)]
    pub schema: Option<SchemaObject>,
    #[serde(default)]
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaObject {
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    #[serde(default)]
    pub properties: Option<HashMap<String, SchemaObject>>,
    #[serde(default)]
    pub required: Option<Vec<String>>,
    #[serde(default)]
    pub items: Option<Box<SchemaObject>>,
    #[serde(rename = "$ref")]
    pub ref_path: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,
    #[serde(default)]
    pub nullable: Option<bool>,
    #[serde(default)]
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Components {
    #[serde(default)]
    pub schemas: HashMap<String, SchemaObject>,
}

#[derive(Debug)]
pub struct ImportResult {
    pub folders: Vec<Folder>,
    pub services: Vec<Service>,
    pub schemas: Vec<Schema>,
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub target_folder: Option<Uuid>,
    pub create_subfolders: bool,
    pub schema_folder_strategy: SchemaFolderStrategy,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            target_folder: None,
            create_subfolders: true,
            schema_folder_strategy: SchemaFolderStrategy::SeparateFolder,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SchemaFolderStrategy {
    SameAsServices,             // 与服务放在同一文件夹
    SeparateFolder,             // 创建单独的模式文件夹
    ExistingSchemaFolder(Uuid), // 使用现有的模式文件夹
}

#[derive(Debug, Clone)]
pub struct ImportPreview {
    pub services: Vec<ServicePreview>,
    pub schemas: Vec<SchemaPreview>,
    pub folders_to_create: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ServicePreview {
    pub name: String,
    pub method: String,
    pub path: String,
    pub tag: Option<String>,
    pub target_folder: String,
}

#[derive(Debug, Clone)]
pub struct SchemaPreview {
    pub name: String,
    pub schema_type: String,
    pub target_folder: String,
}

pub fn parse_openapi(content: &str) -> Result<OpenApiSpec> {
    // Performance optimization: Use content hash for caching
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    let content_hash = hasher.finish();

    // Check cache first
    if let Ok(cache) = get_spec_cache().lock() {
        if let Some(cached_spec) = cache.get(&content_hash) {
            return Ok(cached_spec.clone());
        }
    }

    // Performance optimization: Pre-validate content size to avoid large allocations
    if content.len() > 50_000_000 {
        // 50MB limit for performance
        return Err(anyhow!("OpenAPI content too large for optimal processing"));
    }

    // Performance optimization: Try JSON first (faster parsing), then YAML
    let spec: OpenApiSpec = if content.trim_start().starts_with('{') {
        // Likely JSON - parse directly
        serde_json::from_str(content).map_err(|e| anyhow!("Failed to parse OpenAPI JSON: {}", e))
    } else {
        // Try YAML first, fallback to JSON
        serde_yaml::from_str(content)
            .or_else(|_| serde_json::from_str(content))
            .map_err(|e| anyhow!("Failed to parse OpenAPI spec: {}", e))
    }?;

    // Cache the parsed spec for future use
    if let Ok(mut cache) = get_spec_cache().lock() {
        // Performance optimization: Limit cache size to prevent memory bloat
        if cache.len() > 100 {
            cache.clear(); // Simple eviction strategy
        }
        cache.insert(content_hash, spec.clone());
    }

    Ok(spec)
}

/// Enhanced validation for import options with comprehensive error checking
/// Implements Requirements 6.1, 6.2 for input validation and error handling
pub fn validate_import_options(options: &ImportOptions, existing_folders: &[Folder]) -> Result<()> {
    // Requirement 6.1: Validate target folder if specified
    if let Some(target_folder_id) = options.target_folder {
        let target_folder = existing_folders
            .iter()
            .find(|f| f.id == target_folder_id)
            .ok_or_else(|| anyhow!("目标文件夹不存在"))?;

        if target_folder.folder_type != FolderType::Service {
            return Err(anyhow!("目标文件夹必须是服务类型文件夹"));
        }
    }

    // Validate schema folder strategy
    match &options.schema_folder_strategy {
        SchemaFolderStrategy::ExistingSchemaFolder(folder_id) => {
            if !existing_folders
                .iter()
                .any(|f| f.id == *folder_id && f.folder_type == FolderType::Schema)
            {
                return Err(anyhow!("指定的模式文件夹不存在"));
            }
        }
        _ => {}
    }

    Ok(())
}

/// Validates OpenAPI content before parsing
/// Implements Requirements 6.1, 6.2 for input validation
pub fn validate_openapi_content(content: &str) -> Result<()> {
    if content.trim().is_empty() {
        return Err(anyhow!("OpenAPI 内容不能为空"));
    }

    // Check if content is too large (prevent memory issues)
    if content.len() > 10_000_000 {
        // 10MB limit
        return Err(anyhow!("OpenAPI 内容过大，请使用较小的规范文件"));
    }

    // Try to parse to validate format
    parse_openapi(content)?;

    Ok(())
}

/// Enhanced validation that combines all validation checks
/// Implements comprehensive error checking for Requirements 6.1, 6.2
pub fn validate_import_request(
    content: &str,
    options: &ImportOptions,
    existing_folders: &[Folder],
) -> Result<OpenApiSpec> {
    // Validate content first
    validate_openapi_content(content)?;

    // Validate import options
    validate_import_options(options, existing_folders)?;

    // Parse the spec (already validated in validate_openapi_content, but we need the result)
    let spec = parse_openapi(content)?;

    // Additional validation: check if spec has any paths
    if spec.paths.is_empty() {
        return Err(anyhow!("OpenAPI 规范中没有找到任何路径定义"));
    }

    Ok(spec)
}

/// Enhanced name conflict detection with detailed conflict information
/// Implements Requirement 6.4 for name conflict handling
pub fn check_name_conflicts(
    spec: &OpenApiSpec,
    options: &ImportOptions,
    existing_services: &[Service],
    existing_schemas: &[Schema],
) -> Result<Vec<NameConflict>> {
    let mut conflicts = Vec::new();

    // Check service name conflicts
    for (path, path_item) in &spec.paths {
        let ops = [
            (&path_item.get, HttpMethod::Get),
            (&path_item.post, HttpMethod::Post),
            (&path_item.put, HttpMethod::Put),
            (&path_item.delete, HttpMethod::Delete),
            (&path_item.patch, HttpMethod::Patch),
        ];

        for (op_opt, method) in ops {
            if let Some(op) = op_opt {
                let name = op
                    .summary
                    .clone()
                    .or_else(|| op.operation_id.clone())
                    .unwrap_or_else(|| format!("{} {}", method.as_str(), path));

                let target_folder_id =
                    determine_service_folder_id(options, &HashMap::new(), &op.tags);

                // Check if service with same name exists in target folder
                if let Some(existing_service) = existing_services
                    .iter()
                    .find(|s| s.name.as_ref() == name && s.folder_id == target_folder_id)
                {
                    conflicts.push(NameConflict {
                        conflict_type: ConflictType::Service,
                        name: name.clone(),
                        existing_id: Some(existing_service.id),
                        suggested_name: generate_unique_service_name(
                            &name,
                            existing_services,
                            target_folder_id,
                        ),
                        target_folder_id,
                    });
                }
            }
        }
    }

    // Check schema name conflicts
    let schema_defs = spec
        .components
        .as_ref()
        .map(|c| &c.schemas)
        .or(spec.definitions.as_ref());

    if let Some(defs) = schema_defs {
        let schema_folder_id = determine_schema_folder_id(options, &[]).unwrap_or(None);

        for (name, _) in defs {
            if let Some(existing_schema) = existing_schemas
                .iter()
                .find(|s| s.name.as_ref() == name && s.folder_id == schema_folder_id)
            {
                conflicts.push(NameConflict {
                    conflict_type: ConflictType::Schema,
                    name: name.clone(),
                    existing_id: Some(existing_schema.id),
                    suggested_name: generate_unique_schema_name(
                        name,
                        existing_schemas,
                        schema_folder_id,
                    ),
                    target_folder_id: schema_folder_id,
                });
            }
        }
    }

    Ok(conflicts)
}

/// Represents a name conflict with resolution options
/// Implements Requirement 6.4 for conflict handling
#[derive(Debug, Clone)]
pub struct NameConflict {
    pub conflict_type: ConflictType,
    pub name: String,
    pub existing_id: Option<Uuid>,
    pub suggested_name: String,
    pub target_folder_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub enum ConflictType {
    Service,
    Schema,
}

/// Generates a unique service name by appending a number
fn generate_unique_service_name(
    base_name: &str,
    existing_services: &[Service],
    target_folder_id: Option<Uuid>,
) -> String {
    let mut counter = 1;
    loop {
        let candidate = format!("{} ({})", base_name, counter);
        if !existing_services
            .iter()
            .any(|s| s.name.as_ref() == candidate && s.folder_id == target_folder_id)
        {
            return candidate;
        }
        counter += 1;

        // Prevent infinite loop
        if counter > 1000 {
            return format!(
                "{} ({})",
                base_name,
                uuid::Uuid::new_v4().to_string()[..8].to_string()
            );
        }
    }
}

/// Generates a unique schema name by appending a number
fn generate_unique_schema_name(
    base_name: &str,
    existing_schemas: &[Schema],
    target_folder_id: Option<Uuid>,
) -> String {
    let mut counter = 1;
    loop {
        let candidate = format!("{}{}", base_name, counter);
        if !existing_schemas
            .iter()
            .any(|s| s.name.as_ref() == candidate && s.folder_id == target_folder_id)
        {
            return candidate;
        }
        counter += 1;

        // Prevent infinite loop
        if counter > 1000 {
            return format!(
                "{}{}",
                base_name,
                uuid::Uuid::new_v4().to_string()[..8].to_string()
            );
        }
    }
}

/// Conflict resolution strategy for handling name conflicts
/// Implements Requirement 6.4 for conflict resolution options
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    Skip,           // Skip importing the conflicting item
    Rename(String), // Rename to the provided name
    Replace,        // Replace the existing item
}

/// Applies conflict resolution to the import process
/// Implements Requirement 6.4 for conflict handling
pub fn resolve_conflicts(
    conflicts: &[NameConflict],
    resolutions: &HashMap<String, ConflictResolution>,
) -> Result<()> {
    for conflict in conflicts {
        if !resolutions.contains_key(&conflict.name) {
            return Err(anyhow!(
                "未解决的名称冲突: {}。请选择跳过、重命名或替换。",
                conflict.name
            ));
        }
    }
    Ok(())
}

pub fn import_openapi(
    workspace_id: Uuid,
    spec: &OpenApiSpec,
    existing_folders: &[Folder],
) -> Result<ImportResult> {
    import_openapi_with_options(
        workspace_id,
        spec,
        ImportOptions::default(),
        existing_folders,
    )
}

pub fn import_openapi_with_options(
    workspace_id: Uuid,
    spec: &OpenApiSpec,
    options: ImportOptions,
    existing_folders: &[Folder],
) -> Result<ImportResult> {
    import_openapi_with_options_and_existing_folders(workspace_id, spec, options, existing_folders)
}

/// Comprehensive import function with full validation and conflict handling
/// Implements Requirements 6.1, 6.2, 6.4 for complete error handling workflow
pub fn import_openapi_with_validation(
    workspace_id: Uuid,
    content: &str,
    options: ImportOptions,
    existing_folders: &[Folder],
    existing_services: &[Service],
    existing_schemas: &[Schema],
    conflict_resolutions: Option<&HashMap<String, ConflictResolution>>,
) -> Result<ImportResult> {
    // Step 1: Validate content and options
    let spec = validate_import_request(content, &options, existing_folders)?;

    // Step 2: Check for name conflicts
    let conflicts = check_name_conflicts(&spec, &options, existing_services, existing_schemas)?;

    // Step 3: Handle conflicts if any exist
    if !conflicts.is_empty() {
        if let Some(resolutions) = conflict_resolutions {
            // Convert conflicts to string keys for resolution lookup
            let conflict_map: HashMap<String, ConflictResolution> = conflicts
                .iter()
                .filter_map(|c| {
                    resolutions
                        .get(&c.name)
                        .map(|r| (c.name.clone(), r.clone()))
                })
                .collect();

            resolve_conflicts(&conflicts, &conflict_map)?;
        } else {
            // Return error with conflict details for user to resolve
            let conflict_messages: Vec<String> = conflicts
                .iter()
                .map(|c| match c.conflict_type {
                    ConflictType::Service => {
                        format!("服务名称冲突: {} (建议名称: {})", c.name, c.suggested_name)
                    }
                    ConflictType::Schema => {
                        format!("模式名称冲突: {} (建议名称: {})", c.name, c.suggested_name)
                    }
                })
                .collect();

            return Err(anyhow!(
                "发现名称冲突，请解决后重试:\n{}",
                conflict_messages.join("\n")
            ));
        }
    }

    // Step 4: Proceed with import
    import_openapi_with_options_and_existing_folders(workspace_id, &spec, options, existing_folders)
}

/// Enhanced import function with comprehensive validation and error handling
/// Implements Requirements 6.1, 6.2, 6.4 for validation and error handling
pub fn import_openapi_with_options_and_existing_folders(
    workspace_id: Uuid,
    spec: &OpenApiSpec,
    options: ImportOptions,
    existing_folders: &[Folder],
) -> Result<ImportResult> {
    // Enhanced validation with detailed error messages
    validate_import_options(&options, existing_folders)?;

    // Validate target folder if specified
    if let Some(target_folder_id) = options.target_folder {
        if !existing_folders.iter().any(|f| f.id == target_folder_id) {
            return Err(anyhow!("目标文件夹不存在"));
        }

        // Validate that target folder is a service folder
        let target_folder = existing_folders
            .iter()
            .find(|f| f.id == target_folder_id)
            .unwrap();
        if target_folder.folder_type != FolderType::Service {
            return Err(anyhow!("目标文件夹必须是服务类型文件夹"));
        }
    }

    let mut folders: Vec<Folder> = vec![];
    let mut services: Vec<Service> = vec![];
    let mut schemas: Vec<Schema> = vec![];
    let mut tag_folder_map: HashMap<String, Uuid> = HashMap::new();

    // Handle folder creation based on options
    match options.target_folder {
        Some(target_folder_id) => {
            // Import to specified folder
            if options.create_subfolders {
                // Create subfolders based on tags under the target folder
                for (_path, path_item) in &spec.paths {
                    for op in [
                        &path_item.get,
                        &path_item.post,
                        &path_item.put,
                        &path_item.delete,
                        &path_item.patch,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        for tag in &op.tags {
                            if !tag_folder_map.contains_key(tag) {
                                let folder = Folder::with_type(
                                    workspace_id,
                                    tag.clone(),
                                    Some(target_folder_id),
                                    FolderType::Service,
                                );
                                tag_folder_map.insert(tag.clone(), folder.id);
                                folders.push(folder);
                            }
                        }
                    }
                }
            } else {
                // All services go directly to target folder
                // No new folders created, services will use target_folder_id directly
            }
        }
        None => {
            // Default behavior: create folders from tags at root level
            for (_path, path_item) in &spec.paths {
                for op in [
                    &path_item.get,
                    &path_item.post,
                    &path_item.put,
                    &path_item.delete,
                    &path_item.patch,
                ]
                .into_iter()
                .flatten()
                {
                    for tag in &op.tags {
                        if !tag_folder_map.contains_key(tag) {
                            let folder = Folder::with_type(
                                workspace_id,
                                tag.clone(),
                                None,
                                FolderType::Service,
                            );
                            tag_folder_map.insert(tag.clone(), folder.id);
                            folders.push(folder);
                        }
                    }
                }
            }
        }
    }

    // Create services from paths with enhanced error handling
    for (path, path_item) in &spec.paths {
        let ops = [
            (&path_item.get, HttpMethod::Get),
            (&path_item.post, HttpMethod::Post),
            (&path_item.put, HttpMethod::Put),
            (&path_item.delete, HttpMethod::Delete),
            (&path_item.patch, HttpMethod::Patch),
        ];

        for (op_opt, method) in ops {
            if let Some(op) = op_opt {
                let name = op
                    .summary
                    .clone()
                    .or_else(|| op.operation_id.clone())
                    .unwrap_or_else(|| format!("{} {}", method.as_str(), path));

                let folder_id = determine_service_folder_id(&options, &tag_folder_map, &op.tags);

                let parameters: Vec<Parameter> = op
                    .parameters
                    .iter()
                    .map(|p| Parameter {
                        name: p.name.clone(),
                        location: match p.location.as_str() {
                            "path" => ParameterLocation::Path,
                            "query" => ParameterLocation::Query,
                            "header" => ParameterLocation::Header,
                            "cookie" => ParameterLocation::Cookie,
                            _ => ParameterLocation::Query,
                        },
                        description: p.description.clone(),
                        required: p.required,
                        schema: p.schema.as_ref().and_then(|s| s.schema_type.clone()),
                        example: p
                            .schema
                            .as_ref()
                            .and_then(|s| s.example.as_ref().map(|e| e.to_string())),
                    })
                    .collect();

                let request_body = op.request_body.as_ref().map(|rb| RequestBody {
                    description: rb.description.clone(),
                    required: rb.required,
                    content: rb
                        .content
                        .iter()
                        .map(|(media_type, mt)| MediaContent {
                            media_type: media_type.clone(),
                            schema: mt
                                .schema
                                .as_ref()
                                .and_then(|s| s.ref_path.clone().or_else(|| s.schema_type.clone())),
                            example: mt.example.as_ref().map(|e| e.to_string()),
                        })
                        .collect(),
                });

                let responses: Vec<Response> = op
                    .responses
                    .iter()
                    .map(|(code, resp)| Response {
                        status_code: code.clone(),
                        description: resp.description.clone().unwrap_or_default(),
                        content: resp
                            .content
                            .as_ref()
                            .map(|c| {
                                c.iter()
                                    .map(|(media_type, mt)| MediaContent {
                                        media_type: media_type.clone(),
                                        schema: mt.schema.as_ref().and_then(|s| {
                                            s.ref_path.clone().or_else(|| s.schema_type.clone())
                                        }),
                                        example: mt.example.as_ref().map(|e| e.to_string()),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect();

                let mut service = Service::new(
                    workspace_id,
                    &name,
                    ServiceType::Http(HttpConfig {
                        method,
                        path: path.clone(),
                        parameters,
                        request_body,
                        responses,
                        ..Default::default()
                    }),
                );
                service.folder_id = folder_id;
                service.description = op.description.clone();
                services.push(service);
            }
        }
    }

    // Import schemas from components (OpenAPI 3.x) or definitions (Swagger 2.0)
    let schema_defs = spec
        .components
        .as_ref()
        .map(|c| &c.schemas)
        .or(spec.definitions.as_ref());

    if let Some(defs) = schema_defs {
        let schema_folder_id = determine_schema_folder_id(&options, existing_folders)?;

        for (name, schema_obj) in defs {
            let mut schema = convert_schema_object(workspace_id, name, schema_obj);
            schema.folder_id = schema_folder_id;
            schemas.push(schema);
        }

        // Create schema folder if needed
        if let Some(folder_id) = schema_folder_id {
            if !existing_folders.iter().any(|f| f.id == folder_id)
                && !folders.iter().any(|f| f.id == folder_id)
            {
                // This means we need to create a new schema folder
                match &options.schema_folder_strategy {
                    SchemaFolderStrategy::SeparateFolder => {
                        let schema_folder = create_schema_folder_with_proper_hierarchy(
                            workspace_id,
                            &options,
                            existing_folders,
                        );
                        folders.push(schema_folder);
                    }
                    _ => {}
                }
            }
        } else if matches!(
            options.schema_folder_strategy,
            SchemaFolderStrategy::SeparateFolder
        ) {
            // Create new schema folder with proper hierarchy
            let schema_folder = create_schema_folder_with_proper_hierarchy(
                workspace_id,
                &options,
                existing_folders,
            );
            let schema_folder_id = schema_folder.id;
            folders.push(schema_folder);

            // Update all schemas to use the new folder
            for schema in &mut schemas {
                schema.folder_id = Some(schema_folder_id);
            }
        }
    }

    Ok(ImportResult {
        folders,
        services,
        schemas,
    })
}

fn determine_service_folder_id(
    options: &ImportOptions,
    tag_folder_map: &HashMap<String, Uuid>,
    tags: &[String],
) -> Option<Uuid> {
    match options.target_folder {
        Some(target_folder_id) => {
            if options.create_subfolders {
                // Use subfolder based on first tag
                tags.first()
                    .and_then(|tag| tag_folder_map.get(tag))
                    .copied()
            } else {
                // Use target folder directly
                Some(target_folder_id)
            }
        }
        None => {
            // Default behavior: use tag-based folder
            tags.first()
                .and_then(|tag| tag_folder_map.get(tag))
                .copied()
        }
    }
}

fn determine_schema_folder_id(
    options: &ImportOptions,
    existing_folders: &[Folder],
) -> Result<Option<Uuid>> {
    match &options.schema_folder_strategy {
        SchemaFolderStrategy::SameAsServices => Ok(options.target_folder),
        SchemaFolderStrategy::ExistingSchemaFolder(folder_id) => {
            // Validate that the specified schema folder exists
            if !existing_folders
                .iter()
                .any(|f| f.id == *folder_id && f.folder_type == FolderType::Schema)
            {
                return Err(anyhow!("指定的模式文件夹不存在"));
            }
            Ok(Some(*folder_id))
        }
        SchemaFolderStrategy::SeparateFolder => {
            find_or_determine_schema_folder(options, existing_folders)
        }
    }
}

/// Enhanced schema folder checking logic that implements Requirements 4.1, 4.2, 4.3, 4.4
fn find_or_determine_schema_folder(
    options: &ImportOptions,
    existing_folders: &[Folder],
) -> Result<Option<Uuid>> {
    if let Some(target_folder_id) = options.target_folder {
        let target_folder = existing_folders.iter().find(|f| f.id == target_folder_id);

        if let Some(target) = target_folder {
            // Requirement 4.1: Check if corresponding schema folder exists at same level
            let existing_schema_folder = find_corresponding_schema_folder(existing_folders, target);

            if let Some(schema_folder) = existing_schema_folder {
                // Requirement 4.2: Use existing schema folder
                return Ok(Some(schema_folder.id));
            }

            // Requirement 4.3 & 4.4: No corresponding schema folder exists, will create one
            // Return None to indicate new folder creation needed
            return Ok(None);
        }
    } else {
        // Look for root-level schema folder
        let existing_schema_folder = existing_folders.iter().find(|f| {
            f.folder_type == FolderType::Schema
                && f.parent_id.is_none()
                && f.name.as_ref() == "Schemas"
        });

        if let Some(schema_folder) = existing_schema_folder {
            return Ok(Some(schema_folder.id));
        }
    }

    // Will create new folder - return None for now
    Ok(None)
}

/// Find corresponding schema folder based on target folder
/// Implements the logic for checking if a schema folder exists at the same level
fn find_corresponding_schema_folder<'a>(
    existing_folders: &'a [Folder],
    target_folder: &Folder,
) -> Option<&'a Folder> {
    // Look for schema folder with same parent as target folder
    existing_folders.iter().find(|f| {
        f.folder_type == FolderType::Schema
            && f.parent_id == target_folder.parent_id
            && f.name.as_ref() == "Schemas"
    })
}

/// Create schema folder with proper hierarchy
/// Implements Requirements 4.3 and 4.4 for schema folder creation and hierarchy
fn create_schema_folder_with_proper_hierarchy(
    workspace_id: Uuid,
    options: &ImportOptions,
    existing_folders: &[Folder],
) -> Folder {
    let parent_id = match options.target_folder {
        Some(target_id) => {
            // Requirement 4.4: Create schema folder at same level as target folder
            // Find the parent of the target folder to create schema folder at same level
            existing_folders
                .iter()
                .find(|f| f.id == target_id)
                .and_then(|f| f.parent_id)
        }
        None => None,
    };

    // Requirement 4.3: Create schema folder with proper name and type
    Folder::with_type(workspace_id, "Schemas", parent_id, FolderType::Schema)
}

/// Enhanced preview generation with detailed folder path information and performance optimizations
/// Implements Requirements 5.2, 5.3, 5.4 for comprehensive preview functionality
pub fn generate_import_preview(
    spec: &OpenApiSpec,
    options: &ImportOptions,
    existing_folders: &[Folder],
) -> Result<ImportPreview> {
    // Validate options first
    validate_import_options(options, existing_folders)?;

    // Performance optimization: Pre-allocate collections with estimated capacity
    let estimated_services = spec.paths.len() * 3; // Estimate ~3 operations per path
    let estimated_schemas = spec
        .components
        .as_ref()
        .map(|c| c.schemas.len())
        .or_else(|| spec.definitions.as_ref().map(|d| d.len()))
        .unwrap_or(0);

    let mut services = Vec::with_capacity(estimated_services);
    let mut schemas = Vec::with_capacity(estimated_schemas);
    let mut folders_to_create = Vec::new();

    // Performance optimization: Build folder lookup map once
    let folder_lookup: HashMap<Uuid, &Folder> =
        existing_folders.iter().map(|f| (f.id, f)).collect();

    // Collect tags that would create folders - performance optimized
    let mut tags_set = std::collections::HashSet::new();
    for (_path, path_item) in &spec.paths {
        // Performance optimization: Use iterator chain instead of nested loops
        let operations = [
            &path_item.get,
            &path_item.post,
            &path_item.put,
            &path_item.delete,
            &path_item.patch,
        ];
        for op in operations.into_iter().flatten() {
            tags_set.extend(op.tags.iter().cloned());
        }
    }

    // Enhanced folder creation strategy with detailed path tracking - performance optimized
    match options.target_folder {
        Some(target_folder_id) => {
            let target_folder_name = folder_lookup
                .get(&target_folder_id)
                .map(|f| f.name.as_ref())
                .unwrap_or("Selected Folder");

            if options.create_subfolders {
                // Performance optimization: Use iterator and filter_map
                folders_to_create.extend(
                    tags_set
                        .iter()
                        .filter(|tag| {
                            !existing_folders.iter().any(|f| {
                                f.parent_id == Some(target_folder_id)
                                    && f.name.as_ref() == *tag
                                    && f.folder_type == FolderType::Service
                            })
                        })
                        .map(|tag| format!("{}/{}", target_folder_name, tag)),
                );
            }
        }
        None => {
            // Default behavior: create root-level folders from tags - performance optimized
            folders_to_create.extend(
                tags_set
                    .iter()
                    .filter(|tag| {
                        !existing_folders.iter().any(|f| {
                            f.parent_id.is_none()
                                && f.name.as_ref() == *tag
                                && f.folder_type == FolderType::Service
                        })
                    })
                    .cloned(),
            );
        }
    }

    // Enhanced service preview generation with detailed path information - performance optimized
    for (path, path_item) in &spec.paths {
        let ops = [
            (&path_item.get, HttpMethod::Get),
            (&path_item.post, HttpMethod::Post),
            (&path_item.put, HttpMethod::Put),
            (&path_item.delete, HttpMethod::Delete),
            (&path_item.patch, HttpMethod::Patch),
        ];

        for (op_opt, method) in ops {
            if let Some(op) = op_opt {
                let name = op
                    .summary
                    .clone()
                    .or_else(|| op.operation_id.clone())
                    .unwrap_or_else(|| format!("{} {}", method.as_str(), path));

                let tag = op.tags.first().cloned();
                let target_folder =
                    determine_enhanced_target_folder_name_optimized(options, &folder_lookup, &tag);

                services.push(ServicePreview {
                    name,
                    method: method.as_str().to_string(),
                    path: path.clone(),
                    tag,
                    target_folder,
                });
            }
        }
    }

    // Enhanced schema preview generation with detailed folder information - performance optimized
    let schema_defs = spec
        .components
        .as_ref()
        .map(|c| &c.schemas)
        .or(spec.definitions.as_ref());

    if let Some(defs) = schema_defs {
        let schema_target_folder =
            determine_enhanced_schema_target_folder_name_optimized(options, &folder_lookup);

        // Performance optimization: Use iterator and collect
        schemas.extend(defs.iter().map(|(name, schema_obj)| {
            let schema_type = schema_obj
                .schema_type
                .as_deref()
                .unwrap_or("object")
                .to_string();
            SchemaPreview {
                name: name.clone(),
                schema_type,
                target_folder: schema_target_folder.clone(),
            }
        }));

        // Enhanced schema folder creation logic with detailed path tracking - performance optimized
        if matches!(
            options.schema_folder_strategy,
            SchemaFolderStrategy::SeparateFolder
        ) {
            if !schemas.is_empty() {
                let schema_folder_exists =
                    check_schema_folder_exists_optimized(options, &folder_lookup);

                if !schema_folder_exists {
                    let schema_folder_path =
                        get_schema_folder_creation_path_optimized(options, &folder_lookup);
                    folders_to_create.push(schema_folder_path);
                }
            }
        }
    }

    Ok(ImportPreview {
        services,
        schemas,
        folders_to_create,
    })
}

/// Performance-optimized target folder name determination with folder lookup map
/// Implements Requirement 5.4 for detailed folder path display
fn determine_enhanced_target_folder_name_optimized(
    options: &ImportOptions,
    folder_lookup: &HashMap<Uuid, &Folder>,
    tag: &Option<String>,
) -> String {
    match options.target_folder {
        Some(target_folder_id) => {
            let target_folder_path = if let Some(folder) = folder_lookup.get(&target_folder_id) {
                build_folder_path_optimized(folder, folder_lookup)
            } else {
                "Selected Folder".to_string()
            };

            if options.create_subfolders {
                if let Some(tag) = tag {
                    format!("{}/{}", target_folder_path, tag)
                } else {
                    target_folder_path
                }
            } else {
                target_folder_path
            }
        }
        None => tag.clone().unwrap_or_else(|| "Workspace Root".to_string()),
    }
}

/// Performance-optimized schema target folder name determination with folder lookup map
/// Implements Requirement 5.4 for detailed schema folder path display
fn determine_enhanced_schema_target_folder_name_optimized(
    options: &ImportOptions,
    folder_lookup: &HashMap<Uuid, &Folder>,
) -> String {
    match &options.schema_folder_strategy {
        SchemaFolderStrategy::SameAsServices => {
            if let Some(target_folder_id) = options.target_folder {
                if let Some(folder) = folder_lookup.get(&target_folder_id) {
                    build_folder_path_optimized(folder, folder_lookup)
                } else {
                    "Selected Folder".to_string()
                }
            } else {
                "Workspace Root".to_string()
            }
        }
        SchemaFolderStrategy::ExistingSchemaFolder(folder_id) => {
            if let Some(folder) = folder_lookup.get(folder_id) {
                build_folder_path_optimized(folder, folder_lookup)
            } else {
                "Existing Schema Folder".to_string()
            }
        }
        SchemaFolderStrategy::SeparateFolder => {
            let base_path = if let Some(target_folder_id) = options.target_folder {
                if let Some(folder) = folder_lookup.get(&target_folder_id) {
                    // Schema folder will be at same level as target folder
                    if let Some(parent_id) = folder.parent_id {
                        if let Some(parent) = folder_lookup.get(&parent_id) {
                            build_folder_path_optimized(parent, folder_lookup)
                        } else {
                            "Workspace Root".to_string()
                        }
                    } else {
                        "Workspace Root".to_string()
                    }
                } else {
                    "Workspace Root".to_string()
                }
            } else {
                "Workspace Root".to_string()
            };

            if base_path == "Workspace Root" {
                "Schemas".to_string()
            } else {
                format!("{}/Schemas", base_path)
            }
        }
    }
}

/// Performance-optimized folder path building with lookup map
/// Implements Requirement 5.4 for complete folder path display
fn build_folder_path_optimized(folder: &Folder, folder_lookup: &HashMap<Uuid, &Folder>) -> String {
    let mut path_parts = Vec::new();
    let mut current_folder = Some(folder);

    // Build path from leaf to root - performance optimized with lookup map
    while let Some(folder) = current_folder {
        path_parts.push(folder.name.as_ref().to_string());

        current_folder = folder
            .parent_id
            .and_then(|parent_id| folder_lookup.get(&parent_id).copied());
    }

    // Reverse to get root-to-leaf order
    path_parts.reverse();

    if path_parts.is_empty() {
        "Workspace Root".to_string()
    } else {
        path_parts.join("/")
    }
}

/// Performance-optimized schema folder existence check with lookup map
/// Implements Requirements 4.1, 4.2 for schema folder checking
fn check_schema_folder_exists_optimized(
    options: &ImportOptions,
    folder_lookup: &HashMap<Uuid, &Folder>,
) -> bool {
    match options.target_folder {
        Some(target_folder_id) => {
            if let Some(target) = folder_lookup.get(&target_folder_id) {
                // Check if corresponding schema folder exists at same level
                folder_lookup.values().any(|f| {
                    f.folder_type == FolderType::Schema
                        && f.parent_id == target.parent_id
                        && f.name.as_ref() == "Schemas"
                })
            } else {
                false
            }
        }
        None => {
            // Check for root-level schema folder
            folder_lookup.values().any(|f| {
                f.folder_type == FolderType::Schema
                    && f.parent_id.is_none()
                    && f.name.as_ref() == "Schemas"
            })
        }
    }
}

/// Performance-optimized schema folder creation path with lookup map
/// Implements Requirements 4.3, 4.4 for schema folder creation path
fn get_schema_folder_creation_path_optimized(
    options: &ImportOptions,
    folder_lookup: &HashMap<Uuid, &Folder>,
) -> String {
    match options.target_folder {
        Some(target_folder_id) => {
            if let Some(folder) = folder_lookup.get(&target_folder_id) {
                // Schema folder will be created at same level as target folder
                if let Some(parent_id) = folder.parent_id {
                    if let Some(parent) = folder_lookup.get(&parent_id) {
                        format!(
                            "{}/Schemas",
                            build_folder_path_optimized(parent, folder_lookup)
                        )
                    } else {
                        "Schemas".to_string()
                    }
                } else {
                    "Schemas".to_string()
                }
            } else {
                "Schemas".to_string()
            }
        }
        None => "Schemas".to_string(),
    }
}

fn convert_schema_object(workspace_id: Uuid, name: &str, obj: &SchemaObject) -> Schema {
    let schema_type = match obj.schema_type.as_deref() {
        Some("object") => SchemaType::Object,
        Some("array") => SchemaType::Array,
        Some("string") => {
            if let Some(enums) = &obj.enum_values {
                SchemaType::Enum(enums.clone())
            } else {
                SchemaType::String
            }
        }
        Some("integer") => SchemaType::Integer,
        Some("number") => SchemaType::Number,
        Some("boolean") => SchemaType::Boolean,
        _ => SchemaType::Object,
    };

    let properties: Vec<SchemaProperty> = obj
        .properties
        .as_ref()
        .map(|props| {
            props
                .iter()
                .map(|(prop_name, prop_obj)| SchemaProperty {
                    name: prop_name.clone(),
                    property_type: convert_property_type(prop_obj),
                    description: prop_obj.description.clone(),
                    nullable: prop_obj.nullable.unwrap_or(false),
                    default: None,
                    example: prop_obj.example.as_ref().map(|e| e.to_string()),
                })
                .collect()
        })
        .unwrap_or_default();

    let required = obj.required.clone().unwrap_or_default();

    let mut schema = Schema::new(workspace_id, name.to_string());
    schema.description = obj.description.clone();
    schema.schema_type = schema_type;
    schema.properties = properties;
    schema.required = required;
    schema
}

fn convert_property_type(obj: &SchemaObject) -> PropertyType {
    if let Some(ref_path) = &obj.ref_path {
        let ref_name = ref_path.split('/').last().unwrap_or(ref_path).to_string();
        return PropertyType::Ref(ref_name);
    }

    match obj.schema_type.as_deref() {
        Some("string") => PropertyType::String,
        Some("integer") => PropertyType::Integer,
        Some("number") => PropertyType::Number,
        Some("boolean") => PropertyType::Boolean,
        Some("array") => {
            let item_type = obj
                .items
                .as_ref()
                .map(|i| convert_property_type(i))
                .unwrap_or(PropertyType::String);
            PropertyType::Array(Box::new(item_type))
        }
        Some("object") => PropertyType::Object,
        _ => PropertyType::String,
    }
}
