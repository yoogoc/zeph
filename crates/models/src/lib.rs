pub mod autosurgeon_ext;
mod runtime;

use autosurgeon::{Hydrate, Reconcile};
use gpui::SharedString;
pub use runtime::ContentType;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export autosurgeon extensions for convenience
pub use autosurgeon_ext::{opt_shared_string, shared_string, vec_shared_string};

// === Sync Metadata ===
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncMeta {
    pub sync_version: i64,
    pub source: DataSource,
    pub deleted: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum DataSource {
    #[default]
    Local,
    Remote,
}

// === Application State ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationState {
    pub workspaces: Vec<Workspace>,
    pub opened_workspaces: Vec<Uuid>,
    pub active_workspace_id: Option<Uuid>,
    pub current_view: ApplicationView,
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            workspaces: vec![],
            opened_workspaces: vec![],
            active_workspace_id: None,
            current_view: ApplicationView::Home,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApplicationView {
    Home,
    Workspace(Uuid),
}

// === Workspace ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: SharedString,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub sync: SyncMeta,

    // Content
    #[serde(skip)]
    pub folders: Vec<Folder>,
    #[serde(skip)]
    pub services: Vec<Service>,
    #[serde(skip)]
    pub schemas: Vec<Schema>,
    #[serde(skip)]
    pub environments: Vec<Environment>,

    // Runtime state
    #[serde(skip)]
    pub opened_services: Vec<ContentType>,
    #[serde(skip)]
    pub active_service: Option<ContentType>,
    #[serde(skip)]
    pub active_environment_id: Option<Uuid>,
    #[serde(skip)]
    pub current_view: WorkspaceView,
}

// === Environment ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: SharedString,
    pub base_url: SharedString,
    pub is_default: bool,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub sync: SyncMeta,
    #[serde(skip)]
    pub variables: Vec<EnvironmentVariable>,
}

