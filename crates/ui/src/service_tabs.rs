use gpui::*;
use gpui_component::button::Button;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::Sizable;
use models::Workspace;
use uuid::Uuid;

pub enum ServiceTabsEvent {
    Select(Uuid),
    Close(Uuid),
}

pub struct ServiceTabs {
    workspace: Option<Workspace>,
}

impl EventEmitter<ServiceTabsEvent> for ServiceTabs {}

impl ServiceTabs {
    pub fn new() -> Self {
        Self { workspace: None }
    }

    pub fn set_workspace(&mut self, workspace: Workspace) {
        self.workspace = Some(workspace);
    }
}

impl Render for ServiceTabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(workspace) = &self.workspace else {
            return div().into_any_element();
        };

        if workspace.opened_services.is_empty() {
            return div().into_any_element();
        }

        let service_ids: Vec<_> = workspace.opened_services.clone();
        let selected_index = workspace
            .active_service
            .and_then(|id| workspace.opened_services.iter().position(|&s| s == id))
            .unwrap_or(0);

        let tabs: Vec<_> = workspace
            .opened_services
            .iter()
            .filter_map(|&service_id| {
                workspace
                    .services
                    .iter()
                    .find(|s| s.id == service_id)
                    .map(|service| {
                        Tab::new().pr_1().label(service.name.clone()).suffix(
                            Button::new(format!("close_{}", service_id))
                                .small()
                                .label("×")
                                .on_click(cx.listener(move |_this, _, _, cx| {
                                    cx.emit(ServiceTabsEvent::Close(service_id));
                                    cx.stop_propagation();
                                })),
                        )
                    })
            })
            .collect();

        TabBar::new("service_tabs")
            .selected_index(selected_index)
            .on_click(cx.listener(move |_this, index: &usize, _, cx| {
                if let Some(&service_id) = service_ids.get(*index) {
                    cx.emit(ServiceTabsEvent::Select(service_id));
                }
            }))
            .children(tabs)
            .into_any_element()
    }
}
