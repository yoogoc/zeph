use gpui::*;
use gpui_component::{Root, StyledExt};
use models::{ApplicationView, CreateWorkspace};
use ui::{HomePage, HomePageEvent};
use uuid::Uuid;

pub fn init(window: &mut Window, cx: &mut App) -> Entity<Application> {
    cx.new(|cx| Application::new(window, cx))
}

pub struct Application {
    title_bar: Entity<ui::TitleBar>,
    workspace_area: Entity<ui::WorkspaceArea>,
    home_page: Entity<HomePage>,
    _subscriptions: Vec<Subscription>,
}

impl Application {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let home_page = cx.new(|_| HomePage::new());

        let home_sub = cx.subscribe(&home_page, |this, _, event, cx| match event {
            HomePageEvent::CreateWorkspace(param) => this.create_workspace(param.clone(), cx),
            HomePageEvent::OpenWorkspace(id) => this.open_workspace(*id, cx),
            HomePageEvent::DeleteWorkspace(id) => this.delete_workspace(*id, cx),
        });

        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let registry_sub = cx.subscribe(
            &workspace_registry,
            |_this, _registry, _event: &workspace::WorkspaceEvent, cx| {
                cx.notify();
            },
        );

        Self {
            title_bar: cx.new(|cx| ui::TitleBar::new(window, cx)),
            workspace_area: cx.new(|cx| ui::WorkspaceArea::new(window, cx)),
            home_page,
            _subscriptions: vec![home_sub, registry_sub],
        }
    }

    fn create_workspace(&mut self, param: CreateWorkspace, cx: &mut Context<Self>) {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        workspace_registry.update(cx, |registry, cx| {
            registry.create_workspace(param, cx);
            if let Some(new_workspace) = registry.get_state().workspaces.last() {
                registry.open_workspace(new_workspace.id, cx);
            }
        });
    }

    fn open_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        workspace_registry.update(cx, |registry, cx| {
            registry.open_workspace(workspace_id, cx);
        });
    }

    fn delete_workspace(&mut self, workspace_id: Uuid, cx: &mut Context<Self>) {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        workspace_registry.update(cx, |registry, cx| {
            registry.remove_workspace(workspace_id, cx);
        });
    }

    fn render_home(&mut self, cx: &mut Context<Self>) -> Entity<HomePage> {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let workspaces = workspace_registry.read(cx).get_state().workspaces.clone();

        self.home_page.update(cx, |page, _| {
            page.set_workspaces(workspaces);
        });

        self.home_page.clone()
    }

    fn render_workspace(&self, cx: &mut Context<Self>) -> Entity<ui::WorkspaceArea> {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let workspace = workspace_registry.read(cx).get_active_workspace().cloned();

        self.workspace_area.update(cx, |area, cx| {
            area.set_workspace(workspace, cx);
        });

        self.workspace_area.clone()
    }
}

impl Render for Application {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let app_state = workspace_registry.read(cx).get_state();

        div()
            .v_flex()
            .size_full()
            .child(self.title_bar.clone())
            .child(match &app_state.current_view {
                ApplicationView::Home => self.render_home(cx).into_any_element(),
                ApplicationView::Workspace(_) => self.render_workspace(cx).into_any_element(),
            })
            .children(dialog_layer)
    }
}
