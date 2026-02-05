mod popup_menu;
mod utils;

use crate::ws_sidebar::popup_menu::popup_menu;
use crate::SimpleImportDialog;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::list::ListItem;
use gpui_component::menu::DropdownMenu;
use gpui_component::tree::{tree, TreeEntry, TreeItem, TreeState};
use gpui_component::{h_flex, v_flex, IconName, Sizable, StyledExt, WindowExt};
use models::{ContentType, FolderType};
use std::collections::HashMap;
use utils::build_tree;
use utils::cal_icon;
pub use utils::TreeId;
use uuid::Uuid;
use workspace::WorkspaceRegistry;

pub struct WorkspaceSidebar {
    tree: Entity<TreeState>,
    hovered: Option<SharedString>,
    tree_map: HashMap<SharedString, usize>,
    _subscription: Subscription,
}

impl WorkspaceSidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let tree = cx.new(|cx| TreeState::new(cx));

        // Subscribe to WorkspaceRegistry to auto-refresh when workspace changes
        let registry = WorkspaceRegistry::global(cx);
        let subscription = cx.subscribe(
            &registry,
            |this, _registry, event: &workspace::WorkspaceEvent, cx| match event {
                workspace::WorkspaceEvent::Open(_) | workspace::WorkspaceEvent::Reload => {
                    this.refresh_tree(cx);
                }
                _ => {}
            },
        );

        Self {
            tree,
            hovered: None,
            tree_map: HashMap::new(),
            _subscription: subscription,
        }
    }

    pub fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        self.tree_map.clear();
        let registry = WorkspaceRegistry::global(cx);
        let workspace = registry.read(cx).get_active_workspace().cloned();
        if let Some(workspace) = workspace {
            self.tree.update(cx, |this, cx| {
                this.set_items(build_tree(workspace), cx);
            });
        }
    }

    // Direct operations on WorkspaceRegistry
    pub fn create_service(&self, name: SharedString, folder_id: Option<Uuid>, cx: &mut App) {
        let registry = WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.create_service(name, folder_id, cx));
    }

    pub fn create_schema(&self, name: SharedString, folder_id: Option<Uuid>, cx: &mut App) {
        let registry = WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.create_schema(name, folder_id, cx));
    }

    pub fn create_folder(
        &self,
        name: SharedString,
        parent_id: Option<Uuid>,
        folder_type: FolderType,
        cx: &mut App,
    ) {
        let registry = WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| {
            r.create_folder(name, parent_id, folder_type, cx)
        });
    }

    pub fn delete_service(&self, id: Uuid, _cx: &mut App) {
        // TODO: Implement when WorkspaceRegistry.delete_service is available
        eprintln!("Delete service requested: {}", id);
    }

    pub fn delete_schema(&self, id: Uuid, _cx: &mut App) {
        // TODO: Implement when WorkspaceRegistry.delete_schema is available
        eprintln!("Delete schema requested: {}", id);
    }

    pub fn delete_folder(&self, id: Uuid, _cx: &mut App) {
        // TODO: Implement when WorkspaceRegistry.delete_folder is available
        eprintln!("Delete folder requested: {}", id);
    }

    pub fn rename_service(&self, id: Uuid, new_name: SharedString, cx: &mut App) {
        let registry = WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.rename_service(id, new_name, cx));
    }

    pub fn rename_folder(&self, id: Uuid, new_name: SharedString, cx: &mut App) {
        let registry = WorkspaceRegistry::global(cx);
        registry.update(cx, |r, cx| r.rename_folder(id, new_name, cx));
    }

    /// Open a service by ID
    pub fn open_service(&self, id: Uuid, cx: &mut App) {
        let registry = WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            if let Some(service) = registry.get_service(id) {
                registry.open_content(ContentType::Service(service.clone()), cx);
            }
        });
    }

    /// Open a schema by ID
    pub fn open_schema(&self, _id: Uuid, _cx: &mut App) {
        // TODO: Implement when WorkspaceRegistry.get_schema is available
    }

    /// Open a folder detail page by ID
    pub fn open_folder_detail(&self, id: Uuid, cx: &mut App) {
        let registry = WorkspaceRegistry::global(cx);
        registry.update(cx, |registry, cx| {
            if let Some(folder) = registry.get_folder(id) {
                if folder.folder_type == FolderType::Service {
                    registry.open_content(ContentType::ServiceFolder(folder.clone()), cx);
                } else {
                    registry.open_content(ContentType::SchemaFolder(folder.clone()), cx);
                }
            }
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
                self.open_service(id, cx);
            }
            TreeId::Schema(id) => {
                self.open_schema(id, cx);
            }
            TreeId::ServiceFolder(id) | TreeId::SchemaFolder(id) => {
                self.open_folder_detail(id, cx);
            }
            _ => {}
        }

        cx.notify();
    }

    /// Show the rename dialog for a Service
    /// Implements Requirements 1.1, 1.2
    pub fn show_rename_service_dialog(
        &mut self,
        service_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let registry = WorkspaceRegistry::global(cx);
        let Some(workspace) = registry.read(cx).get_active_workspace() else {
            return;
        };
        let Some(service) = workspace.services.iter().find(|s| s.id == service_id) else {
            return;
        };
        let current_name = service.name.clone();

        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));
        input.update(cx, |state, cx| {
            state.set_value(current_name.to_string(), window, cx);
        });

        let this = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();

            dialog
                .title("重命名 Service")
                .w(px(450.0))
                .confirm()
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_sm().font_semibold().child("名称"))
                        .child(Input::new(&input).w_full()),
                )
                .on_ok(move |_, window, cx| {
                    let new_name = input_clone.read(cx).value();
                    if new_name.trim().is_empty() || new_name.len() > 255 {
                        return false; // 验证失败，不关闭
                    }
                    if let Some(this) = this_clone.upgrade() {
                        this.update(cx, |this, cx| {
                            this.rename_service(service_id, new_name, cx);
                        });
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    /// Show the rename dialog for a Folder
    /// Implements Requirements 2.1, 2.2
    pub fn show_rename_folder_dialog(
        &mut self,
        folder_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let registry = WorkspaceRegistry::global(cx);
        let Some(workspace) = registry.read(cx).get_active_workspace() else {
            return;
        };
        let Some(folder) = workspace.folders.iter().find(|f| f.id == folder_id) else {
            return;
        };
        let current_name = folder.name.clone();

        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));
        input.update(cx, |state, cx| {
            state.set_value(current_name.to_string(), window, cx);
        });

        let this = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();

            dialog
                .title("重命名 Folder")
                .w(px(450.0))
                .confirm()
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .child(div().text_sm().font_semibold().child("名称"))
                        .child(Input::new(&input).w_full()),
                )
                .on_ok(move |_, window, cx| {
                    let new_name = input_clone.read(cx).value();
                    if new_name.trim().is_empty() || new_name.len() > 255 {
                        return false; // 验证失败，不关闭
                    }
                    if let Some(this) = this_clone.upgrade() {
                        this.update(cx, |this, cx| {
                            this.rename_folder(folder_id, new_name, cx);
                        });
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    /// Show the import dialog
    pub fn show_import_dialog(
        &mut self,
        folder_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let folder_name = folder_id.and_then(|id| {
            let registry = WorkspaceRegistry::global(cx);
            registry
                .read(cx)
                .get_folder(id)
                .map(|f| SharedString::from(f.name.clone()))
        });

        let dialog_entity =
            cx.new(|cx| SimpleImportDialog::new(folder_id, folder_name, window, cx));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(shared_t!("openapi.import.title"))
                .w(px(600.0))
                .child(dialog_entity.clone())
        });
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
        let registry = WorkspaceRegistry::global(cx);
        let workspace = registry.read(cx).get_active_workspace().cloned();
        let icon = cal_icon(entry, &workspace);
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
                            })),
                    ),
            )
    }
}
