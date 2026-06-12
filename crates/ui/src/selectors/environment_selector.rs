use crate::state::ServiceState;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, StyledExt, WindowExt};

use crate::dialogs::{EnvironmentManager, EnvironmentManagerEvent};

pub struct EnvironmentSelector {
    service_state: Entity<ServiceState>,
}

impl EnvironmentSelector {
    pub fn new(service_state: Entity<ServiceState>) -> Self {
        Self { service_state }
    }

    fn open_manage_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let environments = self.service_state.read(cx).environments.clone();

        let manager = cx.new(|_| EnvironmentManager::new(environments));

        let service_state = self.service_state.clone();
        cx.subscribe(
            &manager,
            move |_this, _, event: &EnvironmentManagerEvent, cx| match event {
                EnvironmentManagerEvent::Create(name) => {
                    service_state.update(cx, |s, cx| s.create_environment(name.clone(), cx));
                }
                EnvironmentManagerEvent::Update(env) => {
                    service_state.update(cx, |s, cx| s.update_environment(env.clone(), cx));
                }
                EnvironmentManagerEvent::Delete(id) => {
                    service_state.update(cx, |s, cx| s.delete_environment(*id, cx));
                }
                EnvironmentManagerEvent::AddVariable(env_id, var) => {
                    service_state.update(cx, |s, cx| {
                        s.add_environment_variable(*env_id, var.clone(), cx)
                    });
                }
                EnvironmentManagerEvent::UpdateVariable(var) => {
                    service_state
                        .update(cx, |s, cx| s.update_environment_variable(var.clone(), cx));
                }
                EnvironmentManagerEvent::DeleteVariable(id) => {
                    service_state.update(cx, |s, cx| s.delete_environment_variable(*id, cx));
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
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Environment name"));
        let service_state = self.service_state.clone();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let ss = service_state.clone();

            dialog
                .title(shared_t!("env.create"))
                .w(px(400.0))
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        ss.update(cx, |s, cx| s.create_environment(name, cx));
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }
}

impl Render for EnvironmentSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ss = self.service_state.read(cx);

        let environments = ss.environments.clone();
        let active_env = ss
            .active_environment_id
            .and_then(|id| environments.iter().find(|e| e.id == id));

        let active_name = active_env
            .map(|e| e.name.to_string())
            .unwrap_or_else(|| t!("env.not_found").to_string());

        let view = cx.entity();
        let service_state = self.service_state.clone();

        div()
            .h_flex()
            .child(
                Button::new("env_select")
                    .label(active_name)
                    .rounded_r_none()
                    .dropdown_menu({
                        let view = view.clone();
                        let envs = environments.clone();
                        let ss = service_state.clone();
                        move |menu, window, _cx| {
                            let mut m = menu;
                            for env in &envs {
                                let env_id = env.id;
                                let name = env.name.to_string();
                                let ss2 = ss.clone();
                                m = m.item(PopupMenuItem::new(name).on_click(window.listener_for(
                                    &view,
                                    move |_this, _, _, cx| {
                                        ss2.update(cx, |s, cx| {
                                            s.set_active_environment(env_id, cx)
                                        });
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
