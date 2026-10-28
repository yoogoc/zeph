//! Version-aware, offline OpenAPI import. Raw specifications remain authoritative.
//! This module imports executable HTTP operations; it is not a complete OAS validator.
use crate::model::{
    Auth, BodyKind, Folder, OpenApiOperation, Parameter, Project, Protocol, Request,
};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub struct ImportedSpec {
    pub contract_id: Uuid,
    pub folders: Vec<Folder>,
    pub version: String,
    pub requests: Vec<Request>,
    pub diagnostics: Vec<String>,
    pub document: Value,
    pub source: String,
    pub documents: BTreeMap<PathBuf, Value>,
}
struct Documents {
    roots: BTreeMap<PathBuf, Value>,
    diagnostics: Vec<String>,
    allow_files: bool,
}
fn read_document(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path)?;
    if path.extension().is_some_and(|e| e == "json") {
        Ok(serde_json::from_str(&text)?)
    } else {
        Ok(serde_yaml_ng::from_str(&text)?)
    }
}
impl Documents {
    fn resolve(
        &mut self,
        node: &Value,
        file: &Path,
        seen: &mut HashSet<String>,
    ) -> Result<(Value, PathBuf)> {
        let Some(reference) = node.get("$ref").and_then(Value::as_str) else {
            return Ok((node.clone(), file.to_path_buf()));
        };
        let id = format!("{}::{reference}", file.display());
        if !seen.insert(id.clone()) {
            bail!("循环引用: {reference}");
        }
        if reference.contains("://") {
            bail!("外部网络引用未获取: {reference}");
        }
        let (document, fragment) = reference.split_once('#').unwrap_or((reference, ""));
        let target = if document.is_empty() {
            file.to_owned()
        } else {
            if !self.allow_files {
                bail!("远端快照中的外部引用未读取: {reference}");
            }
            file.parent()
                .unwrap_or(Path::new("."))
                .join(document)
                .canonicalize()?
        };
        if !self.roots.contains_key(&target) {
            self.roots.insert(target.clone(), read_document(&target)?);
        }
        let document = &self.roots[&target];
        let referenced = if fragment.is_empty() {
            document.clone()
        } else if fragment.starts_with('/') {
            document
                .pointer(fragment)
                .with_context(|| format!("引用目标不存在: {reference}"))?
                .clone()
        } else {
            bail!("锚点引用暂不支持: {reference}");
        };
        let result = self.resolve(&referenced, &target, seen);
        seen.remove(&id);
        result
    }
    fn resolved(&mut self, node: &Value, file: &Path) -> Result<(Value, PathBuf)> {
        self.resolve(node, file, &mut HashSet::new())
    }
    fn sample(&mut self, schema: &Value, file: &Path, depth: usize) -> Value {
        if depth > 6 {
            return Value::Null;
        }
        if let Some(example) = schema
            .get("example")
            .or_else(|| schema.get("examples").and_then(|v| v.get(0)))
            .or_else(|| schema.get("default"))
            .or_else(|| schema.get("enum").and_then(|v| v.get(0)))
            .or_else(|| schema.get("const"))
        {
            return example.clone();
        }
        if schema.is_boolean() {
            self.diagnostics
                .push("布尔 Schema 已保留；本版本不执行 JSON Schema 验证".into());
            return Value::Null;
        }
        let (s, file) = match self.resolved(schema, file) {
            Ok(v) => v,
            Err(error) => {
                self.diagnostics.push(error.to_string());
                return Value::Null;
            }
        };
        if s != *schema {
            return self.sample(&s, &file, depth + 1);
        }
        if let Some(branch) = s
            .get("oneOf")
            .or_else(|| s.get("anyOf"))
            .and_then(|v| v.get(0))
        {
            return self.sample(branch, &file, depth + 1);
        }
        if let Some(all) = s.get("allOf").and_then(Value::as_array) {
            let mut result = serde_json::Map::new();
            for branch in all {
                if let Value::Object(values) = self.sample(branch, &file, depth + 1) {
                    result.extend(values);
                }
            }
            return Value::Object(result);
        }
        let kind = s
            .get("type")
            .and_then(|t| {
                t.as_str().or_else(|| {
                    t.as_array()
                        .and_then(|v| v.iter().find_map(|t| t.as_str().filter(|t| *t != "null")))
                })
            })
            .unwrap_or(if s.get("properties").is_some() {
                "object"
            } else {
                "string"
            });
        match kind {
            "object" => {
                let mut result = serde_json::Map::new();
                if let Some(properties) = s.get("properties").and_then(Value::as_object) {
                    for (name, property) in properties {
                        if property.get("readOnly") == Some(&Value::Bool(true)) {
                            continue;
                        }
                        result.insert(name.clone(), self.sample(property, &file, depth + 1));
                    }
                }
                Value::Object(result)
            }
            "array" => Value::Array(vec![self.sample(
                s.get("items").unwrap_or(&Value::Null),
                &file,
                depth + 1,
            )]),
            "integer" | "number" => s.get("minimum").cloned().unwrap_or(serde_json::json!(1)),
            "boolean" => Value::Bool(false),
            "null" => Value::Null,
            _ => Value::String(String::new()),
        }
    }
}
fn text_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    }
}
fn scan_special(node: &Value, diagnostics: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            if map.contains_key("$ref") && map.len() > 1 {
                diagnostics.push("$ref 的兄弟关键字已保留，需手动检查其合并语义".into());
            }
            for (key, value) in map {
                if [
                    "$dynamicRef",
                    "$anchor",
                    "$id",
                    "jsonSchemaDialect",
                    "webhooks",
                    "callbacks",
                ]
                .contains(&key.as_str())
                {
                    diagnostics.push(format!("{key} 已保留在原文中，本版本未执行其完整语义"));
                }
                scan_special(value, diagnostics);
            }
        }
        Value::Array(values) => {
            for value in values {
                scan_special(value, diagnostics)
            }
        }
        _ => {}
    }
}

