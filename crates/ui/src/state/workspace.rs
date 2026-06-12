use crate::containers::WorkspaceContent;
use crate::state::ServiceState;
use gpui::*;
use gpui_component::IconName;
use uuid::Uuid;

struct PlaceholderView {
    label: SharedString,
}

impl Render for PlaceholderView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().child(self.label.clone())
    }
}

pub struct ViewTab {
    pub id: SharedString,
    pub title: SharedString,
    pub icon: IconName,
    pub view: AnyView,
}

pub struct WorkspaceState {
    pub workspace_id: Uuid,
    pub tabs: Vec<ViewTab>,
    pub active_idx: usize,
}

impl WorkspaceState {
    pub fn new(workspace_id: Uuid, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let service_state = cx.new(|cx| ServiceState::new(workspace_id, cx));
        let content = cx.new(|cx| WorkspaceContent::new(service_state, window, cx));

        let tabs = vec![
            ViewTab {
                id: "services".into(),
                title: shared_t!("sidebar.services"),
                icon: IconName::Folder,
                view: content.into(),
            },
            ViewTab {
                id: "docs".into(),
                title: shared_t!("sidebar.docs"),
                icon: IconName::BookOpen,
                view: cx
                    .new(|_| PlaceholderView {
                        label: "Documentation - coming soon".into(),
                    })
                    .into(),
            },
            ViewTab {
                id: "history".into(),
                title: shared_t!("sidebar.history"),
                icon: IconName::Eye,
                view: cx
                    .new(|_| PlaceholderView {
                        label: "History - coming soon".into(),
                    })
                    .into(),
            },
            ViewTab {
                id: "settings".into(),
                title: shared_t!("sidebar.settings"),
                icon: IconName::Settings,
                view: cx
                    .new(|_| PlaceholderView {
                        label: "Settings - coming soon".into(),
                    })
                    .into(),
            },
        ];

        Self {
            workspace_id,
            tabs,
            active_idx: 0,
        }
    }

    pub fn switch_view(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.tabs.len() {
            self.active_idx = idx;
            cx.notify();
        }
    }
}
