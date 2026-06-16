use crate::pages::workspace_card::WorkspaceCard;
use crate::state::ApplicationState;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::dialog::DialogButtonProps;
use gpui_component::input::{Input, InputState};
use gpui_component::{IconName, StyledExt, WindowExt};
use models::CreateWorkspace;

pub struct HomePage {
    workspace_state: Entity<ApplicationState>,
}

impl HomePage {
    pub fn new(workspace_state: Entity<ApplicationState>) -> Self {
        Self { workspace_state }
    }

    fn open_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace_state = self.workspace_state.clone();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Workspace name"));
        let create_sample = std::rc::Rc::new(std::cell::Cell::new(true));

        // Use an alert dialog so the OK/Cancel action buttons render (a bare
        // `Dialog` is only a content container and shows no buttons).
        window.open_alert_dialog(cx, move |alert, _window, _cx| {
            let input_body = input.clone();
            let input_ok = input.clone();
            let workspace_state = workspace_state.clone();
            let create_sample_clone = create_sample.clone();
            let create_sample_for_checkbox = create_sample.clone();

            alert
                .title(shared_t!("workspaces.create"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(shared_t!("workspaces.create"))
                        .cancel_text(shared_t!("common.cancel"))
                        .show_cancel(true),
                )
                .content(move |content, _window, _cx| {
                    content.child(
                        div()
                            .v_flex()
                            .gap_3()
                            .child(Input::new(&input_body).w_full())
                            .child(
                                Checkbox::new("create_sample_services")
                                    .label(shared_t!("workspaces.create_sample_services"))
                                    .checked(create_sample_for_checkbox.get())
                                    .on_click({
                                        let create_sample_for_checkbox =
                                            create_sample_for_checkbox.clone();
                                        move |checked, _, _| {
                                            create_sample_for_checkbox.set(*checked);
                                        }
                                    }),
                            ),
                    )
                })
                .on_ok(move |_, window, cx| {
                    let name = input_ok.read(cx).value();
                    if name.trim().is_empty() {
                        return false; // keep open until a name is provided
                    }
                    let create_sample_services = create_sample_clone.get();
                    workspace_state.update(cx, |ws, cx| {
                        ws.create_workspace(
                            CreateWorkspace {
                                name,
                                create_sample_services,
                            },
                            window,
                            cx,
                        );
                    });
                    true
                })
        });
    }
}

impl Render for HomePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspaces = self.workspace_state.read(cx).workspaces.clone();
        let workspace_state = self.workspace_state.clone();

        div()
            .size_full()
            .bg(rgb(0xf8f9fa))
            .py_4()
            .px_8()
            .v_flex()
            .gap_6()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_bold()
                                    .child(shared_t!("welcome.title")),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x6c757d))
                                    .child(shared_t!("welcome.description")),
                            ),
                    )
                    .child(
                        Button::new("create_workspace")
                            .primary()
                            .icon(IconName::Plus)
                            .label(shared_t!("workspaces.create"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_create_dialog(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .child(div().h_flex().flex_wrap().gap_4().children(
                        workspaces.into_iter().map(|w| {
                            let ws = workspace_state.clone();
                            let ws2 = workspace_state.clone();
                            let id = w.id;

                            WorkspaceCard::new(w)
                                .on_open(move |_, window, cx| {
                                    ws.update(cx, |ts, cx| {
                                        ts.open_workspace(id, window, cx);
                                    });
                                })
                                .on_delete(move |_, window, cx| {
                                    ws2.update(cx, |ts, cx| {
                                        ts.delete_workspace(id, window, cx);
                                    });
                                })
                        }),
                    )),
            )
    }
}
