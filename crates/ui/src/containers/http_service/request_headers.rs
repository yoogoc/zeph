use super::http_state::HttpState;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::StyledExt;
use models::Header;

struct HeaderRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
    /// `Change` subscriptions on key/value that push edits back to `HttpState`.
    _subscriptions: [Subscription; 2],
}

pub struct RequestHeaders {
    http_state: Entity<HttpState>,
    rows: Vec<HeaderRow>,
}

impl RequestHeaders {
    pub fn new(
        http_state: Entity<HttpState>,
        headers: Vec<Header>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let rows = headers
            .into_iter()
            .map(|h| {
                let key = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&h.key)
                        .placeholder("Key")
                });
                let value = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&h.value)
                        .placeholder("Value")
                });
                Self::make_row(key, value, cx)
            })
            .collect();

        Self { http_state, rows }
    }

    /// Builds a row, wiring `Change` subscriptions that sync edits to `HttpState`.
    fn make_row(
        key: Entity<InputState>,
        value: Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> HeaderRow {
        let key_sub = cx.subscribe(&key, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.sync(cx);
            }
        });
        let value_sub = cx.subscribe(&value, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.sync(cx);
            }
        });
        HeaderRow {
            key,
            value,
            _subscriptions: [key_sub, value_sub],
        }
    }

    /// Pushes the current rows to `HttpState`.
    fn sync(&self, cx: &mut Context<Self>) {
        let headers = self.headers(cx);
        self.http_state
            .update(cx, |s, cx| s.set_headers(headers, cx));
    }

    pub fn add_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("common.key")));
        let value = cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("common.value")));
        let row = Self::make_row(key, value, cx);
        self.rows.push(row);
        self.sync(cx);
        cx.notify();
    }

    pub fn headers(&self, cx: &App) -> Vec<Header> {
        self.rows
            .iter()
            .map(|row| {
                Header::new(
                    row.key.read(cx).value().to_string(),
                    row.value.read(cx).value().to_string(),
                )
            })
            .collect()
    }
}

impl Render for RequestHeaders {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_1()
            .children(self.rows.iter().enumerate().map(|(i, row)| {
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().child(Input::new(&row.key)))
                    .child(div().flex_1().child(Input::new(&row.value)))
                    .child(
                        Button::new(format!("remove_header_{}", i))
                            .label("×")
                            .w(px(24.0))
                            .h(px(24.0))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.rows.remove(i);
                                this.sync(cx);
                                cx.notify();
                            })),
                    )
            }))
            .when(self.rows.is_empty(), |el| {
                el.child(
                    div()
                        .p_4()
                        .text_center()
                        .text_color(rgb(0x6c757d))
                        .text_sm()
                        .child(shared_t!("header.empty")),
                )
            })
    }
}
