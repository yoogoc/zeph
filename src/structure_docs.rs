//! Presentation of an applied contract, shared by documentation and value suggestions.
use super::{RequestStructure, Section};
use serde_json::Value;

#[derive(Clone, Debug, Default)]
pub struct SchemaNode {
    pub name: String,
    pub kind: String,
    pub location: Option<String>,
    pub required: Option<bool>,
    pub description: String,
    pub values: Vec<String>,
    pub details: Vec<(String, String)>,
    pub reference: Option<String>,
    pub children: Vec<Self>,
}

impl RequestStructure {
    /// Match the wire location as well as the name; header names are case insensitive.
    pub fn value_suggestions(&self, location: &str, name: &str) -> Vec<String> {
        if matches!(location, "formData" | "form") {
            let Some(body) = &self.body else {
                return vec![];
            };
            if let Some(parameters) = body.get("parameters").and_then(Value::as_array) {
                return parameters
                    .iter()
                    .find(|p| p["in"] == "formData" && p["name"] == name)
                    .map(|p| choices(p.get("schema").unwrap_or(p)))
                    .unwrap_or_default();
            }
            let media = if location == "formData" {
                "multipart/form-data"
            } else {
                "application/x-www-form-urlencoded"
            };
            return body
                .get("content")
                .and_then(|c| c.get(media))
                .and_then(|c| c.get("schema"))
                .map(|schema| property_choices(schema, name))
                .unwrap_or_default();
        }
        self.parameters
            .iter()
            .find(|p| {
                p["in"] == location
                    && p["name"].as_str().is_some_and(|n| {
                        if location == "header" {
                            n.eq_ignore_ascii_case(name)
                        } else {
                            n == name
                        }
                    })
            })
            .map(|p| choices(p.get("schema").unwrap_or(p)))
            .unwrap_or_default()
    }

