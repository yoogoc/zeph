use crate::containers::WorkspaceArea;
use crate::state::WorkspaceState;
use gpui::{AnyView, AppContext as _, Context, SharedString, Window};
use models::{CreateWorkspace, Workspace};
use register::GlobalStorage;
use uuid::Uuid;

pub struct ApplicationState {
    pub workspaces: Vec<Workspace>,
    pub tabs: Vec<Tab>,
    pub active_idx: usize,
    pub next_id: usize,
}

pub struct Tab {
    pub id: Uuid,
    pub title: SharedString,
    pub view: AnyView,
}

impl ApplicationState {
    pub fn new() -> Self {
        Self {
            workspaces: vec![],
            tabs: vec![],
            active_idx: 0,
            next_id: 0,
        }
    }

    pub fn init(&mut self, cx: &mut Context<Self>, tab: Tab) {
        self.tabs.clear();
        self.tabs.push(tab);
        let workspaces = GlobalStorage::global(cx)
            .workspace
            .list()
            .inspect_err(|err| log::error!("{:?}", err))
            .unwrap_or(Vec::new());

        self.workspaces = workspaces;
    }

    /// Creates a new workspace (persists it, adds it to the list) and opens it.
    pub fn create_workspace(
        &mut self,
        param: CreateWorkspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let workspace = Workspace::new(param.name);
        let id = workspace.id;

        if let Err(e) = GlobalStorage::global(cx).workspace.insert(&workspace) {
            log::error!("Failed to create workspace: {e}");
            return;
        }
        // TODO: seed sample services when `param.create_sample_services` is set
        // (no sample-service generator exists yet).

        self.workspaces.push(workspace);
        self.open_workspace(id, window, cx);
        cx.notify();
    }

    pub fn open_workspace(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.active_idx = pos;
            return;
        }

        let title = self
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .map(|w| SharedString::from(w.name.clone()))
            .unwrap_or_else(|| "Workspace".into());

        let workspace_state = cx.new(|cx| WorkspaceState::new(id, window, cx));
        let view = cx.new(|_| WorkspaceArea::new(workspace_state));

        self.tabs.push(Tab {
            id,
            title,
            view: view.into(),
        });
        self.active_idx = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self, id: Uuid) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(pos);
            if self.active_idx >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_idx = self.tabs.len() - 1;
            }
        }
    }

    pub fn activate(&mut self, id: Uuid) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.active_idx = pos;
        }
    }

    pub fn delete_workspace(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        todo!()
    }
}
