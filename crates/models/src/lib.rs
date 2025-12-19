use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationState {
    pub workspaces: Vec<Workspace>,
    pub active_workspace_id: Option<Uuid>,
    pub current_view: ApplicationView,
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            workspaces: vec![],
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub collections: Vec<Collection>,
    pub current_view: WorkspaceView,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Workspace {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            collections: vec![],
            current_view: WorkspaceView::Collections,
            created_at: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceView {
    Collections,
    Docs,
    History,
    Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    pub services: Vec<Service>,
}

impl Collection {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            services: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: Uuid,
    pub name: String,
    pub service_type: ServiceType,
}

impl Service {
    pub fn new(name: String, service_type: ServiceType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            service_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceType {
    Http(HttpConfig),
    Grpc(GrpcConfig),
    WebSocket(WebSocketConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<Header>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    pub service_url: String,
    pub method: String,
    pub metadata: Vec<Header>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub url: String,
    pub protocols: Vec<String>,
}
