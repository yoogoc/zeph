use gpui::SharedString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub opened_services: Vec<Uuid>,
    #[serde(skip)]
    pub active_service: Option<Uuid>,
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
    pub base_url: String,
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
            base_url: String::new(),
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
    pub remote_value: Option<String>,
    pub local_value: Option<String>,
    pub description: Option<String>,
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

    pub fn value(&self) -> Option<&str> {
        self.local_value.as_deref().or(self.remote_value.as_deref())
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
    pub create_sample_services: bool
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub description: Option<String>,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub sync: SyncMeta,

    #[serde(skip)]
    pub is_expanded: bool,
}

impl Folder {
    pub fn new(workspace_id: Uuid, name: impl Into<SharedString>, parent_id: Option<Uuid>) -> Self {
        Self::with_type(workspace_id, name, parent_id, FolderType::Service)
    }

    pub fn with_type(workspace_id: Uuid, name: impl Into<SharedString>, parent_id: Option<Uuid>, folder_type: FolderType) -> Self {
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
    pub description: Option<String>,
    pub service_type: ServiceType,
    pub deprecated: bool,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub sync: SyncMeta,
}

impl Service {
    pub fn new(workspace_id: Uuid, name: impl Into<SharedString>, service_type: ServiceType) -> Self {
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

// === HTTP ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub method: HttpMethod,
    pub path: String,
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBody>,
    pub responses: Vec<Response>,
    pub security: Vec<String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            method: HttpMethod::Get,
            path: String::new(),
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub location: ParameterLocation,
    pub description: Option<String>,
    pub required: bool,
    pub schema: Option<String>,
    pub example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBody {
    pub description: Option<String>,
    pub required: bool,
    pub content: Vec<MediaContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaContent {
    pub media_type: String,
    pub schema: Option<String>,
    pub example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub status_code: String,
    pub description: String,
    pub content: Vec<MediaContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub key: SharedString,
    pub value: SharedString,
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
    pub description: Option<String>,
    pub schema_type: SchemaType,
    pub properties: Vec<SchemaProperty>,
    pub required: Vec<String>,
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
    Enum(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaProperty {
    pub name: String,
    pub property_type: PropertyType,
    pub description: Option<String>,
    pub nullable: bool,
    pub default: Option<String>,
    pub example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyType {
    String,
    Integer,
    Number,
    Boolean,
    Array(Box<PropertyType>),
    Ref(String), // Reference to another schema by name
    Object,
}
