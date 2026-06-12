use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputState, TabSize};
use gpui_component::tab::{Tab, TabBar, TabVariant};
use gpui_component::{Sizable, Size, StyledExt};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum BodyType {
    #[default]
    None,
    Json,
    Xml,
    Text,
}

impl BodyType {
    pub fn content_type(&self) -> Option<&'static str> {
        match self {
            BodyType::None => None,
            BodyType::Json => Some("application/json"),
            BodyType::Xml => Some("application/xml"),
            BodyType::Text => Some("text/plain"),
        }
    }

    #[allow(unused)]
    fn language(&self) -> &'static str {
        match self {
            BodyType::Json => "json",
            BodyType::Xml => "xml",
            _ => "text",
        }
    }
}

use super::http_state::HttpState;

pub struct RequestBody {
    body_type: BodyType,
    editor: Entity<InputState>,
    http_state: Entity<HttpState>,
}

impl RequestBody {
    pub fn new(http_state: Entity<HttpState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .line_number(true)
        });

        Self {
            body_type: BodyType::None,
            editor,
            http_state,
        }
    }

    #[allow(unused)]
    pub fn set_body_type(&mut self, body_type: BodyType) {
        self.body_type = body_type;
    }

    #[allow(unused)]
    pub fn set_body(
        &mut self,
        body: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let body = body.into();
        self.editor
            .update(cx, |state, cx| state.set_value(body, window, cx));
    }

    pub fn body(&self, cx: &App) -> String {
        self.editor.read(cx).value().to_string()
    }

    pub fn body_type(&self) -> BodyType {
        self.body_type
    }
}

impl Render for RequestBody {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body_type = self.body_type;

        div()
            .v_flex()
            .gap_2()
            .child(
                TabBar::new("body_type_tabs")
                    .with_variant(TabVariant::Pill)
                    .with_size(Size::Small)
                    .child(Tab::new().label("none"))
                    .child(Tab::new().label("JSON"))
                    .child(Tab::new().label("XML"))
                    .child(Tab::new().label("Text"))
                    .selected_index(body_type as usize)
                    .on_click(cx.listener(|this, ix: &usize, window, cx| {
                        let new_type = match *ix {
                            1 => BodyType::Json,
                            2 => BodyType::Xml,
                            3 => BodyType::Text,
                            _ => BodyType::None,
                        };
                        this.body_type = new_type;
                        this.http_state
                            .update(cx, |s, cx| s.on_body_type_changed(new_type, window, cx));
                        cx.notify();
                    })),
            )
            .when(body_type != BodyType::None, |el| {
                el.child(div().h(px(200.0)).child(Input::new(&self.editor).h_full()))
            })
            .when(body_type == BodyType::None, |el| {
                el.child(
                    div()
                        .p_4()
                        .text_center()
                        .text_color(gpui::rgb(0x6c757d))
                        .text_sm()
                        .child(shared_t!("request.no_body")),
                )
            })
    }
}
