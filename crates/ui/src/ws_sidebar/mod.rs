mod popup_menu;
mod utils;

use crate::rename_dialog::{EntityType, RenameDialog, RenameDialogEvent};
use crate::ws_sidebar::popup_menu::popup_menu;
use crate::{ImportConfig, SimpleImportConfig};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::list::ListItem;
use gpui_component::menu::DropdownMenu;
use gpui_component::tree::{tree, TreeEntry, TreeItem, TreeState};
use gpui_component::{h_flex, v_flex, IconName, Sizable};
use models::{FolderType, Workspace};
use std::collections::HashMap;
use utils::build_tree;
use utils::cal_icon;
pub use utils::TreeId;
use uuid::Uuid;

pub struct WorkspaceSidebar {
    tree: Entity<TreeState>,
    hovered: Option<SharedString>,
    tree_map: HashMap<SharedString, usize>,
    workspace: Option<Workspace>,
    rename_dialog: Option<Entity<RenameDialog>>,
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum WorkspaceSidebarEvent {
    OpenService(Uuid),
    OpenSchema(Uuid),
    OpenFolderDetail(Uuid), // New event for opening folder details
    CreateService(SharedString, Option<Uuid>),
    CreateSchema(SharedString, Option<Uuid>),
    CreateFolder(SharedString, Option<Uuid>, FolderType),
    // ToggleFolder(Uuid),
    ImportOpenApi(ImportConfig),
    // New menu events
    DeleteService(Uuid),
    DeleteSchema(Uuid),
    DeleteFolder(Uuid),
    // Import dialog events
    ShowImportDialog(Option<Uuid>), // Show import dialog with optional target folder ID
    ExecuteImport(SimpleImportConfig), // Execute import with the given configuration
    // Rename events - Requirements 1.1, 1.2, 2.1, 2.2
    // These events are emitted when the user initiates a rename from the context menu
    RenameService(Uuid),
    RenameFolder(Uuid),
    // Confirm rename events - emitted when the user confirms the new name in the dialog
    ConfirmRenameService(Uuid, SharedString),
    ConfirmRenameFolder(Uuid, SharedString),
}

impl EventEmitter<WorkspaceSidebarEvent> for WorkspaceSidebar {}

impl WorkspaceSidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));
        Self {
            tree,
            hovered: None,
            workspace: None,
            tree_map: HashMap::new(),
            rename_dialog: None,
        }
    }

    pub fn set_workspace(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        self.tree_map.clear();
        self.workspace = Some(workspace.clone());
        self.tree.update(cx, |this, cx| {
            this.set_items(build_tree(workspace), cx);
        });
    }

    pub fn set_ix(&mut self, item: TreeId, cx: &mut Context<Self>) {
        self.tree.update(cx, |this, cx| {
            this.set_selected_item(Some(&TreeItem::new(item.to_string().clone(), "")), cx);
        });
    }

    fn handle_selection(&mut self, item: TreeItem, ix: usize, cx: &mut Context<Self>) {
        let selected_item = item.clone();
        self.tree_map.insert(item.id.clone(), ix);
        match TreeId::new(selected_item.id) {
            TreeId::Service(id) => {
                cx.emit(WorkspaceSidebarEvent::OpenService(id));
            }
            TreeId::Schema(id) => {
                cx.emit(WorkspaceSidebarEvent::OpenSchema(id));
            }
            TreeId::ServiceFolder(id) | TreeId::SchemaFolder(id) => {
                cx.emit(WorkspaceSidebarEvent::OpenFolderDetail(id));
            }
            _ => {}
        }

        cx.notify();
    }
    pub fn handle_context_menu_event(
        &mut self,
        event: WorkspaceSidebarEvent,
        cx: &mut Context<Self>,
    ) {
        cx.emit(event);
    }

    /// Show the rename dialog for a Service
    /// Implements Requirements 1.1, 1.2
    pub fn show_rename_service_dialog(
        &mut self,
        service_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Find the service in the workspace to get its current name
        if let Some(workspace) = &self.workspace {
            if let Some(service) = workspace.services.iter().find(|s| s.id == service_id) {
                let current_name = service.name.clone();

                // Create the rename dialog
                let dialog =
                    cx.new(|cx| RenameDialog::new(current_name, EntityType::Service, window, cx));

                // Subscribe to dialog events
                cx.subscribe_in(&dialog, window, move |this, _dialog, event, _window, cx| {
                    match event {
                        RenameDialogEvent::Confirm(new_name) => {
                            // Close the dialog
                            this.rename_dialog = None;
                            // Emit event to update the service name in the database
                            // This will be handled by the parent component (WorkspaceArea)
                            cx.emit(WorkspaceSidebarEvent::ConfirmRenameService(
                                service_id,
                                new_name.clone(),
                            ));
                            cx.notify();
                        }
                        RenameDialogEvent::Cancel => {
                            // Just close the dialog
                            this.rename_dialog = None;
                            cx.notify();
                        }
                    }
                })
                .detach();

                self.rename_dialog = Some(dialog);
                cx.notify();
            }
        }
    }

    /// Show the rename dialog for a Folder
    /// Implements Requirements 2.1, 2.2
    pub fn show_rename_folder_dialog(
        &mut self,
        folder_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Find the folder in the workspace to get its current name
        if let Some(workspace) = &self.workspace {
            if let Some(folder) = workspace.folders.iter().find(|f| f.id == folder_id) {
                let current_name = folder.name.clone();

                // Create the rename dialog
                let dialog =
                    cx.new(|cx| RenameDialog::new(current_name, EntityType::Folder, window, cx));

                // Subscribe to dialog events
                cx.subscribe_in(&dialog, window, move |this, _dialog, event, _window, cx| {
                    match event {
                        RenameDialogEvent::Confirm(new_name) => {
                            // Close the dialog
                            this.rename_dialog = None;
                            // Emit event to update the folder name in the database
                            // This will be handled by the parent component (WorkspaceArea)
                            cx.emit(WorkspaceSidebarEvent::ConfirmRenameFolder(
                                folder_id,
                                new_name.clone(),
                            ));
                            cx.notify();
                        }
                        RenameDialogEvent::Cancel => {
                            // Just close the dialog
                            this.rename_dialog = None;
                            cx.notify();
                        }
                    }
                })
                .detach();

                self.rename_dialog = Some(dialog);
                cx.notify();
            }
        }
    }

    /// Handle rename events by showing the appropriate dialog
    /// Implements Requirements 1.1, 1.2, 2.1, 2.2
    pub fn handle_rename_event(
        &mut self,
        event: &WorkspaceSidebarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            WorkspaceSidebarEvent::RenameService(service_id) => {
                self.show_rename_service_dialog(*service_id, window, cx);
            }
            WorkspaceSidebarEvent::RenameFolder(folder_id) => {
                self.show_rename_folder_dialog(*folder_id, window, cx);
            }
            _ => {}
        }
    }

    // 渲染树节点
    fn render_item(
        &mut self,
        entry: &TreeEntry,
        ix: usize,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> ListItem {
        let item = entry.item();
        let icon = cal_icon(entry, &self.workspace);
        let item_id = item.id.clone();
        ListItem::new(ix)
            .selected(selected)
            .pl(px(10.) * entry.depth() + px(12.)) // Indent based on depth
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(icon)
                            .child(item.label.clone())
                            .overflow_hidden(),
                    )
                    .when_some(self.hovered.clone(), |this, id| {
                        this.when(id == item_id, |this| {
                            this.child(
                                Button::new(format!("action_{}", id))
                                    .icon(IconName::Menu)
                                    .small()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener({
                                            move |_this, _, _window, cx| {
                                                cx.stop_propagation();
                                            }
                                        }),
                                    )
                                    .dropdown_menu({
                                        let item = entry.item().clone();
                                        let view = cx.entity();
                                        move |menu, window, cx| {
                                            popup_menu(menu, &item, &view, window, cx)
                                        }
                                    }),
                            )
                        })
                    }),
            )
            .on_mouse_enter(cx.listener(move |this, _, _, cx| {
                this.hovered = Some(item_id.clone());
                cx.notify();
            }))
            .on_click(cx.listener({
                let item = entry.item().clone();
                move |this, _, _, cx| {
                    this.handle_selection(item.clone(), ix, cx);
                }
            }))
    }
}

impl Render for WorkspaceSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .pt_1()
            .pb_2()
            .pr_1()
            .bg(rgb(0xf5f5f5))
            .child(
                v_flex()
                    .bg(white())
                    .size_full()
                    .rounded_2xl()
                    .child(
                        div()
                            .p_2()
                            .border_1()
                            .rounded_md()
                            .text_sm()
                            .overflow_hidden()
                            .child(shared_t!("common.search")),
                    )
                    .child(
                        v_flex()
                            .id("workspace-sidebar-tree")
                            .justify_start()
                            .h_full()
                            .p_2()
                            .gap_2()
                            // Services Section
                            .child(tree(&self.tree, {
                                let view = cx.entity();
                                move |ix, entry, selected, _window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.render_item(entry, ix, selected, cx)
                                    })
                                }
                            })), // .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                                 //     this.hovered = None;
                                 //     cx.notify();
                                 // })),
                    ),
            )
            // Render rename dialog as an overlay if it exists
            .when_some(self.rename_dialog.clone(), |el, dialog| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgba(0x00000080)) // Semi-transparent black overlay
                        .child(dialog),
                )
            })
    }
}
