//! GrpcResponsePanel: read-only view of the gRPC response (or error) as JSON.

use gpui::*;
use gpui_component::input::{Input, InputState, TabSize};
use gpui_component::v_flex;

pub struct GrpcResponsePanel {
    response: Entity<InputState>,
}

impl GrpcResponsePanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let response = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .searchable(true)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .line_number(true)
        });
        Self { response }
    }

    pub fn clear(&self, window: &mut Window, cx: &mut App) {
        self.response
            .update(cx, |s, cx| s.set_value("", window, cx));
    }

    pub fn set_response(&self, text: impl Into<String>, window: &mut Window, cx: &mut App) {
        let text = text.into();
        self.response
            .update(cx, |s, cx| s.set_value(text, window, cx));
    }

    pub fn set_error(&self, text: impl Into<String>, window: &mut Window, cx: &mut App) {
        self.set_response(text, window, cx);
    }
}

impl Render for GrpcResponsePanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(Input::new(&self.response).disabled(true).h_full())
    }
}
