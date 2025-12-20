use gpui::*;
use gpui_component::StyledExt;
use models::WorkspaceView;
use uuid::Uuid;
use crate::CollectionsView;

pub struct WorkspaceContent {
    current_view: WorkspaceView,
    workspace_id: Option<Uuid>,
    collections_view: Option<Entity<CollectionsView>>,
}

impl WorkspaceContent {
    pub fn new(workspace_id: Option<Uuid>, current_view: WorkspaceView) -> Self {
        Self { current_view, workspace_id, collections_view: None }
    }

    pub fn set_view(&mut self, view: WorkspaceView, workspace_id: Option<Uuid>, cx: &mut Context<Self>) {
        self.current_view = view;
        if self.workspace_id != workspace_id {
            self.workspace_id = workspace_id;
            self.collections_view = None;
        }
        cx.notify();
    }

    fn render_collections(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.collections_view.is_none() {
            self.collections_view = Some(cx.new(|_| CollectionsView::new(self.workspace_id, None)));
        }
        self.collections_view.as_ref().unwrap().clone()
    }

    fn render_placeholder(&self, title: &str, desc: &str) -> impl IntoElement {
        div()
            .flex_1()
            .p_6()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .child(div().text_2xl().font_bold().child(title.to_string()))
                    .child(
                        div()
                            .p_6()
                            .bg(gpui::rgb(0xfafafa))
                            .border_1()
                            .border_color(gpui::rgb(0xe0e0e0))
                            .rounded_lg()
                            .child(desc.to_string())
                    )
            )
    }
}

impl Render for WorkspaceContent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.workspace_id.is_none() {
            return div().child("No workspace selected").into_any_element();
        }
        match self.current_view {
            WorkspaceView::Collections => self.render_collections(cx).into_any_element(),
            WorkspaceView::Docs => self.render_placeholder("Documentation", "API documentation will be displayed here").into_any_element(),
            WorkspaceView::History => self.render_placeholder("History", "Request history will be displayed here").into_any_element(),
            WorkspaceView::Settings => self.render_placeholder("Settings", "Workspace settings will be displayed here").into_any_element(),
        }
    }
}
