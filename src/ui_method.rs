use super::*;
use gpui_kit::base::Popup;

#[derive(Default)]
pub(super) struct MethodUi {
    open: bool,
    filter: String,
    highlighted: Option<usize>,
    scroll: ScrollHandle,
}
impl MethodUi {
    fn choices(&self) -> Vec<&'static str> {
        let filter = self.filter.trim().to_ascii_uppercase();
        HTTP_METHODS
            .into_iter()
            .filter(|method| method.contains(&filter))
            .collect()
    }
}
impl Workbench {
    fn open_method_options(&mut self, cx: &mut Context<Self>) {
        self.method_ui.open = true;
        self.method_ui.filter.clear();
        self.method_ui.highlighted = None;
        self.method_ui.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }
    pub(super) fn method_input_event(
        &mut self,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Focus => self.open_method_options(cx),
            InputEvent::Blur => {
                self.method_ui.open = false;
                cx.notify();
            }
            InputEvent::Change
                if !self.loading && self.method.focus_handle(cx).is_focused(window) =>
            {
                self.method_ui.filter = self.method.read(cx).value().to_string();
                self.method_ui.open = true;
                self.method_ui.highlighted = None;
                cx.notify();
            }
            _ => {}
        }
    }
    fn commit_method_option(&mut self, method: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.current_mut().method = method.into();
        self.method
            .update(cx, |s, cx| s.set_value(method, window, cx));
        self.method_ui.open = false;
        self.focus.focus(window, cx);
        self.dirty(cx);
    }
    fn method_option_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.platform
            || event.keystroke.modifiers.control
            || event.keystroke.modifiers.alt
        {
            return;
        }
        let key = event.keystroke.key.as_str();
        match key {
            "down" | "up" => {
                if !self.method_ui.open {
                    self.open_method_options(cx);
                }
                let count = self.method_ui.choices().len();
                if count > 0 {
                    let next = match self.method_ui.highlighted {
                        None if key == "down" => 0,
                        None => count - 1,
                        Some(i) if key == "down" => (i + 1) % count,
                        Some(i) => (i + count - 1) % count,
                    };
                    self.method_ui.highlighted = Some(next);
                    self.method_ui.scroll.scroll_to_item(next);
                }
                cx.stop_propagation();
                cx.notify();
            }
            "enter" if self.method_ui.open => {
                let choices = self.method_ui.choices();
                if let Some(method) = self.method_ui.highlighted.and_then(|i| choices.get(i)) {
                    self.commit_method_option(method, window, cx);
                } else {
                    let typed = self.method.read(cx).value().trim().to_ascii_uppercase();
                    self.commit_method_option(&typed, window, cx);
                }
                cx.stop_propagation();
            }
            "escape" | "tab" => {
                self.method_ui.open = false;
                if key == "escape" {
                    cx.stop_propagation();
                }
                cx.notify();
            }
            _ => {}
        }
    }
    pub(super) fn method_input(&self, cx: &mut Context<Self>) -> AnyElement {
        let trigger = h_flex()
            .id("http-method-input")
            .relative()
            .w(px(142.))
            .h(px(42.))
            .items_center()
            .border_r_1()
            .border_color(cx.theme().border)
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left {
                    this.method.update(cx, |s, cx| s.focus(window, cx));
                    this.open_method_options(cx);
                }
            }))
            .capture_key_down(
                cx.listener(|this, event, window, cx| this.method_option_key(event, window, cx)),
            )
            .child(
                Input::new(&self.method)
                    .appearance(false)
                    .w_full()
                    .h(px(42.))
                    .px_6()
                    .text_center()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(method_color(&self.current().method))
                    .aria_label("HTTP Method，可选择或手动输入"),
            )
            .child(
                div().absolute().right_2().child(
                    Icon::new(IconName::ChevronDown)
                        .size(px(14.))
                        .text_color(cx.theme().muted_foreground),
                ),
            );
        let mut popup = Popup::new("http-method-options", trigger)
            .flex_shrink_0()
            .offset(px(4.));
        if self.method_ui.open {
            let mut list = v_flex()
                .id("http-method-list")
                .role(Role::ListBox)
                .aria_label("HTTP Method 可选列表")
                .max_h(px(320.))
                .overflow_y_scroll()
                .track_scroll(&self.method_ui.scroll)
                .p_1()
                .gap(px(2.));
            for (index, method) in self.method_ui.choices().into_iter().enumerate() {
                let selected = self.current().method.eq_ignore_ascii_case(method);
                list = list.child(
                    h_flex()
                        .id(("http-method-option", index))
                        .relative()
                        .role(Role::ListBoxOption)
                        .aria_label(method)
                        .aria_selected(selected)
                        .h(px(30.))
                        .flex_shrink_0()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(method_color(method))
                        .bg(if self.method_ui.highlighted == Some(index) || selected {
                            cx.theme().accent
                        } else {
                            transparent_black()
                        })
                        .hover(|s| s.bg(cx.theme().accent))
                        .child(method)
                        .when(selected, |s| {
                            s.child(
                                div()
                                    .absolute()
                                    .right_2()
                                    .child(Icon::new(IconName::Check).size(px(12.))),
                            )
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.commit_method_option(method, window, cx)
                        })),
                );
            }
            if self.method_ui.choices().is_empty() {
                list = list.child(
                    div()
                        .p_2()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("自定义 Method，按 Enter 确认"),
                );
            }
            popup = popup.content(
                v_flex()
                    .w(px(142.))
                    .rounded(px(8.))
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_md()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.method_ui.open = false;
                        cx.notify();
                    }))
                    .child(list),
            );
        }
        popup.into_any_element()
    }
}
