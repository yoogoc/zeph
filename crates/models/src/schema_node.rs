//! Recursive, inline-nestable schema tree for API design.
//!
//! `SchemaNode` is the design-time structure describing the shape of a request
//! parameter, request body, or response. It supports inline nesting (an object
//! field directly holds its child fields) as the primary model, plus
//! `Ref(name)` for fields extracted into a named [`crate::Schema`] (the hybrid
//! model). Conversions to/from JSON Schema and example JSON live here as pure,
//! unit-testable functions — no UI involved.

use gpui::SharedString;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use uuid::Uuid;

fn default_node_id() -> Uuid {
    Uuid::new_v4()
}

/// A node in an inline, recursively-nestable schema tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaNode {
    /// Stable id for CRDT list merging / UI row identity.
    #[serde(default = "default_node_id")]
    pub id: Uuid,
    /// Field name. Empty for an array's item node or a root node.
    #[serde(default)]
    pub name: SharedString,
    pub ty: SchemaNodeType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default)]
    pub description: Option<SharedString>,
    /// Design-time example value for this field (as a JSON-encoded string).
    #[serde(default)]
    pub example: Option<SharedString>,
    #[serde(default)]
    pub default: Option<SharedString>,
}

/// The type of a [`SchemaNode`]. Object/Array carry their nested structure
/// inline; `Ref` points to a named schema extracted out of the tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum SchemaNodeType {
    #[default]
    String,
    Integer,
    Number,
    Boolean,
    Enum(Vec<SharedString>),
    /// Inline object: its properties are child nodes.
    Object(Vec<SchemaNode>),
    /// Array whose item shape is described by the boxed node.
    Array(Box<SchemaNode>),
    /// Reference to a named schema (by name), e.g. an extracted sub-tree.
    Ref(SharedString),
}

