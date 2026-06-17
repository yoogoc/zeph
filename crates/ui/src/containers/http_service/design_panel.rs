//! Design view: edits the API contract (parameters, request body, responses)
//! separately from the debug/send values. Backed by `HttpState`'s design state.

use super::http_state::HttpState;
use super::schema_editor::SchemaEditor;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{h_flex, v_flex, IconName, Sizable};
use models::{MediaContent, Parameter, ParameterLocation, RequestBody, Response, SchemaNode};

/// Per-parameter row inputs (name / description / example), with writeback subs.
struct ParamRow {
    id: usize,
    name: Entity<InputState>,
    example: Entity<InputState>,
    _subs: [Subscription; 2],
}

pub struct DesignPanel {
    http_state: Entity<HttpState>,
    param_rows: Vec<ParamRow>,
    /// Schema editor for the (first) request body media type.
    body_editor: Entity<SchemaEditor>,
    /// Schema editors for each response, by index.
    response_editors: Vec<Entity<SchemaEditor>>,
}

impl DesignPanel {
    pub fn new(http_state: Entity<HttpState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let st = http_state.read(cx);
        let params = st.design_parameters().to_vec();
        let body_node = st
            .design_request_body()
            .and_then(|rb| rb.content.first())
            .and_then(|mc| mc.schema.clone())
            .unwrap_or_else(SchemaNode::object_root);
        let responses = st.design_responses().to_vec();
        let schema_names = st.schema_names();

        let body_editor = {
            let names = schema_names.clone();
            cx.new(|cx| SchemaEditor::new(body_node, names, cx))
        };
        let response_editors = responses
            .iter()
            .map(|r| {
                let node = r
                    .content
                    .first()
                    .and_then(|mc| mc.schema.clone())
                    .unwrap_or_else(SchemaNode::object_root);
                let names = schema_names.clone();
                cx.new(|cx| SchemaEditor::new(node, names, cx))
            })
            .collect();

        let mut this = Self {
            http_state,
            param_rows: Vec::new(),
            body_editor,
            response_editors,
        };
        for (i, p) in params.iter().enumerate() {
            let row = this.make_param_row(i, p, window, cx);
            this.param_rows.push(row);
        }
        this
    }

    fn make_param_row(
        &self,
        id: usize,
        p: &Parameter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ParamRow {
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("name")
                .default_value(p.name.as_ref())
        });
        let example = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("example")
                .default_value(p.example.as_deref().unwrap_or(""))
        });
        let name_sub = cx.subscribe(&name, |this, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.sync_params(cx);
            }
        });
        let example_sub = cx.subscribe(&example, |this, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.sync_params(cx);
            }
        });
        ParamRow {
            id,
            name,
            example,
            _subs: [name_sub, example_sub],
        }
    }

    /// Rebuilds the design parameter list from the row inputs + state metadata,
    /// and writes it back to `HttpState`.
    fn sync_params(&self, cx: &mut Context<Self>) {
        let existing = self.http_state.read(cx).design_parameters().to_vec();
        let params: Vec<Parameter> = self
            .param_rows
            .iter()
            .map(|row| {
                let prev = existing.get(row.id);
                Parameter {
                    name: row.name.read(cx).value().to_string().into(),
                    location: prev
                        .map(|p| p.location.clone())
                        .unwrap_or(ParameterLocation::Query),
                    description: prev.and_then(|p| p.description.clone()),
                    required: prev.map(|p| p.required).unwrap_or(false),
                    schema: prev.and_then(|p| p.schema.clone()),
                    example: {
                        let v = row.example.read(cx).value().to_string();
                        if v.is_empty() {
                            None
                        } else {
                            Some(v.into())
                        }
                    },
                }
            })
            .collect();
        self.http_state
            .update(cx, |s, cx| s.set_design_parameters(params, cx));
    }

    fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new = Parameter {
            name: "param".into(),
            location: ParameterLocation::Query,
            description: None,
            required: false,
            schema: None,
            example: None,
        };
        let id = self.param_rows.len();
        let row = self.make_param_row(id, &new, window, cx);
        self.param_rows.push(row);
        // Append to state.
        self.http_state.update(cx, |s, cx| {
            let mut params = s.design_parameters().to_vec();
            params.push(new);
            s.set_design_parameters(params, cx);
        });
        cx.notify();
    }

    fn set_param_location(&self, id: usize, loc: ParameterLocation, cx: &mut Context<Self>) {
        self.http_state.update(cx, |s, cx| {
            let mut params = s.design_parameters().to_vec();
            if let Some(p) = params.get_mut(id) {
                p.location = loc;
            }
            s.set_design_parameters(params, cx);
        });
    }

    fn toggle_param_required(&self, id: usize, cx: &mut Context<Self>) {
        self.http_state.update(cx, |s, cx| {
            let mut params = s.design_parameters().to_vec();
            if let Some(p) = params.get_mut(id) {
                p.required = !p.required;
            }
            s.set_design_parameters(params, cx);
        });
    }

    /// Writes the body editor's tree back into `HttpState`'s request body.
    fn sync_body(&self, cx: &mut Context<Self>) {
        let node = self.body_editor.read(cx).root().clone();
        self.http_state.update(cx, |s, cx| {
            let media_type = s
                .design_request_body()
                .and_then(|rb| rb.content.first())
                .map(|mc| mc.media_type.clone())
                .unwrap_or_else(|| "application/json".into());
            let required = s
                .design_request_body()
                .map(|rb| rb.required)
                .unwrap_or(false);
            let description = s
                .design_request_body()
                .and_then(|rb| rb.description.clone());
            s.set_design_request_body(
                Some(RequestBody {
                    description,
                    required,
                    content: vec![MediaContent {
                        media_type,
                        schema: Some(node.clone()),
                        example: None,
                    }],
                }),
                cx,
            );
        });
    }

    /// Writes all response editors' trees back into `HttpState`.
    fn sync_responses(&self, cx: &mut Context<Self>) {
        let nodes: Vec<SchemaNode> = self
            .response_editors
            .iter()
            .map(|e| e.read(cx).root().clone())
            .collect();
        self.http_state.update(cx, |s, cx| {
            let mut responses = s.design_responses().to_vec();
            for (i, resp) in responses.iter_mut().enumerate() {
                if let Some(node) = nodes.get(i) {
                    let media_type = resp
                        .content
                        .first()
                        .map(|mc| mc.media_type.clone())
                        .unwrap_or_else(|| "application/json".into());
                    resp.content = vec![MediaContent {
                        media_type,
                        schema: Some(node.clone()),
                        example: None,
                    }];
                }
            }
            s.set_design_responses(responses, cx);
        });
    }

    fn add_response(&mut self, cx: &mut Context<Self>) {
        let names = self.http_state.read(cx).schema_names();
        let editor = cx.new(|cx| SchemaEditor::new(SchemaNode::object_root(), names, cx));
        self.response_editors.push(editor);
        self.http_state.update(cx, |s, cx| {
            let mut responses = s.design_responses().to_vec();
            responses.push(Response {
                status_code: "200".into(),
                description: "".into(),
                content: vec![],
            });
            s.set_design_responses(responses, cx);
        });
        cx.notify();
    }
}

