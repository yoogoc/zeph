use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, StyledExt, WindowExt};
use models::{Environment, EnvironmentVariable, Workspace};
use uuid::Uuid;

use crate::dialogs::{EnvironmentManager, EnvironmentManagerEvent};

pub enum EnvironmentSelectorEvent {
    SelectEnvironment(Uuid),
    CreateEnvironment(String),
    UpdateEnvironment(Environment),
    DeleteEnvironment(Uuid),
    AddVariable(Uuid, EnvironmentVariable),
    UpdateVariable(EnvironmentVariable),
    DeleteVariable(Uuid),
}

pub struct EnvironmentSelector {
    workspace: Option<Workspace>,
}

impl EventEmitter<EnvironmentSelectorEvent> for EnvironmentSelector {}

impl EnvironmentSelector {
    pub fn new() -> Self {
        Self { workspace: None }
    }

    pub fn set_workspace(&mut self, workspace: Workspace) {
        self.workspace = Some(workspace);
    }

    fn open_manage_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let environments = workspace.environments.clone();

        let manager = cx.new(|_| EnvironmentManager::new(environments));

        cx.subscribe(
            &manager,
            |_this, _, event: &EnvironmentManagerEvent, cx| match event {
                EnvironmentManagerEvent::Create(name) => {
                    cx.emit(EnvironmentSelectorEvent::CreateEnvironment(name.clone()));
                }
                EnvironmentManagerEvent::Update(env) => {
                    cx.emit(EnvironmentSelectorEvent::UpdateEnvironment(env.clone()));
                }
                EnvironmentManagerEvent::Delete(id) => {
                    cx.emit(EnvironmentSelectorEvent::DeleteEnvironment(*id));
                }
                EnvironmentManagerEvent::AddVariable(env_id, var) => {
                    cx.emit(EnvironmentSelectorEvent::AddVariable(*env_id, var.clone()));
                }
                EnvironmentManagerEvent::UpdateVariable(var) => {
                    cx.emit(EnvironmentSelectorEvent::UpdateVariable(var.clone()));
                }
                EnvironmentManagerEvent::DeleteVariable(id) => {
                    cx.emit(EnvironmentSelectorEvent::DeleteVariable(*id));
                }
            },
        )
        .detach();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(shared_t!("env.title"))
                .w(px(800.0))
                .child(manager.clone())
        });
    }

    fn open_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Environment name"));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let this_clone = this.clone();

            dialog
                .title(shared_t!("env.create"))
                .w(px(400.0))
                .confirm()
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        if let Some(this) = this_clone.upgrade() {
                            this.update(cx, |_, cx| {
                                cx.emit(EnvironmentSelectorEvent::CreateEnvironment(name));
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }
}

impl Render for EnvironmentSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(workspace) = &self.workspace else {
            return div().into_any_element();
        };

        let environments = &workspace.environments;
        let active_env = workspace
            .active_environment_id
            .and_then(|id| environments.iter().find(|e| e.id == id));

        let active_name = active_env
            .map(|e| e.name.to_string())
            .unwrap_or_else(|| t!("env.not_found").to_string());

        let view = cx.entity();

        div()
            .h_flex()
            .child(
                Button::new("env_select")
                    .label(active_name)
                    .rounded_r_none()
                    .dropdown_menu({
                        let view = view.clone();
                        let envs = environments.clone();
                        move |menu, window, _cx| {
                            let mut m = menu;
                            for env in &envs {
                                let env_id = env.id;
                                let name = env.name.to_string();
                                m = m.item(PopupMenuItem::new(name).on_click(window.listener_for(
                                    &view,
                                    move |_this, _, _, cx| {
                                        cx.emit(EnvironmentSelectorEvent::SelectEnvironment(
                                            env_id,
                                        ));
                                    },
                                )));
                            }
                            m.separator().item(
                                PopupMenuItem::new(shared_t!("env.create"))
                                    .icon(IconName::Plus)
                                    .on_click(window.listener_for(&view, |this, _, window, cx| {
                                        this.open_create_dialog(window, cx);
                                    })),
                            )
                        }
                    }),
            )
            .child(
                Button::new("env_manage")
                    .icon(IconName::Settings)
                    .rounded_l_none()
                    .border_l_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_manage_dialog(window, cx);
                    })),
            )
            .into_any_element()
    }
}
