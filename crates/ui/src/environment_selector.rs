use std::collections::HashMap;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};
use gpui_component::{IconName, StyledExt, WindowExt};
use models::{Environment, EnvironmentVariable, Workspace};
use uuid::Uuid;

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
        let Some(workspace) = &self.workspace else { return };
        let environments = workspace.environments.clone();

        let manager = cx.new(|_| EnvironmentManager::new(environments));

        cx.subscribe(&manager, |_this, _, event: &EnvironmentManagerEvent, cx| {
            match event {
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
            }
        }).detach();

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
        let active_env = workspace.active_environment_id
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
                                m = m.item(PopupMenuItem::new(name).on_click(
                                    window.listener_for(&view, move |_this, _, _, cx| {
                                        cx.emit(EnvironmentSelectorEvent::SelectEnvironment(env_id));
                                    }),
                                ));
                            }
                            m.separator()
                                .item(PopupMenuItem::new(shared_t!("env.create")).icon(IconName::Plus).on_click(
                                    window.listener_for(&view, |this, _, window, cx| {
                                        this.open_create_dialog(window, cx);
                                    }),
                                ))
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

// === Environment Manager Component ===

pub enum EnvironmentManagerEvent {
    Create(String),
    Update(Environment),
    Delete(Uuid),
    AddVariable(Uuid, EnvironmentVariable),
    UpdateVariable(EnvironmentVariable),
    DeleteVariable(Uuid),
}

struct VarInputs {
    name: Entity<InputState>,
    value: Entity<InputState>,
}

pub struct EnvironmentManager {
    environments: Vec<Environment>,
    selected_env_id: Option<Uuid>,
    base_url_input: Option<Entity<InputState>>,
    new_var_name: Option<Entity<InputState>>,
    var_inputs: HashMap<Uuid, VarInputs>,
}

impl EventEmitter<EnvironmentManagerEvent> for EnvironmentManager {}

impl EnvironmentManager {
    pub fn new(environments: Vec<Environment>) -> Self {
        let selected = environments.first().map(|e| e.id);
        Self {
            environments,
            selected_env_id: selected,
            base_url_input: None,
            new_var_name: None,
            var_inputs: HashMap::new(),
        }
    }

    fn selected_env(&self) -> Option<&Environment> {
        self.selected_env_id.and_then(|id| self.environments.iter().find(|e| e.id == id))
    }

    fn render_env_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_id = self.selected_env_id;

        div()
            .w(px(200.0))
            .h_full()
            .border_r_1()
            .border_color(rgb(0xe0e0e0))
            .v_flex()
            .child(
                div()
                    .p_2()
                    .border_b_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(
                        Button::new("add_env")
                            .w_full()
                            .icon(IconName::Plus)
                            .label(shared_t!("env.create"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_create_env_dialog(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_y_hidden()
                    .v_flex()
                    .children(self.environments.iter().map(|env| {
                        let env_id = env.id;
                        let is_selected = selected_id == Some(env_id);

                        div()
                            .id(format!("env_{}", env_id))
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .when(is_selected, |el| el.bg(rgb(0xe3f2fd)))
                            .hover(|el| el.bg(rgb(0xf5f5f5)))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(if is_selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                    .child(env.name.clone()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected_env_id = Some(env_id);
                                this.base_url_input = None;
                                this.var_inputs.clear();
                                cx.notify();
                            }))
                    })),
            )
    }

    fn render_env_detail(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(env) = self.selected_env().cloned() else {
            return div()
                .flex_1()
                .h_full()
                .items_center()
                .justify_center()
                .text_color(rgb(0x6c757d))
                .child(shared_t!("env.select"))
                .into_any_element();
        };

        let env_id = env.id;
        let env_name = env.name.clone();
        let variables = env.variables.clone();

        if self.base_url_input.is_none() {
            self.base_url_input = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(&env.base_url)
                    .placeholder("https://api.example.com")
            }));
        }

