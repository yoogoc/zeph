use crate::state::ServiceState;
use crate::utils::cal_icon;
use gpui::*;
use gpui_component::button::{Button, ButtonRounded};
use gpui_component::input::{Input, InputState};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{IconName, Sizable};
use models::ContentType;

pub struct ServiceTabs {
    service_state: Entity<ServiceState>,
    editing_tab: Option<ContentType>,
    edit_input: Option<Entity<InputState>>,
}

impl ServiceTabs {
    // Re-renders automatically when `service_state` changes: GPUI tracks the
    // entities `read` during `render` and invalidates this view on their `notify`.
    pub fn new(service_state: Entity<ServiceState>) -> Self {
        Self {
            service_state,
            editing_tab: None,
            edit_input: None,
        }
    }

    fn start_editing(
        &mut self,
        content_type: ContentType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current_name = content_type.name();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));
        let name_string = current_name.to_string();
        input.update(cx, |state, cx| {
            state.set_value(name_string, window, cx);
        });

        self.editing_tab = Some(content_type);
        self.edit_input = Some(input);
        cx.notify();
    }

    fn finish_editing(&mut self, cx: &mut Context<Self>) {
        if let (Some(content_type), Some(input)) = (self.editing_tab.take(), self.edit_input.take())
        {
            let new_name = input.read(cx).value();
            let trimmed = new_name.trim();
            if !trimmed.is_empty() && trimmed.len() <= 255 {
                if let ContentType::Service(service) = &content_type {
                    let service_id = service.id;
                    let new_name = new_name.clone();
                    self.service_state.update(cx, |s, cx| {
                        s.rename_service(service_id, new_name, cx);
                    });
                }
            }
            self.editing_tab = None;
            self.edit_input = None;
            cx.notify();
        }
    }

    fn cancel_editing(&mut self, cx: &mut Context<Self>) {
        self.editing_tab = None;
        self.edit_input = None;
        cx.notify();
    }
}

impl Render for ServiceTabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ss = self.service_state.read(cx);

        if ss.opened_services.is_empty() {
            return div().into_any_element();
        }

        let service_ids: Vec<_> = ss.opened_services.clone();
        let active_service = ss.active_service.clone();
        let selected_index = active_service
            .as_ref()
            .and_then(|active| service_ids.iter().position(|s| s.eq(active)))
            .unwrap_or(0);

        let close_button = |ct: ContentType| {
            Button::new(format!("close_{}", ct.id()))
                .xsmall()
                .rounded(ButtonRounded::Large)
                .icon(IconName::Close)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let ct = ct.clone();
                    this.service_state.update(cx, |s, cx| {
                        s.close_content(ct, cx);
                    });
                    cx.stop_propagation();
                }))
        };

        let tabs: Vec<_> = ss
            .opened_services
            .iter()
            .enumerate()
            .map(|(_, ct)| {
                let ct = ct.clone();
                let is_editing = self
                    .editing_tab
                    .as_ref()
                    .map_or(false, |editing| editing.eq(&ct));

                if is_editing {
                    if let Some(input) = &self.edit_input {
                        return Tab::new()
                            .px_1()
                            .prefix(cal_icon(&ct))
                            .child(
                                div()
                                    .on_key_down(cx.listener(
                                        move |this, event: &KeyDownEvent, _, cx| {
                                            if event.keystroke.key == "enter" {
                                                this.finish_editing(cx);
                                            } else if event.keystroke.key == "escape" {
                                                this.cancel_editing(cx);
                                            }
                                        },
                                    ))
                                    .child(Input::new(input).w(px(150.0))),
                            )
                            .suffix(close_button(ct));
                    }
                }
                Tab::new()
                    .px_1()
                    .label(ct.name())
                    .prefix(cal_icon(&ct))
                    .suffix(close_button(ct))
            })
            .collect();

        let service_state = self.service_state.clone();

        div()
            .size_full()
            .pl_1()
            .child(
                TabBar::new("service_tabs")
                    .selected_index(selected_index)
                    .with_variant(TabVariant::Pill)
                    .on_click(cx.listener(move |this, index: &usize, window, cx| {
                        if let Some(ct) = service_ids.get(*index) {
                            let is_already_selected = service_state
                                .read(cx)
                                .active_service
                                .as_ref()
                                .map_or(false, |active| active.eq(ct));

                            if is_already_selected && this.editing_tab.is_none() {
                                this.start_editing(ct.clone(), window, cx);
                            } else {
                                let ct = ct.clone();
                                this.service_state.update(cx, |s, cx| {
                                    s.set_active_content(ct, cx);
                                });
                            }
                        }
                    }))
                    .children(tabs),
            )
            .into_any_element()
    }
}
