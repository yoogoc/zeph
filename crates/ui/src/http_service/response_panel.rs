use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputState};
use gpui_component::tab::{Tab, TabBar};
use gpui_component::StyledExt;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum ResponseTab {
    #[default]
    Body,
    Headers,
    Request,
}

pub struct ResponsePanel {
    response: Entity<InputState>,
    headers: Entity<InputState>,
    request: Entity<InputState>,
    active_tab: ResponseTab,
}

impl ResponsePanel {
    pub fn new(
        response: Entity<InputState>,
        headers: Entity<InputState>,
        request: Entity<InputState>,
    ) -> Self {
        Self {
            response,
            headers,
            request,
            active_tab: ResponseTab::default(),
        }
    }
}

impl Render for ResponsePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;

        div()
            .v_flex()
            .gap_2()
            .flex_1()
            .min_h_0()
            .child(
                TabBar::new("response_tabs")
                    .child(Tab::new().label("Body"))
                    .child(Tab::new().label("Headers"))
                    .child(Tab::new().label("Request"))
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
                    .min_h_0()
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