    pub fn documentation(&self, section: Section) -> Vec<SchemaNode> {
        let mut budget = 2000;
        match section {
            Section::Parameters => self
                .parameters
                .iter()
                .map(|p| parameter(p, &mut budget))
                .collect(),
            Section::Body => self
                .body
                .as_ref()
                .map(|body| body_nodes(body, &mut budget))
                .unwrap_or_default(),
            Section::Responses => self
                .responses
                .iter()
                .map(|(status, response)| {
                    let mut children = content_nodes(response, &mut budget);
                    if let Some(schema) = response.get("schema") {
                        children.push(schema_node("响应体", schema, 0, &mut budget));
                    }
                    if let Some(headers) = response.get("headers").and_then(Value::as_object) {
                        children.push(SchemaNode {
                            name: "响应头".into(),
                            kind: "headers".into(),
                            children: headers
                                .iter()
                                .map(|(name, header)| {
                                    let mut node = schema_node(
                                        name,
                                        header.get("schema").unwrap_or(header),
                                        0,
                                        &mut budget,
                                    );
                                    node.description = text(header, "description").to_owned();
                                    node
                                })
                                .collect(),
                            ..Default::default()
                        });
                    }
                    SchemaNode {
                        name: status.clone(),
                        kind: "response".into(),
                        description: text(response, "description").into(),
                        reference: response
                            .get("$ref")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        children,
                        ..Default::default()
                    }
                })
                .collect(),
            Section::Models => self
                .models
                .iter()
                .map(|(reference, schema)| {
                    let name = reference
                        .rsplit('/')
                        .next()
                        .unwrap_or(reference)
                        .replace("~1", "/")
                        .replace("~0", "~");
                    let mut node = schema_node(&name, schema, 0, &mut budget);
                    node.location = Some("模型".into());
                    node
                })
                .collect(),
        }
    }
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(_) | Value::Bool(_) | Value::Null => Some(value.to_string()),
        _ => None,
    }
}
fn add_unique(result: &mut Vec<String>, value: String) {
    if !result.contains(&value) {
        result.push(value);
    }
}
fn choices(schema: &Value) -> Vec<String> {
    fn intersect(result: &mut Option<Vec<Value>>, values: Vec<Value>) {
        if let Some(current) = result {
            current.retain(|v| values.contains(v));
        } else {
            *result = Some(values);
        }
    }
    fn collect(schema: &Value, depth: usize) -> Option<Vec<Value>> {
        if depth > 10 {
            return None;
        }
        let mut result = schema.get("enum").and_then(Value::as_array).cloned();
        if let Some(value) = schema.get("const") {
            intersect(&mut result, vec![value.clone()]);
        }
        if schema["type"] == "boolean" {
            intersect(&mut result, vec![Value::Bool(true), Value::Bool(false)]);
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(branches) = schema.get(key).and_then(Value::as_array) {
                let mut union = vec![];
                for branch in branches {
                    if let Some(values) = collect(branch, depth + 1) {
                        union.extend(values);
                    }
                }
                if !union.is_empty() {
                    intersect(&mut result, union);
                }
            }
        }
        if let Some(branches) = schema.get("allOf").and_then(Value::as_array) {
            for branch in branches {
                if let Some(values) = collect(branch, depth + 1) {
                    intersect(&mut result, values);
                }
            }
        }
        result
    }
    collect(schema, 0)
        .into_iter()
        .flatten()
        .filter_map(|v| scalar(&v))
        .fold(vec![], |mut result, value| {
            add_unique(&mut result, value);
            result
        })
}
fn property_choices(schema: &Value, name: &str) -> Vec<String> {
    if let Some(property) = schema.get("properties").and_then(|p| p.get(name)) {
        return choices(property);
    }
    schema
        .get("allOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|branch| property_choices(branch, name))
        .fold(vec![], |mut result, value| {
            add_unique(&mut result, value);
            result
        })
}
fn kind(schema: &Value) -> String {
    if let Some(allowed) = schema.as_bool() {
        return if allowed { "任意值" } else { "不允许" }.into();
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        return types
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" | ");
    }
    let mut value = text(schema, "type").to_owned();
    if value.is_empty() {
        value = if schema.get("properties").is_some() {
            "object"
        } else if schema.get("items").is_some() {
            "array"
        } else if schema.get("oneOf").is_some() {
            "oneOf"
        } else if schema.get("anyOf").is_some() {
            "anyOf"
        } else if schema.get("allOf").is_some() {
            "allOf"
        } else if schema.get("$ref").is_some() {
            "引用"
        } else {
            "未指定类型"
        }
        .into();
    }
    if schema["nullable"] == true && value != "null" {
        value.push_str(" | null");
    }
    let format = text(schema, "format");
    if !format.is_empty() {
        value.push_str(&format!(" · {format}"));
    }
    value
}
fn summary(value: &Value) -> String {
    let result = match value {
        Value::Object(v) => format!("对象 · {} 个字段", v.len()),
        Value::Array(v) => format!("数组 · {} 项", v.len()),
        _ => scalar(value).unwrap_or_default(),
    };
    if result.chars().count() > 140 {
        format!("{}…", result.chars().take(140).collect::<String>())
    } else {
        result
    }
}
fn details(schema: &Value) -> Vec<(String, String)> {
    let mut result = vec![];
    for (key, label) in [
        ("default", "默认值"),
        ("example", "示例"),
        ("minimum", "最小值"),
        ("maximum", "最大值"),
        ("minLength", "最短长度"),
        ("maxLength", "最长长度"),
        ("minItems", "最少项数"),
        ("maxItems", "最多项数"),
        ("pattern", "匹配规则"),
        ("multipleOf", "倍数"),
    ] {
        if let Some(value) = schema.get(key) {
            result.push((label.into(), summary(value)));
        }
    }
    if let Some(value) = schema
        .get("examples")
        .and_then(Value::as_array)
        .and_then(|v| v.first())
    {
        result.push(("示例".into(), summary(value)));
    }
    for (key, label) in [
        ("deprecated", "已弃用"),
        ("readOnly", "只读"),
        ("writeOnly", "只写"),
        ("uniqueItems", "元素不重复"),
    ] {
        if schema[key] == true {
            result.push((label.into(), String::new()));
        }
    }
    for (key, label) in [("exclusiveMinimum", "大于"), ("exclusiveMaximum", "小于")] {
        if let Some(value) = schema.get(key).filter(|v| v.is_number()) {
            result.push((label.into(), summary(value)));
        }
    }
    result
}
fn schema_node(name: &str, schema: &Value, depth: usize, budget: &mut usize) -> SchemaNode {
    let mut node = SchemaNode {
        name: name.into(),
        kind: kind(schema),
        description: text(schema, "description").into(),
        values: choices(schema),
        details: details(schema),
        reference: schema
            .get("$ref")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..Default::default()
    };
    if depth >= 10 || *budget == 0 {
        node.details
            .push(("结构较深".into(), "展开已到显示层级上限".into()));
        return node;
    }
    *budget -= 1;
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (name, property) in properties.iter().take(*budget) {
            let mut child = schema_node(name, property, depth + 1, budget);
            child.required = Some(
                schema
                    .get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|required| required.iter().any(|v| v == name)),
            );
            node.children.push(child);
        }
    }
    if let Some(items) = schema.get("items") {
        node.children
            .push(schema_node("数组元素", items, depth + 1, budget));
    }
    if let Some(items) = schema.get("prefixItems").and_then(Value::as_array) {
        for (i, item) in items.iter().enumerate() {
            node.children.push(schema_node(
                &format!("第 {} 项", i + 1),
                item,
                depth + 1,
                budget,
            ));
        }
    }
    for (key, label) in [
        ("allOf", "全部满足"),
        ("oneOf", "满足其中一种"),
        ("anyOf", "满足至少一种"),
    ] {
        if let Some(branches) = schema.get(key).and_then(Value::as_array) {
            for (i, branch) in branches.iter().enumerate() {
                node.children.push(schema_node(
                    &format!("{label} · {}", i + 1),
                    branch,
                    depth + 1,
                    budget,
                ));
            }
        }
    }
    if let Some(extra) = schema.get("additionalProperties") {
        if extra.is_object() {
            node.children
                .push(schema_node("其他字段", extra, depth + 1, budget));
        } else if *extra == false {
            node.details.push(("额外字段".into(), "不允许".into()));
        }
    }
    node
}
fn parameter(parameter: &Value, budget: &mut usize) -> SchemaNode {
    let mut node = schema_node(
        text(parameter, "name"),
        parameter.get("schema").unwrap_or(parameter),
        0,
        budget,
    );
    let location = text(parameter, "in");
    node.location = Some(
        match location {
            "query" => "Query",
            "path" => "Path",
            "header" => "Header",
            "cookie" => "Cookie",
            "formData" => "表单",
            _ => location,
        }
        .into(),
    );
    node.required = Some(location == "path" || parameter["required"] == true);
    if !text(parameter, "description").is_empty() {
        node.description = text(parameter, "description").into();
    }
    if let Some(example) = parameter.get("example") {
        node.details.push(("示例".into(), summary(example)));
    }
    if let Some(examples) = parameter.get("examples").and_then(Value::as_object) {
        for (name, example) in examples {
            if let Some(value) = example.get("value") {
                node.details
                    .push((format!("示例 · {name}"), summary(value)));
            }
        }
    }
    if parameter["deprecated"] == true {
        node.details.push(("已弃用".into(), String::new()));
    }
    node
}
fn content_nodes(definition: &Value, budget: &mut usize) -> Vec<SchemaNode> {
    definition
        .get("content")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(media, content)| {
            let mut node = schema_node(
                match media.as_str() {
                    "application/json" => "JSON",
                    "multipart/form-data" => "form-data",
                    "application/x-www-form-urlencoded" => "表单",
                    _ => media,
                },
                content.get("schema").unwrap_or(&Value::Null),
                0,
                budget,
            );
            node.location = Some(media.clone());
            if let Some(example) = content.get("example") {
                node.details.push(("示例".into(), summary(example)));
            }
            node
        })
        .collect()
}
fn body_nodes(body: &Value, budget: &mut usize) -> Vec<SchemaNode> {
    let mut nodes = content_nodes(body, budget);
    if let Some(parameters) = body.get("parameters").and_then(Value::as_array) {
        nodes.extend(parameters.iter().map(|p| parameter(p, budget)));
    }
    for node in &mut nodes {
        if body["required"] == true {
            node.required = Some(true);
        }
        if !text(body, "description").is_empty() {
            node.description = text(body, "description").into();
        }
    }
    if nodes.is_empty() {
        nodes.push(SchemaNode {
            name: "请求体".into(),
            kind: "未提供结构".into(),
            reference: body.get("$ref").and_then(Value::as_str).map(str::to_owned),
            ..Default::default()
        });
    }
    nodes
}
