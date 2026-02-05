use crate::ws_sidebar::{TreeId, WorkspaceSidebar, WorkspaceSidebarEvent};
use gpui::*;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::tree::TreeItem;
use models::FolderType;

pub fn popup_menu(
    menu: PopupMenu,
    item: &TreeItem,
    view: &Entity<WorkspaceSidebar>,
    window: &mut Window,
    _cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let mut menu = menu;
    let id = TreeId::new(item.id.clone());

    // Event-based actions (need parent coordination)
    let mut detail_event: Option<WorkspaceSidebarEvent> = None;
    let mut import_event: Option<WorkspaceSidebarEvent> = None;

    // Direct action parameters
    let mut create_service_params: Option<(SharedString, Option<uuid::Uuid>)> = None;
    let mut create_schema_params: Option<(SharedString, Option<uuid::Uuid>)> = None;
    let mut create_folder_params: Option<(SharedString, Option<uuid::Uuid>, FolderType)> = None;
    let mut rename_service_id: Option<uuid::Uuid> = None;
    let mut rename_folder_id: Option<uuid::Uuid> = None;
    let mut delete_service_id: Option<uuid::Uuid> = None;
    let mut delete_schema_id: Option<uuid::Uuid> = None;
    let mut delete_folder_id: Option<uuid::Uuid> = None;

    match id {
        TreeId::ServiceRoot => {
            create_service_params = Some(("New Service".into(), None));
            create_folder_params = Some(("New Folder".into(), None, FolderType::Service));
            import_event = Some(WorkspaceSidebarEvent::ShowImportDialog(None));
        }
        TreeId::SchemaRoot => {
            create_schema_params = Some(("New Schema".into(), None));
            create_folder_params = Some(("New Folder".into(), None, FolderType::Schema));
        }
        TreeId::Service(id) => {
            detail_event = Some(WorkspaceSidebarEvent::OpenService(id));
            rename_service_id = Some(id);
            delete_service_id = Some(id);
        }
        TreeId::Schema(id) => {
            detail_event = Some(WorkspaceSidebarEvent::OpenSchema(id));
            delete_schema_id = Some(id);
        }
        TreeId::ServiceFolder(id) => {
            detail_event = Some(WorkspaceSidebarEvent::OpenFolderDetail(id));
            create_service_params = Some(("New Service".into(), Some(id)));
            create_folder_params = Some(("New Folder".into(), Some(id), FolderType::Service));
            import_event = Some(WorkspaceSidebarEvent::ShowImportDialog(Some(id)));
            rename_folder_id = Some(id);
            delete_folder_id = Some(id);
        }
        TreeId::SchemaFolder(id) => {
            detail_event = Some(WorkspaceSidebarEvent::OpenFolderDetail(id));
            create_schema_params = Some(("New Schema".into(), Some(id)));
            create_folder_params = Some(("New Folder".into(), Some(id), FolderType::Schema));
            rename_folder_id = Some(id);
            delete_folder_id = Some(id);
        }
    }

    // Event-based: View Details (needs parent to open tabs)
    if let Some(event) = detail_event {
        menu = menu.item(
            PopupMenuItem::new("View Details").on_click(window.listener_for(
                view,
                move |_this, _, _, cx| {
                    cx.emit(event.clone());
                    cx.stop_propagation();
                },
            )),
        );
    }

    // Direct: Create Service
    if let Some((name, folder_id)) = create_service_params {
        menu = menu.item(
            PopupMenuItem::new("New Service").on_click(window.listener_for(
                view,
                move |this, _, _, cx| {
                    this.create_service(name.clone(), folder_id, cx);
                    cx.stop_propagation();
                },
            )),
        );
    }

    // Direct: Create Schema
    if let Some((name, folder_id)) = create_schema_params {
        menu = menu.item(
            PopupMenuItem::new("New Schema").on_click(window.listener_for(
                view,
                move |this, _, _, cx| {
                    this.create_schema(name.clone(), folder_id, cx);
                    cx.stop_propagation();
                },
            )),
        );
    }

    // Direct: Create Folder
    if let Some((name, parent_id, folder_type)) = create_folder_params {
        menu = menu.item(
            PopupMenuItem::new("New Folder").on_click(window.listener_for(
                view,
                move |this, _, _, cx| {
                    this.create_folder(name.clone(), parent_id, folder_type.clone(), cx);
                    cx.stop_propagation();
                },
            )),
        );
    }

    // Event-based: Import OpenAPI (needs parent's import_handler)
    if let Some(event) = import_event {
        menu = menu.item(
            PopupMenuItem::new("导入 OpenAPI").on_click(window.listener_for(
                view,
                move |_this, _, _, cx| {
                    cx.emit(event.clone());
                    cx.stop_propagation();
                },
            )),
        );
    }

    // Direct: Rename Service
    if let Some(service_id) = rename_service_id {
        menu = menu.item(PopupMenuItem::new("Rename").on_click(window.listener_for(
            view,
            move |this, _, window, cx| {
                this.show_rename_service_dialog(service_id, window, cx);
                cx.stop_propagation();
            },
        )));
    }

    // Direct: Rename Folder
    if let Some(folder_id) = rename_folder_id {
        menu = menu.item(PopupMenuItem::new("Rename").on_click(window.listener_for(
            view,
            move |this, _, window, cx| {
                this.show_rename_folder_dialog(folder_id, window, cx);
                cx.stop_propagation();
            },
        )));
    }

    // Direct: Delete Service
    if let Some(service_id) = delete_service_id {
        menu = menu.item(PopupMenuItem::new("Delete").on_click(window.listener_for(
            view,
            move |this, _, _, cx| {
                this.delete_service(service_id, cx);
                cx.stop_propagation();
            },
        )));
    }

    // Direct: Delete Schema
    if let Some(schema_id) = delete_schema_id {
        menu = menu.item(PopupMenuItem::new("Delete").on_click(window.listener_for(
            view,
            move |this, _, _, cx| {
                this.delete_schema(schema_id, cx);
                cx.stop_propagation();
            },
        )));
    }

    // Direct: Delete Folder
    if let Some(folder_id) = delete_folder_id {
        menu = menu.item(PopupMenuItem::new("Delete").on_click(window.listener_for(
            view,
            move |this, _, _, cx| {
                this.delete_folder(folder_id, cx);
                cx.stop_propagation();
            },
        )));
    }

    menu
}
