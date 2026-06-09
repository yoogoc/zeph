use gpui::prelude::*;
use gpui::*;
use gpui_component::{v_flex, Root};
use i18n::shared_t;
use models::CreateWorkspace;
use ui::state::{ApplicationState, Tab};
use ui::HomePage;
use uuid::Uuid;

pub fn init(window: &mut Window, cx: &mut App) -> Entity<Application> {
    cx.new(|cx| Application::new(window, cx))
}

pub struct Application {
    title_bar: Entity<ui::TitleBar>,

    workspace_state: Entity<ApplicationState>,

    _subscriptions: Vec<Subscription>,
}

impl Application {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace_state = cx.new(|_| ApplicationState::new());

        let ws = workspace_state.clone();
        let home_page = cx.new(|_| HomePage::new(ws));

        workspace_state.update(cx, |ws, cx| {
            ws.init(
                cx,
                Tab {
                    id: Default::default(),
                    title: shared_t!("common.home"),
                    view: home_page.into(),
                },
            );
        });

        // let home_sub = cx.subscribe(&home_page, |this, _, event, cx| match event {
        //     HomePageEvent::CreateWorkspace(param) => this.create_workspace(param.clone(), cx),
        //     HomePageEvent::OpenWorkspace(id) => this.open_workspace(*id, cx),
        //     HomePageEvent::DeleteWorkspace(id) => this.delete_workspace(*id, cx),
        // });

        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let registry_sub = cx.subscribe(
            &workspace_registry,
            |_this, _registry, _event: &workspace::WorkspaceEvent, cx| {
                cx.notify();
            },
        );

        let ws2 = workspace_state.clone();
        Self {
            title_bar: cx.new(|cx| ui::TitleBar::new(window, cx, ws2)),
            // workspace_area: cx.new(|cx| ui::WorkspaceArea::new(window, cx)),
            workspace_state,
            _subscriptions: vec![registry_sub],
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

    // fn render_workspace(&self, cx: &mut Context<Self>) -> Entity<ui::WorkspaceArea> {
    //     let workspace_registry = workspace::WorkspaceRegistry::global(cx);
    //     let workspace = workspace_registry.read(cx).get_active_workspace().cloned();
    //
    //     self.workspace_area.update(cx, |area, cx| {
    //         area.set_workspace(workspace, cx);
    //     });
    //
    //     self.workspace_area.clone()
    // }
}

impl Render for Application {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        // let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        // let app_state = workspace_registry.read(cx).get_state();

        v_flex()
            .size_full()
            .child(self.title_bar.clone())
            .when_some(
                {
                    let ws = self.workspace_state.read(cx);
                    ws.tabs.get(ws.active_idx).map(|t| t.view.clone())
                },
                |d, view| d.child(view),
            )
            .children(dialog_layer)
    }
}
