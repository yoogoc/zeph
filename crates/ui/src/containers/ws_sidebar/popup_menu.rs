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

    let mut detail = None;
    let mut create_service = None;
    let mut create_schema = None;
    let mut create_folder = None;
    let mut rename = None;
    let mut delete = None;
    let mut import_openapi = None;
    match id {
        TreeId::ServiceRoot => {
            create_service = Some(WorkspaceSidebarEvent::CreateService(
                "New Service".into(),
                None,
            ));
            create_folder = Some(WorkspaceSidebarEvent::CreateFolder(
                "New Folder".into(),
                None,
                FolderType::Service,
            ));
            // Implements Requirement 1.3, 1.4 - Import OpenAPI from service root with no target folder
            import_openapi = Some(WorkspaceSidebarEvent::ShowImportDialog(None));
        }
        TreeId::SchemaRoot => {
            create_schema = Some(WorkspaceSidebarEvent::CreateSchema(
                "New Schema".into(),
                None,
            ));
            create_folder = Some(WorkspaceSidebarEvent::CreateFolder(
                "New Folder".into(),
                None,
                FolderType::Schema,
            ));
        }
        TreeId::Service(id) => {
            detail = Some(WorkspaceSidebarEvent::OpenService(id));
            // Implements Requirements 1.1, 2.1 - Add Rename menu item for Service
            rename = Some(WorkspaceSidebarEvent::RenameService(id));
            delete = Some(WorkspaceSidebarEvent::DeleteService(id));
        }
        TreeId::Schema(id) => {
            detail = Some(WorkspaceSidebarEvent::OpenSchema(id));
            delete = Some(WorkspaceSidebarEvent::DeleteSchema(id));
        }
        TreeId::ServiceFolder(id) => {
            detail = Some(WorkspaceSidebarEvent::OpenFolderDetail(id.clone()));
            create_service = Some(WorkspaceSidebarEvent::CreateService(
                "New Service".into(),
                Some(id.clone()),
            ));
            create_folder = Some(WorkspaceSidebarEvent::CreateFolder(
                "New Folder".into(),
                Some(id.clone()),
                FolderType::Service,
            ));
            // Implements Requirement 1.1, 1.2 - Import OpenAPI with target folder preset
            import_openapi = Some(WorkspaceSidebarEvent::ShowImportDialog(Some(id.clone())));
            // Implements Requirements 1.1, 2.1 - Add Rename menu item for Folder
            rename = Some(WorkspaceSidebarEvent::RenameFolder(id.clone()));
            delete = Some(WorkspaceSidebarEvent::DeleteFolder(id));
        }
        TreeId::SchemaFolder(id) => {
            detail = Some(WorkspaceSidebarEvent::OpenFolderDetail(id));
            create_schema = Some(WorkspaceSidebarEvent::CreateSchema(
                "New Schema".into(),
                Some(id),
            ));
            create_folder = Some(WorkspaceSidebarEvent::CreateFolder(
                "New Folder".into(),
                Some(id),
                FolderType::Schema,
            ));
            // Implements Requirements 1.1, 2.1 - Add Rename menu item for Folder
            rename = Some(WorkspaceSidebarEvent::RenameFolder(id));
            delete = Some(WorkspaceSidebarEvent::DeleteFolder(id));
        }
    }

    if let Some(event) = detail {
        menu = menu.item(
            PopupMenuItem::new("View Details").on_click(window.listener_for(
                &view,
                move |this, _, _window, cx| {
                    this.handle_context_menu_event(event.clone(), cx);

                    cx.stop_propagation();
                },
            )),
        );
    }
    if let Some(event) = create_service {
        menu = menu.item(
            PopupMenuItem::new("New Service").on_click(window.listener_for(
                &view,
                move |this, _, _window, cx| {
                    this.handle_context_menu_event(event.clone(), cx);

                    cx.stop_propagation();
                },
            )),
        )
    }

    if let Some(event) = create_schema {
        menu = menu.item(
            PopupMenuItem::new("New Schema").on_click(window.listener_for(
                &view,
                move |this, _, _window, cx| {
                    this.handle_context_menu_event(event.clone(), cx);

                    cx.stop_propagation();
                },
            )),
        )
    }

    if let Some(event) = create_folder {
        menu = menu.item(
            PopupMenuItem::new("New Folder").on_click(window.listener_for(
                &view,
                move |this, _, _window, cx| {
                    this.handle_context_menu_event(event.clone(), cx);

                    cx.stop_propagation();
                },
            )),
        )
    }

    // Import OpenAPI menu item
    // Implements Requirements 1.1, 1.2, 1.3, 1.4
    if let Some(event) = import_openapi {
        menu = menu.item(
            PopupMenuItem::new("导入 OpenAPI").on_click(window.listener_for(
                &view,
                move |this, _, _window, cx| {
                    this.handle_context_menu_event(event.clone(), cx);

                    cx.stop_propagation();
                },
            )),
        )
    }

    // Rename menu item
    // Implements Requirements 1.1, 2.1 - Add Rename menu item for Service and Folder
    if let Some(event) = rename {
        menu = menu.item(PopupMenuItem::new("Rename").on_click(window.listener_for(
            &view,
            move |this, _, _window, cx| {
                this.handle_context_menu_event(event.clone(), cx);

                cx.stop_propagation();
            },
        )))
    }

    if let Some(event) = delete {
        menu = menu.item(PopupMenuItem::new("Delete").on_click(window.listener_for(
            &view,
            move |this, _, _window, cx| {
                this.handle_context_menu_event(event.clone(), cx);

                cx.stop_propagation();
            },
        )))
    }

    menu
}
