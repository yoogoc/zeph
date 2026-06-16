use crate::containers::{ServiceContent, ServiceTabs};
use crate::selectors::EnvironmentSelector;
use crate::state::{EnvironmentState, ServiceState};
use crate::ws_sidebar::WorkspaceSidebar as WsSidebar;
use gpui::*;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::{h_flex, v_flex};

actions!(zeph, [NewRequest]);

const CONTEXT: &str = "WorkspaceContent";

pub struct WorkspaceContent {
    service_state: Entity<ServiceState>,
    service_tabs: Entity<ServiceTabs>,
    sidebar: Entity<WsSidebar>,
    env_selector: Entity<EnvironmentSelector>,
    service_content: Entity<ServiceContent>,
    focus_handle: FocusHandle,
}

impl WorkspaceContent {
    pub fn new(
        service_state: Entity<ServiceState>,
        env_state: Entity<EnvironmentState>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Bind Cmd+N → NewRequest within this view's key context. Idempotent
        // across workspace re-opens (identical bindings simply resolve the same).
        cx.bind_keys([KeyBinding::new("cmd-n", NewRequest, Some(CONTEXT))]);

        let sidebar =
            cx.new(|cx| WsSidebar::new(service_state.clone(), |_| div().into_any_element(), cx));

        let service_tabs = cx.new(|_| ServiceTabs::new(service_state.clone()));
        let env_selector = cx.new(|_| EnvironmentSelector::new(env_state));
        let service_content = cx.new(|_| ServiceContent::new(service_state.clone()));

        Self {
            service_state,
            service_tabs,
            sidebar,
            env_selector,
            service_content,
            focus_handle: cx.focus_handle(),
        }
    }

    fn on_new_request(&mut self, _: &NewRequest, window: &mut Window, cx: &mut Context<Self>) {
        self.service_state.update(cx, |s, cx| {
            s.create_untitled_request(
                models::ServiceType::Http(models::HttpConfig::default()),
                None,
                window,
                cx,
            );
        });
    }
}

impl Render for WorkspaceContent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus-tracked wrapper so the Cmd+N action resolves within this view's
        // key context. `flex` + a `flex_1`/`size_full` child keeps the resizable
        // group filling the wrapper (a plain block child would collapse and blank
        // out the sidebar/tree).
        div()
            .size_full()
            .flex()
            .track_focus(&self.focus_handle)
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::on_new_request))
            .child(
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
                                                .child(
                                                    div().flex_1().child(self.service_tabs.clone()),
                                                )
                                                .child(
                                                    div().p_2().child(self.env_selector.clone()),
                                                ),
                                        )
                                        .child(self.service_content.clone()),
                                ),
                        ),
                    ),
            )
    }
}
