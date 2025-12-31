use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{h_flex, IconName, StyledExt, WindowExt};
use gpui_component::checkbox::Checkbox;
use models::{CreateWorkspace, Workspace};
use uuid::Uuid;

pub enum HomePageEvent {
    CreateWorkspace(CreateWorkspace),
    OpenWorkspace(Uuid),
    DeleteWorkspace(Uuid),
}

pub struct HomePage {
    workspaces: Vec<Workspace>,
}

impl EventEmitter<HomePageEvent> for HomePage {}

impl HomePage {
    pub fn new() -> Self {
        Self { workspaces: vec![] }
    }

    pub fn set_workspaces(&mut self, workspaces: Vec<Workspace>) {
        self.workspaces = workspaces;
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
                .confirm()
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
                                })
                        )
                )
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            let create_sample_services = create_sample_clone.get();
                            this.update(cx, |_, cx| {
                                cx.emit(HomePageEvent::CreateWorkspace(CreateWorkspace { 
                                    name, 
                                    create_sample_services 
                                }));
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }

    fn render_workspace_card(&self, workspace: &Workspace, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace_id = workspace.id;
        let created = chrono::DateTime::from_timestamp(workspace.created_at, 0)
            .map(|dt: chrono::DateTime<chrono::Utc>| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default();

        div()
            .id(format!("workspace_card_{}", workspace_id))
            .w(px(280.0))
            .p_4()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xe0e0e0))
            .rounded_lg()
            .shadow_sm()
            .cursor_pointer()
            .hover(|s| s.border_color(rgb(0x007bff)).shadow_md())
            .on_click(cx.listener(move |_this, _, _, cx| {
                cx.emit(HomePageEvent::OpenWorkspace(workspace_id));
            }))
            .child(
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
                                                    .child(workspace.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x6c757d))
                                                    .child(shared_t!("workspaces.created_at", at = created)),
                                            ),
                                    ),
                            )
                            .child(
                                Button::new(format!("delete_{}", workspace_id))
                                    .ghost()
                                    .compact()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(move |_this, _, _, cx| {
                                        cx.emit(HomePageEvent::DeleteWorkspace(workspace_id));
                                        cx.stop_propagation();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x6c757d))
                            .child(
                                workspace
                                    .description
                                    .clone()
                                    .unwrap_or_else(|| t!("workspaces.default_description").into()),
                            ),
                    ),
            )
    }
}

impl Render for HomePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .child(div().text_2xl().font_bold().child(shared_t!("welcome.title")))
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
                    .child(
                        div()
                            .h_flex()
                            .flex_wrap()
                            .gap_4()
                            .children(
                                self.workspaces
                                    .iter()
                                    .map(|w| self.render_workspace_card(w, cx)),
                            ),
                    ),
            )
    }
}
