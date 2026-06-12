use crate::utils::cal_icon as cal_icon_ct;
use gpui::{IntoElement, SharedString};
use gpui_component::tree::TreeEntry;
use gpui_component::IconName;
use models::{ContentType, Schema, Service};
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
