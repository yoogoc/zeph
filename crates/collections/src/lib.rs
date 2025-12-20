use gpui::{App, AppContext, Context, Entity, Global};
use models::{Collection, Service, ServiceType, HttpConfig, HttpMethod};
use uuid::Uuid;
use std::collections::HashMap;

pub fn init(cx: &mut App) {
    CollectionRegistry::set_global(cx.new(CollectionRegistry::new), cx);
}

struct GlobalCollectionRegistry(Entity<CollectionRegistry>);
impl Global for GlobalCollectionRegistry {}

pub struct CollectionRegistry {
    // workspace_id -> collections
    workspace_collections: HashMap<Uuid, Vec<Collection>>,
}

impl CollectionRegistry {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalCollectionRegistry>().0.clone()
    }
    
    fn set_global(registry: Entity<Self>, cx: &mut App) {
        cx.set_global(GlobalCollectionRegistry(registry));
    }
    
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { 
            workspace_collections: HashMap::new(),
        }
    }
    
    pub fn get_collections(&self, workspace_id: Uuid) -> Vec<Collection> {
        self.workspace_collections.get(&workspace_id).cloned().unwrap_or_default()
    }
    
    pub fn get_collection(&self, workspace_id: Uuid, collection_id: Uuid) -> Option<&Collection> {
        self.workspace_collections
            .get(&workspace_id)?
            .iter()
            .find(|c| c.id == collection_id)
    }
    
    pub fn ensure_workspace_collections(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if !self.workspace_collections.contains_key(&workspace_id) {
            // Create sample collection for new workspace
            let mut sample_collection = Collection::new("Sample API".into());
            
            // Add sample services
            let get_users = Service::new(
                "Get Users".into(),
                ServiceType::Http(HttpConfig {
                    method: HttpMethod::Get,
                    url: "https://api.example.com/users".into(),
                    headers: vec![],
                    body: None,
                })
            );
            
            let create_user = Service::new(
                "Create User".into(),
                ServiceType::Http(HttpConfig {
                    method: HttpMethod::Post,
                    url: "https://api.example.com/users".into(),
                    headers: vec![],
                    body: Some("{}".into()),
                })
            );

            let lakekeeper = Service::new(
                "lakekeeper openapi".into(),
                ServiceType::Http(HttpConfig {
                    method: HttpMethod::Get,
                    url: "https://docs.lakekeeper.io/docs/nightly/api/management-open-api.yaml".into(),
                    headers: vec![],
                    body: Some("{}".into()),
                })
            );
            let json = Service::new(
                "httpbin json".into(),
                ServiceType::Http(HttpConfig {
                    method: HttpMethod::Get,
                    url: "https://httpbin.org/json".into(),
                    headers: vec![],
                    body: Some("{}".into()),
                })
            );

            let stream_byte = Service::new(
                "httpbin stream byte".into(),
                ServiceType::Http(HttpConfig {
                    method: HttpMethod::Get,
                    url: "https://httpbin.org/stream-bytes/5".into(),
                    headers: vec![],
                    body: Some("{}".into()),
                })
            );
            let stream_json = Service::new(
                "httpbin stream json".into(),
                ServiceType::Http(HttpConfig {
                    method: HttpMethod::Get,
                    url: "https://httpbin.org/stream/5".into(),
                    headers: vec![],
                    body: Some("{}".into()),
                })
            );

            
            sample_collection.services.push(get_users);
            sample_collection.services.push(create_user);
            sample_collection.services.push(lakekeeper);
            sample_collection.services.push(json);
            sample_collection.services.push(stream_byte);
            sample_collection.services.push(stream_json);

            self.workspace_collections.insert(workspace_id, vec![sample_collection]);
            cx.notify();
        }
    }
    
    pub fn open_service(&mut self, workspace_id: Uuid, collection_id: Uuid, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(collections) = self.workspace_collections.get_mut(&workspace_id) {
            if let Some(collection) = collections.iter_mut().find(|c| c.id == collection_id) {
                if !collection.opened_services.contains(&service_id) {
                    collection.opened_services.push(service_id);
                }
                collection.active_service = Some(service_id);
                cx.notify();
            }
        }
    }
    
    pub fn close_service(&mut self, workspace_id: Uuid, collection_id: Uuid, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(collections) = self.workspace_collections.get_mut(&workspace_id) {
            if let Some(collection) = collections.iter_mut().find(|c| c.id == collection_id) {
                collection.opened_services.retain(|&id| id != service_id);
                if collection.active_service == Some(service_id) {
                    collection.active_service = collection.opened_services.first().copied();
                }
                cx.notify();
            }
        }
    }
    
    pub fn set_active_service(&mut self, workspace_id: Uuid, collection_id: Uuid, service_id: Uuid, cx: &mut Context<Self>) {
        if let Some(collections) = self.workspace_collections.get_mut(&workspace_id) {
            if let Some(collection) = collections.iter_mut().find(|c| c.id == collection_id) {
                collection.active_service = Some(service_id);
                cx.notify();
            }
        }
    }
}
