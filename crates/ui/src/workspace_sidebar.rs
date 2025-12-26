use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, StyledExt, WindowExt};
use models::{Folder, FolderType, Schema, Service, ServiceType, Workspace};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use workspace::ConflictResolution;

use crate::{ImportDialog, ImportDialogEvent, ImportConfig};

pub enum WorkspaceSidebarEvent {
    OpenService(Uuid),
    OpenSchema(Uuid),
    CreateService(String, Option<Uuid>),
    CreateSchema(String, Option<Uuid>),
    CreateFolder(String, Option<Uuid>, FolderType),
    ToggleFolder(Uuid),
    ImportOpenApi(ImportConfig),
    PreviewImport(ImportConfig),
    RequestImportPreview(ImportConfig),
    ResolveImportConflicts(ImportConfig, HashMap<String, ConflictResolution>),
}

pub struct WorkspaceSidebar {
    workspace: Option<Workspace>,
    pub expanded_folders: HashSet<Uuid>,
    expanded_sections: (bool, bool), // (services, schemas)
}

impl EventEmitter<WorkspaceSidebarEvent> for WorkspaceSidebar {}

impl WorkspaceSidebar {
    pub fn new() -> Self {
        Self {
            workspace: None,
            expanded_folders: HashSet::new(),
            expanded_sections: (true, true),
        }
    }

    pub fn set_workspace(&mut self, workspace: Workspace) {
        if self.workspace.is_none() {
            for folder in &workspace.folders {
                self.expanded_folders.insert(folder.id);
            }
        }
        self.workspace = Some(workspace);
    }

    fn open_create_service_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Service name"));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();

