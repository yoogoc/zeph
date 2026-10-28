//! Read-only definition views. Debug values are never mistaken for contract examples.
use crate::{
    apifox::RequestBinding,
    model::{BodyKind, Parameter, Request},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Component, Path, PathBuf},
};

#[path = "structure_docs.rs"]
mod docs;
pub use docs::SchemaNode;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Parameters,
    Body,
    Responses,
    Models,
}
impl Section {
    pub const ALL: [Self; 4] = [Self::Parameters, Self::Body, Self::Responses, Self::Models];
    pub fn label(self) -> &'static str {
        match self {
            Self::Parameters => "参数",
            Self::Body => "Body",
            Self::Responses => "响应",
            Self::Models => "模型",
        }
    }
    pub fn empty_message(self) -> &'static str {
        match self {
            Self::Parameters => "此接口未定义请求参数",
            Self::Body => "此接口未定义请求体",
            Self::Responses => "此接口未定义响应结构",
            Self::Models => "此接口未引用共享模型",
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct RequestStructure {
    pub source: String,
    pub defined: bool,
    pub parameters: Vec<Value>,
    pub body: Option<Value>,
    pub responses: BTreeMap<String, Value>,
    pub models: BTreeMap<String, Value>,
    pub diagnostics: Vec<String>,
}
impl RequestStructure {
    pub fn local(request: &Request) -> Self {
        fn params(rows: &[Parameter], location: &str) -> Vec<Value> {
            rows.iter().filter(|p|!p.name.is_empty()).map(|p|json!({"name":p.name,"in":location,"enabled":p.enabled,"description":p.description})).collect()
        }
        let mut parameters = params(&request.query, "query");
        parameters.extend(params(&request.path, "path"));
        parameters.extend(params(&request.headers, "header"));
        let body = request
            .schema
            .as_ref()
            .map(|schema| json!({"content":{media_type(request.body.kind):{"schema":schema}}}));
        Self {
            source: if body.is_some() {
                "已保存的请求体 Schema".into()
            } else {
                "本地请求 · 尚未关联接口定义".into()
            },
            defined: body.is_some(),
            parameters,
            body,
            ..Self::default()
        }
    }
    pub fn from_apifox(binding: &RequestBinding, name: &str) -> Self {
        let definition = &binding.definition;
        let contract = &definition["contract"];
        let origin = PathBuf::from("apifox.json");
        let root = json!({"components":contract.get("components"),"definitions":contract.get("definitions")});
        let mut resolver = Resolver::new(origin.clone(), BTreeMap::from([(origin.clone(), root)]));
        resolver.references = contract
            .get("dependencies")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let mut result = build(
            contract.get("operation").unwrap_or(&Value::Null),
            contract.get("pathParameters"),
            &origin,
            &mut resolver,
        );
        // Older snapshots retained only a body schema; it remains useful while newer contracts have no body.
        if result.body.is_none()
            && let Some(schema) = definition.get("schema").filter(|v| !v.is_null())
        {
            result.body = Some(
                json!({"content":{"application/json":{"schema":resolver.expand(schema,&origin,0,&mut HashSet::new())}}}),
            );
        }
        result.models = resolver.models;
        result.diagnostics = resolver.diagnostics;
        result.source = format!("Apifox · {name} · 已应用定义");
        result.defined = true;
        result
    }
    /// Uses saved documents only. Missing original files do not prevent offline inspection.
    pub fn from_saved(request: &Request, document: Value, manifest: Value) -> Result<Self> {
        let binding = request
            .openapi
            .as_ref()
            .context("请求尚未关联 OpenAPI 定义")?;
        let source = manifest
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("openapi.json");
        let origin = PathBuf::from(source);
        let mut documents = BTreeMap::new();
        if let Some(files) = manifest.get("files").and_then(Value::as_array) {
            for file in files {
                if let (Some(path), Some(value)) = (
                    file.get("path").and_then(Value::as_str),
                    file.get("document"),
                ) {
                    documents.insert(PathBuf::from(path), value.clone());
                }
            }
        }
        let title = document
            .pointer("/info/title")
            .and_then(Value::as_str)
            .unwrap_or("OpenAPI 文档")
            .to_owned();
        documents.insert(origin.clone(), document);
        let mut resolver = Resolver::new(origin.clone(), documents);
        let root = &resolver.documents[&origin];
        let item = root
            .get("paths")
            .and_then(|v| v.get(&binding.path))
            .context("快照中未找到对应接口路径")?
            .clone();
        let (item, item_origin) = resolver.resolve_top(&item, &origin, &mut HashSet::new())?;
        let operation = item
            .get(binding.method.to_lowercase())
            .context("快照中未找到对应 HTTP Method")?;
        let mut result = build(
            operation,
            item.get("parameters"),
            &item_origin,
            &mut resolver,
        );
        if root_is_v2(&resolver.documents[&origin])
            && let Some(parameters) = operation.get("parameters").and_then(Value::as_array)
        {
            let body_parameters = parameters
                .iter()
                .chain(
                    item.get("parameters")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten(),
                )
                .map(|p| resolver.expand(p, &item_origin, 0, &mut HashSet::new()))
                .filter(|p| {
                    matches!(
                        p.get("in").and_then(Value::as_str),
                        Some("body" | "formData")
                    )
                })
                .collect::<Vec<_>>();
            if !body_parameters.is_empty() {
                result.body = Some(
                    json!({"consumes":operation.get("consumes").or_else(||resolver.documents[&origin].get("consumes")),"parameters":body_parameters}),
                );
            }
        }
        result.source = format!("OpenAPI · {title}");
        result.defined = true;
        result.models = resolver.models;
        result.diagnostics = resolver.diagnostics;
        Ok(result)
    }
    pub fn count(&self, section: Section) -> usize {
        match section {
            Section::Parameters => self.parameters.len(),
            Section::Body => usize::from(self.body.is_some()),
            Section::Responses => self.responses.len(),
            Section::Models => self.models.len(),
        }
    }
    pub fn default_section(&self) -> Section {
        Section::ALL
            .into_iter()
            .find(|s| self.count(*s) > 0)
            .unwrap_or_default()
    }
    pub fn value(&self, section: Section) -> Option<Value> {
        if self.count(section) == 0 {
            return None;
        }
        Some(match section {
            Section::Parameters => json!(self.parameters),
            Section::Body => self.body.clone()?,
            Section::Responses => json!(self.responses),
            Section::Models => json!(self.models),
        })
    }
}
fn media_type(kind: BodyKind) -> &'static str {
    match kind {
        BodyKind::FormData => "multipart/form-data",
        BodyKind::UrlEncoded => "application/x-www-form-urlencoded",
        BodyKind::Binary => "application/octet-stream",
        BodyKind::Xml => "application/xml",
        BodyKind::Text => "text/plain",
        _ => "application/json",
    }
}
fn root_is_v2(root: &Value) -> bool {
    root.get("swagger").and_then(Value::as_str) == Some("2.0")
}

