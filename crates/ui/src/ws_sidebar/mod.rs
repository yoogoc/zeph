mod popup_menu;
mod utils;

use crate::ws_sidebar::popup_menu::popup_menu;
use crate::ImportConfig;
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
use workspace::ConflictResolution;

pub struct WorkspaceSidebar {
    tree: Entity<TreeState>,
    hovered: Option<SharedString>,
    tree_map: HashMap<SharedString, usize>,
    workspace: Option<Workspace>,
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
            workspace: None,
            tree_map: HashMap::new(),
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
                    .id("workspace-sidebar-tree")
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
                    }))
                    // .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                    //     this.hovered = None;
                    //     cx.notify();
                    // })),
            )
    }
}