            dialog
                .title("Create Service")
                .w(px(400.0))
                .confirm()
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            this.update(cx, |_, cx| {
                                cx.emit(WorkspaceSidebarEvent::CreateService(name, None));
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    fn open_create_schema_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Schema name"));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();

            dialog
                .title("Create Schema")
                .w(px(400.0))
                .confirm()
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            this.update(cx, |_, cx| {
                                cx.emit(WorkspaceSidebarEvent::CreateSchema(name, None));
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    fn open_create_folder_dialog(&mut self, folder_type: FolderType, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Folder name"));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();
            let ft = folder_type.clone();

            dialog
                .title("Create Folder")
                .w(px(400.0))
                .confirm()
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            this.update(cx, |_, cx| {
                                cx.emit(WorkspaceSidebarEvent::CreateFolder(name, None, ft.clone()));
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    fn open_import_openapi_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _this = cx.entity().downgrade();
        let import_dialog = cx.new(|cx| ImportDialog::new(window, cx));
        
        // Set available folders for the dialog
        if let Some(workspace) = &self.workspace {
            let service_folders: Vec<Folder> = workspace.folders
                .iter()
                .filter(|f| f.folder_type == FolderType::Service)
                .cloned()
                .collect();
            
            import_dialog.update(cx, |dialog, cx| {
                dialog.set_folders(service_folders, cx);
            });
        }

        // Subscribe to import dialog events
        let import_dialog_clone = import_dialog.clone();
        cx.subscribe(&import_dialog, move |_this, _dialog, event, cx| {
            match event {
                ImportDialogEvent::Import(config) => {
                    cx.emit(WorkspaceSidebarEvent::ImportOpenApi(config.clone()));
                }
                ImportDialogEvent::RequestPreview(config) => {
                    cx.emit(WorkspaceSidebarEvent::RequestImportPreview(config.clone()));
                }
                ImportDialogEvent::ResolveConflicts(config, resolutions) => {
                    cx.emit(WorkspaceSidebarEvent::ResolveImportConflicts(config.clone(), resolutions.clone()));
                }
                ImportDialogEvent::Cancel => {
                    // Dialog will be closed automatically
                }
            }
        }).detach();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("Import OpenAPI")
                .w(px(700.0))
                .max_h(px(600.0))
                .child(import_dialog_clone.clone())
                .on_ok(move |_, window, cx| {
                    window.close_dialog(cx);
                    true
                })
        });
    }

    fn render_service_item(&self, service: &Service, is_active: bool, depth: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let service_id = service.id;
        let method_color = match &service.service_type {
            ServiceType::Http(config) => match config.method {
                models::HttpMethod::Get => rgb(0x28a745),
                models::HttpMethod::Post => rgb(0x007bff),
                models::HttpMethod::Put => rgb(0xffc107),
                models::HttpMethod::Delete => rgb(0xdc3545),
                _ => rgb(0x6c757d),
            },
            ServiceType::Grpc(_) => rgb(0x6f42c1),
            ServiceType::WebSocket(_) => rgb(0x17a2b8),
        };

        let method_label = match &service.service_type {
            ServiceType::Http(config) => config.method.as_str(),
            ServiceType::Grpc(_) => "gRPC",
            ServiceType::WebSocket(_) => "WS",
        };

        Button::new(format!("service_{}", service_id))
            .w_full()
            .justify_start()
            .when(is_active, |btn: Button| btn.primary())
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .pl(px(depth as f32 * 16.0))
                    .child(
                        div()
                            .text_xs()
                            .px_1()
                            .rounded(px(2.0))
                            .bg(method_color)
                            .text_color(rgb(0xffffff))
                            .child(method_label),
                    )
                    .child(div().text_sm().overflow_hidden().child(service.name.clone())),
            )
            .on_click(cx.listener(move |_this, _, _, cx| {
                cx.emit(WorkspaceSidebarEvent::OpenService(service_id));
            }))
    }

    fn render_schema_item(&self, schema: &Schema, depth: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let schema_id = schema.id;

        Button::new(format!("schema_{}", schema_id))
            .w_full()
            .justify_start()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .pl(px(depth as f32 * 16.0))
                    .child(gpui_component::Icon::new(IconName::File).size_4().text_color(rgb(0x6f42c1)))
                    .child(div().text_sm().overflow_hidden().child(schema.name.clone())),
            )
            .on_click(cx.listener(move |_this, _, _, cx| {
                cx.emit(WorkspaceSidebarEvent::OpenSchema(schema_id));
            }))
    }

    fn render_folder_item(&self, folder: &Folder, depth: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let folder_id = folder.id;
        let is_expanded = self.expanded_folders.contains(&folder_id);
        let icon = if is_expanded { IconName::ChevronDown } else { IconName::ChevronRight };

        Button::new(format!("folder_{}", folder_id))
            .w_full()
            .justify_start()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_1()
                    .pl(px(depth as f32 * 16.0))
                    .child(gpui_component::Icon::new(icon).size_4())
                    .child(gpui_component::Icon::new(IconName::Folder).size_4())
                    .child(div().text_sm().child(folder.name.clone())),
            )
            .on_click(cx.listener(move |_this, _, _, cx| {
                cx.emit(WorkspaceSidebarEvent::ToggleFolder(folder_id));
            }))
    }

    fn render_service_tree(&self, parent_id: Option<Uuid>, depth: usize, active_service: Option<Uuid>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(workspace) = &self.workspace else { return vec![] };
        let mut elements: Vec<AnyElement> = vec![];

        for folder in workspace.folders.iter().filter(|f| f.parent_id == parent_id && matches!(f.folder_type, FolderType::Service)) {
            elements.push(self.render_folder_item(folder, depth, cx).into_any_element());
            if self.expanded_folders.contains(&folder.id) {
                elements.extend(self.render_service_tree(Some(folder.id), depth + 1, active_service, cx));
            }
        }

        for service in workspace.services.iter().filter(|s| s.folder_id == parent_id) {
            let is_active = active_service == Some(service.id);
            elements.push(self.render_service_item(service, is_active, depth, cx).into_any_element());
        }

        elements
    }

    fn render_schema_tree(&self, parent_id: Option<Uuid>, depth: usize, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(workspace) = &self.workspace else { return vec![] };
        let mut elements: Vec<AnyElement> = vec![];

        for folder in workspace.folders.iter().filter(|f| f.parent_id == parent_id && matches!(f.folder_type, FolderType::Schema)) {
            elements.push(self.render_folder_item(folder, depth, cx).into_any_element());
            if self.expanded_folders.contains(&folder.id) {
                elements.extend(self.render_schema_tree(Some(folder.id), depth + 1, cx));
            }
        }

        for schema in workspace.schemas.iter().filter(|s| s.folder_id == parent_id) {
            elements.push(self.render_schema_item(schema, depth, cx).into_any_element());
        }

        elements
    }
}

impl Render for WorkspaceSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(workspace) = &self.workspace else {
            return div().into_any_element();
        };