impl Render for DesignPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let params = self.http_state.read(cx).design_parameters().to_vec();
        let responses = self.http_state.read(cx).design_responses().to_vec();
        let body_editor = self.body_editor.clone();

        let section = |title: &str| {
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .pt_2()
                .child(title.to_string())
        };

        let loc_label = |l: &ParameterLocation| match l {
            ParameterLocation::Query => "query",
            ParameterLocation::Path => "path",
            ParameterLocation::Header => "header",
            ParameterLocation::Cookie => "cookie",
        };
        let next_loc = |l: &ParameterLocation| match l {
            ParameterLocation::Query => ParameterLocation::Path,
            ParameterLocation::Path => ParameterLocation::Header,
            ParameterLocation::Header => ParameterLocation::Cookie,
            ParameterLocation::Cookie => ParameterLocation::Query,
        };

        v_flex()
            .size_full()
            .gap_2()
            .p_2()
            // === Parameters ===
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(section("Parameters"))
                    .child(
                        Button::new("add_param")
                            .icon(IconName::Plus)
                            .label("Add")
                            .xsmall()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_param(window, cx)),
                            ),
                    ),
            )
            .children(self.param_rows.iter().map(|row| {
                let id = row.id;
                let loc = params
                    .get(id)
                    .map(|p| p.location.clone())
                    .unwrap_or(ParameterLocation::Query);
                let required = params.get(id).map(|p| p.required).unwrap_or(false);
                let next = next_loc(&loc);
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().w(px(140.)).child(Input::new(&row.name)))
                    .child(
                        Button::new(SharedString::from(format!("param_loc_{id}")))
                            .label(loc_label(&loc))
                            .xsmall()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_param_location(id, next.clone(), cx);
                            })),
                    )
                    .child(
                        Checkbox::new(SharedString::from(format!("param_req_{id}")))
                            .checked(required)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_param_required(id, cx);
                            })),
                    )
                    .child(div().flex_1().child(Input::new(&row.example)))
            }))
            // === Request Body ===
            .child(section("Request Body"))
            .child(
                div()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .rounded_md()
                    .p_2()
                    .child(body_editor),
            )
            .child(
                Button::new("sync_body")
                    .label("Apply body schema")
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| this.sync_body(cx))),
            )
            // === Responses ===
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(section("Responses"))
                    .child(
                        Button::new("add_response")
                            .icon(IconName::Plus)
                            .label("Add")
                            .xsmall()
                            .on_click(cx.listener(|this, _, _, cx| this.add_response(cx))),
                    ),
            )
            .children(self.response_editors.iter().enumerate().map(|(i, editor)| {
                let code = responses
                    .get(i)
                    .map(|r| r.status_code.to_string())
                    .unwrap_or_default();
                v_flex()
                    .gap_1()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .rounded_md()
                    .p_2()
                    .child(div().text_sm().child(format!("Status {code}")))
                    .child(editor.clone())
            }))
            .child(
                Button::new("sync_responses")
                    .label("Apply responses")
                    .xsmall()
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| this.sync_responses(cx))),
            )
    }
}
