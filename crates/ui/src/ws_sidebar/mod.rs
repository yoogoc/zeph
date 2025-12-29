mod utils;

use std::collections::HashMap;
use crate::ws_sidebar::utils::{cal_icon, TreeId};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::list::ListItem;
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::tree::{tree, TreeEntry, TreeItem, TreeState};
use gpui_component::{h_flex, v_flex, IconName, Sizable};
use uuid::Uuid;
use models::{FolderType, Workspace};
use utils::build_tree;
use workspace::ConflictResolution;
use crate::ImportConfig;

pub struct WorkspaceSidebar {
    tree: Entity<TreeState>,
    hovered: Option<SharedString>,
}

#[derive(Debug, Clone)]
pub enum WorkspaceSidebarEvent {
    OpenService(Uuid),
    OpenSchema(Uuid),
    OpenFolderDetail(Uuid), // New event for opening folder details
    CreateService(SharedString, Option<Uuid>),
    CreateSchema(SharedString, Option<Uuid>),
    CreateFolder(SharedString, Option<Uuid>, FolderType),
    // ToggleFolder(Uuid),
    ImportOpenApi(ImportConfig),
    PreviewImport(ImportConfig),
    RequestImportPreview(ImportConfig),
    ResolveImportConflicts(ImportConfig, HashMap<SharedString, ConflictResolution>),
    // New menu events
    DeleteService(Uuid),
    DeleteSchema(Uuid),
    DeleteFolder(Uuid),
}

impl EventEmitter<WorkspaceSidebarEvent> for WorkspaceSidebar {}

