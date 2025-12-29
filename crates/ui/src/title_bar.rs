use gpui::*;
use gpui_component::button::Button;
use gpui_component::tab::{Tab, TabBar};
use gpui_component::{Sizable, TitleBar as GTitleBar};
use models::ApplicationView;

pub struct TitleBar {
    selected_index: usize,
}

impl TitleBar {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        cx.subscribe(
            &workspace_registry,
            |_this, _registry, _event: &workspace::WorkspaceEvent, cx| {
                cx.notify();
            },
        )
        .detach();

        Self { selected_index: 0 }
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

    fn close_workspace(&mut self, workspace_id: uuid::Uuid, cx: &mut Context<Self>) {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        workspace_registry.update(cx, |registry, cx| {
            registry.close_workspace(workspace_id, cx);
        });
    }
}

impl Render for TitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let app_state = workspace_registry.read(cx).get_state();
        let opened_workspaces = workspace_registry.read(cx).get_opened_workspaces();

        let current_view = &app_state.current_view;
        self.selected_index = match current_view {
            ApplicationView::Home => 0,
            ApplicationView::Workspace(id) => opened_workspaces
                .iter()
                .position(|w| w.id == *id)
                .map(|i| i + 1)
                .unwrap_or(0),
        };

        let workspace_ids: Vec<_> = opened_workspaces.iter().map(|w| w.id).collect();

        GTitleBar::new().child(
            TabBar::new("workspace_tabs")
                .menu(true)
                .selected_index(self.selected_index)
                .on_click(cx.listener(move |this, index: &usize, _, cx| {
                    if *index == 0 {
                        this.switch_to_home(cx);
                    } else if let Some(&workspace_id) = workspace_ids.get(*index - 1) {
                        this.switch_to_workspace(workspace_id, cx);
                    }
                    this.selected_index = *index;
                    cx.notify();
                }))
                .items_center()
                .gap_2()
                .py_2()
                .child(
                    Tab::new()
                        .label("Home")
                        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default()),
                )
                .children(opened_workspaces.iter().map(|workspace| {
                    let workspace_id = workspace.id;
                    Tab::new()
                        .label(workspace.name.clone())
                        .pr_1()
                        .suffix(
                            Button::new(format!("close_{}", workspace_id))
                                .small()
                                .label("×")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.close_workspace(workspace_id, cx);
                                    cx.stop_propagation();
                                })),
                        )
                        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                }))
                .h_full(),
        )
    }
}
