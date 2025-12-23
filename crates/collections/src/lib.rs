use gpui::{App, AppContext, Context, Entity, Global};
use models::{Collection, Folder, Service, ServiceType, HttpConfig, HttpMethod};
use storage::{Database, CollectionStore, FolderStore, ServiceStore, EnvironmentStore};
use uuid::Uuid;
use std::collections::HashMap;
use std::sync::Arc;

pub fn init(cx: &mut App) {
    CollectionRegistry::set_global(cx.new(CollectionRegistry::new), cx);
}

struct GlobalCollectionRegistry(Entity<CollectionRegistry>);
impl Global for GlobalCollectionRegistry {}

pub struct CollectionRegistry {
    db: Arc<Database>,
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
        let db_path = Self::db_path();
        let db = Database::open(&db_path).expect("Failed to open database");
        
        Self { 
            db: Arc::new(db),
            workspace_collections: HashMap::new(),
        }
    }

    fn db_path() -> std::path::PathBuf {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("zeph");
        std::fs::create_dir_all(&data_dir).ok();
        data_dir.join("zeph.db")
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

    pub fn load_workspace_collections(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        let store = CollectionStore::new(self.db.conn());
        let folder_store = FolderStore::new(self.db.conn());
        let service_store = ServiceStore::new(self.db.conn());
        let env_store = EnvironmentStore::new(self.db.conn());

        if let Ok(mut collections) = store.list_by_workspace(workspace_id) {
            for collection in &mut collections {
                collection.folders = folder_store.list_by_collection(collection.id).unwrap_or_default();
                collection.services = service_store.list_by_collection(collection.id).unwrap_or_default();
                collection.environments = env_store.list_by_collection(collection.id).unwrap_or_default();
            }
            self.workspace_collections.insert(workspace_id, collections);
            cx.notify();
        }
    }
    
    pub fn ensure_workspace_collections(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        if self.workspace_collections.contains_key(&workspace_id) {
            return;
        }

        self.load_workspace_collections(workspace_id, cx);

        if self.workspace_collections.get(&workspace_id).map(|c| c.is_empty()).unwrap_or(true) {
            self.create_sample_collection(workspace_id, cx);
        }
    }

    fn create_sample_collection(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        let mut collection = Collection::new(workspace_id, "Sample API");
        
        let services = vec![
            ("Get Users", HttpMethod::Get, "https://api.example.com/users"),
            ("Create User", HttpMethod::Post, "https://api.example.com/users"),
            ("httpbin json", HttpMethod::Get, "https://httpbin.org/json"),
            ("httpbin stream", HttpMethod::Get, "https://httpbin.org/stream/5"),
        ];

        for (name, method, path) in services {
            let service = Service::new(
                collection.id,
                name,
                ServiceType::Http(HttpConfig {
                    method,
                    path: path.into(),
                    ..Default::default()
                })
            );
            collection.services.push(service);
        }

        // Save to database
        let store = CollectionStore::new(self.db.conn());
        let service_store = ServiceStore::new(self.db.conn());
        
        store.insert(&collection).ok();
        for service in &collection.services {
            service_store.insert(service).ok();
        }

        self.workspace_collections.insert(workspace_id, vec![collection]);
        cx.notify();
    }

    pub fn create_collection(&mut self, workspace_id: Uuid, name: impl Into<gpui::SharedString>, cx: &mut Context<Self>) -> Uuid {
        let collection = Collection::new(workspace_id, name);
        let id = collection.id;

        let store = CollectionStore::new(self.db.conn());
        store.insert(&collection).ok();

        self.workspace_collections
            .entry(workspace_id)
            .or_default()
            .push(collection);
        
        cx.notify();
        id
    }

    pub fn create_service(&mut self, workspace_id: Uuid, collection_id: Uuid, service: Service, cx: &mut Context<Self>) {
        let store = ServiceStore::new(self.db.conn());
        store.insert(&service).ok();

        if let Some(collections) = self.workspace_collections.get_mut(&workspace_id) {
            if let Some(collection) = collections.iter_mut().find(|c| c.id == collection_id) {
                collection.services.push(service);
                cx.notify();
            }
        }
    }

    pub fn create_folder(&mut self, workspace_id: Uuid, collection_id: Uuid, folder: Folder, cx: &mut Context<Self>) {
        let store = FolderStore::new(self.db.conn());
        store.insert(&folder).ok();

        if let Some(collections) = self.workspace_collections.get_mut(&workspace_id) {
            if let Some(collection) = collections.iter_mut().find(|c| c.id == collection_id) {
                collection.folders.push(folder);
                cx.notify();
            }
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

    pub fn delete_service(&mut self, workspace_id: Uuid, collection_id: Uuid, service_id: Uuid, cx: &mut Context<Self>) {
        let store = ServiceStore::new(self.db.conn());
        store.delete(service_id).ok();

        if let Some(collections) = self.workspace_collections.get_mut(&workspace_id) {
            if let Some(collection) = collections.iter_mut().find(|c| c.id == collection_id) {
                collection.services.retain(|s| s.id != service_id);
                collection.opened_services.retain(|&id| id != service_id);
                if collection.active_service == Some(service_id) {
                    collection.active_service = collection.opened_services.first().copied();
                }
                cx.notify();
            }
        }
    }

    pub fn delete_folder(&mut self, workspace_id: Uuid, collection_id: Uuid, folder_id: Uuid, cx: &mut Context<Self>) {
        let store = FolderStore::new(self.db.conn());
        store.delete(folder_id).ok();

        if let Some(collections) = self.workspace_collections.get_mut(&workspace_id) {
            if let Some(collection) = collections.iter_mut().find(|c| c.id == collection_id) {
                collection.folders.retain(|f| f.id != folder_id);
                cx.notify();
            }
        }
    }

    pub fn delete_collection(&mut self, workspace_id: Uuid, collection_id: Uuid, cx: &mut Context<Self>) {
        let store = CollectionStore::new(self.db.conn());
        store.delete(collection_id).ok();

        if let Some(collections) = self.workspace_collections.get_mut(&workspace_id) {
            collections.retain(|c| c.id != collection_id);
            cx.notify();
        }
    }
}