pub fn import(path: &Path) -> Result<ImportedSpec> {
    let file = path.canonicalize()?;
    let root = read_document(&file)?;
    import_document(root, file, true)
}

/// Remote documents may resolve in-document pointers, never local files or other URLs.
pub fn import_value(root: Value) -> Result<ImportedSpec> {
    import_document(root, PathBuf::from("apifox-snapshot.json"), false)
}

fn import_document(root: Value, file: PathBuf, allow_files: bool) -> Result<ImportedSpec> {
    let version = root
        .get("swagger")
        .or_else(|| root.get("openapi"))
        .and_then(Value::as_str)
        .context("文档缺少 swagger / openapi 版本字段")?
        .to_owned();
    if version != "2.0" && !version.starts_with("3.0.") && !version.starts_with("3.1.") {
        bail!("暂不支持 OpenAPI {version}");
    }
    let v2 = version == "2.0";
    let mut documents = Documents {
        roots: BTreeMap::from([(file.clone(), root.clone())]),
        diagnostics: vec![],
        allow_files,
    };
    scan_special(&root, &mut documents.diagnostics);
    let base = if v2 {
        let scheme = root
            .get("schemes")
            .and_then(|v| v.get(0))
            .and_then(Value::as_str)
            .unwrap_or("http");
        format!(
            "{scheme}://{}{}",
            root.get("host")
                .and_then(Value::as_str)
                .unwrap_or("localhost:8080"),
            root.get("basePath").and_then(Value::as_str).unwrap_or("")
        )
    } else {
        server_url(&root).unwrap_or_else(|| "{{base_url}}".into())
    };
    let paths = root
        .get("paths")
        .and_then(Value::as_object)
        .context("文档缺少 paths 对象")?;
    let contract_id = Uuid::new_v4();
    let title = root
        .pointer("/info/title")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("OpenAPI 文档");
    let mut grouping = Project::empty(title);
    grouping.openapi_folder(contract_id, title, None);
    let has_directories = paths.values().any(|item| {
        item.as_object()
            .is_some_and(|item| item.values().any(|op| op.get("x-apifox-folder").is_some()))
    });
    for tag in root
        .get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !has_directories
            && let Some(name) = tag
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
        {
            grouping.openapi_folder(contract_id, title, Some(name));
        }
    }
    let mut requests = vec![];
    for (route, path_node) in paths {
        let (item, item_file) = match documents.resolved(path_node, &file) {
            Ok(value) => value,
            Err(error) => {
                documents.diagnostics.push(format!("{route}: {error}"));
                continue;
            }
        };
        for method in [
            "get", "post", "put", "patch", "delete", "head", "options", "trace",
        ] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let mut request = Request::new(
                Protocol::Http,
                operation
                    .get("summary")
                    .or_else(|| operation.get("operationId"))
                    .and_then(Value::as_str)
                    .unwrap_or(route),
            );
            request.method = method.to_uppercase();
            let mut seen = HashSet::new();
            let tags = operation
                .get("tags")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|s| !s.trim().is_empty() && seen.insert((*s).to_owned()))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let directory = operation
                .get("x-apifox-folder")
                .and_then(Value::as_str)
                .map(parse_apifox_directory)
                .transpose()?;
            request.folder_id = Some(if let Some(path) = &directory {
                grouping.openapi_directory(contract_id, title, path)?
            } else {
                grouping.openapi_folder(contract_id, title, tags.first().map(String::as_str))
            });
            request.openapi = Some(OpenApiOperation {
                contract_id,
                path: route.clone(),
                method: method.into(),
                operation_id: operation
                    .get("operationId")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                tags,
                directory,
            });
            request.source = Some(format!("{}#{method}:{route}", file.display()));
            request.description = operation
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into();
            let server = if v2 {
                base.clone()
            } else {
                server_url(operation)
                    .or_else(|| server_url(&item))
                    .unwrap_or_else(|| base.clone())
            };
            let server = if server.starts_with('/') {
                format!("{{{{base_url}}}}{}", server)
            } else {
                server
            };
            request.url = format!("{}{route}", server.trim_end_matches('/'));
            let mut parameters = BTreeMap::new();
            for node in item
                .get("parameters")
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
                match documents.resolved(node, &item_file) {
                    Ok((p, origin)) => {
                        let name = p.get("name").and_then(Value::as_str).unwrap_or("");
                        let location = p.get("in").and_then(Value::as_str).unwrap_or("");
                        parameters.insert((location.to_owned(), name.to_owned()), (p, origin));
                    }
                    Err(error) => documents.diagnostics.push(error.to_string()),
                }
            }
            for ((location, name), (p, origin)) in parameters {
                let schema = p.get("schema").unwrap_or(&p);
                let sample = p
                    .get("example")
                    .cloned()
                    .unwrap_or_else(|| documents.sample(schema, &origin, 0));
                let mut parameter = Parameter::new(name, text_value(&sample));
                parameter.description = p
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into();
                parameter.enabled =
                    p.get("required") == Some(&Value::Bool(true)) || !parameter.value.is_empty();
                if p.get("style")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s != "form" && s != "simple")
                    || p.get("collectionFormat").is_some()
                    || sample.is_array()
                    || sample.is_object()
                {
                    documents.diagnostics.push(format!(
                        "{} {route}: 参数 {} 的序列化需手动检查",
                        request.method, parameter.name
                    ));
                }
                match location.as_str() {
                    "query" => request.query.push(parameter),
                    "path" => {
                        parameter.enabled = true;
                        request.path.push(parameter)
                    }
                    "header" => request.headers.push(parameter),
                    "cookie" => documents
                        .diagnostics
                        .push(format!("{route}: Cookie 参数需在 Headers 中配置")),
                    "body" => {
                        request.body.kind = BodyKind::Json;
                        request.body.text = serde_json::to_string_pretty(&sample)?;
                        request.schema = Some(
                            documents
                                .resolved(schema, &origin)
                                .map(|(s, _)| s)
                                .unwrap_or_else(|_| schema.clone()),
                        );
                    }
                    "formData" => {
                        request.body.kind = if p.get("type").and_then(Value::as_str) == Some("file")
                        {
                            BodyKind::FormData
                        } else {
                            BodyKind::UrlEncoded
                        };
                        parameter.file = p.get("type").and_then(Value::as_str) == Some("file");
                        request.body.fields.push(parameter);
                    }
                    _ => {}
                }
            }
            if v2
                && (request.body.fields.iter().any(|p| p.file)
                    || operation
                        .get("consumes")
                        .or_else(|| root.get("consumes"))
                        .and_then(Value::as_array)
                        .is_some_and(|types| types.iter().any(|t| t == "multipart/form-data")))
            {
                request.body.kind = BodyKind::FormData;
            }
            if !v2 && let Some(node) = operation.get("requestBody") {
                match documents.resolved(node, &item_file) {
                    Ok((body, origin)) => {
                        if let Some(content) = body.get("content").and_then(Value::as_object) {
                            let selected = content
                                .get_key_value("application/json")
                                .or_else(|| content.iter().next());
                            if let Some((mime, media)) = selected {
                                if media.get("encoding").is_some() {
                                    documents.diagnostics.push(format!(
                                        "{} {route}: multipart / form encoding 需手动检查",
                                        request.method
                                    ));
                                }
                                let schema = media.get("schema").unwrap_or(&Value::Null);
                                let sample = media
                                    .get("example")
                                    .cloned()
                                    .or_else(|| {
                                        media
                                            .get("examples")
                                            .and_then(Value::as_object)
                                            .and_then(|v| v.values().next())
                                            .and_then(|v| v.get("value"))
                                            .cloned()
                                    })
                                    .unwrap_or_else(|| documents.sample(schema, &origin, 0));
                                request.schema = Some(
                                    documents
                                        .resolved(schema, &origin)
                                        .map(|(s, _)| s)
                                        .unwrap_or_else(|_| schema.clone()),
                                );
                                request.body.kind = match mime.as_str() {
                                    "multipart/form-data" => BodyKind::FormData,
                                    "application/x-www-form-urlencoded" => BodyKind::UrlEncoded,
                                    "application/octet-stream" => BodyKind::Binary,
                                    "application/xml" | "text/xml" => BodyKind::Xml,
                                    m if m.contains("json") => BodyKind::Json,
                                    _ => BodyKind::Text,
                                };
                                if matches!(
                                    request.body.kind,
                                    BodyKind::FormData | BodyKind::UrlEncoded
                                ) {
                                    if let Value::Object(values) = sample {
                                        request.body.fields = values
                                            .iter()
                                            .map(|(k, v)| Parameter::new(k, text_value(v)))
                                            .collect();
                                    }
                                    if let Some(properties) =
                                        schema.get("properties").and_then(Value::as_object)
                                    {
                                        for field in &mut request.body.fields {
                                            field.file = properties
                                                .get(&field.name)
                                                .and_then(|p| p.get("format"))
                                                .and_then(Value::as_str)
                                                == Some("binary");
                                        }
                                    }
                                } else {
                                    request.body.text = if request.body.kind == BodyKind::Json {
                                        serde_json::to_string_pretty(&sample)?
                                    } else {
                                        text_value(&sample)
                                    };
                                    request.headers.push(Parameter::new("Content-Type", mime));
                                }
                                if content.len() > 1 {
                                    documents.diagnostics.push(format!(
                                        "{} {route}: 默认选择 {mime}，其他媒体类型请在 Body 中切换",
                                        request.method
                                    ));
                                }
                            }
                        }
                    }
                    Err(error) => documents.diagnostics.push(error.to_string()),
                }
            }
            let security = operation.get("security").or_else(|| root.get("security"));
            if security.is_some_and(|s| s.as_array().is_some_and(|a| !a.is_empty())) {
                documents.diagnostics.push(format!(
                    "{} {route}: 安全方案已保留，请在认证页配置凭据",
                    request.method
                ));
                request.auth = Auth::None;
            }
            requests.push(request);
        }
    }
    documents.diagnostics.sort();
    documents.diagnostics.dedup();
    Ok(ImportedSpec {
        contract_id,
        folders: grouping.folders,
        version,
        requests,
        diagnostics: documents.diagnostics,
        document: root,
        source: file.to_string_lossy().into_owned(),
        documents: documents.roots,
    })
}

/// Apifox folder separators are '/' with '\/' and '\\' representing literal characters.
pub fn parse_apifox_directory(value: &str) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some(next @ ('/' | '\\')) => current.push(next),
                _ => bail!("Apifox 目录转义无效"),
            },
            '/' => {
                if current.trim().is_empty() {
                    bail!("Apifox 目录包含空层级");
                }
                parts.push(current.trim().to_owned());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_owned());
    } else if !value.is_empty() {
        bail!("Apifox 目录包含空层级");
    }
    if parts.len() > 31 {
        bail!("Apifox 目录嵌套超过 31 层");
    }
    Ok(parts)
}
fn server_url(node: &Value) -> Option<String> {
    let server = node.get("servers")?.get(0)?;
    let mut value = server.get("url")?.as_str()?.to_owned();
    if let Some(vars) = server.get("variables").and_then(Value::as_object) {
        for (key, item) in vars {
            value = value.replace(
                &format!("{{{key}}}"),
                &text_value(item.get("default").unwrap_or(&Value::Null)),
            );
        }
    }
    Some(value)
}