impl Environment {
    pub fn new(workspace_id: Uuid, name: impl Into<SharedString>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            name: name.into(),
            base_url: "".into(),
            is_default: false,
            sort_order: 0,
            created_at: now,
            updated_at: now,
            sync: SyncMeta::default(),
            variables: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentVariable {
    pub id: Uuid,
    pub environment_id: Uuid,
    pub name: SharedString,
    pub var_type: VariableType,
    pub remote_value: Option<SharedString>,
    pub local_value: Option<SharedString>,
    pub description: Option<SharedString>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl EnvironmentVariable {
    pub fn new(environment_id: Uuid, name: impl Into<SharedString>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: Uuid::new_v4(),
            environment_id,
            name: name.into(),
            var_type: VariableType::Default,
            remote_value: None,
            local_value: None,
            description: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn value(&self) -> Option<SharedString> {
        self.local_value.clone().or(self.remote_value.clone())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum VariableType {
    #[default]
    Default,
    Secret,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkspace {
    pub name: SharedString,
    pub create_sample_services: bool,
}

impl Workspace {
    pub fn new(name: impl Into<SharedString>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            created_at: now,
            updated_at: now,
            sync: SyncMeta::default(),
            folders: vec![],
            services: vec![],
            schemas: vec![],
            environments: vec![],
            opened_services: vec![],
            active_service: None,
            active_environment_id: None,
            current_view: WorkspaceView::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum WorkspaceView {
    #[default]
    Services,
    Docs,
    History,
    Settings,
}

// === Folder ===
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FolderType {
    Service,
    Schema,
}

impl Default for FolderType {
    fn default() -> Self {
        Self::Service
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub folder_type: FolderType,
    pub name: SharedString,
    pub description: Option<SharedString>,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub sync: SyncMeta,

    // New fields for common headers functionality
    #[serde(default)]
    pub common_headers: Vec<Header>,
    #[serde(default)]
    pub common_search_params: Vec<SearchParameter>,

    #[serde(skip)]
    pub is_expanded: bool,
}

impl Folder {
    pub fn new(workspace_id: Uuid, name: impl Into<SharedString>, parent_id: Option<Uuid>) -> Self {
        Self::with_type(workspace_id, name, parent_id, FolderType::Service)
    }

    pub fn with_type(
        workspace_id: Uuid,
        name: impl Into<SharedString>,
        parent_id: Option<Uuid>,
        folder_type: FolderType,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            parent_id,
            folder_type,
            name: name.into(),
            description: None,
            sort_order: 0,
            created_at: now,
            updated_at: now,
            sync: SyncMeta::default(),
            common_headers: Vec::new(),
            common_search_params: Vec::new(),
            is_expanded: true,
        }
    }
}

// === Service ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub folder_id: Option<Uuid>,
    pub name: SharedString,
    pub description: Option<SharedString>,
    pub service_type: ServiceType,
    pub deprecated: bool,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub sync: SyncMeta,
}

impl Service {
    pub fn new(
        workspace_id: Uuid,
        name: impl Into<SharedString>,
        service_type: ServiceType,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            folder_id: None,
            name: name.into(),
            description: None,
            service_type,
            deprecated: false,
            sort_order: 0,
            created_at: now,
            updated_at: now,
            sync: SyncMeta::default(),
        }
    }
}

// === Service Types ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceType {
    Http(HttpConfig),
    Grpc(GrpcConfig),
    WebSocket(WebSocketConfig),
}

// === HTTP Body Types ===

/// Body content type for HTTP requests
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Reconcile, Hydrate,
)]
pub enum BodyType {
    #[default]
    None,
    Json,
    Xml,
    Text,
    FormData,
    UrlEncoded,
    Binary,
}

impl BodyType {
    pub fn as_content_type(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Json => Some("application/json"),
            Self::Xml => Some("application/xml"),
            Self::Text => Some("text/plain"),
            Self::FormData => Some("multipart/form-data"),
            Self::UrlEncoded => Some("application/x-www-form-urlencoded"),
            Self::Binary => Some("application/octet-stream"),
        }
    }
}

/// A chunk of body content for CRDT text support
#[derive(Debug, Clone, Serialize, Deserialize, Reconcile, Hydrate, PartialEq)]
pub struct BodyChunk {
    /// Chunk index for ordering and CRDT merging
    #[key]
    pub index: u32,
    /// Text content (stored as String for serde, converted to Text in CRDT layer)
    pub content: String,
}

/// HTTP request body with chunked storage for large content
#[derive(Debug, Clone, Serialize, Deserialize, Reconcile, Hydrate, PartialEq)]
pub struct HttpBody {
    pub body_type: BodyType,
    /// Chunked content for efficient CRDT handling of large bodies
    pub chunks: Vec<BodyChunk>,
}

impl Default for HttpBody {
    fn default() -> Self {
        Self {
            body_type: BodyType::None,
            chunks: vec![],
        }
    }
}

impl HttpBody {
    /// Chunk size threshold: 64KB
    const CHUNK_SIZE: usize = 65536;

    /// Create a new HttpBody with the given type and empty content
    pub fn new(body_type: BodyType) -> Self {
        Self {
            body_type,
            chunks: vec![],
        }
    }

    /// Create a body from content string, automatically chunking if needed
    pub fn from_content(body_type: BodyType, content: &str) -> Self {
        if content.is_empty() {
            return Self::new(body_type);
        }

        let chunks: Vec<BodyChunk> = content
            .as_bytes()
            .chunks(Self::CHUNK_SIZE)
            .enumerate()
            .map(|(i, chunk)| BodyChunk {
                index: i as u32,
                content: std::str::from_utf8(chunk).unwrap_or("").to_string(),
            })
            .collect();

        Self { body_type, chunks }
    }

    /// Merge all chunks into a single content string
    pub fn to_content(&self) -> String {
        let mut sorted = self.chunks.clone();
        sorted.sort_by_key(|c| c.index);
        sorted.iter().map(|c| c.content.as_str()).collect()
    }

    /// Check if the body is empty
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty() || self.chunks.iter().all(|c| c.content.is_empty())
    }

    /// Update the content, re-chunking if necessary
    pub fn set_content(&mut self, content: &str) {
        if content.is_empty() {
            self.chunks.clear();
            return;
        }

        self.chunks = content
            .as_bytes()
            .chunks(Self::CHUNK_SIZE)
            .enumerate()
            .map(|(i, chunk)| BodyChunk {
                index: i as u32,
                content: std::str::from_utf8(chunk).unwrap_or("").to_string(),
            })
            .collect();
    }
}

// === HTTP Draft Config (CRDT-enabled) ===

/// HTTP draft configuration for CRDT-based collaborative editing.
///
/// This struct contains only the editable fields of HttpConfig,
/// all of which support autosurgeon's Reconcile and Hydrate traits.
/// Use this for draft storage and undo/redo operations.
#[derive(Debug, Clone, Serialize, Deserialize, Reconcile, Hydrate, PartialEq)]
pub struct HttpDraftConfig {
    pub method: HttpMethod,
    #[autosurgeon(with = "shared_string")]
    pub path: SharedString,
    #[serde(default)]
    pub headers: Vec<Header>,
    #[serde(default)]
    pub query_params: Vec<QueryParameter>,
    #[serde(default)]
    pub path_params: Vec<PathParameter>,
    #[serde(default)]
    pub body: Option<HttpBody>,
    #[serde(default = "default_use_base_url")]
    pub use_base_url: bool,
}

impl Default for HttpDraftConfig {
    fn default() -> Self {
        Self {
            method: HttpMethod::Get,
            path: "".into(),
            headers: vec![],
            query_params: vec![],
            path_params: vec![],
            body: None,
            use_base_url: default_use_base_url(),
        }
    }
}

impl HttpDraftConfig {
    /// Create a draft config from HttpConfig
    pub fn from_http_config(config: &HttpConfig) -> Self {
        Self {
            method: config.method,
            path: config.path.clone(),
            headers: config.headers.clone(),
            query_params: config.query_params.clone(),
            path_params: config.path_params.clone(),
            body: config.body.clone(),
            use_base_url: config.use_base_url,
        }
    }

    /// Apply draft changes to an HttpConfig, preserving OpenAPI metadata
    pub fn apply_to_http_config(&self, config: &mut HttpConfig) {
        config.method = self.method;
        config.path = self.path.clone();
        config.headers = self.headers.clone();
        config.query_params = self.query_params.clone();
        config.path_params = self.path_params.clone();
        config.body = self.body.clone();
        config.use_base_url = self.use_base_url;
    }

    /// Convert to HttpConfig (OpenAPI metadata will be empty)
    pub fn to_http_config(&self) -> HttpConfig {
        HttpConfig {
            method: self.method,
            path: self.path.clone(),
            headers: self.headers.clone(),
            query_params: self.query_params.clone(),
            path_params: self.path_params.clone(),
            body: self.body.clone(),
            use_base_url: self.use_base_url,
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security: vec![],
        }
    }
}

// === HTTP ===

/// HTTP request configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpConfig {
    pub method: HttpMethod,
    pub path: SharedString,
    /// Top-level headers for the request (editable by user)
    #[serde(default)]
    pub headers: Vec<Header>,
    #[serde(default)]
    pub query_params: Vec<QueryParameter>,
    #[serde(default)]
    pub path_params: Vec<PathParameter>,
    /// Request body with chunked content support
    #[serde(default)]
    pub body: Option<HttpBody>,
    #[serde(default = "default_use_base_url")]
    pub use_base_url: bool,

    // === OpenAPI metadata (read-only, from spec import) ===
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBody>,
    #[serde(default)]
    pub responses: Vec<Response>,
    #[serde(default)]
    pub security: Vec<SharedString>,
}

fn default_use_base_url() -> bool {
    true
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            method: HttpMethod::Get,
            path: "".into(),
            headers: vec![],
            query_params: vec![],
            path_params: vec![],
            body: None,
            use_base_url: default_use_base_url(),
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reconcile, Hydrate)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Parameter {
    pub name: SharedString,
    pub location: ParameterLocation,
    pub description: Option<SharedString>,
    pub required: bool,
    pub schema: Option<SharedString>,
    pub example: Option<SharedString>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestBody {
    pub description: Option<SharedString>,
    pub required: bool,
    pub content: Vec<MediaContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaContent {
    pub media_type: SharedString,
    pub schema: Option<SharedString>,
    pub example: Option<SharedString>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub status_code: SharedString,
    pub description: SharedString,
    pub content: Vec<MediaContent>,
}

/// HTTP Header with CRDT support for collaborative editing
#[derive(Debug, Clone, Serialize, Deserialize, Reconcile, Hydrate, PartialEq)]
pub struct Header {
    /// Unique identifier for CRDT list merging
    #[key]
    #[serde(default = "default_uuid")]
    pub id: Uuid,
    #[autosurgeon(with = "shared_string")]
    pub key: SharedString,
    #[autosurgeon(with = "shared_string")]
    pub value: SharedString,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    #[autosurgeon(with = "opt_shared_string")]
    pub description: Option<SharedString>,
}

fn default_enabled() -> bool {
    true
}

/// Default function for generating new UUIDs during deserialization
/// This enables backward compatibility with old data that doesn't have id fields
fn default_uuid() -> Uuid {
    Uuid::new_v4()
}

impl Header {
    pub fn new(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            id: Uuid::new_v4(),
            key: key.into(),
            value: value.into(),
            enabled: true,
            description: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParameter {
    pub key: SharedString,
    pub value: SharedString,
}

impl SearchParameter {
    pub fn new(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

// === Query and Path Parameters ===

/// Query parameter with enable/disable state for HTTP requests and CRDT support
#[derive(Debug, Clone, Serialize, Deserialize, Reconcile, Hydrate, PartialEq)]
pub struct QueryParameter {
    /// Unique identifier for CRDT list merging
    #[key]
    #[serde(default = "default_uuid")]
    pub id: Uuid,
    #[autosurgeon(with = "shared_string")]
    pub key: SharedString,
    #[autosurgeon(with = "shared_string")]
    pub value: SharedString,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    #[autosurgeon(with = "opt_shared_string")]
    pub description: Option<SharedString>,
}

impl QueryParameter {
    pub fn new(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            id: Uuid::new_v4(),
            key: key.into(),
            value: value.into(),
            enabled: true,
            description: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Path parameter value storage for HTTP requests with CRDT support
#[derive(Debug, Clone, Serialize, Deserialize, Reconcile, Hydrate, PartialEq)]
pub struct PathParameter {
    /// Unique identifier for CRDT list merging
    #[key]
    #[serde(default = "default_uuid")]
    pub id: Uuid,
    #[autosurgeon(with = "shared_string")]
    pub name: SharedString,
    #[autosurgeon(with = "shared_string")]
    pub value: SharedString,
    #[serde(default)]
    #[autosurgeon(with = "opt_shared_string")]
    pub description: Option<SharedString>,
}

impl PathParameter {
    pub fn new(name: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            value: value.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

// === gRPC ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    pub service_url: String,
    pub package: String,
    pub service: String,
    pub method: String,
    pub request_type: String,
    pub response_type: String,
    pub streaming: StreamingType,
    pub metadata: Vec<Header>,
    pub proto_content: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum StreamingType {
    #[default]
    Unary,
    ClientStream,
    ServerStream,
    Bidirectional,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            service_url: String::new(),
            package: String::new(),
            service: String::new(),
            method: String::new(),
            request_type: String::new(),
            response_type: String::new(),
            streaming: StreamingType::default(),
            metadata: vec![],
            proto_content: None,
        }
    }
}

// === WebSocket ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub url: String,
    pub protocols: Vec<String>,
    pub headers: Vec<Header>,
    pub messages: Vec<WsMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub name: String,
    pub content: String,
    pub content_type: WsContentType,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum WsContentType {
    #[default]
    Text,
    Binary,
    Json,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            protocols: vec![],
            headers: vec![],
            messages: vec![],
        }
    }
}

// === OpenAPI Schema (Data Models) ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub folder_id: Option<Uuid>,
    pub name: SharedString,
    pub description: Option<SharedString>,
    pub schema_type: SchemaType,
    pub properties: Vec<SchemaProperty>,
    pub required: Vec<SharedString>,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub sync: SyncMeta,
}

impl Schema {
    pub fn new(workspace_id: Uuid, name: impl Into<SharedString>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            folder_id: None,
            name: name.into(),
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![],
            required: vec![],
            sort_order: 0,
            created_at: now,
            updated_at: now,
            sync: SyncMeta::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum SchemaType {
    #[default]
    Object,
    Array,
    String,
    Integer,
    Number,
    Boolean,
    Enum(Vec<SharedString>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaProperty {
    pub name: SharedString,
    pub property_type: PropertyType,
    pub description: Option<SharedString>,
    pub nullable: bool,
    pub default: Option<SharedString>,
    pub example: Option<SharedString>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyType {
    String,
    Integer,
    Number,
    Boolean,
    Array(Box<PropertyType>),
    Ref(SharedString), // Reference to another schema by name
    Object,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_parameter_new() {
        let param = QueryParameter::new("limit", "10");
        assert_eq!(param.key.as_ref(), "limit");
        assert_eq!(param.value.as_ref(), "10");
        assert!(param.enabled);
        assert!(param.description.is_none());
        // New: verify id is set
        assert!(!param.id.is_nil());
    }

    #[test]
    fn test_query_parameter_with_enabled() {
        let param = QueryParameter::new("page", "1").with_enabled(false);
        assert!(!param.enabled);
    }

    #[test]
    fn test_query_parameter_with_description() {
        let param = QueryParameter::new("sort", "asc").with_description("Sort order");
        assert_eq!(param.description.as_ref().unwrap().as_ref(), "Sort order");
    }

    #[test]
    fn test_query_parameter_builder_chain() {
        let param = QueryParameter::new("filter", "active")
            .with_enabled(false)
            .with_description("Filter by status");

        assert_eq!(param.key.as_ref(), "filter");
        assert_eq!(param.value.as_ref(), "active");
        assert!(!param.enabled);
        assert_eq!(
            param.description.as_ref().unwrap().as_ref(),
            "Filter by status"
        );
    }

    #[test]
    fn test_path_parameter_new() {
        let param = PathParameter::new("userId", "123");
        assert_eq!(param.name.as_ref(), "userId");
        assert_eq!(param.value.as_ref(), "123");
        assert!(param.description.is_none());
        // New: verify id is set
        assert!(!param.id.is_nil());
    }

    #[test]
    fn test_path_parameter_with_description() {
        let param = PathParameter::new("postId", "456").with_description("Post identifier");
        assert_eq!(
            param.description.as_ref().unwrap().as_ref(),
            "Post identifier"
        );
    }

    #[test]
    fn test_http_config_default_includes_params() {
        let config = HttpConfig::default();
        assert!(config.query_params.is_empty());
        assert!(config.path_params.is_empty());
        assert!(config.headers.is_empty());
        assert_eq!(config.method, HttpMethod::Get);
        assert!(config.use_base_url);
        assert!(config.body.is_none());
    }

    #[test]
    fn test_http_config_with_params() {
        let mut config = HttpConfig::default();
        config.query_params.push(QueryParameter::new("limit", "10"));
        config.path_params.push(PathParameter::new("id", "123"));

        assert_eq!(config.query_params.len(), 1);
        assert_eq!(config.path_params.len(), 1);
        assert_eq!(config.query_params[0].key.as_ref(), "limit");
        assert_eq!(config.path_params[0].name.as_ref(), "id");
    }

    #[test]
    fn test_http_config_multiple_params() {
        let mut config = HttpConfig::default();
        config.query_params.push(QueryParameter::new("limit", "10"));
        config
            .query_params
            .push(QueryParameter::new("page", "2").with_enabled(false));
        config.path_params.push(PathParameter::new("userId", "123"));
        config.path_params.push(PathParameter::new("postId", "456"));

        assert_eq!(config.query_params.len(), 2);
        assert_eq!(config.path_params.len(), 2);
        assert!(config.query_params[0].enabled);
        assert!(!config.query_params[1].enabled);
    }

    #[test]
    fn test_header_new() {
        let header = Header::new("Content-Type", "application/json");
        assert_eq!(header.key.as_ref(), "Content-Type");
        assert_eq!(header.value.as_ref(), "application/json");
        assert!(header.enabled);
        assert!(header.description.is_none());
        assert!(!header.id.is_nil());
    }

    #[test]
    fn test_header_builder_chain() {
        let header = Header::new("Authorization", "Bearer token")
            .with_enabled(false)
            .with_description("Auth header");

        assert_eq!(header.key.as_ref(), "Authorization");
        assert!(!header.enabled);
        assert_eq!(header.description.as_ref().unwrap().as_ref(), "Auth header");
    }

    #[test]
    fn test_http_body_empty() {
        let body = HttpBody::new(BodyType::Json);
        assert!(body.is_empty());
        assert_eq!(body.body_type, BodyType::Json);
        assert_eq!(body.to_content(), "");
    }

    #[test]
    fn test_http_body_small_content() {
        let content = r#"{"key": "value"}"#;
        let body = HttpBody::from_content(BodyType::Json, content);

        assert!(!body.is_empty());
        assert_eq!(body.chunks.len(), 1);
        assert_eq!(body.to_content(), content);
    }

    #[test]
    fn test_http_body_set_content() {
        let mut body = HttpBody::new(BodyType::Text);
        body.set_content("Hello, World!");

        assert_eq!(body.to_content(), "Hello, World!");

        body.set_content("");
        assert!(body.is_empty());
    }

    #[test]
    fn test_body_type_content_type() {
        assert_eq!(BodyType::Json.as_content_type(), Some("application/json"));
        assert_eq!(BodyType::Xml.as_content_type(), Some("application/xml"));
        assert_eq!(BodyType::Text.as_content_type(), Some("text/plain"));
        assert_eq!(BodyType::None.as_content_type(), None);
    }

    #[test]
    fn test_http_config_with_headers_and_body() {
        let mut config = HttpConfig::default();
        config
            .headers
            .push(Header::new("Accept", "application/json"));
        config.body = Some(HttpBody::from_content(BodyType::Json, r#"{"test": true}"#));

        assert_eq!(config.headers.len(), 1);
        assert!(config.body.is_some());
        assert_eq!(
            config.body.as_ref().unwrap().to_content(),
            r#"{"test": true}"#
        );
    }
}
