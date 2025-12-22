use gpui::SharedString;
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
    pub name: SharedString,
    pub collections: Vec<Collection>,
    pub current_view: WorkspaceView,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Workspace {
    pub fn new(name: SharedString) -> Self {
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
    pub name: SharedString,
    pub services: Vec<Service>,
    pub folders: Vec<Folder>,
    pub opened_services: Vec<Uuid>, // Track opened services for tabs
    pub active_service: Option<Uuid>, // Currently active service
}

impl Collection {
    pub fn new(name: SharedString) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            services: vec![],
            folders: vec![],
            opened_services: vec![],
            active_service: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub name: SharedString,
    pub parent_id: Option<Uuid>, // None for root folders
    pub service_ids: Vec<Uuid>,
    pub is_expanded: bool,
}

impl Folder {
    pub fn new(name: SharedString, parent_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            parent_id,
            service_ids: vec![],
            is_expanded: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: Uuid,
    pub name: SharedString,
    pub service_type: ServiceType,
    pub folder_id: Option<Uuid>, // None for root level services
}

impl Service {
    pub fn new(name: SharedString, service_type: ServiceType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            service_type,
            folder_id: None,
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
    pub url: SharedString,
    pub headers: Vec<Header>,
    pub body: Option<SharedString>,
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
pub struct Header {
    pub key: SharedString,
    pub value: SharedString,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    pub service_url: SharedString,
    pub method: SharedString,
    pub metadata: Vec<Header>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub url: SharedString,
    pub protocols: Vec<SharedString>,
}
