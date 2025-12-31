use gpui::*;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::tree::TreeItem;
use models::FolderType;
use crate::ws_sidebar::{TreeId, WorkspaceSidebar, WorkspaceSidebarEvent};

pub fn popup_menu(menu: PopupMenu, item: &TreeItem, view: &Entity<WorkspaceSidebar>, window: &mut Window, _cx: &mut Context<PopupMenu>) -> PopupMenu {
    let mut menu = menu;
    let id = TreeId::new(item.id.clone());

    let mut detail = None;
    let mut create_service = None;
    let mut create_schema = None;
    let mut create_folder = None;
    let mut delete = None;
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
                Some(id),
            ));
            create_folder = Some(WorkspaceSidebarEvent::CreateFolder(
                "New Folder".into(),
                Some(id),
                FolderType::Service,
            ));
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
