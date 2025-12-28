mod service;

use crate::{ImportConfig, MenuType};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::list::ListItem;
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::tree::{tree, TreeItem, TreeState};
use gpui_component::{h_flex, v_flex, Icon, IconName, StyledExt};
use log::info;
use models::{FolderType, Workspace};
use service::ServiceHeader;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use workspace::ConflictResolution;

#[derive(Debug)]
pub enum WorkspaceSidebarEvent {
    OpenService(Uuid),
    OpenSchema(Uuid),
    OpenFolderDetail(Uuid), // New event for opening folder details
    CreateService(String, Option<Uuid>),
    CreateSchema(String, Option<Uuid>),
    CreateFolder(String, Option<Uuid>, FolderType),
    ToggleFolder(Uuid),
    ImportOpenApi(ImportConfig),
    PreviewImport(ImportConfig),
    RequestImportPreview(ImportConfig),
    ResolveImportConflicts(ImportConfig, HashMap<String, ConflictResolution>),
    // New menu events
    DeleteService(Uuid),
    DeleteSchema(Uuid),
    DeleteFolder(Uuid),
    ShowContextMenu(Uuid, MenuType),
}

pub struct WorkspaceSidebar {
    workspace: Workspace,
    pub expanded_folders: HashSet<Uuid>,
    expanded_sections: (bool, bool), // (services, schemas)
    tree: Entity<TreeState>,
    header: Entity<ServiceHeader>,
}

impl WorkspaceSidebar {
    pub fn new(workspace: Workspace, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| {
            TreeState::new(cx).items(vec![
                TreeItem::new("services", "Services")
                    .expanded(true)
                    .child(TreeItem::new("src/lib.rs", "lib.rs"))
                    .child(TreeItem::new("src/main.rs", "main.rs")),
                TreeItem::new("schemas", "Schemas")
                    .expanded(true)
                    .child(TreeItem::new("src/lib.rs", "lib.rs"))
                    .child(TreeItem::new("src/main.rs", "main.rs")),
            ])
        });

        let header = cx.new(|cx| ServiceHeader::new());
        Self {
            workspace,
            expanded_folders: HashSet::new(),
            expanded_sections: (true, true),
            tree,
            header,
        }
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
                    .child(tree(&self.tree, |ix, entry, selected, window, cx| {
                        // entry.is_folder()
                        let item = entry.item();
                        let icon = if !entry.is_folder() {
                            IconName::File
                        } else if entry.is_expanded() {
                            IconName::FolderOpen
                        } else {
                            IconName::Folder
                        };
                        ListItem::new(ix)
                            .selected(selected)
                            .pl(px(16.) * entry.depth() + px(12.)) // Indent based on depth
                            .child(h_flex().gap_2().child(icon).child(item.label.clone()))
                    })),
            )
    }
}
