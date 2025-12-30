use crate::{Sidebar, SidebarEvent, WorkspaceContent};
use gpui::*;
use gpui_component::h_flex;
use models::{Workspace, WorkspaceView};
use workspace::WorkspaceEvent;

pub struct WorkspaceArea {
    sidebar: Entity<Sidebar>,
    content: Entity<WorkspaceContent>,
    workspace_id: Option<uuid::Uuid>,
    _subscriptions: Vec<Subscription>,
}

impl WorkspaceArea {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|_| Sidebar::new(None, WorkspaceView::Services));
        let content = cx.new(|cx| WorkspaceContent::new(None, window, cx));

        let sidebar_sub = cx.subscribe(&sidebar, |this, _, event, cx| {
            let SidebarEvent::SwitchView(view) = event;
            this.switch_view(view.clone(), cx);
        });

        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let registry_sub = cx.subscribe(
            &workspace_registry,
            |this, _registry, event: &workspace::WorkspaceEvent, cx| match event {
                WorkspaceEvent::Open(workspace) => this.set_workspace(Some(workspace.clone()), cx),
                WorkspaceEvent::Close => this.set_workspace(None, cx),
                _ => {}
            },
        );

        Self {
            sidebar,
            content,
            workspace_id: None,
            _subscriptions: vec![sidebar_sub, registry_sub],
        }
    }

    fn switch_view(&mut self, view: WorkspaceView, cx: &mut Context<Self>) {
        if let Some(id) = self.workspace_id {
            let registry = workspace::WorkspaceRegistry::global(cx);
            registry.update(cx, |r, cx| r.set_workspace_view(id, view, cx));
        }
    }

    pub fn set_workspace(&mut self, workspace: Option<Workspace>, cx: &mut Context<Self>) {
        let (id, view) = workspace
            .map(|w| (Some(w.id), w.current_view))
            .unwrap_or((None, WorkspaceView::Services));

        self.workspace_id = id;
        self.sidebar.update(cx, |s, _| s.set_view(view.clone(), id));
        self.content.update(cx, |c, cx| c.set_view(view, id, cx));
    }
}

impl Render for WorkspaceArea {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(self.sidebar.clone())
            .child(self.content.clone())
    }
}
