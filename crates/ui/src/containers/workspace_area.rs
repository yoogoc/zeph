use crate::containers::Sidebar;
use crate::state::WorkspaceState;
use gpui::*;
use gpui_component::h_flex;

pub struct WorkspaceArea {
    workspace_state: Entity<WorkspaceState>,
}

impl WorkspaceArea {
    pub fn new(workspace_state: Entity<WorkspaceState>) -> Self {
        Self { workspace_state }
    }
}

impl Render for WorkspaceArea {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace_state.read(cx);
        let active_idx = ws.active_idx;
        let active_view = ws.tabs[active_idx].view.clone();
        let tab_info: Vec<(SharedString, SharedString, gpui_component::IconName)> = ws
            .tabs
            .iter()
            .map(|t| (t.id.clone(), t.title.clone(), t.icon.clone()))
            .collect();

        let workspace_state = self.workspace_state.clone();

        h_flex()
            .size_full()
            .child(
                Sidebar::new(active_idx, tab_info).on_switch_view(move |idx, _window, cx| {
                    workspace_state.update(cx, |ws, cx| {
                        ws.switch_view(idx, cx);
                    });
                }),
            )
            .child(active_view)
    }
}