impl SchemaNode {
    /// A fresh scalar string node with the given name.
    pub fn new(name: impl Into<SharedString>, ty: SchemaNodeType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            ty,
            required: false,
            nullable: false,
            description: None,
            example: None,
            default: None,
        }
    }

    /// An empty object root node (the common starting point for a body).
    pub fn object_root() -> Self {
        Self::new("", SchemaNodeType::Object(Vec::new()))
    }

    // ===== JSON Schema conversion =====

    /// Renders this node as a JSON Schema value.
    pub fn to_json_schema(&self) -> Value {
        let mut obj = Map::new();
        match &self.ty {
            SchemaNodeType::String => {
                obj.insert("type".into(), json!("string"));
            }
            SchemaNodeType::Integer => {
                obj.insert("type".into(), json!("integer"));
            }
            SchemaNodeType::Number => {
                obj.insert("type".into(), json!("number"));
            }
            SchemaNodeType::Boolean => {
                obj.insert("type".into(), json!("boolean"));
            }
            SchemaNodeType::Enum(values) => {
                obj.insert("type".into(), json!("string"));
                obj.insert(
                    "enum".into(),
                    Value::Array(values.iter().map(|v| json!(v.to_string())).collect()),
                );
            }
            SchemaNodeType::Object(props) => {
                obj.insert("type".into(), json!("object"));
                let mut properties = Map::new();
                let mut required = Vec::new();
                for p in props {
                    properties.insert(p.name.to_string(), p.to_json_schema());
                    if p.required {
                        required.push(json!(p.name.to_string()));
                    }
                }
                obj.insert("properties".into(), Value::Object(properties));
                if !required.is_empty() {
                    obj.insert("required".into(), Value::Array(required));
                }
            }
            SchemaNodeType::Array(item) => {
                obj.insert("type".into(), json!("array"));
                obj.insert("items".into(), item.to_json_schema());
            }
            SchemaNodeType::Ref(name) => {
                obj.insert("$ref".into(), json!(format!("#/components/schemas/{name}")));
                return Value::Object(obj);
            }
        }
        if self.nullable {
            obj.insert("nullable".into(), json!(true));
        }
        if let Some(d) = &self.description {
            obj.insert("description".into(), json!(d.to_string()));
        }
        if let Some(e) = &self.example {
            if let Ok(v) = serde_json::from_str::<Value>(e) {
                obj.insert("example".into(), v);
            } else {
                obj.insert("example".into(), json!(e.to_string()));
            }
        }
        Value::Object(obj)
    }

    /// Parses a JSON Schema value into a node tree. `name` is the field name to
    /// assign to the resulting node (empty for roots/array items).
    pub fn from_json_schema(name: impl Into<SharedString>, schema: &Value) -> Self {
        let name = name.into();
        let obj = match schema.as_object() {
            Some(o) => o,
            None => return SchemaNode::new(name, SchemaNodeType::String),
        };

        // $ref → Ref(name)
        if let Some(Value::String(r)) = obj.get("$ref") {
            let ref_name = r.rsplit('/').next().unwrap_or(r).to_string();
            return SchemaNode::new(name, SchemaNodeType::Ref(ref_name.into()));
        }

        let ty = match obj.get("type").and_then(|t| t.as_str()) {
            Some("object") => {
                let required: Vec<String> = obj
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut props = Vec::new();
                if let Some(Value::Object(p)) = obj.get("properties") {
                    for (k, v) in p {
                        let mut child = SchemaNode::from_json_schema(k.clone(), v);
                        child.required = required.iter().any(|r| r == k);
                        props.push(child);
                    }
                }
                SchemaNodeType::Object(props)
            }
            Some("array") => {
                let item = obj
                    .get("items")
                    .map(|i| SchemaNode::from_json_schema("", i))
                    .unwrap_or_else(|| SchemaNode::new("", SchemaNodeType::String));
                SchemaNodeType::Array(Box::new(item))
            }
            Some("integer") => SchemaNodeType::Integer,
            Some("number") => SchemaNodeType::Number,
            Some("boolean") => SchemaNodeType::Boolean,
            _ => {
                if let Some(Value::Array(values)) = obj.get("enum") {
                    SchemaNodeType::Enum(
                        values
                            .iter()
                            .map(|v| match v {
                                Value::String(s) => s.clone().into(),
                                other => other.to_string().into(),
                            })
                            .collect(),
                    )
                } else {
                    SchemaNodeType::String
                }
            }
        };

        let mut node = SchemaNode::new(name, ty);
        node.nullable = obj
            .get("nullable")
            .and_then(|n| n.as_bool())
            .unwrap_or(false);
        node.description = obj
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string().into());
        node.example = obj.get("example").map(|e| e.to_string().into());
        node
    }

    // ===== Example JSON generation (Design → Debug default values) =====

    /// Builds an example JSON value for this node, using each node's `example`
    /// / `default` where present, else a type-appropriate zero value. `Ref`
    /// nodes resolve to a placeholder (the named schema isn't inlined here).
    pub fn to_example_json(&self) -> Value {
        if let Some(e) = self.example.as_ref().or(self.default.as_ref()) {
            if let Ok(v) = serde_json::from_str::<Value>(e) {
                return v;
            }
            // A non-JSON example string is taken literally for scalar types.
            if !matches!(
                self.ty,
                SchemaNodeType::Object(_) | SchemaNodeType::Array(_)
            ) {
                return json!(e.to_string());
            }
        }
        match &self.ty {
            SchemaNodeType::String => json!(""),
            SchemaNodeType::Integer => json!(0),
            SchemaNodeType::Number => json!(0.0),
            SchemaNodeType::Boolean => json!(false),
            SchemaNodeType::Enum(values) => values
                .first()
                .map(|v| json!(v.to_string()))
                .unwrap_or(json!("")),
            SchemaNodeType::Object(props) => {
                let mut map = Map::new();
                for p in props {
                    map.insert(p.name.to_string(), p.to_example_json());
                }
                Value::Object(map)
            }
            SchemaNodeType::Array(item) => json!([item.to_example_json()]),
            SchemaNodeType::Ref(name) => json!(format!("<{name}>")),
        }
    }

    // ===== Debug value → design example write-back (Save as example) =====

    /// Writes runtime values from `value` back into the tree's `example` fields,
    /// matching object properties by name and arrays by their item shape. Used
    /// by "save debug value as example".
    pub fn apply_example(&mut self, value: &Value) {
        match (&mut self.ty, value) {
            (SchemaNodeType::Object(props), Value::Object(map)) => {
                for p in props.iter_mut() {
                    if let Some(v) = map.get(p.name.as_ref()) {
                        p.apply_example(v);
                    }
                }
            }
            (SchemaNodeType::Array(item), Value::Array(arr)) => {
                if let Some(first) = arr.first() {
                    item.apply_example(first);
                }
                self.example = Some(value.to_string().into());
            }
            // Scalars / refs: store the JSON-encoded value as the example.
            _ => {
                self.example = Some(value.to_string().into());
            }
        }
    }
}

// ===== Extract / inline (Object subtree ⇄ named schema Ref) =====

/// An object subtree extracted into a named schema definition. The caller
/// persists this as a [`crate::Schema`]-equivalent and replaces the original
/// node with the returned [`SchemaNodeType::Ref`].
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedSchema {
    pub name: SharedString,
    /// The extracted object node (its `ty` is `Object`).
    pub node: SchemaNode,
}

