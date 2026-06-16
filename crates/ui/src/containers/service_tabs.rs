use crate::state::ServiceState;
use gpui::*;
use gpui_component::button::{Button, ButtonRounded};
use gpui_component::input::{Input, InputState};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{IconName, Sizable};
use uuid::Uuid;

pub struct ServiceTabs {
    service_state: Entity<ServiceState>,
    edit_input: Option<Entity<InputState>>,
}

impl ServiceTabs {
    // Re-renders automatically when `service_state` changes: GPUI tracks the
    // entities `read` during `render` and invalidates this view on their `notify`.
    pub fn new(service_state: Entity<ServiceState>) -> Self {
        Self {
            service_state,
            edit_input: None,
        }
    }

    fn start_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ss = self.service_state.read(cx);
        let ac = ss.active_content();
        if let Some(ov) = ac {
            let name = ov.view.title(cx);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入新名称"));
            input.update(cx, |state, cx| {
                state.set_value(name, window, cx);
            });

            self.edit_input = Some(input);
            cx.notify();
        }
    }

    fn finish_editing(&mut self, cx: &mut Context<Self>) {
        let ss = self.service_state.read(cx);
        let ei = self.edit_input.clone();
        let ac = ss.active_content();
        if let Some(ov) = ac {
            if let Some(input) = ei {
                let new_name = input.read(cx).value();
                let trimmed = new_name.trim();
                if !trimmed.is_empty() && trimmed.len() <= 255 {
                    let service_id = ov.view.id(cx);
                    let new_name = new_name.clone();
                    self.service_state.update(cx, |s, cx| {
                        s.rename_service(service_id, new_name, cx);
                    });
                }
            }
        }

        self.edit_input = None;
        cx.notify();
    }

    fn cancel_editing(&mut self, cx: &mut Context<Self>) {
        self.edit_input = None;
        cx.notify();
    }
}

impl Render for ServiceTabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ss = self.service_state.read(cx);

        // "+" button to open a new untitled request; always available.
        let new_request_button = Button::new("new_untitled_request")
            .xsmall()
            .rounded(ButtonRounded::Large)
            .icon(IconName::Plus)
            .on_click(cx.listener(|this, _, window, cx| {
                this.service_state.update(cx, |s, cx| {
                    s.create_untitled_request(None, window, cx);
                });
            }));

        if ss.open_views().is_empty() {
            return div().pl_1().child(new_request_button).into_any_element();
        }

        let current_active_view_id = ss.active_view_ix;

        // (id, title, icon) for each open tab, sourced from the cached view.
        let tab_data: Vec<(Uuid, SharedString, AnyElement)> = ss
            .open_views()
            .iter()
            .map(|entry| {
                (
                    entry.view.id(cx),
                    entry.view.title(cx),
                    entry.view.tab_icon(cx),
                )
            })
            .collect();

        let close_button = |ix: usize, id: Uuid| {
            Button::new(format!("close_{}", id))
                .xsmall()
                .rounded(ButtonRounded::Large)
                .icon(IconName::Close)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.service_state.update(cx, |s, cx| {
                        s.close_content(ix, cx);
                    });
                    cx.stop_propagation();
                }))
        };

        let tabs: Vec<_> = tab_data
            .into_iter()
            .enumerate()
            .map(|(ix, (id, title, icon))| {
                let is_editing = self
                    .edit_input
                    .as_ref()
                    .map_or(false, |_| Some(ix).eq(&current_active_view_id));

                if is_editing {
                    if let Some(input) = &self.edit_input {
                        return Tab::new()
                            .px_1()
                            .prefix(icon)
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
                            .suffix(close_button(ix, id));
                    }
                }
                Tab::new()
                    .px_1()
                    .label(title)
                    .prefix(icon)
                    .suffix(close_button(ix, id))
            })
            .collect();

        div()
            .size_full()
            .pl_1()
            .flex()
            .items_center()
            .child(
                TabBar::new("service_tabs")
                    .selected_index(current_active_view_id.unwrap_or_default())
                    .with_variant(TabVariant::Pill)
                    .on_click(cx.listener({
                        let ss = self.service_state.clone();
                        move |this, index: &usize, window, cx| {
                            if Some(*index).eq(&current_active_view_id) {
                                this.start_editing(window, cx);
                            } else {
                                ss.update(cx, |s, cx| {
                                    s.set_active_content(*index, cx);
                                });
                            }
                        }
                    }))
                    .children(tabs),
            )
            .child(new_request_button)
            .into_any_element()
    }
}
