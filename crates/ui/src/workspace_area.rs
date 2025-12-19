use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::{button::{Button, ButtonVariants}, *};
use models::{Workspace, WorkspaceView};
use crate::CollectionsView;

pub struct WorkspaceArea {
    workspace: Option<Workspace>,
    collections_view: Option<Entity<CollectionsView>>,
}

impl WorkspaceArea {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { 
            workspace: None,
            collections_view: None,
        }
    }
    
    pub fn set_workspace(&mut self, workspace: Option<Workspace>) {
        self.workspace = workspace;
        // Reset collections view when workspace changes
        self.collections_view = None;
    }
    
    fn switch_workspace_view(&mut self, view: WorkspaceView, cx: &mut Context<Self>) {
        if let Some(workspace) = &self.workspace {
            let workspace_registry = workspace::WorkspaceRegistry::global(cx);
            workspace_registry.update(cx, |registry, cx| {
                registry.set_workspace_view(workspace.id, view, cx);
            });
        }
    }
    
    fn render_sidebar_button(&mut self, 
        id: String, 
        icon: String, 
        tooltip: String, 
        is_active: bool, 
        view: WorkspaceView, 
        cx: &mut Context<Self>
    ) -> impl IntoElement {
        Button::new(id)
            .w(px(48.0))
            .h(px(48.0))
            .label(icon)
            .tooltip(tooltip)
            .when(is_active, |btn: Button| btn.primary())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.switch_workspace_view(view.clone(), cx);
            }))
    }
    
    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_view = self.workspace.as_ref()
            .map(|w| w.current_view.clone())
            .unwrap_or(WorkspaceView::Collections);
            
        div()
            .w(px(60.0))
            .h_full()
            .bg(gpui::rgb(0xf5f5f5))
            .border_r_1()
            .border_color(gpui::rgb(0xe0e0e0))
            .p_2()
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        self.render_sidebar_button(
                            "collections".to_string(),
                            "📁".to_string(),
                            "Collections".to_string(),
                            matches!(current_view, WorkspaceView::Collections),
                            WorkspaceView::Collections,
                            cx
                        )
                    )
                    .child(
                        self.render_sidebar_button(
                            "docs".to_string(),
                            "📖".to_string(),
                            "Documentation".to_string(),
                            matches!(current_view, WorkspaceView::Docs),
                            WorkspaceView::Docs,
                            cx
                        )
                    )
                    .child(
                        self.render_sidebar_button(
                            "history".to_string(),
                            "🕒".to_string(),
                            "History".to_string(),
                            matches!(current_view, WorkspaceView::History),
                            WorkspaceView::History,
                            cx
                        )
                    )
                    .child(
                        self.render_sidebar_button(
                            "settings".to_string(),
                            "⚙️".to_string(),
                            "Settings".to_string(),
                            matches!(current_view, WorkspaceView::Settings),
                            WorkspaceView::Settings,
                            cx
                        )
                    )
            )
    }
    
    fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(workspace) = &self.workspace {
            match workspace.current_view {
                WorkspaceView::Collections => self.render_collections(cx).into_any_element(),
                WorkspaceView::Docs => self.render_docs().into_any_element(),
                WorkspaceView::History => self.render_history().into_any_element(),
                WorkspaceView::Settings => self.render_settings().into_any_element(),
            }
        } else {
            div().child("No workspace selected").into_any_element()
        }
    }
    
    fn render_collections(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace_id = self.workspace.as_ref().map(|w| w.id);
        
        // Always recreate collections view when workspace changes or doesn't exist
        if self.collections_view.is_none() || 
           (workspace_id.is_some() && self.collections_view.is_some()) {
            
            self.collections_view = Some(cx.new(|_| CollectionsView::new(workspace_id, None)));
        }
        
        self.collections_view.as_ref().unwrap().clone()
    }
    
    fn render_docs(&self) -> impl IntoElement {
        div()
            .flex_1()
            .p_6()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .text_2xl()
                            .font_bold()
                            .child("Documentation")
                    )
                    .child(
                        div()
                            .p_6()
                            .bg(gpui::rgb(0xfafafa))
                            .border_1()
                            .border_color(gpui::rgb(0xe0e0e0))
                            .rounded_lg()
                            .child("API documentation will be displayed here")
                    )
            )
    }
    
    fn render_history(&self) -> impl IntoElement {
        div()
            .flex_1()
            .p_6()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .text_2xl()
                            .font_bold()
                            .child("History")
                    )
                    .child(
                        div()
                            .p_6()
                            .bg(gpui::rgb(0xfafafa))
                            .border_1()
                            .border_color(gpui::rgb(0xe0e0e0))
                            .rounded_lg()
                            .child("Request history will be displayed here")
                    )
            )
    }
    
    fn render_settings(&self) -> impl IntoElement {
        div()
            .flex_1()
            .p_6()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .text_2xl()
                            .font_bold()
                            .child("Settings")
                    )
                    .child(
                        div()
                            .p_6()
                            .bg(gpui::rgb(0xfafafa))
                            .border_1()
                            .border_color(gpui::rgb(0xe0e0e0))
                            .rounded_lg()
                            .child("Workspace settings will be displayed here")
                    )
            )
    }
}

impl Render for WorkspaceArea {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .size_full()
            .child(self.render_sidebar(cx))
            .child(self.render_content(cx))
    }
}