impl WorkspaceSidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));
        Self {
            tree,
            hovered: None,
        }
    }

    pub fn set_workspace(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        self.tree.update(cx, |this, cx| {
            this.set_items(build_tree(workspace), cx);
        });
    }

    fn handle_selection(&mut self, item: TreeItem, cx: &mut Context<Self>) {
        let selected_item = item.clone();
        if !item.is_folder() {
            let id = TreeId::new(selected_item.id);
            match id {
                TreeId::Service(id) => {
                    cx.emit(WorkspaceSidebarEvent::OpenService(id));
                }
                TreeId::Schema(_id) => {}
                TreeId::ServiceRoot => {}
                TreeId::SchemaRoot => {}
            }
        }

        cx.notify();
    }
    pub fn handle_context_menu_event(
        &mut self,
        event: WorkspaceSidebarEvent,
        cx: &mut Context<Self>,
    ) {
        // Enhanced validation with detailed error handling
        let validation_result = match &event {
            WorkspaceSidebarEvent::DeleteService(_service_id) => Ok(()),
            WorkspaceSidebarEvent::DeleteSchema(_schema_id) => Ok(()),
            WorkspaceSidebarEvent::DeleteFolder(_folder_id) => Ok(()),
            WorkspaceSidebarEvent::CreateService(_name, _parent_id) => {
                // if name.trim().is_empty() {
                //     Err("Service name cannot be empty".to_string())
                // } else if let Some(parent_id) = parent_id {
                //     Ok(())
                // } else {
                //     Ok(()) // No parent is valid
                // }
                Ok(())
            }
            WorkspaceSidebarEvent::CreateSchema(_name, _parent_id) => {
                Ok(())
                // if name.trim().is_empty() {
                //     Err("Schema name cannot be empty".to_string())
                // } else if let Some(parent_id) = parent_id {
                //     Ok(())
                // } else {
                //     Ok(()) // No parent is valid
                // }
            }
            WorkspaceSidebarEvent::CreateFolder(name, parent_id, _folder_type) => {
                if name.trim().is_empty() {
                    Err("Folder name cannot be empty".to_string())
                } else if let Some(_parent_id) = parent_id {
                    // self.validate_project_id(*parent_id)
                    Ok(())
                } else {
                    Ok(()) // No parent is valid
                }
            }
            _ => Ok(()), // Other events are always valid
        };

        match validation_result {
            Ok(_) => {
                cx.emit(event);
            }
            Err(error) => {
                eprintln!(
                    "Warning: Invalid menu operation attempted: {:?}. Error: {}",
                    event, error
                );
                // Optionally, you could emit an error event or show a user notification here
            }
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
        let icon = cal_icon(entry);
        let item_id = item.id.clone();
        ListItem::new(ix)
            .selected(selected)
            .pl(px(16.) * entry.depth() + px(12.)) // Indent based on depth
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
                                    .on_mouse_down(MouseButton::Left, cx.listener({
                                        move |_this, _, _window, cx| {
                                            cx.stop_propagation();
                                        }
                                    }))
                                    .dropdown_menu({
                                        let item = entry.item().clone();
                                        let view = cx.entity();
                                        move |menu, window, _cx| {
                                            let mut menu = menu;
                                            let id = TreeId::new(item.id.clone());

                                            let mut detail = None;
                                            let mut create_service = None;
                                            let mut create_schema = None;
                                            let mut create_folder = None;
                                            let mut delete = None;
                                            if item.is_folder() {
                                                match id {
                                                    TreeId::ServiceRoot => {
                                                        create_service = Some(
                                                            WorkspaceSidebarEvent::CreateService(
                                                                "New Service".into(),
                                                                None,
                                                            ),
                                                        );
                                                        create_folder = Some(
                                                            WorkspaceSidebarEvent::CreateFolder(
                                                                "New Folder".into(),
                                                                None,
                                                                FolderType::Service,
                                                            ),
                                                        );
                                                    }
                                                    TreeId::SchemaRoot => {
                                                        create_schema = Some(
                                                            WorkspaceSidebarEvent::CreateSchema(
                                                                "New Schema".into(),
                                                                None,
                                                            ),
                                                        );
                                                        create_folder = Some(
                                                            WorkspaceSidebarEvent::CreateFolder(
                                                                "New Folder".into(),
                                                                None,
                                                                FolderType::Schema,
                                                            ),
                                                        );
                                                    }
                                                    TreeId::Service(id) => {
                                                        detail = Some(
                                                            WorkspaceSidebarEvent::OpenFolderDetail(
                                                                id.clone(),
                                                            ),
                                                        );
                                                        create_service = Some(
                                                            WorkspaceSidebarEvent::CreateService(
                                                                "New Service".into(),
                                                                Some(id),
                                                            ),
                                                        );
                                                        create_folder = Some(
                                                            WorkspaceSidebarEvent::CreateFolder(
                                                                "New Folder".into(),
                                                                Some(id),
                                                                FolderType::Service,
                                                            ),
                                                        );
                                                        delete = Some(
                                                            WorkspaceSidebarEvent::DeleteFolder(id),
                                                        );
                                                    }
                                                    TreeId::Schema(id) => {
                                                        detail = Some(
                                                            WorkspaceSidebarEvent::OpenFolderDetail(
                                                                id,
                                                            ),
                                                        );
                                                        create_schema = Some(
                                                            WorkspaceSidebarEvent::CreateSchema(
                                                                "New Schema".into(),
                                                                Some(id),
                                                            ),
                                                        );
                                                        create_folder = Some(
                                                            WorkspaceSidebarEvent::CreateFolder(
                                                                "New Folder".into(),
                                                                Some(id),
                                                                FolderType::Schema,
                                                            ),
                                                        );
                                                        delete = Some(
                                                            WorkspaceSidebarEvent::DeleteFolder(id),
                                                        );
                                                    }
                                                }
                                            } else {
                                                match id {
                                                    TreeId::ServiceRoot => {}
                                                    TreeId::SchemaRoot => {}
                                                    TreeId::Service(id) => {
                                                        detail = Some(
                                                            WorkspaceSidebarEvent::OpenService(id),
                                                        );
                                                        delete = Some(
                                                            WorkspaceSidebarEvent::DeleteService(
                                                                id,
                                                            ),
                                                        );
                                                    }
                                                    TreeId::Schema(id) => {
                                                        detail = Some(
                                                            WorkspaceSidebarEvent::OpenSchema(id),
                                                        );
                                                        delete = Some(
                                                            WorkspaceSidebarEvent::DeleteSchema(id),
                                                        );
                                                    }
                                                }
                                            }

                                            if let Some(event) = detail {
                                                menu = menu.item(
                                                    PopupMenuItem::new("View Details").on_click(
                                                        window.listener_for(
                                                            &view,
                                                            move |this, _, _window, cx| {
                                                                this.handle_context_menu_event(
                                                                    event.clone(),
                                                                    cx,
                                                                );

                                                                cx.stop_propagation();
                                                            },
                                                        ),
                                                    ),
                                                );
                                            }
                                            if let Some(event) = create_service {
                                                menu = menu.item(
                                                    PopupMenuItem::new("New Service").on_click(
                                                        window.listener_for(
                                                            &view,
                                                            move |this, _, _window, cx| {
                                                                this.handle_context_menu_event(
                                                                    event.clone(),
                                                                    cx,
                                                                );

                                                                cx.stop_propagation();
                                                            },
                                                        ),
                                                    ),
                                                )
                                            }

                                            if let Some(event) = create_schema {
                                                menu = menu.item(
                                                    PopupMenuItem::new("New Schema").on_click(
                                                        window.listener_for(
                                                            &view,
                                                            move |this, _, _window, cx| {
                                                                this.handle_context_menu_event(
                                                                    event.clone(),
                                                                    cx,
                                                                );

                                                                cx.stop_propagation();
                                                            },
                                                        ),
                                                    ),
                                                )
                                            }

                                            if let Some(event) = create_folder {
                                                menu = menu.item(
                                                    PopupMenuItem::new("New Folder").on_click(
                                                        window.listener_for(
                                                            &view,
                                                            move |this, _, _window, cx| {
                                                                this.handle_context_menu_event(
                                                                    event.clone(),
                                                                    cx,
                                                                );

                                                                cx.stop_propagation();
                                                            },
                                                        ),
                                                    ),
                                                )
                                            }

                                            if let Some(event) = delete {
                                                menu = menu.item(
                                                    PopupMenuItem::new("Delete").on_click(
                                                        window.listener_for(
                                                            &view,
                                                            move |this, _, _window, cx| {
                                                                this.handle_context_menu_event(
                                                                    event.clone(),
                                                                    cx,
                                                                );

                                                                cx.stop_propagation();
                                                            },
                                                        ),
                                                    ),
                                                )
                                            }

                                            menu
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
                    this.handle_selection(item.clone(), cx);
                }
            }))
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
                        move |ix, entry, selected, _window, cx| {
                            view.update(cx, |this, cx| this.render_item(entry, ix, selected, cx))
                        }
                    })),
            )
    }
}
