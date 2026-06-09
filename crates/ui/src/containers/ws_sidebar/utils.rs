use crate::utils::cal_icon as cal_icon_ct;
use gpui::{IntoElement, SharedString};
use gpui_component::tree::{TreeEntry, TreeItem};
use gpui_component::IconName;
use models::{ContentType, Folder, FolderType, Schema, Service};
use uuid::Uuid;

pub const SERVICE_ID: &str = "services";
pub const SCHEMA_ID: &str = "schemas";

#[derive(Debug)]
pub enum TreeId {
    ServiceRoot,
    SchemaRoot,
    ServiceFolder(Uuid),
    SchemaFolder(Uuid),
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
        } else if parts.len() == 2 {
            match x {
                SERVICE_ID => TreeId::Service(Uuid::parse_str(&parts[1]).unwrap()),
                SCHEMA_ID => TreeId::Schema(Uuid::parse_str(&parts[1]).unwrap()),
                _ => unreachable!(),
            }
        } else if parts.len() == 3 {
            match x {
                SERVICE_ID => TreeId::ServiceFolder(Uuid::parse_str(&parts[2]).unwrap()),
                SCHEMA_ID => TreeId::SchemaFolder(Uuid::parse_str(&parts[2]).unwrap()),
                _ => unreachable!(),
            }
        } else {
            unreachable!()
        }
    }

    pub fn from_content(value: &ContentType) -> Self {
        match value {
            ContentType::ServiceFolder(folder) => Self::ServiceFolder(folder.id),
            ContentType::SchemaFolder(folder) => Self::SchemaFolder(folder.id),
            ContentType::Service(service) => Self::Service(service.id),
            ContentType::Schema(schema) => Self::Schema(schema.id),
        }
    }

    pub fn to_string(&self) -> SharedString {
        match self {
            TreeId::ServiceRoot => SERVICE_ID.into(),
            TreeId::SchemaRoot => SCHEMA_ID.into(),
            TreeId::Service(id) => format!("{}/{}", SERVICE_ID, id).into(),
            TreeId::Schema(id) => format!("{}/{}", SCHEMA_ID, id).into(),
            TreeId::ServiceFolder(id) => format!("{}/folder/{}", SERVICE_ID, id).into(),
            TreeId::SchemaFolder(id) => format!("{}/folder/{}", SCHEMA_ID, id).into(),
        }
    }
}

impl From<SharedString> for TreeId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

pub fn build_tree(folders: &[Folder], services: &[Service], schemas: &[Schema]) -> Vec<TreeItem> {
    vec![
        TreeItem::new(SERVICE_ID, shared_t!("workspaces.service"))
            .expanded(true)
            .children(build_service_tree(None, 0, folders, services)),
        TreeItem::new(SCHEMA_ID, shared_t!("workspaces.schema"))
            .expanded(true)
            .children(build_schema_tree(None, 0, folders, schemas)),
    ]
}

pub fn build_service_tree(
    parent_id: Option<Uuid>,
    depth: usize,
    folders: &[Folder],
    services: &[Service],
) -> Vec<TreeItem> {
    let mut items = vec![];
    for folder in folders
        .iter()
        .filter(|f| f.parent_id == parent_id && matches!(f.folder_type, FolderType::Service))
    {
        items.push(
            TreeItem::new(
                format!("{}/folder/{}", SERVICE_ID, folder.id.to_string()),
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
    folders: &[Folder],
    schemas: &[Schema],
) -> Vec<TreeItem> {
    let mut items = vec![];
    for folder in folders
        .iter()
        .filter(|f| f.parent_id == parent_id && matches!(f.folder_type, FolderType::Schema))
    {
        items.push(
            TreeItem::new(
                format!("{}/folder/{}", SCHEMA_ID, folder.id.to_string()),
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

    for schema in schemas.iter().filter(|s| s.folder_id == parent_id) {
        items.push(TreeItem::new(
            format!("{}/{}", SCHEMA_ID, schema.id.to_string()),
            schema.name.clone(),
        ));
    }

    items
}

pub fn cal_icon(entry: &TreeEntry, services: &[Service], schemas: &[Schema]) -> impl IntoElement {
    let id = TreeId::new(entry.item().id.clone());
    match id {
        TreeId::Service(id) => match services.iter().find(|s| s.id == id) {
            Some(service) => cal_icon_ct(&ContentType::Service(service.clone())).into_any_element(),
            None => IconName::File.into_any_element(),
        },
        TreeId::Schema(id) => match schemas.iter().find(|s| s.id == id) {
            Some(schema) => cal_icon_ct(&ContentType::Schema(schema.clone())).into_any_element(),
            None => IconName::File.into_any_element(),
        },
        _ => cal_folder_icon(entry).into_any_element(),
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
