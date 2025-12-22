use gpui::prelude::FluentBuilder;
use gpui::*;

pub struct RequestBody {
    body: Option<SharedString>,
}

impl RequestBody {
    pub fn new(body: Option<SharedString>) -> Self {
        Self { body }
    }
}

impl Render for RequestBody {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_h(px(120.0))
            .p_3()
            .bg(gpui::rgb(0xffffff))
            .border_1()
            .border_color(gpui::rgb(0xdee2e6))
            .rounded_md()
            .text_sm()
            .when_some(self.body.clone(), |this, body| this.child(body))
    }
}
