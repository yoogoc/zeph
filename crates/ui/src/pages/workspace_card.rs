use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{h_flex, IconName, StyledExt};
use models::Workspace;

#[derive(IntoElement)]
pub struct WorkspaceCard {
    workspace: Workspace,
    on_open: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    on_delete: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl WorkspaceCard {
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            on_open: None,
            on_delete: None,
        }
    }

    pub fn on_open(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Box::new(handler));
        self
    }

    pub fn on_delete(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for WorkspaceCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let workspace_id = self.workspace.id;
        let created = chrono::DateTime::from_timestamp(self.workspace.created_at, 0)
            .map(|dt: chrono::DateTime<chrono::Utc>| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default();

        let mut card = div()
            .id(format!("workspace_card_{}", workspace_id))
            .w(px(280.0))
            .p_4()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xe0e0e0))
            .rounded_lg()
            .shadow_sm()
            .cursor_pointer()
            .hover(|s| s.border_color(rgb(0x007bff)).shadow_md());

        if let Some(on_open) = self.on_open {
            card = card.on_click(move |event, window, cx| {
                on_open(event, window, cx);
            });
        }

        let mut delete_btn = Button::new(format!("delete_{}", workspace_id))
            .ghost()
            .compact()
            .icon(IconName::Close);

        if let Some(on_delete) = self.on_delete {
            delete_btn = delete_btn.on_click(move |event, window, cx| {
                cx.stop_propagation();
                on_delete(event, window, cx);
            });
        }

        card.child(
            div()
                .v_flex()
                .gap_3()
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    h_flex()
                                        .w(px(40.0))
                                        .h(px(40.0))
                                        .rounded_md()
                                        .bg(rgb(0x007bff))
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            gpui_component::Icon::new(IconName::Folder)
                                                .size_5()
                                                .text_color(rgb(0xffffff)),
                                        ),
                                )
                                .child(
                                    div()
                                        .v_flex()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_semibold()
                                                .child(self.workspace.name.clone()),
                                        )
                                        .child(div().text_xs().text_color(rgb(0x6c757d)).child(
                                            shared_t!("workspaces.created_at", at = created),
                                        )),
                                ),
                        )
                        .child(delete_btn),
                )
                .child(
                    div().text_sm().text_color(rgb(0x6c757d)).child(
                        self.workspace
                            .description
                            .clone()
                            .unwrap_or_else(|| t!("workspaces.default_description").into()),
                    ),
                ),
        )
    }
}
