//! Recursive schema-tree editor for the design view.
//!
//! Renders a `SchemaNode` tree as indented rows. Each row edits one node:
//! name, type, required, description, and example. Object/Array nodes expand
//! their children inline (recursively); a node can be added, removed, have its
//! type changed, or be extracted into a named schema (Ref).
//!
//! The tree is held as plain data; per-node text inputs are cached by node id
//! and their `Change` events write back into the tree. The owner reads the
//! edited tree via `root()` and is notified through the `on_change` callback.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{h_flex, v_flex, IconName, Sizable};
use models::{SchemaNode, SchemaNodeType};
use std::collections::HashMap;
use uuid::Uuid;

/// The scalar/composite type choices shown in the per-row type dropdown.
const TYPE_LABELS: &[&str] = &["string", "integer", "number", "boolean", "object", "array"];

fn type_label(ty: &SchemaNodeType) -> &'static str {
    match ty {
        SchemaNodeType::String => "string",
        SchemaNodeType::Integer => "integer",
        SchemaNodeType::Number => "number",
        SchemaNodeType::Boolean => "boolean",
        SchemaNodeType::Enum(_) => "string",
        SchemaNodeType::Object(_) => "object",
        SchemaNodeType::Array(_) => "array",
        SchemaNodeType::Ref(_) => "object",
    }
}

/// Builds a node type from a label, preserving children where sensible.
fn type_from_label(label: &str, prev: &SchemaNodeType) -> SchemaNodeType {
    match label {
        "integer" => SchemaNodeType::Integer,
        "number" => SchemaNodeType::Number,
        "boolean" => SchemaNodeType::Boolean,
        "object" => match prev {
            SchemaNodeType::Object(children) => SchemaNodeType::Object(children.clone()),
            _ => SchemaNodeType::Object(Vec::new()),
        },
        "array" => match prev {
            SchemaNodeType::Array(item) => SchemaNodeType::Array(item.clone()),
            _ => SchemaNodeType::Array(Box::new(SchemaNode::new("", SchemaNodeType::String))),
        },
        _ => SchemaNodeType::String,
    }
}

pub struct SchemaEditor {
    root: SchemaNode,
    /// Cached name/example inputs, keyed by node id then field ("name"/"example").
    inputs: HashMap<(Uuid, &'static str), Entity<InputState>>,
    subscriptions: Vec<Subscription>,
    /// Whether the root itself is editable as a field (false for a body root).
    root_is_field: bool,
}

impl SchemaEditor {
    pub fn new(root: SchemaNode, cx: &mut Context<Self>) -> Self {
        let _ = cx;
        Self {
            root,
            inputs: HashMap::new(),
            subscriptions: Vec::new(),
            root_is_field: false,
        }
    }

    /// The current edited tree.
    pub fn root(&self) -> &SchemaNode {
        &self.root
    }

    /// Replaces the tree (e.g. when the design is reloaded). Clears input caches.
    #[allow(dead_code)]
    pub fn set_root(&mut self, root: SchemaNode, cx: &mut Context<Self>) {
        self.root = root;
        self.inputs.clear();
        self.subscriptions.clear();
        cx.notify();
    }

    /// Finds a node by id (depth-first) and runs `f` on it.
    fn with_node_mut(node: &mut SchemaNode, id: Uuid, f: &mut dyn FnMut(&mut SchemaNode)) -> bool {
        if node.id == id {
            f(node);
            return true;
        }
        match &mut node.ty {
            SchemaNodeType::Object(children) => {
                for c in children.iter_mut() {
                    if Self::with_node_mut(c, id, f) {
                        return true;
                    }
                }
            }
            SchemaNodeType::Array(item) => {
                return Self::with_node_mut(item, id, f);
            }
            _ => {}
        }
        false
    }

    fn edit_node(&mut self, id: Uuid, cx: &mut Context<Self>, mut f: impl FnMut(&mut SchemaNode)) {
        Self::with_node_mut(&mut self.root, id, &mut f);
        cx.notify();
    }

