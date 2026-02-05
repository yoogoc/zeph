use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::StyledExt;
use models::Header;

struct HeaderRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
}

pub struct RequestHeaders {
    rows: Vec<HeaderRow>,
}

impl RequestHeaders {
    pub fn new(headers: Vec<Header>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let rows = headers
            .into_iter()
            .map(|h| HeaderRow {
                key: cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&h.key)
                        .placeholder("Key")
                }),
                value: cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(&h.value)
                        .placeholder("Value")
                }),
            })
            .collect();

        Self { rows }
    }

    pub fn add_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rows.push(HeaderRow {
            key: cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("common.key"))),
            value: cx.new(|cx| InputState::new(window, cx).placeholder(shared_t!("common.key"))),
        });
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