fn build(
    operation: &Value,
    path_parameters: Option<&Value>,
    origin: &Path,
    resolver: &mut Resolver,
) -> RequestStructure {
    let mut result = RequestStructure::default();
    for parameter in path_parameters
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            operation
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
    {
        let value = resolver.expand(parameter, origin, 0, &mut HashSet::new());
        if matches!(
            value.get("in").and_then(Value::as_str),
            Some("body" | "formData")
        ) {
            continue;
        }
        if let Some(index) = result.parameters.iter().position(|p| {
            p.get("in") == value.get("in")
                && p.get("name") == value.get("name")
                && value.get("name").is_some()
        }) {
            result.parameters[index] = value;
        } else {
            result.parameters.push(value);
        }
    }
    result.body = operation
        .get("requestBody")
        .map(|v| resolver.expand(v, origin, 0, &mut HashSet::new()));
    if let Some(responses) = operation.get("responses").and_then(Value::as_object) {
        for (status, value) in responses {
            result.responses.insert(
                status.clone(),
                resolver.expand(value, origin, 0, &mut HashSet::new()),
            );
        }
    }
    result
}

struct Resolver {
    origin: PathBuf,
    documents: BTreeMap<PathBuf, Value>,
    references: BTreeMap<String, Value>,
    models: BTreeMap<String, Value>,
    diagnostics: Vec<String>,
    remaining: usize,
}
impl Resolver {
    fn new(origin: PathBuf, documents: BTreeMap<PathBuf, Value>) -> Self {
        Self {
            origin,
            documents,
            references: BTreeMap::new(),
            models: BTreeMap::new(),
            diagnostics: vec![],
            remaining: 20000,
        }
    }
    fn lookup(&self, reference: &str, origin: &Path) -> Result<(Value, PathBuf)> {
        if let Some(value) = self.references.get(reference) {
            return Ok((value.clone(), origin.to_owned()));
        }
        if reference.contains("://") {
            bail!("外部网络引用未读取：{reference}");
        }
        let (file, fragment) = reference.split_once('#').unwrap_or((reference, ""));
        let path = if file.is_empty() {
            origin.to_owned()
        } else {
            normalize(&origin.parent().unwrap_or(Path::new(".")).join(file))
        };
        let document = self
            .documents
            .get(&path)
            .with_context(|| format!("未找到已保存的引用文档：{file}"))?;
        let fragment = percent_encoding::percent_decode_str(fragment).decode_utf8()?;
        let value = if fragment.is_empty() {
            document
        } else {
            document
                .pointer(&fragment)
                .with_context(|| format!("未找到引用：{reference}"))?
        };
        Ok((value.clone(), path))
    }
    fn resolve_top(
        &self,
        value: &Value,
        origin: &Path,
        seen: &mut HashSet<String>,
    ) -> Result<(Value, PathBuf)> {
        let Some(reference) = value.get("$ref").and_then(Value::as_str) else {
            return Ok((value.clone(), origin.to_owned()));
        };
        if !seen.insert(format!("{}::{reference}", origin.display())) {
            bail!("引用形成循环：{reference}");
        }
        let (resolved, origin) = self.lookup(reference, origin)?;
        let (mut resolved, origin) = self.resolve_top(&resolved, &origin, seen)?;
        if let (Some(target), Some(siblings)) = (resolved.as_object_mut(), value.as_object()) {
            for (key, value) in siblings {
                if key != "$ref" {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
        Ok((resolved, origin))
    }
    fn note(&mut self, message: String) {
        if self.diagnostics.len() < 12 && !self.diagnostics.contains(&message) {
            self.diagnostics.push(message);
        }
    }
    fn expand(
        &mut self,
        value: &Value,
        origin: &Path,
        depth: usize,
        trail: &mut HashSet<String>,
    ) -> Value {
        if depth > 12 || self.remaining == 0 {
            self.note("较深结构保留引用，可在“模型”中查看原始定义。".into());
            return value.clone();
        }
        self.remaining -= 1;
        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            let key = format!("{}::{reference}", origin.display());
            match self.lookup(reference, origin) {
                Ok((resolved, target)) => {
                    let label = if origin == self.origin {
                        reference.to_owned()
                    } else {
                        format!("{}::{reference}", origin.display())
                    };
                    self.models.insert(label, resolved.clone());
                    if !trail.insert(key.clone()) {
                        return value.clone();
                    }
                    let mut expanded = self.expand(&resolved, &target, depth + 1, trail);
                    trail.remove(&key);
                    if let (Some(object), Some(siblings)) =
                        (expanded.as_object_mut(), value.as_object())
                    {
                        for (key, value) in siblings {
                            if key != "$ref" {
                                object.insert(
                                    key.clone(),
                                    self.expand_member(key, value, origin, depth + 1, trail),
                                );
                            }
                        }
                    }
                    return expanded;
                }
                Err(error) => {
                    self.note(error.to_string());
                    return value.clone();
                }
            }
        }
        match value {
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            self.expand_member(key, value, origin, depth + 1, trail),
                        )
                    })
                    .collect(),
            ),
            Value::Array(array) => Value::Array(
                array
                    .iter()
                    .map(|v| self.expand(v, origin, depth + 1, trail))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }
    fn expand_member(
        &mut self,
        key: &str,
        value: &Value,
        origin: &Path,
        depth: usize,
        trail: &mut HashSet<String>,
    ) -> Value {
        // Examples, defaults and enum entries are literal data, even if they contain a `$ref` key.
        if matches!(key, "example" | "default" | "const" | "enum" | "value")
            || (key == "examples" && value.is_array())
        {
            return value.clone();
        }
        if matches!(
            key,
            "properties" | "patternProperties" | "$defs" | "definitions" | "examples"
        ) && let Some(entries) = value.as_object()
        {
            return Value::Object(
                entries
                    .iter()
                    .map(|(name, value)| (name.clone(), self.expand(value, origin, depth, trail)))
                    .collect(),
            );
        }
        self.expand(value, origin, depth, trail)
    }
}
fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            _ => result.push(part.as_os_str()),
        }
    }
    result
}
