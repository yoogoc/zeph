pub mod item;
mod popup_menu;
mod utils;

use crate::item::item;
use crate::state::ServiceState;
use gpui::*;
use gpui_component::tree::{tree, TreeEntry, TreeItem};
use gpui_component::v_flex;
use models::FolderType;
use std::collections::HashMap;
use std::rc::Rc;
pub use utils::{TreeId, SCHEMA_ID, SERVICE_ID};
use uuid::Uuid;

pub struct WorkspaceSidebar {
    service_state: Entity<ServiceState>,
    hovered: Option<SharedString>,
    tree_map: HashMap<SharedString, usize>,
    get_icon: Rc<dyn Fn(&TreeEntry) -> AnyElement>,
}

impl WorkspaceSidebar {
    pub fn new(
        service_state: Entity<ServiceState>,
        get_icon: impl Fn(&TreeEntry) -> AnyElement + 'static,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            service_state,
            hovered: None,
            tree_map: HashMap::new(),
            get_icon: Rc::new(get_icon),
        }
    }

    pub fn create_service(&self, name: SharedString, folder_id: Option<Uuid>, cx: &mut App) {
        self.service_state.update(cx, |s, cx| {
            s.create_service(name, folder_id, cx);
        });
    }

    pub fn create_schema(&self, name: SharedString, folder_id: Option<Uuid>, cx: &mut App) {
        self.service_state.update(cx, |s, cx| {
            s.create_schema(name, folder_id, cx);
        });
    }

    pub fn create_folder(
        &self,
        name: SharedString,
        parent_id: Option<Uuid>,
        folder_type: FolderType,
        cx: &mut App,
    ) {
        self.service_state.update(cx, |s, cx| {
            s.create_folder(name, parent_id, folder_type, cx);
        });
    }

    pub fn delete_service(&self, id: Uuid, cx: &mut App) {
        self.service_state.update(cx, |s, cx| {
            s.delete_service(id, cx);
        });
    }

    pub fn delete_schema(&self, id: Uuid, _cx: &mut App) {
        // TODO: storage/ServiceState lacks schema delete support yet.
        eprintln!("Delete schema requested: {}", id);
    }

    pub fn delete_folder(&self, id: Uuid, _cx: &mut App) {
        // TODO: storage/ServiceState lacks folder delete support yet.
        eprintln!("Delete folder requested: {}", id);
    }

    pub fn rename_service(&self, id: Uuid, new_name: SharedString, cx: &mut App) {
        self.service_state.update(cx, |s, cx| {
            s.rename_service(id, new_name, cx);
        });
    }

    pub fn rename_folder(&self, id: Uuid, new_name: SharedString, cx: &mut App) {
        self.service_state.update(cx, |s, cx| {
            s.rename_folder(id, new_name, cx);
        });
    }

    pub fn open_service(&self, id: Uuid, window: &mut Window, cx: &mut App) {
        self.service_state.update(cx, |s, cx| {
            s.open_service(id, window, cx);
        });
    }

    pub fn open_schema(&self, _id: Uuid, _window: &mut Window, _cx: &mut App) {
        // TODO: schema detail page not implemented yet.
    }

    pub fn open_folder_detail(&self, id: Uuid, window: &mut Window, cx: &mut App) {
        self.service_state.update(cx, |s, cx| {
            s.open_folder_detail(id, window, cx);
        });
    }

    pub fn set_ix(&mut self, item: TreeId, cx: &mut Context<Self>) {
        let tree = self.service_state.read(cx).tree().clone();
        tree.update(cx, |this, cx| {
            this.set_selected_item(Some(&TreeItem::new(item.to_string().clone(), "")), cx);
        });
    }

    fn handle_selection(
        &mut self,
        item: TreeItem,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tree_map.insert(item.id.clone(), ix);
        match TreeId::new(item.id) {
            TreeId::Service(id) => self.open_service(id, window, cx),
            TreeId::Schema(id) => self.open_schema(id, window, cx),
            TreeId::ServiceFolder(id) | TreeId::SchemaFolder(id) => {
                self.open_folder_detail(id, window, cx)
            }
            _ => {}
        }
        cx.notify();
    }

    // pub fn show_rename_service_dialog(
    //     &mut self,
    //     service_id: Uuid,
    //     window: &mut Window,
    //     cx: &mut Context<Self>,
    // ) {
    //     let ss = self.service_state.read(cx);
    //     let Some(service) = ss.services.iter().find(|s| s.id == service_id) else {
    //         return;
    //     };
    //     let current_name = service.name.clone();

    //
    //     let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));
    //     input.update(cx, |state, cx| {
    //         state.set_value(current_name.to_string(), window, cx);
    //     });
    //
    //     let this = cx.entity().downgrade();
    //
    //     window.open_dialog(cx, move |dialog, _window, _cx| {
    //         let input_clone = input.clone();
    //         let this_clone = this.clone();
    //
    //         dialog
    //             .title("重命名 Service")
    //             .w(px(450.0))
    //             .child(
    //                 div()
    //                     .v_flex()
    //                     .gap_2()
    //                     .child(div().text_sm().font_semibold().child("名称"))
    //                     .child(Input::new(&input).w_full()),
    //             )
    //             .on_ok(move |_, window, cx| {
    //                 let new_name = input_clone.read(cx).value();
    //                 if new_name.trim().is_empty() || new_name.len() > 255 {
    //                     return false;
    //                 }
    //                 if let Some(this) = this_clone.upgrade() {
    //                     this.update(cx, |this, cx| {
    //                         this.rename_service(service_id, new_name, cx);
    //                     });
    //                 }
    //                 window.close_dialog(cx);
    //                 true
    //             })
    //     });
    // }

    // pub fn show_rename_folder_dialog(
    //     &mut self,
    //     folder_id: Uuid,
    //     window: &mut Window,
    //     cx: &mut Context<Self>,
    // ) {
    //     let ss = self.service_state.read(cx);
    //     let Some(folder) = ss.folders.iter().find(|f| f.id == folder_id) else {
    //         return;
    //     };
    //     let current_name = folder.name.clone();
    //
    //     let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));
    //     input.update(cx, |state, cx| {
    //         state.set_value(current_name.to_string(), window, cx);
    //     });
    //
    //     let this = cx.entity().downgrade();
    //
    //     window.open_dialog(cx, move |dialog, _window, _cx| {
    //         let input_clone = input.clone();
    //         let this_clone = this.clone();
    //
    //         dialog
    //             .title("重命名 Folder")
    //             .w(px(450.0))
    //             .child(
    //                 div()
    //                     .v_flex()
    //                     .gap_2()
    //                     .child(div().text_sm().font_semibold().child("名称"))
    //                     .child(Input::new(&input).w_full()),
    //             )
    //             .on_ok(move |_, window, cx| {
    //                 let new_name = input_clone.read(cx).value();
    //                 if new_name.trim().is_empty() || new_name.len() > 255 {
    //                     return false;
    //                 }
    //                 if let Some(this) = this_clone.upgrade() {
    //                     this.update(cx, |this, cx| {
    //                         this.rename_folder(folder_id, new_name, cx);
    //                     });
    //                 }
    //                 window.close_dialog(cx);
    //                 true
    //             })
    //     });
    // }

    // pub fn show_import_dialog(
    //     &mut self,
    //     folder_id: Option<Uuid>,
    //     window: &mut Window,
    //     cx: &mut Context<Self>,
    // ) {
    //     let folder_name = folder_id.and_then(|id| {
    //         self.service_state
    //             .read(cx)
    //             .get_folder(id)
    //             .map(|f| SharedString::from(f.name.clone()))
    //     });
    //
    //     let dialog_entity =
    //         cx.new(|cx| SimpleImportDialog::new(folder_id, folder_name, window, cx));
    //
    //     window.open_dialog(cx, move |dialog, _window, _cx| {
    //         dialog
    //             .title(shared_t!("openapi.import.title"))
    //             .w(px(600.0))
    //             .child(dialog_entity.clone())
    //     });
    // }
}

impl Render for WorkspaceSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let get_icon = self.get_icon.clone();
        let hovered = self.hovered.clone();
        let service_state = self.service_state.clone();
        let tree_state = service_state.read(cx).tree().clone();
        let entity = cx.entity();
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
                            .child(tree(&tree_state, {
                                move |ix, entry, selected, _window, _cx| {
                                    let entry_item = entry.item().clone();
                                    let enter_entity = entity.clone();
                                    let enter_id = entry_item.id.clone();
                                    let click_entity = entity.clone();
                                    let click_item = entry_item.clone();
                                    item(
                                        entry,
                                        ix,
                                        selected,
                                        get_icon(entry),
                                        hovered.clone(),
                                        service_state.clone(),
                                    )
                                    .on_mouse_enter(move |_, _, cx| {
                                        enter_entity.update(cx, |this, cx| {
                                            this.hovered = Some(enter_id.clone());
                                            cx.notify();
                                        });
                                    })
                                    .on_click(
                                        move |_, window, cx| {
                                            click_entity.update(cx, |this, cx| {
                                                this.handle_selection(
                                                    click_item.clone(),
                                                    ix,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        },
                                    )
                                }
                            })),
                    ),
            )
    }
}
