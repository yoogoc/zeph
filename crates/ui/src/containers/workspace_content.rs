use crate::containers::{ServiceContent, ServiceTabs};
use crate::selectors::EnvironmentSelector;
use crate::state::ServiceState;
use crate::ws_sidebar::WorkspaceSidebar as WsSidebar;
use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::{h_flex, v_flex};

pub struct WorkspaceContent {
    service_state: Entity<ServiceState>,
    service_tabs: Entity<ServiceTabs>,
    sidebar: Entity<WsSidebar>,
    env_selector: Entity<EnvironmentSelector>,
    service_content: Entity<ServiceContent>,
}

impl WorkspaceContent {
    pub fn new(
        service_state: Entity<ServiceState>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sidebar =
            cx.new(|cx| WsSidebar::new(service_state.clone(), |_| div().into_any_element(), cx));

        let service_tabs = cx.new(|_| ServiceTabs::new(service_state.clone()));
        let env_selector = cx.new(|_| EnvironmentSelector::new(service_state.clone()));
        let service_content = cx.new(|cx| ServiceContent::new(service_state.clone(), cx));

        Self {
            service_state,
            service_tabs,
            sidebar,
            env_selector,
            service_content,
        }
    }
}

impl Render for WorkspaceContent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        h_resizable("workspace-content")
            .child(resizable_panel().size(px(200.)).child(self.sidebar.clone()))
            .child(
                resizable_panel().child(
                    div()
                        .size_full()
                        .pt_1()
                        .pb_2()
                        .pr_2()
                        .bg(rgb(0xf5f5f5))
                        .child(
                            v_flex()
                                .rounded_2xl()
                                .bg(white())
                                .flex_1()
                                .h_full()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .justify_between()
                                        .border_b_1()
                                        .border_color(rgb(0xe0e0e0))
                                        .child(div().flex_1().child(self.service_tabs.clone()))
                                        .child(div().p_2().child(self.env_selector.clone())),
                                )
                                .child(self.service_content.clone()),
                        ),
                ),
            )
    }
}
