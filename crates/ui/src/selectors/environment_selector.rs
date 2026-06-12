use crate::state::EnvironmentState;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, StyledExt, WindowExt};

use crate::dialogs::EnvironmentManager;

pub struct EnvironmentSelector {
    env_state: Entity<EnvironmentState>,
}

impl EnvironmentSelector {
    pub fn new(env_state: Entity<EnvironmentState>) -> Self {
        Self { env_state }
    }

    fn open_manage_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let env_state = self.env_state.clone();
        let manager = cx.new(|cx| EnvironmentManager::new(env_state, cx));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(shared_t!("env.title"))
                .w(px(800.0))
                .child(manager.clone())
        });
    }

    fn open_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Environment name"));
        let env_state = self.env_state.clone();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_clone = input.clone();
            let es = env_state.clone();

            dialog
                .title(shared_t!("env.create"))
                .w(px(400.0))
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let name = input_clone.read(cx).value().to_string();
                    if !name.trim().is_empty() {
                        es.update(cx, |s, cx| s.create_environment(name, cx));
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }
}

impl Render for EnvironmentSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let es = self.env_state.read(cx);

        let environments = es.environments.clone();
        let active_env = es
            .active_environment_id
            .and_then(|id| environments.iter().find(|e| e.id == id));

        let active_name = active_env
            .map(|e| e.name.to_string())
            .unwrap_or_else(|| t!("env.not_found").to_string());

        let view = cx.entity();
        let env_state = self.env_state.clone();

        div()
            .h_flex()
            .child(
                Button::new("env_select")
                    .label(active_name)
                    .rounded_r_none()
                    .dropdown_menu({
                        let view = view.clone();
                        let envs = environments.clone();
                        let es = env_state.clone();
                        move |menu, window, _cx| {
                            let mut m = menu;
                            for env in &envs {
                                let env_id = env.id;
                                let name = env.name.to_string();
                                let es2 = es.clone();
                                m = m.item(PopupMenuItem::new(name).on_click(window.listener_for(
                                    &view,
                                    move |_this, _, _, cx| {
                                        es2.update(cx, |s, cx| {
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
