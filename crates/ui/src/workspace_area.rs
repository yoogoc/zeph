use gpui::*;
use gpui_component::{button::Button, *};
use models::Workspace;

pub struct WorkspaceArea {
    workspace: Option<Workspace>,
}

impl WorkspaceArea {
    pub fn new() -> Self {
        Self { workspace: None }
    }
    
    pub fn set_workspace(&mut self, workspace: Option<Workspace>) {
        self.workspace = workspace;
    }
    
    fn render_sidebar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(250.0))
            .h_full()
            .bg(gpui::rgb(0xf5f5f5))
            .border_r_1()
            .border_color(gpui::rgb(0xe0e0e0))
            .p_4()
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        Button::new("collections")
                            .w_full()
                            .label("Collections")
                            .on_click(|_, _, _| println!("Collections clicked"))
                    )
                    .child(
                        Button::new("docs")
                            .w_full()
                            .label("Docs")
                            .on_click(|_, _, _| println!("Docs clicked"))
                    )
                    .child(
                        Button::new("history")
                            .w_full()
                            .label("History")
                            .on_click(|_, _, _| println!("History clicked"))
                    )
                    .child(
                        Button::new("settings")
                            .w_full()
                            .label("Settings")
                            .on_click(|_, _, _| println!("Settings clicked"))
                    )
            )
    }
    
    fn render_content(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .p_4()
            .child(
                if let Some(workspace) = &self.workspace {
                    div()
                        .v_flex()
                        .gap_4()
                        .child(
                            div()
                                .text_xl()
                                .font_semibold()
                                .child(format!("Workspace: {}", workspace.name))
                        )
                        .child("Content area - Collections, Docs, History, Settings will be displayed here")
                } else {
                    div().child("No workspace selected")
                }
            )
    }
}

impl Default for WorkspaceArea {
    fn default() -> Self {
        Self::new()
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
