use crate::pages::workspace_card::WorkspaceCard;
use crate::state::ApplicationState;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::{IconName, StyledExt, WindowExt};
use models::CreateWorkspace;

pub enum HomePageEvent {
    CreateWorkspace(CreateWorkspace),
}

pub struct HomePage {
    workspace_state: Entity<ApplicationState>,
}

impl EventEmitter<HomePageEvent> for HomePage {}

impl HomePage {
    pub fn new(workspace_state: Entity<ApplicationState>) -> Self {
        Self { workspace_state }
    }

    fn open_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Workspace name"));
        let create_sample = std::rc::Rc::new(std::cell::Cell::new(true));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();
            let create_sample_clone = create_sample.clone();
            let create_sample_for_checkbox = create_sample.clone();

            dialog
                .title(shared_t!("workspaces.create"))
                .w(px(400.0))
                .child(
                    div()
                        .v_flex()
                        .gap_3()
                        .child(Input::new(&input).w_full())
                        .child(
                            Checkbox::new("create_sample_services")
                                .label(shared_t!("workspaces.create_sample_services"))
                                .checked(create_sample_for_checkbox.get())
                                .on_click(move |checked, _, _| {
                                    create_sample_for_checkbox.set(*checked);
                                }),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            let create_sample_services = create_sample_clone.get();
                            this.update(cx, |_, cx| {
                                cx.emit(HomePageEvent::CreateWorkspace(CreateWorkspace {
                                    name,
                                    create_sample_services,
                                }));
                            });
                        }
                    }
                    window.close_dialog(cx);
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
