//! SchemaDetailPage: maintenance page for a named `models::Schema`.
//!
//! Edits the schema's name, description, and field tree (via the recursive
//! `SchemaEditor`). Changes are buffered in this view; a "Save" button writes
//! them back through `ServiceState::update_schema`. The field tree is bridged
//! to/from `SchemaNode` on load/save (`SchemaNode::from_schema` /
//! `write_into_schema`).

use crate::containers::http_service::SchemaEditor;
use crate::state::ServiceState;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{h_flex, v_flex, IconName};
use models::{Schema, SchemaNode};
use uuid::Uuid;

pub struct SchemaDetailPage {
    service_state: Entity<ServiceState>,
    /// Snapshot of the schema this view renders; used for tab title/id.
    schema: Schema,
    name_input: Entity<InputState>,
    description_input: Entity<InputState>,
    /// Recursive editor over the schema's top-level field tree.
    fields_editor: Entity<SchemaEditor>,
}

impl SchemaDetailPage {
    pub fn new(
        service_state: Entity<ServiceState>,
        schema: Schema,
        schema_names: Vec<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Schema name")
                .default_value(schema.name.as_ref())
        });
        let description_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Description")
                .default_value(schema.description.as_deref().unwrap_or(""))
        });
        let root = SchemaNode::from_schema(&schema);
        let fields_editor = cx.new(|cx| SchemaEditor::new(root, schema_names, cx));

        Self {
            service_state,
            schema,
            name_input,
            description_input,
            fields_editor,
        }
    }

    /// Collects the edited name/description/field-tree and persists via state.
    fn save(&mut self, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().to_string();
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.len() > 255 {
            return;
        }
        let description = self.description_input.read(cx).value().to_string();
        let node = self.fields_editor.read(cx).root().clone();

        let mut updated = self.schema.clone();
        updated.name = trimmed.to_string().into();
        updated.description = if description.trim().is_empty() {
            None
        } else {
            Some(description.into())
        };
        node.write_into_schema(&mut updated);

        self.service_state
            .update(cx, |s, cx| s.update_schema(updated.clone(), cx));
        self.schema = updated;
        cx.notify();
    }
}

impl Render for SchemaDetailPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // If the schema was deleted out from under us, show an empty state.
        let exists = self
            .service_state
            .read(cx)
            .get_schema(self.schema.id)
            .is_some();
        if !exists {
            return div()
                .flex_1()
                .h_full()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(rgb(0x9ca3af))
                        .child("This schema no longer exists."),
                )
                .into_any_element();
        }

        let label = |text: &str| {
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .pt_2()
                .child(text.to_string())
        };

        v_flex()
            .size_full()
            .py_2()
            .px_4()
            .gap_2()
            // Header: name + Save
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.name_input)))
                    .child(
                        Button::new("save_schema")
                            .icon(IconName::Check)
                            .label("Save")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    ),
            )
            .child(label("Description"))
            .child(Input::new(&self.description_input))
            // Field tree
            .child(label("Fields"))
            .child(
                div()
                    .flex_1()
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .rounded_md()
                    .p_2()
                    .child(self.fields_editor.clone()),
            )
            .into_any_element()
    }
}

// Schema edits are buffered and persisted via the in-page Save button, so the
// default `ContentView` save/is_modified behavior is sufficient.
impl crate::state::ContentView for SchemaDetailPage {
    fn id(&self) -> Uuid {
        self.schema.id
    }

    fn title(&self) -> SharedString {
        self.schema.name.clone()
    }

    fn tab_icon(&self) -> AnyElement {
        IconName::LayoutDashboard.into_any_element()
    }

    fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {
        self.schema.name = title;
        cx.notify();
    }
}