        if self.new_var_name.is_none() {
            self.new_var_name = Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder(shared_t!("variable.name"))
            }));
        }

        // Initialize var inputs
        for var in &variables {
            if !self.var_inputs.contains_key(&var.id) {
                let var_name = var.name.to_string();
                let var_value = var.value().unwrap_or("".into());
                let name_input = cx.new(|cx| InputState::new(window, cx).default_value(&var_name));
                let value_input = cx.new(|cx| InputState::new(window, cx).default_value(var_value));
                self.var_inputs.insert(var.id, VarInputs { name: name_input, value: value_input });
            }
        }

        let base_url_input = self.base_url_input.clone().unwrap();
        let new_var_name = self.new_var_name.clone().unwrap();

        div()
            .flex_1()
            .h_full()
            .v_flex()
            .overflow_y_hidden()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .p_3()
                    .border_b_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(div().text_lg().font_semibold().child(env_name))
                    .child(
                        Button::new("delete_env")
                            .ghost()
                            .icon(IconName::Delete)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.emit(EnvironmentManagerEvent::Delete(env_id));
                                this.environments.retain(|e| e.id != env_id);
                                this.selected_env_id = this.environments.first().map(|e| e.id);
                                this.base_url_input = None;
                                this.var_inputs.clear();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_y_hidden()
                    .p_3()
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .child(div().text_sm().font_semibold().child(shared_t!("env.base_url")))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(Input::new(&base_url_input).flex_1())
                                    .child(
                                        Button::new("save_url")
                                            .label(shared_t!("save"))
                                            .primary()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some(input) = &this.base_url_input {
                                                    let url = input.read(cx).value();
                                                    if let Some(e) = this.environments.iter_mut().find(|e| e.id == env_id) {
                                                        e.base_url = url;
                                                        cx.emit(EnvironmentManagerEvent::Update(e.clone()));
                                                    }
                                                }
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(div().text_sm().font_semibold().child(shared_t!("env.variable")))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(Input::new(&new_var_name).w(px(200.0)))
                                    .child(
                                        Button::new("add_var")
                                            .icon(IconName::Plus)
                                            .label(shared_t!("common.add"))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                if let Some(input) = &this.new_var_name {
                                                    let name = input.read(cx).value().to_string();
                                                    if !name.trim().is_empty() {
                                                        let var = EnvironmentVariable::new(env_id, &name);
                                                        cx.emit(EnvironmentManagerEvent::AddVariable(env_id, var.clone()));
                                                        if let Some(e) = this.environments.iter_mut().find(|e| e.id == env_id) {
                                                            e.variables.push(var);
                                                        }
                                                        input.update(cx, |s, cx| s.set_value("", window, cx));
                                                        this.var_inputs.clear();
                                                        cx.notify();
                                                    }
                                                }
                                            })),
                                    ),
                            )
                            .child(self.render_variables_table(variables, cx)),
                    ),
            )
            .into_any_element()
    }

    fn render_variables_table(&self, variables: Vec<EnvironmentVariable>, cx: &mut Context<Self>) -> impl IntoElement {
        if variables.is_empty() {
            return div()
                .p_4()
                .text_color(rgb(0x6c757d))
                .text_sm()
                .child(shared_t!("variable.not_found"))
                .into_any_element();
        }

        div()
            .v_flex()
            .border_1()
            .border_color(rgb(0xe0e0e0))
            .rounded_md()
            .child(
                div()
                    .h_flex()
                    .bg(rgb(0xf8f9fa))
                    .border_b_1()
                    .border_color(rgb(0xe0e0e0))
                    .child(div().w(px(140.0)).px_3().py_2().text_sm().font_semibold().child(shared_t!("variable.name")))
                    .child(div().flex_1().px_3().py_2().text_sm().font_semibold().child(shared_t!("variable.value")))
                    .child(div().w(px(100.0)).px_3().py_2().text_sm().font_semibold().child(""))
            )
            .children(variables.into_iter().map(|var| {
                let var_id = var.id;
                let var_clone = var.clone();
                let var_name = var.name.clone();
                let var_value = var.value().unwrap_or("".into());

                let inputs = self.var_inputs.get(&var_id);

                div()
                    .h_flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(0xf0f0f0))
                    .child(
                        div().w(px(140.0)).px_2().py_1().child(
                            inputs.map(|i| Input::new(&i.name).w_full().into_any_element())
                                .unwrap_or_else(|| div().text_sm().child(var_name).into_any_element())
                        )
                    )
                    .child(
                        div().flex_1().px_2().py_1().child(
                            inputs.map(|i| Input::new(&i.value).w_full().into_any_element())
                                .unwrap_or_else(|| div().text_sm().child(var_value).into_any_element())
                        )
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .px_2()
                            .py_1()
                            .h_flex()
                            .gap_1()
                            .child(
                                Button::new(format!("save_{}", var_id))
                                    .ghost()
                                    .compact()
                                    .icon(IconName::Check)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(inputs) = this.var_inputs.get(&var_id) {
                                            let name = inputs.name.read(cx).value();
                                            let value = inputs.value.read(cx).value();
                                            let mut updated = var_clone.clone();
                                            updated.name = name;
                                            updated.local_value = Some(value);
                                            cx.emit(EnvironmentManagerEvent::UpdateVariable(updated.clone()));
                                            for e in &mut this.environments {
                                                if let Some(v) = e.variables.iter_mut().find(|v| v.id == var_id) {
                                                    *v = updated.clone();
                                                }
                                            }
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Button::new(format!("del_{}", var_id))
                                    .ghost()
                                    .compact()
                                    .icon(IconName::Delete)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.emit(EnvironmentManagerEvent::DeleteVariable(var_id));
                                        for e in &mut this.environments {
                                            e.variables.retain(|v| v.id != var_id);
                                        }
                                        this.var_inputs.remove(&var_id);
                                        cx.notify();
                                    })),
                            ),
                    )
            }))
            .into_any_element()
    }

    fn open_create_env_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("env.title")));

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
                            this.update(cx, |this, cx| {
                                cx.emit(EnvironmentManagerEvent::Create(name.clone()));
                                let ws_id = this.environments.first().map(|e| e.workspace_id).unwrap_or(Uuid::new_v4());
                                let new_env = Environment::new(ws_id, &name);
                                this.selected_env_id = Some(new_env.id);
                                this.environments.push(new_env);
                                this.base_url_input = None;
                                this.var_inputs.clear();
                                cx.notify();
                            });
                        }
                    }
                    window.close_dialog(cx);
                    true
                })
        });
    }
}

impl Render for EnvironmentManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(450.0))
            .h_flex()
            .child(self.render_env_list(cx))
            .child(self.render_env_detail(window, cx))
    }
}
