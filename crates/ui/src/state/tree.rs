//! Sidebar tree construction from workspace data.
//!
//! Produces the `TreeItem` list consumed by `WorkspaceSidebar`. The item ids
//! follow the format parsed by [`crate::ws_sidebar::TreeId`], using the shared
//! `SERVICE_ID` / `SCHEMA_ID` prefixes.

use crate::ws_sidebar::{SCHEMA_ID, SERVICE_ID};
use gpui::SharedString;
use gpui_component::tree::TreeItem;
use models::{Folder, FolderType, Schema, Service};
use uuid::Uuid;

pub fn build_tree(folders: &[Folder], services: &[Service], schemas: &[Schema]) -> Vec<TreeItem> {
    vec![
        TreeItem::new(SERVICE_ID, shared_t!("workspaces.service"))
            .expanded(true)
            .children(build_service_tree(None, folders, services)),
        TreeItem::new(SCHEMA_ID, shared_t!("workspaces.schema"))
            .expanded(true)
            .children(build_schema_tree(None, folders, schemas)),
    ]
}

fn build_service_tree(
    parent_id: Option<Uuid>,
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
                format!("{}/folder/{}", SERVICE_ID, folder.id),
                folder.name.clone(),
            )
            .children(build_service_tree(Some(folder.id), folders, services)),
        );
    }

    for service in services.iter().filter(|s| s.folder_id == parent_id) {
        items.push(TreeItem::new(
            format!("{}/{}", SERVICE_ID, service.id),
            service.name.clone(),
        ));
    }

    items
}

fn build_schema_tree(
    parent_id: Option<Uuid>,
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
                format!("{}/folder/{}", SCHEMA_ID, folder.id),
                folder.name.clone(),
            )
            .children(build_schema_tree(Some(folder.id), folders, schemas)),
        );
    }

    for schema in schemas.iter().filter(|s| s.folder_id == parent_id) {
        items.push(TreeItem::new(
            format!("{}/{}", SCHEMA_ID, schema.id),
            schema.name.clone(),
        ));
    }

    items
}
