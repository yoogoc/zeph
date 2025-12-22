use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::StyledExt;
use models::Header;

pub enum RequestHeadersEvent {
    Remove(usize),
}

pub struct RequestHeaders {
    headers: Vec<Header>,
}

impl EventEmitter<RequestHeadersEvent> for RequestHeaders {}

impl RequestHeaders {
    pub fn new(headers: Vec<Header>) -> Self {
        Self { headers }
    }
}

impl Render for RequestHeaders {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_1()
            .children(self.headers.iter().enumerate().map(|(i, header)| {
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .p_2()
                            .bg(gpui::rgb(0xf8f9fa))
                            .border_1()
                            .border_color(gpui::rgb(0xdee2e6))
                            .rounded_md()
                            .text_sm()
                            .child(header.key.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_2()
                            .bg(gpui::rgb(0xf8f9fa))
                            .border_1()
                            .border_color(gpui::rgb(0xdee2e6))
                            .rounded_md()
                            .text_sm()
                            .child(header.value.clone()),
                    )
                    .child(
                        Button::new(format!("remove_header_{}", i))
                            .label("×")
                            .w(px(24.0))
                            .h(px(24.0))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(RequestHeadersEvent::Remove(i));
                            })),
                    )
            }))
            .when(self.headers.is_empty(), |el| {
                el.child(
                    div()
                        .p_4()
                        .text_center()
                        .text_color(gpui::rgb(0x6c757d))
                        .text_sm()
                        .child("No headers added"),
                )
            })
    }
}
