use gpui::*;
use gpui_component::list::ListItem;
use gpui_component::tree::{tree, TreeItem, TreeState};
use gpui_component::{h_flex, v_flex, IconName};
use models::{Folder, FolderType, Schema, Service, Workspace};
use uuid::Uuid;

pub fn build_tree(workspace: Workspace) -> Vec<TreeItem> {
    vec![
        TreeItem::new("services", "Services")
            .expanded(true)
            .children(build_service_tree(
                None,
                0,
                &workspace.folders,
                &workspace.services,
            )),
        TreeItem::new("schemas", "Schemas")
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
            TreeItem::new(folder.id.to_string(), folder.name.clone()).children(build_service_tree(
                Some(folder.id),
                depth + 1,
                folders,
                services,
            )),
        );
    }

    // Add services
    for service in services.iter().filter(|s| s.folder_id == parent_id) {
        items.push(TreeItem::new(service.id.to_string(), service.name.clone()));
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
            TreeItem::new(folder.id.to_string(), folder.name.clone()).children(build_schema_tree(
                Some(folder.id),
                depth + 1,
                folders,
                schemas,
            )),
        );
    }

    // Add schemas
    for schema in schemas.iter().filter(|s| s.folder_id == parent_id) {
        items.push(TreeItem::new(schema.id.to_string(), schema.name.clone()));
    }

    items
}

pub struct WorkspaceSidebar {
    tree: Entity<TreeState>,
    selected_item: Option<TreeItem>,
}

impl WorkspaceSidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));
        Self {
            tree,
            selected_item: None,
        }
    }

    pub fn set_workspace(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        self.tree.update(cx, |this, cx| {
            this.set_items(build_tree(workspace), cx);
        });
    }

    fn handle_selection(&mut self, item: TreeItem, cx: &mut Context<Self>) {
        self.selected_item = Some(item.clone());
        println!("Selected: {} ({})", item.label, item.id);
        cx.notify();
    }
}

impl Render for WorkspaceSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .h_full()
            .w_48()
            .border_r_1()
            .border_color(rgb(0xe0e0e0))
            .child(
                div()
                    .p_2()
                    .border_1()
                    .rounded_md()
                    .text_sm()
                    .overflow_hidden()
                    .child("Search"),
            )
            .child(
                v_flex()
                    .justify_start()
                    .h_full()
                    .p_2()
                    .gap_2()
                    // Services Section
                    .child(tree(&self.tree, {
                        let view = cx.entity();
                        move |ix, entry, selected, window, cx| {
                            let item = entry.item();
                            let icon = if !entry.is_folder() {
                                IconName::File
                            } else if entry.is_expanded() {
                                IconName::FolderOpen
                            } else {
                                IconName::Folder
                            };
                            view.update(cx, |this, cx| {
                                ListItem::new(ix)
                                    .selected(selected)
                                    .pl(px(16.) * entry.depth() + px(12.)) // Indent based on depth
                                    .child(h_flex().gap_2().child(icon).child(item.label.clone()))
                                    .on_click(cx.listener({
                                        let item = entry.item().clone();
                                        move |this, _, _, cx| {
                                            this.handle_selection(item.clone(), cx);
                                        }
                                    }))
                            })
                        }
                    })),
            )
    }
}
