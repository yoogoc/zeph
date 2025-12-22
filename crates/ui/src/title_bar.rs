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
        let app_state = workspace_registry.read(cx).get_state();
        let current_view = app_state.current_view.clone();
        let workspaces = app_state.workspaces.clone();
        let selected_index = match current_view {
            ApplicationView::Home => 0,
            ApplicationView::Workspace(id) => workspaces
                .iter()
                .position(|workspace| workspace.id == id)
                .map(|index| index + 1)
                .unwrap_or(0),
        };

        Self { selected_index }
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

impl Render for TitleBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let app_state = workspace_registry.read(cx).get_state();
        let workspaces = app_state.workspaces.clone();

        let workspace_ids = workspaces
            .iter()
            .map(|workspace| workspace.id)
            .collect::<Vec<_>>();

        GTitleBar::new().child(
            TabBar::new("workspace_tabs")
                .selected_index(self.selected_index)
                .on_click(cx.listener(move |this, index, _, cx| {
                    if this.selected_index.ne(index) {
                        if *index == 0 {
                            this.switch_to_home(cx);
                        } else {
                            let workspace_id = workspace_ids.get(*index - 1);
                            if let Some(&workspace_id) = workspace_id {
                                this.switch_to_workspace(workspace_id, cx);
                            }
                        }
                    }
                    this.selected_index = *index;

                    cx.notify();
                }))
                .items_center()
                .gap_2()
                .py_2()
                .child(Tab::new().label("Home").on_mouse_down(
                    MouseButton::Left,
                    move |_, window, cx| {
                        window.prevent_default();
                    },
                ))
                .children(app_state.workspaces.clone().iter().map(|workspace| {
                    let workspace_id = workspace.id;
                    let workspace_name = workspace.name.clone();

                    Tab::new()
                        .label(workspace_name)
                        .px_1()
                        .suffix(
                            Button::new(format!("close_{}", workspace_id))
                                .small()
                                .label("×")
                                .on_click(cx.listener(move |_this, _, _, _cx| {
                                    println!("close {workspace_id}")
                                })),
                        )
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            window.prevent_default();
                        })
                }))
                .h_full(),
        )
    }
}
