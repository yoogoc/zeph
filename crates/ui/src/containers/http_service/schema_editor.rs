//! Recursive schema-tree editor for the design view.
//!
//! Renders a `SchemaNode` tree as indented rows. Each row edits one node:
//! name, type, required, description, and example. Object/Array nodes expand
//! their children inline (recursively); a node can be added, removed, have its
//! type changed (via a select that also lists "reference schema" options), or
//! be extracted into a named schema (Ref).
//!
//! The tree is held as plain data; per-node text inputs and the type select are
//! cached by node id and their events write back into the tree. The owner reads
//! the edited tree via `root()`.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{Select, SelectEvent, SelectItem, SelectState};
use gpui_component::{h_flex, v_flex, IconName, IndexPath, Sizable};
use models::{SchemaNode, SchemaNodeType};
use std::collections::HashMap;
use uuid::Uuid;

/// The built-in scalar/composite type choices (shown before any schema refs).
const BUILTIN_TYPES: &[&str] = &["string", "integer", "number", "boolean", "object", "array"];

/// One option in the per-node type select: a built-in type or a reference to a
/// named schema.
#[derive(Clone, PartialEq)]
enum TypeChoice {
    Builtin(SharedString),
    Ref(SharedString),
}

impl SelectItem for TypeChoice {
    type Value = TypeChoice;
    fn title(&self) -> SharedString {
        match self {
            TypeChoice::Builtin(s) => s.clone(),
            TypeChoice::Ref(name) => format!("$ref: {name}").into(),
        }
    }
    fn value(&self) -> &Self::Value {
        self
    }
}

fn type_label(ty: &SchemaNodeType) -> SharedString {
    match ty {
        SchemaNodeType::String => "string".into(),
        SchemaNodeType::Integer => "integer".into(),
        SchemaNodeType::Number => "number".into(),
        SchemaNodeType::Boolean => "boolean".into(),
        SchemaNodeType::Enum(_) => "string".into(),
        SchemaNodeType::Object(_) => "object".into(),
        SchemaNodeType::Array(_) => "array".into(),
        SchemaNodeType::Ref(name) => format!("$ref: {name}").into(),
    }
}

/// Builds a node type from a chosen option, preserving children where sensible.
fn type_from_choice(choice: &TypeChoice, prev: &SchemaNodeType) -> SchemaNodeType {
    match choice {
        TypeChoice::Ref(name) => SchemaNodeType::Ref(name.clone()),
        TypeChoice::Builtin(label) => match label.as_ref() {
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
        },
    }
}

pub struct SchemaEditor {
    root: SchemaNode,
    /// Names of available named schemas, offered as `$ref` type options.
    schema_names: Vec<SharedString>,
    /// Cached text inputs, keyed by node id then field ("name"/"example"/"description").
    inputs: HashMap<(Uuid, &'static str), Entity<InputState>>,
    /// Cached type selects, keyed by node id.
    type_selects: HashMap<Uuid, Entity<SelectState<Vec<TypeChoice>>>>,
    subscriptions: Vec<Subscription>,
    /// Whether the root itself is editable as a field (false for a body root).
    root_is_field: bool,
}

impl SchemaEditor {
    pub fn new(root: SchemaNode, schema_names: Vec<SharedString>, cx: &mut Context<Self>) -> Self {
        let _ = cx;
        Self {
            root,
            schema_names,
            inputs: HashMap::new(),
            type_selects: HashMap::new(),
            subscriptions: Vec::new(),
            root_is_field: false,
        }
    }

    /// The current edited tree.
    pub fn root(&self) -> &SchemaNode {
        &self.root
    }

    /// Replaces the tree (e.g. when the design is reloaded). Clears all caches.
    #[allow(dead_code)]
    pub fn set_root(&mut self, root: SchemaNode, cx: &mut Context<Self>) {
        self.root = root;
        self.inputs.clear();
        self.type_selects.clear();
        self.subscriptions.clear();
        cx.notify();
    }

    /// All type options: the built-ins followed by one `$ref` per named schema.
    fn type_options(&self) -> Vec<TypeChoice> {
        let mut opts: Vec<TypeChoice> = BUILTIN_TYPES
            .iter()
            .map(|s| TypeChoice::Builtin((*s).into()))
            .collect();
        opts.extend(self.schema_names.iter().map(|n| TypeChoice::Ref(n.clone())));
        opts
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
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.inputs.get(&(id, field)) {
            return input.clone();
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(initial)
        });
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

    /// Gets (or lazily creates) the cached type select for a node.
    fn type_select_for(
        &mut self,
        id: Uuid,
        ty: &SchemaNodeType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SelectState<Vec<TypeChoice>>> {
        if let Some(sel) = self.type_selects.get(&id) {
            return sel.clone();
        }
        let options = self.type_options();
        let current = type_label(ty);
        let idx = options
            .iter()
            .position(|o| o.title() == current)
            .map(IndexPath::new);
        let select = cx.new(|cx| SelectState::new(options, idx, window, cx));
        let sub = cx.subscribe(
            &select,
            move |this, _, ev: &SelectEvent<Vec<TypeChoice>>, cx| {
                if let SelectEvent::Confirm(Some(choice)) = ev {
                    let choice = choice.clone();
                    this.edit_node(id, cx, |node| {
                        node.ty = type_from_choice(&choice, &node.ty);
                    });
                }
            },
        );
        self.subscriptions.push(sub);
        self.type_selects.insert(id, select.clone());
        select
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
        let is_root = parent_id.is_none();
        let is_field = parent_id.is_some() || self.root_is_field;

        let name_input = self.input_for(id, "name", node.name.as_ref(), "name", window, cx);
        let example_input = self.input_for(
            id,
            "example",
            node.example.as_deref().unwrap_or(""),
            "example",
            window,
            cx,
        );
        let description_input = self.input_for(
            id,
            "description",
            node.description.as_deref().unwrap_or(""),
            "description",
            window,
            cx,
        );
        let type_select = self.type_select_for(id, &node.ty, window, cx);

        let row = h_flex()
            .gap_2()
            .items_center()
            .pl(px(depth as f32 * 16.0))
            .when(is_field, |el| {
                el.child(div().w(px(140.)).child(Input::new(&name_input)))
            })
            // Root node: a clear "root" badge in place of the (absent) name field.
            .when(!is_field && is_root, |el| {
                el.child(
                    div().w(px(140.)).child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(rgb(0xeef2ff))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x4338ca))
                            .child("root"),
                    ),
                )
            })
            .child(
                div()
                    .w(px(130.))
                    .child(Select::new(&type_select).placeholder("type").small()),
            )
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
            .child(div().w(px(140.)).child(Input::new(&example_input)))
            .child(div().flex_1().child(Input::new(&description_input)))
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
