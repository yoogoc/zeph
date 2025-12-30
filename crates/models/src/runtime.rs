use crate::{Folder, Schema, Service};
use gpui::SharedString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    ServiceFolder(Folder),
    SchemaFolder(Folder),
    Service(Service),
    Schema(Schema),
}

impl ContentType {
    pub fn name(&self) -> SharedString {
        match self {
            ContentType::ServiceFolder(folder) => folder.name.clone(),
            ContentType::SchemaFolder(folder) => folder.name.clone(),
            ContentType::Service(service) => service.name.clone(),
            ContentType::Schema(schema) => schema.name.clone(),
        }
    }
    pub fn id(&self) -> Uuid {
        match self {
            ContentType::ServiceFolder(folder) => folder.id.clone(),
            ContentType::SchemaFolder(folder) => folder.id.clone(),
            ContentType::Service(service) => service.id.clone(),
            ContentType::Schema(schema) => schema.id.clone(),
        }
    }
}

impl PartialEq for ContentType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ServiceFolder(a), Self::ServiceFolder(b)) => a.id == b.id,
            (Self::SchemaFolder(a), Self::SchemaFolder(b)) => a.id == b.id,
            (Self::Service(a), Self::Service(b)) => a.id == b.id,
            (Self::Schema(a), Self::Schema(b)) => a.id == b.id,
            _ => false,
        }
    }
}
