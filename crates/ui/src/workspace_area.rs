use gpui::*;
use gpui_component::{button::{Button, ButtonVariants}, *};
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
    
    fn go_back_to_home(&mut self, cx: &mut Context<Self>) {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        workspace_registry.update(cx, |registry, cx| {
            registry.go_to_home(cx);
        });
    }
    
    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
                        Button::new("back_home")
                            .w_full()
                            .label("← Back to Home")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.go_back_to_home(cx);
                            }))
                    )
                    .child(div().h_px().bg(gpui::rgb(0xe0e0e0)).my_2()) // Divider
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
                                .text_2xl()
                                .font_bold()
                                .child(format!("Workspace: {}", workspace.name))
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(gpui::rgb(0x666666))
                                .child(format!("Created: {}", workspace.created_at.format("%Y-%m-%d %H:%M")))
                        )
                        .child(
                            div()
                                .mt_8()
                                .p_6()
                                .bg(gpui::rgb(0xfafafa))
                                .border_1()
                                .border_color(gpui::rgb(0xe0e0e0))
                                .rounded_lg()
                                .child("Content area - Collections, Docs, History, Settings will be displayed here")
                        )
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