        let active_service = workspace.active_service;
        let (services_expanded, schemas_expanded) = self.expanded_sections;
        let view = cx.entity();

        let service_tree = if services_expanded {
            self.render_service_tree(None, 0, active_service, cx)
        } else {
            vec![]
        };

        let schema_tree = if schemas_expanded {
            self.render_schema_tree(None, 0, cx)
        } else {
            vec![]
        };

        div()
            .w(px(300.0))
            .h_full()
            .bg(rgb(0xf8f9fa))
            .border_r_1()
            .border_color(rgb(0xe0e0e0))
            .v_flex()
            .child(
                div()
                    .p_2()
                    .child(
                        div()
                            .p_2()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xdee2e6))
                            .rounded_md()
                            .text_sm()
                            .text_color(rgb(0x999999))
                            .child("Search..."),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_y_hidden()
                    .p_2()
                    .v_flex()
                    .gap_2()
                    // Services Section
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .id("services_header")
                                            .h_flex()
                                            .items_center()
                                            .gap_1()
                                            .cursor_pointer()
                                            .child(gpui_component::Icon::new(if services_expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size_4())
                                            .child(div().text_sm().font_semibold().child("Services"))
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.expanded_sections.0 = !this.expanded_sections.0;
                                            })),
                                    )
                                    .child(
                                        Button::new("add_service_btn")
                                            .ghost()
                                            .compact()
                                            .icon(IconName::Plus)
                                            .dropdown_menu({
                                                let view = view.clone();
                                                move |menu, window, _cx| {
                                                    menu.item(PopupMenuItem::new("Add Service").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_create_service_dialog(window, cx);
                                                        }),
                                                    ))
                                                    .item(PopupMenuItem::new("Add Folder").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_create_folder_dialog(FolderType::Service, window, cx);
                                                        }),
                                                    ))
                                                    .separator()
                                                    .item(PopupMenuItem::new("Import OpenAPI").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_import_openapi_dialog(window, cx);
                                                        }),
                                                    ))
                                                }
                                            }),
                                    ),
                            )
                            .when(services_expanded, |el| el.child(div().v_flex().gap_px().children(service_tree))),
                    )
                    // Schemas Section
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .id("schemas_header")
                                            .h_flex()
                                            .items_center()
                                            .gap_1()
                                            .cursor_pointer()
                                            .child(gpui_component::Icon::new(if schemas_expanded { IconName::ChevronDown } else { IconName::ChevronRight }).size_4())
                                            .child(div().text_sm().font_semibold().child("Schemas"))
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.expanded_sections.1 = !this.expanded_sections.1;
                                            })),
                                    )
                                    .child(
                                        Button::new("add_schema_btn")
                                            .ghost()
                                            .compact()
                                            .icon(IconName::Plus)
                                            .dropdown_menu({
                                                let view = view.clone();
                                                move |menu, window, _cx| {
                                                    menu.item(PopupMenuItem::new("Add Schema").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_create_schema_dialog(window, cx);
                                                        }),
                                                    ))
                                                    .item(PopupMenuItem::new("Add Folder").on_click(
                                                        window.listener_for(&view, |this, _, window, cx| {
                                                            this.open_create_folder_dialog(FolderType::Schema, window, cx);
                                                        }),
                                                    ))
                                                }
                                            }),
                                    ),
                            )
                            .when(schemas_expanded, |el| el.child(div().v_flex().gap_px().children(schema_tree))),
                    ),
            )
            .into_any_element()
    }
}
