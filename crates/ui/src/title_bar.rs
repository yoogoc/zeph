use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    TitleBar as GTitleBar, *,
};
use models::ApplicationView;

pub struct TitleBar {}

impl TitleBar {
    pub fn new() -> Self {
        Self {}
    }

    fn switch_to_home(&mut self, cx: &mut Context<Self>) {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        workspace_registry.update(cx, |registry, cx| {
            registry.go_to_home(cx);
        });
    }

    fn switch_to_workspace(&mut self, workspace_id: uuid::Uuid, cx: &mut Context<Self>) {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        workspace_registry.update(cx, |registry, cx| {
            registry.set_active_workspace(workspace_id, cx);
        });
    }
}

impl Default for TitleBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for TitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let app_state = workspace_registry.read(cx).get_state();
        let current_view = app_state.current_view.clone();
        let workspaces = app_state.workspaces.clone();

        GTitleBar::new().child(
            div()
                .h_flex()
                .items_center()
                .gap_2()
                .py_2()
                .child(
                    Button::new("tab_home")
                        .label("Home")
                        .when(
                            matches!(current_view, ApplicationView::Home),
                            |btn: Button| btn.primary(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.switch_to_home(cx);
                        })),
                )
                .children(workspaces.iter().map(|workspace| {
                    let is_active = matches!(
                        current_view,
                        ApplicationView::Workspace(id) if id == workspace.id
                    );
                    let workspace_id = workspace.id;
                    let workspace_name = workspace.name.clone();

                    Button::new(format!("tab_{}", workspace.id))
                        .label(workspace_name)
                        .when(is_active, |btn: Button| btn.primary())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.switch_to_workspace(workspace_id, cx);
                        }))
                })),
        )
    }
}
