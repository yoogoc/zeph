use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputState, TabSize};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::v_flex;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum ResponseTab {
    #[default]
    Body,
    Headers,
    Request,
}

/// Owns the response views (body / headers / request-info inputs) and renders
/// them. `HttpState::send_request` writes results here via the setter methods.
pub struct ResponsePanel {
    response: Entity<InputState>,
    headers: Entity<InputState>,
    request: Entity<InputState>,
    active_tab: ResponseTab,
}

impl ResponsePanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let response = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .searchable(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .line_number(true)
        });
        let headers = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("text")
                .line_number(true)
        });
        let request = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("text")
                .line_number(true)
        });

        Self {
            response,
            headers,
            request,
            active_tab: ResponseTab::default(),
        }
    }

    /// Writes the human-readable request info (shown on the Request tab).
    pub fn set_request_info(&self, text: impl Into<String>, window: &mut Window, cx: &mut App) {
        let text = text.into();
        self.request
            .update(cx, |state, cx| state.set_value(text, window, cx));
    }

    /// Clears the response body and headers (before sending a new request).
    pub fn clear_response(&self, window: &mut Window, cx: &mut App) {
        self.response
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.headers
            .update(cx, |state, cx| state.set_value("", window, cx));
    }

    pub fn set_response_headers(&self, text: impl Into<String>, window: &mut Window, cx: &mut App) {
        let text = text.into();
        self.headers
            .update(cx, |state, cx| state.set_value(text, window, cx));
    }

    pub fn set_response_body(&self, text: impl Into<String>, window: &mut Window, cx: &mut App) {
        let text = text.into();
        self.response
            .update(cx, |state, cx| state.set_value(text, window, cx));
    }

    /// Appends to the response body (for streaming/SSE responses).
    pub fn append_response_body(&self, text: impl Into<String>, window: &mut Window, cx: &mut App) {
        let text = text.into();
        self.response
            .update(cx, |state, cx| state.insert(text, window, cx));
    }

    /// Writes an error message into the response body view.
    pub fn set_error(&self, text: impl Into<String>, window: &mut Window, cx: &mut App) {
        self.set_response_body(text, window, cx);
    }
}

impl Render for ResponsePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;

        v_flex()
            .gap_2()
            .flex_1()
            .min_h_0()
            .h_full()
            .child(
                TabBar::new("response_tabs")
                    .with_variant(TabVariant::Segmented)
                    .child(Tab::new().label(shared_t!("response.body")))
                    .child(Tab::new().label(shared_t!("response.header")))
                    .child(Tab::new().label(shared_t!("response.request")))
                    .selected_index(active_tab as usize)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.active_tab = match *ix {
                            0 => ResponseTab::Body,
                            1 => ResponseTab::Headers,
                            _ => ResponseTab::Request,
                        };
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .when(active_tab == ResponseTab::Body, |el| {
                        el.child(Input::new(&self.response).disabled(true).h_full())
                    })
                    .when(active_tab == ResponseTab::Headers, |el| {
                        el.child(Input::new(&self.headers).disabled(true).h_full())
                    })
                    .when(active_tab == ResponseTab::Request, |el| {
                        el.child(Input::new(&self.request).disabled(true).h_full())
                    }),
            )
    }
}
