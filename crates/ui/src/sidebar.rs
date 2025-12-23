use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::StyledExt;
use models::WorkspaceView;
use uuid::Uuid;

pub struct Sidebar {
    current_view: WorkspaceView,
    workspace_id: Option<Uuid>,
}

pub enum SidebarEvent {
    SwitchView(WorkspaceView),
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    pub fn new(workspace_id: Option<Uuid>, current_view: WorkspaceView) -> Self {
        Self { current_view, workspace_id }
    }

    pub fn set_view(&mut self, view: WorkspaceView, workspace_id: Option<Uuid>) {
        self.current_view = view;
        self.workspace_id = workspace_id;
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let buttons: Vec<(&str, &str, &str, WorkspaceView)> = vec![
            ("services", "📁", "Services", WorkspaceView::Services),
            ("docs", "📖", "Documentation", WorkspaceView::Docs),
            ("history", "🕒", "History", WorkspaceView::History),
            ("settings", "⚙️", "Settings", WorkspaceView::Settings),
        ];

        div()
            .w(px(60.0))
            .h_full()
            .bg(rgb(0xf5f5f5))
            .border_r_1()
            .border_color(rgb(0xe0e0e0))
            .p_2()
            .child(
                div().v_flex().gap_2().children(buttons.into_iter().map(|(id, icon, tooltip, view)| {
                    let is_active = std::mem::discriminant(&self.current_view) == std::mem::discriminant(&view);
                    Button::new(id)
                        .w(px(48.0))
                        .h(px(48.0))
                        .label(icon)
                        .tooltip(tooltip)
                        .when(is_active, |btn: Button| btn.primary())
                        .on_click(cx.listener(move |_, _, _, cx| cx.emit(SidebarEvent::SwitchView(view.clone()))))
                })),
            )
    }
}
