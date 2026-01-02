use crate::utils::cal_icon;
use gpui::*;
use gpui_component::button::{Button, ButtonRounded};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{IconName, Sizable};
use models::{ContentType, Workspace};

pub enum ServiceTabsEvent {
    Select(ContentType),
    Close(ContentType),
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
            .clone()
            .and_then(|id| workspace.opened_services.iter().position(|s| s.eq(&id)))
            .unwrap_or(0);

        let tabs: Vec<_> = workspace
            .opened_services
            .iter()
            .map(|ct| {
                let ct = ct.clone();
                Tab::new()
                    .px_1()
                    .label(ct.name())
                    .prefix(cal_icon(&ct))
                    .suffix(
                        Button::new(format!("close_{}", ct.id()))
                            .xsmall()
                            .rounded(ButtonRounded::Large)
                            .icon(IconName::Close)
                            .on_click(cx.listener(move |_this, _, _, cx| {
                                cx.emit(ServiceTabsEvent::Close(ct.clone()));
                                cx.stop_propagation();
                            })),
                    )
            })
            .collect();

        div().size_full().pl_1().child(
            TabBar::new("service_tabs")
                .selected_index(selected_index)
                .with_variant(TabVariant::Pill)
                .on_click(cx.listener(move |_this, index: &usize, _, cx| {
                    if let Some(ct) = service_ids.get(*index) {
                        cx.emit(ServiceTabsEvent::Select(ct.clone()));
                    }
                }))
                .children(tabs),
        ).into_any_element()
    }
}
