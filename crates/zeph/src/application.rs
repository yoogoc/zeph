use gpui::*;
use gpui_component::{button::{Button, ButtonVariants}, *};
use models::ApplicationView;

pub fn init(window: &mut Window, cx: &mut App) -> Entity<Application> {
    cx.new(|cx| Application::new(window, cx))
}

#[derive(Debug)]
pub struct Application {
    title_bar: Entity<ui::TitleBar>,
    workspace_area: Entity<ui::WorkspaceArea>,
}

impl Application {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            title_bar: cx.new(|_| ui::TitleBar::new()),
            workspace_area: cx.new(|_| ui::WorkspaceArea::new()),
        }
    }
    
    fn render_home(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .items_center()
                    .child(
                        div()
                            .text_2xl()
                            .font_bold()
                            .child("Welcome to Zeph")
                    )
                    .child("Your API testing companion")
                    .child(
                        Button::new("create_workspace")
                            .primary()
                            .label("Create Workspace")
                            .on_click(|_, _, _| println!("Create workspace clicked"))
                    )
            )
    }
    
    fn render_workspace(&self, _cx: &mut Context<Self>) -> Entity<ui::WorkspaceArea> {
        // Get workspace from registry and update workspace_area
        self.workspace_area.clone()
    }
}

impl Render for Application {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace_registry = workspace::WorkspaceRegistry::global(cx);
        let app_state = workspace_registry.read(cx).get_state();
        
        div()
            .v_flex()
            .size_full()
            .child(self.title_bar.clone())
            .child(
                match &app_state.current_view {
                    ApplicationView::Home => self.render_home(cx).into_any_element(),
                    ApplicationView::Workspace(_) => self.render_workspace(cx).into_any_element(),
                }
            )
    }
}