/// Extracts `node` (which must be an object) into a named schema, replacing the
/// original in-place with a `Ref(name)`. Returns the extracted definition, or
/// `None` if the node is not an object.
pub fn extract_to_named(
    node: &mut SchemaNode,
    name: impl Into<SharedString>,
) -> Option<ExtractedSchema> {
    if !matches!(node.ty, SchemaNodeType::Object(_)) {
        return None;
    }
    let name = name.into();
    let mut extracted = node.clone();
    extracted.name = name.clone();
    node.ty = SchemaNodeType::Ref(name.clone());
    Some(ExtractedSchema {
        name,
        node: extracted,
    })
}

/// Inlines a `Ref(name)` node back into the given object definition (the
/// reverse of [`extract_to_named`]). Returns `true` if it was a matching ref.
pub fn inline_ref(node: &mut SchemaNode, name: &str, definition: &SchemaNode) -> bool {
    if let SchemaNodeType::Ref(r) = &node.ty {
        if r.as_ref() == name {
            let field_name = node.name.clone();
            *node = definition.clone();
            node.name = field_name;
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_user() -> SchemaNode {
        let mut name = SchemaNode::new("name", SchemaNodeType::String);
        name.required = true;
        name.example = Some("\"ada\"".into());
        let age = SchemaNode::new("age", SchemaNodeType::Integer);
        let mut tags = SchemaNode::new(
            "tags",
            SchemaNodeType::Array(Box::new(SchemaNode::new("", SchemaNodeType::String))),
        );
        tags.required = false;
        SchemaNode::new("", SchemaNodeType::Object(vec![name, age, tags]))
    }

    #[test]
    fn json_schema_round_trips_nested() {
        let node = sample_user();
        let schema = node.to_json_schema();
        let back = SchemaNode::from_json_schema("", &schema);

        // Structure round-trips (ids differ, so compare shapes).
        assert!(matches!(back.ty, SchemaNodeType::Object(_)));
        if let SchemaNodeType::Object(props) = &back.ty {
            assert_eq!(props.len(), 3);
            assert_eq!(props[0].name.as_ref(), "name");
            assert!(props[0].required);
            assert!(matches!(props[0].ty, SchemaNodeType::String));
            assert!(matches!(props[1].ty, SchemaNodeType::Integer));
            assert!(matches!(props[2].ty, SchemaNodeType::Array(_)));
        }
    }

    #[test]
    fn json_schema_required_array_emitted() {
        let node = sample_user();
        let schema = node.to_json_schema();
        let required = schema.get("required").and_then(|r| r.as_array()).unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], json!("name"));
    }

    #[test]
    fn ref_round_trips() {
        let node = SchemaNode::new("user", SchemaNodeType::Ref("User".into()));
        let schema = node.to_json_schema();
        assert_eq!(schema["$ref"], json!("#/components/schemas/User"));
        let back = SchemaNode::from_json_schema("user", &schema);
        assert!(matches!(back.ty, SchemaNodeType::Ref(r) if r.as_ref() == "User"));
    }

    #[test]
    fn example_json_uses_examples_and_defaults() {
        let node = sample_user();
        let ex = node.to_example_json();
        assert_eq!(ex["name"], json!("ada")); // from example
        assert_eq!(ex["age"], json!(0)); // type zero
        assert_eq!(ex["tags"], json!([""])); // array of string
    }

    #[test]
    fn apply_example_writes_back_by_name() {
        let mut node = sample_user();
        let value = json!({ "name": "grace", "age": 42 });
        node.apply_example(&value);
        if let SchemaNodeType::Object(props) = &node.ty {
            assert_eq!(props[0].example.as_deref(), Some("\"grace\""));
            assert_eq!(props[1].example.as_deref(), Some("42"));
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn extract_and_inline_round_trip() {
        let mut root = SchemaNode::new(
            "",
            SchemaNodeType::Object(vec![SchemaNode::new(
                "address",
                SchemaNodeType::Object(vec![SchemaNode::new("city", SchemaNodeType::String)]),
            )]),
        );
        // Extract the `address` object.
        if let SchemaNodeType::Object(props) = &mut root.ty {
            let extracted = extract_to_named(&mut props[0], "Address").expect("extract");
            assert!(matches!(props[0].ty, SchemaNodeType::Ref(ref r) if r.as_ref() == "Address"));
            assert_eq!(props[0].name.as_ref(), "address"); // field name preserved
            assert_eq!(extracted.name.as_ref(), "Address");

            // Inline it back.
            let ok = inline_ref(&mut props[0], "Address", &extracted.node);
            assert!(ok);
            assert!(matches!(props[0].ty, SchemaNodeType::Object(_)));
            assert_eq!(props[0].name.as_ref(), "address");
        }
    }
}
