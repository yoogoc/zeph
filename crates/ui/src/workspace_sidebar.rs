use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, StyledExt};
use models::{Folder, Service, ServiceType, Workspace};
use std::collections::HashSet;
use uuid::Uuid;

pub enum WorkspaceSidebarEvent {
    OpenService(Uuid),
    CreateService(Option<Uuid>),
    ToggleFolder(Uuid),
}

pub struct WorkspaceSidebar {
    workspace: Option<Workspace>,
    pub expanded_folders: HashSet<Uuid>,
}

impl EventEmitter<WorkspaceSidebarEvent> for WorkspaceSidebar {}

impl WorkspaceSidebar {
    pub fn new() -> Self {
        Self {
            workspace: None,
            expanded_folders: HashSet::new(),
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

    fn render_service_item(
        &self,
        service: &Service,
        is_active: bool,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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

    fn render_folder_item(
        &self,
        folder: &Folder,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let folder_id = folder.id;
        let is_expanded = self.expanded_folders.contains(&folder_id);
        let icon = if is_expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };

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

    fn render_tree(
        &self,
        parent_id: Option<Uuid>,
        depth: usize,
        active_service: Option<Uuid>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(workspace) = &self.workspace else {
            return vec![];
        };

        let mut elements: Vec<AnyElement> = vec![];

        for folder in workspace.folders.iter().filter(|f| f.parent_id == parent_id) {
            elements.push(self.render_folder_item(folder, depth, cx).into_any_element());
            if self.expanded_folders.contains(&folder.id) {
                elements.extend(self.render_tree(Some(folder.id), depth + 1, active_service, cx));
            }
        }

        for service in workspace.services.iter().filter(|s| s.folder_id == parent_id) {
            let is_active = active_service == Some(service.id);
            elements.push(self.render_service_item(service, is_active, depth, cx).into_any_element());
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
        let tree_elements = self.render_tree(None, 0, active_service, cx);

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
                    .h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .p_2()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xdee2e6))
                            .rounded_md()
                            .text_sm()
                            .text_color(rgb(0x999999))
                            .child("Search..."),
                    )
                    .child(
                        Button::new("add_service")
                            .compact()
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|_this, _, _, cx| {
                                cx.emit(WorkspaceSidebarEvent::CreateService(None));
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_2()
                    .v_flex()
                    .gap_px()
                    .children(tree_elements),
            )
            .into_any_element()
    }
}
