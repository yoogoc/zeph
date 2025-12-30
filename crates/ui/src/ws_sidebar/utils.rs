use gpui::SharedString;
use gpui_component::tree::{TreeEntry, TreeItem};
use gpui_component::IconName;
use models::{ContentType, Folder, FolderType, Schema, Service, Workspace};
use uuid::Uuid;

pub const SERVICE_ID: &str = "services";
pub const SCHEMA_ID: &str = "schemas";

#[derive(Debug)]
pub enum TreeId {
    ServiceRoot,
    SchemaRoot,
    Service(Uuid),
    Schema(Uuid),
}

impl TreeId {
    pub fn new(value: SharedString) -> Self {
        let parts: Vec<&str> = value.split("/").collect();
        let x = parts[0];
        if parts.len() == 1 {
            match x {
                SERVICE_ID => TreeId::ServiceRoot,
                SCHEMA_ID => TreeId::SchemaRoot,
                _ => unreachable!(),
            }
        } else {
            match x {
                SERVICE_ID => TreeId::Service(Uuid::parse_str(&parts[1]).unwrap()),
                SCHEMA_ID => TreeId::Schema(Uuid::parse_str(&parts[1]).unwrap()),
                _ => unreachable!(),
            }
        }
    }

    pub fn from_content(value: &ContentType) -> Self {
        match value {
            ContentType::ServiceFolder(folder) => Self::Service(folder.id),
            ContentType::SchemaFolder(folder) => Self::Service(folder.id),
            ContentType::Service(service) => Self::Service(service.id),
            ContentType::Schema(schema) => Self::Service(schema.id),
        }
    }

    pub fn to_string(&self) -> SharedString {
        match self {
            TreeId::ServiceRoot => SERVICE_ID.into(),
            TreeId::SchemaRoot => SCHEMA_ID.into(),
            TreeId::Service(id) => format!("{}/{}", SERVICE_ID, id).into(),
            TreeId::Schema(id) => format!("{}/{}", SCHEMA_ID, id).into(),
        }
    }
}

impl From<SharedString> for TreeId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

pub fn build_tree(workspace: Workspace) -> Vec<TreeItem> {
    vec![
        TreeItem::new(SERVICE_ID, "Services")
            .expanded(true)
            .children(build_service_tree(
                None,
                0,
                &workspace.folders,
                &workspace.services,
            )),
        TreeItem::new(SCHEMA_ID, "Schemas")
            .expanded(true)
            .children(build_schema_tree(
                None,
                0,
                &workspace.folders,
                &workspace.schemas,
            )),
    ]
}

pub fn build_service_tree(
    parent_id: Option<Uuid>,
    depth: usize,
    folders: &Vec<Folder>,
    services: &Vec<Service>,
) -> Vec<TreeItem> {
    let mut items = vec![];
    // Add folders first
    for folder in folders
        .iter()
        .filter(|f| f.parent_id == parent_id && matches!(f.folder_type, FolderType::Service))
    {
        // TODO Skip if we've reached maximum depth

        items.push(
            TreeItem::new(
                format!("{}/{}", SERVICE_ID, folder.id.to_string()),
                folder.name.clone(),
            )
            .children(build_service_tree(
                Some(folder.id),
                depth + 1,
                folders,
                services,
            )),
        );
    }

    // Add services
    for service in services.iter().filter(|s| s.folder_id == parent_id) {
        items.push(TreeItem::new(
            format!("{}/{}", SERVICE_ID, service.id.to_string()),
            service.name.clone(),
        ));
    }

    items
}
pub fn build_schema_tree(
    parent_id: Option<Uuid>,
    depth: usize,
    folders: &Vec<Folder>,
    schemas: &Vec<Schema>,
) -> Vec<TreeItem> {
    let mut items = vec![];
    // Add folders first
    for folder in folders
        .iter()
        .filter(|f| f.parent_id == parent_id && matches!(f.folder_type, FolderType::Schema))
    {
        // TODO Skip if we've reached maximum depth

        items.push(
            TreeItem::new(
                format!("{}/{}", SCHEMA_ID, folder.id.to_string()),
                folder.name.clone(),
            )
            .children(build_schema_tree(
                Some(folder.id),
                depth + 1,
                folders,
                schemas,
            )),
        );
    }

    // Add schemas
    for schema in schemas.iter().filter(|s| s.folder_id == parent_id) {
        items.push(TreeItem::new(
            format!("{}/{}", SCHEMA_ID, schema.id.to_string()),
            schema.name.clone(),
        ));
    }

    items
}

pub fn cal_icon(entry: &TreeEntry) -> IconName {
    if entry.is_folder() {
        cal_folder_icon(entry)
    } else {
        IconName::File
    }
}

pub fn cal_folder_icon(entry: &TreeEntry) -> IconName {
    if entry.item().id.eq(SERVICE_ID) || entry.item().id.eq(SCHEMA_ID) {
        if entry.is_expanded() {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        }
    } else {
        if entry.is_expanded() {
            IconName::FolderOpen
        } else {
            IconName::Folder
        }
    }
}
