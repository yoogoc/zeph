use crate::state::ServiceState;
use crate::ws_sidebar::TreeId;
use gpui::*;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::tree::TreeItem;
use models::FolderType;

/// Detail action type for View Details menu item
#[derive(Clone)]
enum DetailAction {
    Service(uuid::Uuid),
    Schema(uuid::Uuid),
    Folder(uuid::Uuid),
}

pub fn popup_menu(
    menu: PopupMenu,
    item: &TreeItem,
    service_state: Entity<ServiceState>,
    _window: &mut Window,
    _cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let mut menu = menu;
    let id = TreeId::new(item.id.clone());

    // Direct action parameters
    let mut detail_action: Option<DetailAction> = None;
    let mut create_service_params: Option<(SharedString, Option<uuid::Uuid>)> = None;
    let mut create_schema_params: Option<(SharedString, Option<uuid::Uuid>)> = None;
    let mut create_folder_params: Option<(SharedString, Option<uuid::Uuid>, FolderType)> = None;
    let mut rename_service_id: Option<uuid::Uuid> = None;
    let mut rename_folder_id: Option<uuid::Uuid> = None;
    let mut delete_service_id: Option<uuid::Uuid> = None;
    let mut delete_schema_id: Option<uuid::Uuid> = None;
    let mut delete_folder_id: Option<uuid::Uuid> = None;
    let mut import_folder_id: Option<Option<uuid::Uuid>> = None;

    match id {
        TreeId::ServiceRoot => {
            create_service_params = Some(("New Service".into(), None));
            create_folder_params = Some(("New Folder".into(), None, FolderType::Service));
            import_folder_id = Some(None);
        }
        TreeId::SchemaRoot => {
            create_schema_params = Some(("New Schema".into(), None));
            create_folder_params = Some(("New Folder".into(), None, FolderType::Schema));
        }
        TreeId::Service(id) => {
            detail_action = Some(DetailAction::Service(id));
            rename_service_id = Some(id);
            delete_service_id = Some(id);
        }
        TreeId::Schema(id) => {
            detail_action = Some(DetailAction::Schema(id));
            delete_schema_id = Some(id);
        }
        TreeId::ServiceFolder(id) => {
            detail_action = Some(DetailAction::Folder(id));
            create_service_params = Some(("New Service".into(), Some(id)));
            create_folder_params = Some(("New Folder".into(), Some(id), FolderType::Service));
            import_folder_id = Some(Some(id));
            rename_folder_id = Some(id);
            delete_folder_id = Some(id);
        }
        TreeId::SchemaFolder(id) => {
            detail_action = Some(DetailAction::Folder(id));
            create_schema_params = Some(("New Schema".into(), Some(id)));
            create_folder_params = Some(("New Folder".into(), Some(id), FolderType::Schema));
            rename_folder_id = Some(id);
            delete_folder_id = Some(id);
        }
    }

    // Direct: View Details
    if let Some(action) = detail_action {
        let service_state = service_state.clone();
        menu = menu.item(
            PopupMenuItem::new("View Details").on_click(move |_, window, cx| {
                service_state.update(cx, |s, cx| match action {
                    DetailAction::Service(id) => s.open_service(id, window, cx),
                    // TODO: schema detail page not implemented yet.
                    DetailAction::Schema(_) => {}
                    DetailAction::Folder(id) => s.open_folder_detail(id, window, cx),
                });
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Create Service
    if let Some((name, folder_id)) = create_service_params {
        let service_state = service_state.clone();
        menu = menu.item(
            PopupMenuItem::new("New Service").on_click(move |_, _window, cx| {
                service_state.update(cx, |s, cx| {
                    s.create_service(name.clone(), folder_id, cx);
                });
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Create Schema
    if let Some((name, folder_id)) = create_schema_params {
        let service_state = service_state.clone();
        menu = menu.item(
            PopupMenuItem::new("New Schema").on_click(move |_, _window, cx| {
                service_state.update(cx, |s, cx| {
                    s.create_schema(name.clone(), folder_id, cx);
                });
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Create Folder
    if let Some((name, parent_id, folder_type)) = create_folder_params {
        let service_state = service_state.clone();
        menu = menu.item(
            PopupMenuItem::new("New Folder").on_click(move |_, _window, cx| {
                service_state.update(cx, |s, cx| {
                    s.create_folder(name.clone(), parent_id, folder_type.clone(), cx);
                });
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Import OpenAPI
    if let Some(folder_id) = import_folder_id {
        menu = menu.item(
            PopupMenuItem::new("导入 OpenAPI").on_click(move |_, _window, cx| {
                // TODO: import dialog not wired into ServiceState yet.
                let _ = folder_id;
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Rename Service
    if let Some(service_id) = rename_service_id {
        menu = menu.item(
            PopupMenuItem::new("Rename").on_click(move |_, _window, cx| {
                // TODO: needs a rename dialog to collect the new name
                // before calling ServiceState::rename_service.
                let _ = service_id;
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Rename Folder
    if let Some(folder_id) = rename_folder_id {
        menu = menu.item(
            PopupMenuItem::new("Rename").on_click(move |_, _window, cx| {
                // TODO: needs a rename dialog to collect the new name
                // before calling ServiceState::rename_folder.
                let _ = folder_id;
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Delete Service
    if let Some(service_id) = delete_service_id {
        let service_state = service_state.clone();
        menu = menu.item(
            PopupMenuItem::new("Delete").on_click(move |_, _window, cx| {
                service_state.update(cx, |s, cx| {
                    s.delete_service(service_id, cx);
                });
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Delete Schema
    if let Some(schema_id) = delete_schema_id {
        menu = menu.item(
            PopupMenuItem::new("Delete").on_click(move |_, _window, cx| {
                // TODO: ServiceState/storage lacks schema delete support yet.
                let _ = schema_id;
                cx.stop_propagation();
            }),
        );
    }

    // Direct: Delete Folder
    if let Some(folder_id) = delete_folder_id {
        menu = menu.item(
            PopupMenuItem::new("Delete").on_click(move |_, _window, cx| {
                // TODO: ServiceState/storage lacks folder delete support yet.
                let _ = folder_id;
                cx.stop_propagation();
            }),
        );
    }

    menu
}
