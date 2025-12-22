use gpui::*;
use gpui_component::*;
use models::{Workspace, WorkspaceView};
use crate::{Sidebar, WorkspaceContent};

pub struct WorkspaceArea {
    sidebar: Entity<Sidebar>,
    content: Entity<WorkspaceContent>,
}

impl WorkspaceArea {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Subscribe to workspace registry changes
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        cx.subscribe(&workspace_registry, |this, registry, _event: &workspace::WorkspaceEvent, cx| {
            let workspace = registry.read(cx).get_active_workspace().cloned();
            this.set_workspace(workspace, cx);
        }).detach();

        Self {
            sidebar: cx.new(|_| Sidebar::new(None, WorkspaceView::Collections)),
            content: cx.new(|_| WorkspaceContent::new(None, WorkspaceView::Collections)),
        }
    }

    pub fn set_workspace(&mut self, workspace: Option<Workspace>, cx: &mut Context<Self>) {
        let (id, view) = workspace
            .map(|w| (Some(w.id), w.current_view))
            .unwrap_or((None, WorkspaceView::Collections));
        
        self.sidebar.update(cx, |s, _| s.set_view(view.clone(), id));
        self.content.update(cx, |c, cx| c.set_view(view, id, cx));
    }
}

impl Render for WorkspaceArea {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .size_full()
            .child(self.sidebar.clone())
            .child(self.content.clone())
    }
}
