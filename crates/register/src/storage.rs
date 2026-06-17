use crate::example::SERVICE_SIMPLES;
use gpui::{App, Context, Global};
use log::error;
use models::{CreateWorkspace, HttpConfig, Service, ServiceType, Workspace};
use std::sync::Arc;
use storage::{
    Database, DraftStore, EnvironmentStore, EnvironmentVariableStore, FolderQueryParameterStore,
    FolderStore, PathParameterStore, QueryParameterStore, SchemaStore, ServiceStore,
    WorkspaceStore,
};
use uuid::Uuid;

pub struct GlobalStorage {
    // db: Arc<Database>,
    pub workspace: Arc<WorkspaceStore>,
    pub folder: Arc<FolderStore>,
    pub service: Arc<ServiceStore>,
    pub schema: Arc<SchemaStore>,
    pub env: Arc<EnvironmentStore>,
    pub var: Arc<EnvironmentVariableStore>,
    pub query_param: Arc<QueryParameterStore>,
    pub path_param: Arc<PathParameterStore>,
    pub folder_query_param: Arc<FolderQueryParameterStore>,
    pub draft: Arc<DraftStore>,
}

impl Global for GlobalStorage {}

pub fn init(cx: &mut App) {
    let db_path = db_path();
    let db = Database::open(&db_path).expect("Failed to open database");
    let db = Arc::new(db);

    let workspace = Arc::new(WorkspaceStore::new(db.conn()));
    let folder = Arc::new(FolderStore::new(db.conn()));
    let service = Arc::new(ServiceStore::new(db.conn()));
    let schema = Arc::new(SchemaStore::new(db.conn()));
    let env = Arc::new(EnvironmentStore::new(db.conn()));
    let var = Arc::new(EnvironmentVariableStore::new(db.conn()));
    let query_param = Arc::new(QueryParameterStore::new(db.conn()));
    let path_param = Arc::new(PathParameterStore::new(db.conn()));
    let folder_query_param = Arc::new(FolderQueryParameterStore::new(db.conn()));
    let draft = Arc::new(DraftStore::new(db.conn()));

    cx.set_global(GlobalStorage {
        // db,
        workspace,
        folder,
        service,
        schema,
        env,
        var,
        query_param,
        path_param,
        folder_query_param,
        draft,
    });
}

fn db_path() -> std::path::PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zeph");
    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("zeph.db")
}

impl GlobalStorage {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn create_workspace(&self, param: CreateWorkspace, _cx: &mut Context<Self>) {
        let workspace = Workspace::new(param.name);
        self.workspace.insert(&workspace).ok();
        if param.create_sample_services {
            self.create_sample_services(workspace.id);
        }
    }

    fn create_sample_services(&self, workspace_id: Uuid) {
        for (name, method, path) in SERVICE_SIMPLES.iter() {
            let service = Service::new(
                workspace_id,
                name,
                ServiceType::Http(HttpConfig {
                    method: method.clone(),
                    path: path.into(),
                    ..Default::default()
                }),
            );
            if let Err(err) = self.service.insert(&service) {
                error!("Failed to create service: {}", err);
            }
        }
    }
}
