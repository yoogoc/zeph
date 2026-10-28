use super::*;
use gpui_kit::base::Popup;

#[derive(Default)]
pub(super) struct ValueUi {
    active: Option<ValueOptions>,
    committed: Option<(EntityId, String)>,
}
struct ValueOptions {
    input: Entity<InputState>,
    row: usize,
    list: Suggestions,
    scroll: ScrollHandle,
}
struct Suggestions {
    values: Vec<String>,
    matches: Vec<usize>,
    highlighted: Option<usize>,
}
impl Suggestions {
    fn new(values: Vec<String>) -> Self {
        Self {
            matches: (0..values.len()).collect(),
            values,
            highlighted: None,
        }
    }
    fn filter(&mut self, text: &str) {
        let query = text.to_lowercase();
        self.matches = self
            .values
            .iter()
            .enumerate()
            .filter(|(_, v)| v.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.highlighted = None;
    }
    fn navigate(&mut self, down: bool) {
        let count = self.matches.len();
        if count == 0 {
            self.highlighted = None;
            return;
        }
        self.highlighted = Some(match self.highlighted {
            None if down => 0,
            None => count - 1,
            Some(i) if down => (i + 1) % count,
            Some(i) => (i + count - 1) % count,
        });
    }
    fn selected(&self) -> Option<String> {
        self.highlighted
            .and_then(|i| self.matches.get(i))
            .map(|i| self.values[*i].clone())
    }
}
impl Workbench {
    pub(super) fn refresh_focused_value_options(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(row) = self
            .params
            .iter()
            .position(|p| p.value.focus_handle(cx).is_focused(window))
        {
            self.open_value_options(row, cx);
        }
    }
    fn parameter_choices(&self, row: usize) -> Vec<String> {
        let Some(parameter) = self.param_data().get(row) else {
            return vec![];
        };
        if self.current().protocol != Protocol::Http
            || self.structure_ui.request != Some(self.current().id)
            || parameter.file
        {
            return vec![];
        }
        let location = match self.tab {
            Tab::Query => "query",
            Tab::Path => "path",
            Tab::Headers => "header",
            Tab::Body if self.current().body.kind == BodyKind::FormData => "formData",
            Tab::Body if self.current().body.kind == BodyKind::UrlEncoded => "form",
            _ => return vec![],
        };
        self.structure_ui
            .data
            .value_suggestions(location, &parameter.name)
    }
    fn open_value_options(&mut self, row: usize, cx: &mut Context<Self>) {
        let values = self.parameter_choices(row);
        let Some(inputs) = self.params.get(row) else {
            return;
        };
        if values.is_empty() {
            self.value_ui.active = None;
            return;
        }
        self.value_ui.active = Some(ValueOptions {
            row,
            input: inputs.value.clone(),
            list: Suggestions::new(values),
            scroll: ScrollHandle::new(),
        });
        cx.notify();
    }
    pub(super) fn parameter_value_event(
        &mut self,
        row: usize,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Focus => self.open_value_options(row, cx),
            InputEvent::Blur => {
                if self
                    .value_ui
                    .active
                    .as_ref()
                    .is_some_and(|a| a.input == *input)
                {
                    self.value_ui.active = None;
                    cx.notify();
                }
            }
            InputEvent::Change if input.focus_handle(cx).is_focused(window) => {
                let text = input.read(cx).value().to_string();
                if self
                    .value_ui
                    .committed
                    .take()
                    .is_some_and(|(id, value)| id == input.entity_id() && value == text)
                {
                    return;
                }
                if self
                    .value_ui
                    .active
                    .as_ref()
                    .is_none_or(|a| a.input != *input)
                {
                    self.open_value_options(row, cx);
                }
                if let Some(active) = &mut self.value_ui.active {
                    active.list.filter(&text);
                    active.scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                }
            }
            _ => {}
        }
    }
    fn commit_value_option(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active) = self.value_ui.active.take() else {
            return;
        };
        if self
            .params
            .get(active.row)
            .is_none_or(|p| p.value != active.input)
        {
            return;
        }
        if let Some(parameter) = self.param_data_mut().get_mut(active.row) {
            parameter.value = value.clone();
        }
        self.value_ui.committed = Some((active.input.entity_id(), value.clone()));
        active
            .input
            .update(cx, |input, cx| input.set_value(value, window, cx));
        // Keep the caret in the editable input without reopening its suggestions.
        self.dirty(cx);
    }
    fn value_option_key(
        &mut self,
        row: usize,
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
        if matches!(key, "down" | "up") {
            if self.value_ui.active.is_none() {
                self.open_value_options(row, cx);
            }
            if let Some(active) = &mut self.value_ui.active {
                active.list.navigate(key == "down");
                if let Some(selected) = active.list.highlighted {
                    active.scroll.scroll_to_item(selected);
                }
                cx.stop_propagation();
                cx.notify();
            }
        } else if self.value_ui.active.as_ref().is_some_and(|a| a.row == row) {
            match key {
                "escape" => {
                    self.value_ui.active = None;
                    cx.stop_propagation();
                    cx.notify();
                }
                "enter" => {
                    let value = self
                        .value_ui
                        .active
                        .as_ref()
                        .and_then(|a| a.list.selected());
                    if let Some(value) = value {
                        self.commit_value_option(value, window, cx);
                    } else {
                        self.value_ui.active = None;
                        cx.notify();
                    }
                    cx.stop_propagation();
                }
                "tab" => {
                    self.value_ui.active = None;
                    cx.notify();
                }
                _ => {}
            }
        }
    }
    pub(super) fn parameter_value_input(
        &self,
        row: usize,
        parameter: &Parameter,
        inputs: &ParamInputs,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = Input::new(&inputs.value)
            .small()
            .appearance(false)
            .w_full()
            .aria_label(format!("参数 {} 的值", parameter.name));
        let choices = self.parameter_choices(row);
        if choices.is_empty() {
            return div().flex_1().min_w_0().child(input).into_any_element();
        }
        let trigger = div()
            .id(("value-input-trigger", row))
            .w_full()
            .capture_any_mouse_down(cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                if event.button == MouseButton::Left {
                    this.open_value_options(row, cx);
                }
            }))
            .capture_key_down(cx.listener(move |this, event, window, cx| {
                this.value_option_key(row, event, window, cx)
            }))
            .child(input);
        let mut popup = Popup::new(("value-input-options", row), trigger)
            .flex_1()
            .min_w_0()
            .offset(px(4.));
        if let Some(active) = self
            .value_ui
            .active
            .as_ref()
            .filter(|a| a.input == inputs.value)
        {
            let mut list = v_flex()
                .id(("value-option-list", row))
                .role(Role::ListBox)
                .aria_label(format!("{} 的可选值", parameter.name))
                .max_h(px(224.))
                .overflow_y_scroll()
                .track_scroll(&active.scroll)
                .p_1()
                .gap(px(2.));
            for (position, index) in active.list.matches.iter().enumerate() {
                let value = active.list.values[*index].clone();
                let selected = parameter.value == value;
                let label = if value.is_empty() {
                    "（空字符串）".into()
                } else {
                    value.clone()
                };
                list = list.child(
                    h_flex()
                        .id(("value-option", position))
                        .role(Role::ListBoxOption)
                        .aria_label(label.clone())
                        .aria_selected(selected)
                        .min_h(px(30.))
                        .px_2()
                        .py_1()
                        .gap_2()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .bg(if active.list.highlighted == Some(position) {
                            cx.theme().accent
                        } else {
                            transparent_black()
                        })
                        .hover(|s| s.bg(cx.theme().accent))
                        .text_sm()
                        .child(div().flex_1().min_w_0().child(label))
                        .when(selected, |s| {
                            s.child(
                                Icon::new(IconName::Check)
                                    .size(px(13.))
                                    .text_color(cx.theme().primary),
                            )
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.commit_value_option(value.clone(), window, cx)
                        })),
                );
            }
            if active.list.matches.is_empty() {
                list = list.child(
                    div()
                        .p_3()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("没有匹配项，继续使用输入的值"),
                );
            }
            popup = popup.content(
                v_flex()
                    .id(("value-option-popup", row))
                    .w(px(300.))
                    .rounded(px(8.))
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_md()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.value_ui.active = None;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_size(px(11.))
                            .text_color(cx.theme().muted_foreground)
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child("可选值 · 支持手动输入"),
                    )
                    .child(list),
            );
        }
        popup.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::Suggestions;
    fn values() -> Suggestions {
        Suggestions::new(vec![
            "pending".into(),
            "binding".into(),
            "running".into(),
            "".into(),
        ])
    }
    #[test]
    fn focus_shows_all_without_overwriting_the_current_value() {
        let s = values();
        assert_eq!(s.matches.len(), 4);
        assert!(s.selected().is_none());
    }
    #[test]
    fn typing_filters_and_unmatched_variables_are_not_replaced() {
        let mut s = values();
        s.filter("ING");
        assert_eq!(s.matches, vec![0, 1, 2]);
        s.navigate(true);
        assert_eq!(s.selected().as_deref(), Some("pending"));
        s.filter("{{state}}");
        assert!(s.matches.is_empty());
        s.navigate(true);
        assert!(s.selected().is_none());
        s.filter("");
        assert_eq!(s.matches.len(), 4);
        assert!(s.selected().is_none());
    }
    #[test]
    fn keyboard_navigation_wraps_and_can_select_an_empty_string() {
        let mut s = values();
        s.navigate(false);
        assert_eq!(s.selected().as_deref(), Some(""));
        s.navigate(true);
        assert_eq!(s.selected().as_deref(), Some("pending"));
        s.filter("run");
        s.navigate(true);
        assert_eq!(s.selected().as_deref(), Some("running"));
        s.navigate(false);
        assert_eq!(s.selected().as_deref(), Some("running"));
    }
}