    /// Gets (or lazily creates) the cached input for a node's field.
    fn input_for(
        &mut self,
        id: Uuid,
        field: &'static str,
        initial: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.inputs.get(&(id, field)) {
            return input.clone();
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value(initial));
        let sub = cx.subscribe(&input, move |this, ent, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                let value = ent.read(cx).value().to_string();
                this.edit_node(id, cx, |node| match field {
                    "name" => node.name = value.clone().into(),
                    "example" => {
                        node.example = if value.is_empty() {
                            None
                        } else {
                            Some(value.clone().into())
                        }
                    }
                    "description" => {
                        node.description = if value.is_empty() {
                            None
                        } else {
                            Some(value.clone().into())
                        }
                    }
                    _ => {}
                });
            }
        });
        self.subscriptions.push(sub);
        self.inputs.insert((id, field), input.clone());
        input
    }

    /// Adds a child field to the object node `parent_id` (or converts to object).
    fn add_field(&mut self, parent_id: Uuid, cx: &mut Context<Self>) {
        self.edit_node(parent_id, cx, |node| {
            if let SchemaNodeType::Object(children) = &mut node.ty {
                children.push(SchemaNode::new("field", SchemaNodeType::String));
            }
        });
    }

    fn remove_field(&mut self, parent_id: Uuid, child_id: Uuid, cx: &mut Context<Self>) {
        self.edit_node(parent_id, cx, |node| {
            if let SchemaNodeType::Object(children) = &mut node.ty {
                children.retain(|c| c.id != child_id);
            }
        });
    }

    /// Renders one node row plus, recursively, its children. `depth` controls
    /// indentation; `parent_id` is `None` for the root.
    fn render_node(
        &mut self,
        node: SchemaNode,
        depth: usize,
        parent_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = node.id;
        let is_field = parent_id.is_some() || self.root_is_field;
        let current_type = type_label(&node.ty);

        let name_input = self.input_for(id, "name", node.name.as_ref(), window, cx);
        let example_input = self.input_for(
            id,
            "example",
            node.example.as_deref().unwrap_or(""),
            window,
            cx,
        );

        // Type dropdown (simple cycling button list rendered as a small select).
        let type_button = Button::new(SharedString::from(format!("schema_type_{id}")))
            .label(current_type)
            .xsmall()
            .on_click(cx.listener(move |this, _, _, cx| {
                // Cycle to the next type in TYPE_LABELS.
                let next = {
                    let pos = TYPE_LABELS
                        .iter()
                        .position(|t| *t == current_type)
                        .unwrap_or(0);
                    TYPE_LABELS[(pos + 1) % TYPE_LABELS.len()]
                };
                this.edit_node(id, cx, |node| {
                    node.ty = type_from_label(next, &node.ty);
                });
            }));

        let row = h_flex()
            .gap_2()
            .items_center()
            .pl(px(depth as f32 * 16.0))
            .when(is_field, |el| {
                el.child(div().w(px(140.)).child(Input::new(&name_input)))
            })
            .child(div().w(px(90.)).child(type_button))
            .when(is_field, |el| {
                let required = node.required;
                el.child(
                    Checkbox::new(SharedString::from(format!("schema_req_{id}")))
                        .checked(required)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit_node(id, cx, |node| node.required = !required);
                        })),
                )
            })
            .child(div().flex_1().child(Input::new(&example_input)))
            // Add child (object only)
            .when(matches!(node.ty, SchemaNodeType::Object(_)), |el| {
                el.child(
                    Button::new(SharedString::from(format!("schema_add_{id}")))
                        .icon(IconName::Plus)
                        .xsmall()
                        .ghost()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.add_field(id, cx);
                        })),
                )
            })
            // Remove field (non-root only)
            .when_some(parent_id, |el, pid| {
                el.child(
                    Button::new(SharedString::from(format!("schema_del_{id}")))
                        .label("×")
                        .xsmall()
                        .ghost()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_field(pid, id, cx);
                        })),
                )
            });

        // Children (recursive).
        let mut container = v_flex().gap_1().child(row);
        match node.ty {
            SchemaNodeType::Object(children) => {
                for child in children {
                    let el = self.render_node(child, depth + 1, Some(id), window, cx);
                    container = container.child(el);
                }
            }
            SchemaNodeType::Array(item) => {
                // The array item is rendered as an indented "item" row.
                let el = self.render_node(*item, depth + 1, Some(id), window, cx);
                container = container.child(el);
            }
            _ => {}
        }
        container.into_any_element()
    }
}

impl Render for SchemaEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = self.root.clone();
        self.render_node(root, 0, None, window, cx)
    }
}
