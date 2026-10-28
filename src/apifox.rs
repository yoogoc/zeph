//! Apifox pulls are snapshots. Baselines advance only for explicitly applied resources.
use crate::model::{Folder, OpenApiOperation, Project, Request};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

#[path = "apifox_client.rs"]
mod client;
pub use client::{ApiFailure, FailureKind, fetch, read_token, remove_token, save_token};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceConfig {
    pub name: String,
    pub project_id: String,
    pub branch_id: Option<u64>,
    pub module_id: Option<u64>,
}
impl SourceConfig {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("请输入连接名称");
        }
        if self.project_id.is_empty() || !self.project_id.bytes().all(|b| b.is_ascii_digit()) {
            bail!("项目 ID 必须为数字");
        }
        if self.branch_id == Some(0) || self.module_id == Some(0) {
            bail!("分支和模块 ID 必须为正整数");
        }
        Ok(())
    }
    pub fn same_scope(&self, other: &Self) -> bool {
        self.project_id == other.project_id
            && self.branch_id == other.branch_id
            && self.module_id == other.module_id
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncSource {
    pub id: Uuid,
    pub config: SourceConfig,
    pub root_folder: Option<Uuid>,
    pub paused: bool,
    pub check_minutes: u16,
    pub auto_apply: bool,
    pub follow_folders: bool,
    pub authorization_required: bool,
    pub failures: u32,
    pub next_check_at: i64,
    pub last_checked_at: Option<String>,
    pub last_error: Option<String>,
    pub latest: Option<Snapshot>,
    pub bindings: Vec<RequestBinding>,
    pub folders: Vec<FolderBinding>,
    pub runs: Vec<SyncRun>,
    pub generation: u64,
}
impl SyncSource {
    pub fn new(config: SourceConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config,
            root_folder: None,
            paused: false,
            check_minutes: 0,
            auto_apply: false,
            follow_folders: true,
            authorization_required: false,
            failures: 0,
            next_check_at: 0,
            last_checked_at: None,
            last_error: None,
            latest: None,
            bindings: vec![],
            folders: vec![],
            runs: vec![],
            generation: 0,
        }
    }
    pub fn due(&self, now: i64) -> bool {
        !self.paused
            && !self.authorization_required
            && self.check_minutes > 0
            && now >= self.next_check_at
    }
    pub fn received(&mut self, snapshot: Snapshot) {
        self.latest = Some(snapshot);
        self.last_error = None;
        self.failures = 0;
        self.authorization_required = false;
        self.last_checked_at = Some(Utc::now().to_rfc3339());
        self.next_check_at = Utc::now().timestamp() + i64::from(self.check_minutes.max(1)) * 60;
    }
    pub fn failed(&mut self, error: &ApiFailure) {
        self.failures = self.failures.saturating_add(1);
        self.authorization_required = error.kind == FailureKind::Authorization;
        self.last_error = Some(error.message.clone());
        self.last_checked_at = Some(Utc::now().to_rfc3339());
        let delay = error
            .retry_after
            .unwrap_or(30u64.saturating_mul(1u64 << self.failures.min(7)))
            .clamp(30, 3600);
        self.next_check_at = Utc::now().timestamp() + delay as i64;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub document: Value,
    pub digest: String,
    pub fetched_at: String,
    pub endpoints: Vec<RemoteEndpoint>,
    pub directories: Vec<Vec<String>>,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteEndpoint {
    pub key: String,
    pub locator: String,
    pub definition: Value,
    pub template: Request,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestBinding {
    pub key: String,
    pub locator: String,
    pub request_id: Uuid,
    pub baseline: Value,
    pub definition: Value,
    pub remote_missing: bool,
    pub locally_deleted: bool,
    pub compatibility: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderBinding {
    pub folder_id: Uuid,
    pub remote_path: Vec<String>,
    pub baseline_name: String,
    pub baseline_parent: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncRun {
    pub id: Uuid,
    pub at: String,
    pub summary: String,
    pub digest: String,
    pub generation: u64,
    pub undo: Option<UndoBatch>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UndoBatch {
    bindings: Vec<RequestBinding>,
    binding_ids: HashSet<Uuid>,
    folders: Vec<FolderBinding>,
    requests: Vec<RequestPatch>,
    folder_patches: Vec<FolderPatch>,
    generation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RequestPatch {
    id: Uuid,
    before: Option<Value>,
    after: Value,
    creation_hash: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct FolderPatch {
    before: Option<Folder>,
    after: Folder,
}

pub fn validate_sources(project: &Project) -> Result<()> {
    let mut ids = HashSet::new();
    let mut requests = HashSet::new();
    for s in &project.sync_sources {
        s.config.validate()?;
        if !ids.insert(s.id) {
            bail!("数据源 ID 重复，原文件未修改");
        }
        let mut keys = HashSet::new();
        for binding in &s.bindings {
            if !keys.insert(binding.key.as_str()) || !requests.insert(binding.request_id) {
                bail!("同步来源绑定重复，原文件未修改");
            }
        }
    }
    Ok(())
}

pub fn digest(value: &Value) -> String {
    fn sorted(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.clone(), sorted(v)))
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
            _ => v.clone(),
        }
    }
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&sorted(value)).expect("JSON value"))
    )
}

impl Snapshot {
    pub fn parse(document: Value) -> Result<Self> {
        fn validate_refs(v: &Value, root: &Value) -> Result<()> {
            match v {
                Value::Object(map) => {
                    if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                        let pointer = reference
                            .strip_prefix('#')
                            .context("远端快照包含外部引用，请在 Apifox 导出中包含完整定义")?;
                        if !pointer.is_empty()
                            && (!pointer.starts_with('/') || root.pointer(pointer).is_none())
                        {
                            bail!("远端快照包含无法解析的引用");
                        }
                    }
                    for value in map.values() {
                        validate_refs(value, root)?;
                    }
                }
                Value::Array(a) => {
                    for value in a {
                        validate_refs(value, root)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }
        validate_refs(&document, &document)?;
        if document
            .pointer("/info/title")
            .and_then(Value::as_str)
            .is_none()
            || document
                .pointer("/info/version")
                .and_then(Value::as_str)
                .is_none()
        {
            bail!("远端文档缺少 info.title / info.version");
        }
        fn path_item<'a>(root: &'a Value, mut node: &'a Value) -> Result<&'a Value> {
            let mut seen = HashSet::new();
            while let Some(reference) = node.get("$ref").and_then(Value::as_str) {
                if !seen.insert(reference) {
                    bail!("路径项包含循环引用");
                }
                node = root
                    .pointer(reference.strip_prefix('#').context("外部路径引用未支持")?)
                    .context("路径引用无法解析")?;
            }
            if !node.is_object() {
                bail!("路径项不是对象");
            }
            Ok(node)
        }
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .context("远端文档缺少 paths 对象")?;
        let mut expected_operations = 0;
        for item in paths.values() {
            let item = path_item(&document, item)?;
            for method in [
                "get", "post", "put", "patch", "delete", "head", "options", "trace",
            ] {
                if let Some(operation) = item.get(method) {
                    if !operation.is_object() {
                        bail!("接口操作不是对象");
                    }
                    expected_operations += 1;
                }
            }
        }
        let imported = crate::openapi::import_value(document.clone())?;
        if imported
            .diagnostics
            .iter()
            .any(|d| d.contains("循环引用") || d.contains("引用目标不存在"))
        {
            bail!("远端文档包含无法解析的引用，快照未更新");
        }
        if imported.requests.len() != expected_operations {
            bail!("部分接口无法解析，远端快照未更新");
        }
        let mut op_counts = HashMap::new();
        for r in &imported.requests {
            if let Some(id) = r
                .openapi
                .as_ref()
                .and_then(|b| b.operation_id.as_ref())
                .filter(|id| !id.is_empty())
            {
                *op_counts.entry(id.clone()).or_insert(0usize) += 1;
            }
        }
        let mut endpoints = Vec::new();
        let mut directories = Vec::new();
        for mut request in imported.requests {
            let op = request.openapi.as_ref().context("导出接口缺少来源信息")?;
            let directory = op
                .directory
                .clone()
                .unwrap_or_else(|| op.tags.first().cloned().into_iter().collect());
            for n in 1..=directory.len() {
                if !directories.contains(&directory[..n].to_vec()) {
                    directories.push(directory[..n].to_vec());
                }
            }
            let locator = format!("{} {}", op.method, op.path);
            let key = op
                .operation_id
                .as_ref()
                .filter(|id| op_counts.get(*id) == Some(&1))
                .map(|id| format!("operation:{id}"))
                .unwrap_or_else(|| format!("endpoint:{locator}"));
            let item = path_item(&document, &paths[&op.path])?;
            let mut operation = item.get(&op.method).cloned().unwrap_or(Value::Null);
            if let Some(object) = operation.as_object_mut() {
                // Canonical fields avoid duplicate changes; originals remain in the snapshot.
                object.remove("summary");
                object.remove("description");
                object.remove("x-apifox-folder");
            }
            let path_parameters = item.get("parameters").cloned().unwrap_or(Value::Null);
            let security = operation
                .get("security")
                .or_else(|| document.get("security"))
                .cloned()
                .unwrap_or(Value::Null);
            let dependencies =
                contract_dependencies(&document, &operation, &path_parameters, &security);
            if let Some(object) = operation.as_object_mut() {
                object.remove("security");
            }
            let definition = json!({"name":request.name,"method":request.method,"url":request.url,"path":op.path,"description":request.description,"folder":directory,
                "schema":request.schema,"contract":{"operation":operation,"pathParameters":path_parameters,"dependencies":dependencies,"security":security}});
            request.folder_id = None;
            request.openapi = None;
            request.source = None;
            endpoints.push(RemoteEndpoint {
                key,
                locator,
                definition,
                template: request,
            });
        }
        directories.sort_by_key(Vec::len);
        Ok(Self {
            digest: digest(&document),
            document,
            fetched_at: Utc::now().to_rfc3339(),
            endpoints,
            directories,
            diagnostics: imported.diagnostics,
        })
    }
}

fn contract_dependencies(
    document: &Value,
    operation: &Value,
    parameters: &Value,
    security: &Value,
) -> BTreeMap<String, Value> {
    fn visit(value: &Value, document: &Value, dependencies: &mut BTreeMap<String, Value>) {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str)
                    && !dependencies.contains_key(reference)
                    && let Some(target) = reference
                        .strip_prefix('#')
                        .and_then(|p| document.pointer(p))
                {
                    dependencies.insert(reference.into(), target.clone());
                    visit(target, document, dependencies);
                }
                for value in object.values() {
                    visit(value, document, dependencies);
                }
            }
            Value::Array(array) => {
                for value in array {
                    visit(value, document, dependencies);
                }
            }
            _ => {}
        }
    }
    let mut dependencies = BTreeMap::new();
    visit(operation, document, &mut dependencies);
    visit(parameters, document, &mut dependencies);
    if let Some(requirements) = security.as_array() {
        for requirement in requirements {
            if let Some(requirement) = requirement.as_object() {
                for name in requirement.keys() {
                    for prefix in ["/components/securitySchemes/", "/securityDefinitions/"] {
                        let pointer = format!("{prefix}{}", escape_pointer(name));
                        if let Some(target) = document.pointer(&pointer) {
                            dependencies.insert(format!("#{pointer}"), target.clone());
                            visit(target, document, &mut dependencies);
                        }
                    }
                }
            }
        }
    }
    dependencies
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Updated,
    Conflict,
    Removed,
    LocalDeleted,
    Folder,
}
impl ChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "新增",
            Self::Updated => "更新",
            Self::Conflict => "冲突",
            Self::Removed => "远端移除",
            Self::LocalDeleted => "本地已删除",
            Self::Folder => "目录调整",
        }
    }
}
#[derive(Clone, Debug)]
pub struct FieldChange {
    pub path: String,
    pub base: Option<Value>,
    pub local: Option<Value>,
    pub remote: Option<Value>,
    pub conflict: bool,
}
#[derive(Clone, Debug)]
pub struct Change {
    pub key: String,
    pub kind: ChangeKind,
    pub name: String,
    pub method: String,
    pub binding: Option<usize>,
    pub remote: Option<usize>,
    pub fields: Vec<FieldChange>,
    pub current: Value,
    pub folder: Option<usize>,
    pub directory: Option<Vec<String>>,
}
#[derive(Clone, Debug)]
pub struct SyncPlan {
    pub source_id: Uuid,
    pub project_hash: String,
    pub snapshot_hash: String,
    pub changes: Vec<Change>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Local,
    Remote,
}
#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub included: HashSet<String>,
    /// Keys are (resource key, JSON pointer); '$' resolves a local deletion.
    pub resolutions: HashMap<(String, String), Resolution>,
    pub archive: HashSet<String>,
}
impl Selection {
    pub fn safe(plan: &SyncPlan) -> Self {
        Self {
            included: plan
                .changes
                .iter()
                .filter(|c| {
                    matches!(
                        c.kind,
                        ChangeKind::Added | ChangeKind::Updated | ChangeKind::Folder
                    ) && !c.fields.iter().any(|f| f.conflict)
                })
                .map(|c| c.key.clone())
                .collect(),
            ..Self::default()
        }
    }
}
fn escape_pointer(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}
fn differences(
    base: Option<&Value>,
    local: Option<&Value>,
    remote: Option<&Value>,
    path: &str,
    out: &mut Vec<FieldChange>,
) {
    if remote == base || local == remote {
        return;
    }
    if let (Some(Value::Object(b)), Some(Value::Object(l)), Some(Value::Object(r))) =
        (base, local, remote)
    {
        let keys = b
            .keys()
            .chain(l.keys())
            .chain(r.keys())
            .collect::<std::collections::BTreeSet<_>>();
        for key in keys {
            differences(
                b.get(key),
                l.get(key),
                r.get(key),
                &format!("{path}/{}", escape_pointer(key)),
                out,
            );
        }
    } else {
        out.push(FieldChange {
            path: path.into(),
            base: base.cloned(),
            local: local.cloned(),
            remote: remote.cloned(),
            conflict: local != base,
        });
    }
}
fn set_pointer(value: &mut Value, path: &str, new: Option<Value>) -> Result<()> {
    if path.is_empty() {
        *value = new.unwrap_or(Value::Null);
        return Ok(());
    }
    let (parent, key) = path.rsplit_once('/').context("变更路径无效")?;
    let key = key.replace("~1", "/").replace("~0", "~");
    let object = if parent.is_empty() {
        value
    } else {
        value.pointer_mut(parent).context("变更父节点不存在")?
    };
    let object = object.as_object_mut().context("变更目标不是对象")?;
    if let Some(new) = new {
        object.insert(key, new);
    } else {
        object.remove(&key);
    }
    Ok(())
}
fn relative_folder(project: &Project, source: &SyncSource, folder: Option<Uuid>) -> Value {
    if folder == source.root_folder {
        return json!([]);
    }
    if let Some(binding) = source.folders.iter().find(|b| Some(b.folder_id) == folder)
        && let Ok(actual) = project.folder(binding.folder_id)
        && actual.name == binding.baseline_name
        && actual.parent_id == binding.baseline_parent
    {
        return json!(binding.remote_path);
    }
    let mut cursor = folder;
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    while let Some(id) = cursor {
        if Some(id) == source.root_folder {
            path.reverse();
            return json!(path);
        }
        if !seen.insert(id) {
            break;
        }
        let Ok(f) = project.folder(id) else {
            break;
        };
        path.push(f.name.clone());
        cursor = f.parent_id;
    }
    // Absolute local placements are opaque overrides, never interpreted as remote directories.
    json!({"localFolderId":folder})
}
fn current_definition(
    project: &Project,
    source: &SyncSource,
    binding: &RequestBinding,
) -> Option<Value> {
    let request = project
        .requests
        .iter()
        .find(|r| r.id == binding.request_id)?;
    let mut value = binding.definition.clone();
    value["name"] = request.name.clone().into();
    value["method"] = request.method.clone().into();
    value["url"] = request.url.clone().into();
    value["description"] = request.description.clone().into();
    value["schema"] = request.schema.clone().unwrap_or(Value::Null);
    value["folder"] = relative_folder(project, source, request.folder_id);
    Some(value)
}
fn folder_value(name: &str, parent: Option<Uuid>) -> Value {
    json!({"name":name,"parent":parent})
}

pub fn plan(project: &Project, source_id: Uuid) -> Result<SyncPlan> {
    let source = project
        .sync_sources
        .iter()
        .find(|s| s.id == source_id)
        .context("数据源不存在")?;
    if let Some(root) = source.root_folder {
        project
            .folder(root)
            .context("同步目标文件夹已删除，请重新设置连接目标")?;
    }
    let snapshot = source.latest.as_ref().context("请先检查更新")?;
    let mut changes = Vec::new();
    let mut matched = HashSet::new();
    for (remote_index, endpoint) in snapshot.endpoints.iter().enumerate() {
        let by_key = source.bindings.iter().position(|b| b.key == endpoint.key);
        let by_locator = source
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.locator == endpoint.locator)
            .collect::<Vec<_>>();
        let binding_index = by_key.or_else(|| (by_locator.len() == 1).then(|| by_locator[0].0));
        if let Some(index) = binding_index {
            if !matched.insert(index) {
                bail!("接口身份存在歧义，请先修正重复的 operationId");
            }
            let binding = &source.bindings[index];
            let current = current_definition(project, source, binding);
            if current.is_none() {
                if !binding.locally_deleted || binding.baseline != endpoint.definition {
                    changes.push(Change {
                        key: endpoint.key.clone(),
                        kind: ChangeKind::LocalDeleted,
                        name: endpoint.template.name.clone(),
                        method: endpoint.template.method.clone(),
                        binding: Some(index),
                        remote: Some(remote_index),
                        fields: vec![],
                        current: Value::Null,
                        folder: None,
                        directory: None,
                    });
                }
                continue;
            }
            let current = current.unwrap();
            let mut fields = Vec::new();
            differences(
                Some(&binding.baseline),
                Some(&current),
                Some(&endpoint.definition),
                "",
                &mut fields,
            );
            if !source.follow_folders {
                fields.retain(|f| f.path != "/folder");
            }
            if !fields.is_empty()
                || binding.baseline != endpoint.definition
                || binding.remote_missing
                || binding.key != endpoint.key
            {
                let kind = if fields.iter().any(|f| f.conflict) {
                    ChangeKind::Conflict
                } else {
                    ChangeKind::Updated
                };
                changes.push(Change {
                    key: endpoint.key.clone(),
                    kind,
                    name: endpoint.template.name.clone(),
                    method: endpoint.template.method.clone(),
                    binding: Some(index),
                    remote: Some(remote_index),
                    fields,
                    current,
                    folder: None,
                    directory: None,
                });
            }
        } else {
            changes.push(Change {
                key: endpoint.key.clone(),
                kind: ChangeKind::Added,
                name: endpoint.template.name.clone(),
                method: endpoint.template.method.clone(),
                binding: None,
                remote: Some(remote_index),
                fields: vec![],
                current: Value::Null,
                folder: None,
                directory: None,
            });
        }
    }
    for (index, binding) in source.bindings.iter().enumerate() {
        if !matched.contains(&index) && !binding.remote_missing {
            changes.push(Change {
                key: format!("removed:{}", binding.key),
                kind: ChangeKind::Removed,
                name: binding.definition["name"]
                    .as_str()
                    .unwrap_or("已移除接口")
                    .into(),
                method: binding.definition["method"]
                    .as_str()
                    .unwrap_or("HTTP")
                    .into(),
                binding: Some(index),
                remote: None,
                fields: vec![],
                current: current_definition(project, source, binding).unwrap_or(Value::Null),
                folder: None,
                directory: None,
            });
        }
    }
    // Folder renames are inferred only from an unambiguous complete set of matched descendants.
    if source.follow_folders {
        for (index, binding) in source.folders.iter().enumerate() {
            let old_members = source
                .bindings
                .iter()
                .filter(|b| {
                    b.baseline["folder"].as_array().is_some_and(|a| {
                        a.starts_with(
                            &binding
                                .remote_path
                                .iter()
                                .map(|s| json!(s))
                                .collect::<Vec<_>>(),
                        )
                    })
                })
                .map(|b| b.key.clone())
                .collect::<HashSet<_>>();
            if snapshot.directories.contains(&binding.remote_path) || old_members.is_empty() {
                continue;
            }
            let candidates = snapshot
                .directories
                .iter()
                .filter(|p| p.len() == binding.remote_path.len())
                .filter(|p| {
                    let members = snapshot
                        .endpoints
                        .iter()
                        .filter(|e| {
                            e.definition["folder"].as_array().is_some_and(|a| {
                                a.starts_with(&p.iter().map(|s| json!(s)).collect::<Vec<_>>())
                            })
                        })
                        .map(|e| e.key.clone())
                        .collect::<HashSet<_>>();
                    members == old_members && !source.folders.iter().any(|f| &f.remote_path == *p)
                })
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                continue;
            }
            let directory = candidates[0].clone();
            let Some(folder) = project.folders.iter().find(|f| f.id == binding.folder_id) else {
                continue;
            };
            let base = folder_value(&binding.baseline_name, binding.baseline_parent);
            let current = folder_value(&folder.name, folder.parent_id);
            let mut remote = current.clone();
            remote["name"] = json!(directory.last().unwrap());
            let mut fields = Vec::new();
            differences(Some(&base), Some(&current), Some(&remote), "", &mut fields);
            if !fields.is_empty() || directory != binding.remote_path {
                changes.push(Change {
                    key: format!("folder:{}", binding.folder_id),
                    kind: if fields.iter().any(|f| f.conflict) {
                        ChangeKind::Conflict
                    } else {
                        ChangeKind::Folder
                    },
                    name: directory.last().unwrap().clone(),
                    method: "目录".into(),
                    binding: None,
                    remote: None,
                    fields,
                    current,
                    folder: Some(index),
                    directory: Some(directory),
                });
            }
        }
    }
    Ok(SyncPlan {
        source_id,
        project_hash: digest(&serde_json::to_value(project)?),
        snapshot_hash: snapshot.digest.clone(),
        changes,
    })
}

fn request_projection(r: &Request) -> Value {
    json!({"name":r.name,"method":r.method,"url":r.url,"description":r.description,"schema":r.schema,"folder_id":r.folder_id,"source":r.source,"openapi":r.openapi})
}
fn assign_projection(r: &mut Request, v: &Value) -> Result<()> {
    r.name = v["name"].as_str().context("请求名称无效")?.into();
    r.method = v["method"].as_str().context("请求方法无效")?.into();
    r.url = v["url"].as_str().context("请求地址无效")?.into();
    r.description = v["description"].as_str().unwrap_or_default().into();
    r.schema = (!v["schema"].is_null()).then(|| v["schema"].clone());
    r.folder_id = serde_json::from_value(v["folder_id"].clone())?;
    r.source = serde_json::from_value(v["source"].clone())?;
    r.openapi = serde_json::from_value(v["openapi"].clone())?;
    Ok(())
}
fn ensure_root(project: &mut Project, source: &mut SyncSource) -> Result<Uuid> {
    if let Some(id) = source.root_folder {
        project.folder(id)?;
        return Ok(id);
    }
    let base = source.config.name.clone();
    let mut name = base.clone();
    let mut n = 2;
    while project
        .folders
        .iter()
        .any(|f| f.parent_id.is_none() && f.name == name)
    {
        name = format!("{base} ({n})");
        n += 1;
    }
    let id = project.add_folder(&name, None)?;
    source.root_folder = Some(id);
    Ok(id)
}
fn ensure_directory(
    project: &mut Project,
    source: &mut SyncSource,
    path: &[String],
) -> Result<Uuid> {
    let mut parent = ensure_root(project, source)?;
    for n in 1..=path.len() {
        if let Some(binding) = source.folders.iter().find(|b| b.remote_path == path[..n])
            && project.folder(binding.folder_id).is_ok()
        {
            parent = binding.folder_id;
            continue;
        }
        let base = &path[n - 1];
        let mut name = base.clone();
        let mut suffix = 2;
        while project
            .folders
            .iter()
            .any(|f| f.parent_id == Some(parent) && f.name == name)
        {
            name = format!("{base} ({suffix})");
            suffix += 1;
        }
        let id = project.add_folder(&name, Some(parent))?;
        let binding = FolderBinding {
            folder_id: id,
            remote_path: path[..n].to_vec(),
            baseline_name: name,
            baseline_parent: Some(parent),
        };
        if let Some(old) = source
            .folders
            .iter_mut()
            .find(|b| b.remote_path == path[..n])
        {
            *old = binding;
        } else {
            source.folders.push(binding);
        }
        parent = id;
    }
    Ok(parent)
}
fn apply_definition(
    project: &mut Project,
    source: &mut SyncSource,
    id: Uuid,
    value: &Value,
    template: &Request,
    preserve_folder: bool,
) -> Result<()> {
    let folder = if preserve_folder {
        project
            .requests
            .iter()
            .find(|r| r.id == id)
            .and_then(|r| r.folder_id)
    } else if let Some(path) = value["folder"].as_array() {
        let path = path
            .iter()
            .map(|v| v.as_str().context("目录名称无效").map(str::to_owned))
            .collect::<Result<Vec<_>>>()?;
        Some(ensure_directory(project, source, &path)?)
    } else {
        serde_json::from_value(value["folder"]["localFolderId"].clone())?
    };
    let r = project
        .requests
        .iter_mut()
        .find(|r| r.id == id)
        .context("本地请求已删除")?;
    r.name = value["name"].as_str().context("接口名称无效")?.into();
    r.method = value["method"].as_str().context("接口方法无效")?.into();
    r.url = value["url"].as_str().context("接口地址无效")?.into();
    r.description = value["description"].as_str().unwrap_or_default().into();
    r.schema = (!value["schema"].is_null()).then(|| value["schema"].clone());
    r.folder_id = folder;
    // Definition metadata may change; previously entered values, toggles, files and custom rows survive.
    for (draft, remote, case_insensitive) in [
        (&mut r.query, &template.query, false),
        (&mut r.path, &template.path, false),
        (&mut r.headers, &template.headers, true),
    ] {
        for param in remote {
            if let Some(existing) = draft.iter_mut().find(|p| {
                if case_insensitive {
                    p.name.eq_ignore_ascii_case(&param.name)
                } else {
                    p.name == param.name
                }
            }) {
                existing.description = param.description.clone();
            } else {
                draft.push(param.clone());
            }
        }
    }
    r.source = Some(format!("apifox:{}:{}", source.config.project_id, source.id));
    let op = &value["contract"]["operation"];
    r.openapi = Some(OpenApiOperation {
        contract_id: source.id,
        path: value["path"].as_str().unwrap_or_default().into(),
        method: r.method.to_lowercase(),
        operation_id: op
            .get("operationId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        tags: op
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        directory: value["folder"].as_array().map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        }),
    });
    Ok(())
}

/// Applies a reviewed plan against the exact project revision it was built from.
pub fn apply(project: &mut Project, plan: &SyncPlan, selection: &Selection) -> Result<usize> {
    if digest(&serde_json::to_value(&*project)?) != plan.project_hash {
        bail!("本地内容已变化，请刷新变更预览后重新选择");
    }
    let mut next = project.clone();
    let source_index = next
        .sync_sources
        .iter()
        .position(|s| s.id == plan.source_id)
        .context("数据源不存在")?;
    let mut source = next.sync_sources.remove(source_index);
    let snapshot = source.latest.clone().context("请先检查更新")?;
    if snapshot.digest != plan.snapshot_hash {
        bail!("远端快照已变化，请刷新预览");
    }
    let before_requests = next
        .requests
        .iter()
        .map(|r| (r.id, request_projection(r)))
        .collect::<HashMap<_, _>>();
    let before_folders = next.folders.clone();
    let mut undo = UndoBatch {
        bindings: source.bindings.clone(),
        binding_ids: HashSet::new(),
        folders: source.folders.clone(),
        requests: vec![],
        folder_patches: vec![],
        generation: source.generation,
    };
    let mut count = 0;
    // Apply selected folder identities first so matching requests reuse the renamed folder ID.
    for change in plan
        .changes
        .iter()
        .filter(|c| c.folder.is_some() && selection.included.contains(&c.key))
    {
        let index = change.folder.unwrap();
        let binding = source.folders[index].clone();
        let mut value = change.current.clone();
        resolve_fields(change, selection, &mut value)?;
        next.rename_folder(
            binding.folder_id,
            value["name"].as_str().context("目录名称无效")?,
        )?;
        source.folders[index].remote_path = change.directory.clone().unwrap();
        source.folders[index].baseline_name =
            change.directory.as_ref().unwrap().last().unwrap().clone();
        count += 1;
    }
    for change in plan
        .changes
        .iter()
        .filter(|c| c.folder.is_none() && selection.included.contains(&c.key))
    {
        if change.kind == ChangeKind::Removed {
            let b = &mut source.bindings[change.binding.unwrap()];
            b.remote_missing = true;
            if selection.archive.contains(&change.key)
                && next.requests.iter().any(|r| r.id == b.request_id)
            {
                let archive = ensure_directory(&mut next, &mut source, &["已移除".into()])?;
                let id = source.bindings[change.binding.unwrap()].request_id;
                next.move_request_to_folder(id, Some(archive))?;
            }
            count += 1;
            continue;
        }
        let endpoint = &snapshot.endpoints[change.remote.unwrap()];
        let existing = change.binding.map(|i| source.bindings[i].clone());
        if change.kind == ChangeKind::LocalDeleted {
            let resolution = selection
                .resolutions
                .get(&(change.key.clone(), "$".into()))
                .context("请选择恢复接口或保留本地删除")?;
            if *resolution == Resolution::Local {
                let binding = &mut source.bindings[change.binding.unwrap()];
                binding.baseline = endpoint.definition.clone();
                binding.key = endpoint.key.clone();
                binding.locator = endpoint.locator.clone();
                binding.locally_deleted = true;
                binding.remote_missing = false;
                count += 1;
                continue;
            }
        }
        let id = existing
            .as_ref()
            .map(|b| b.request_id)
            .unwrap_or_else(Uuid::new_v4);
        let new_request = !next.requests.iter().any(|r| r.id == id);
        if new_request {
            let mut request = endpoint.template.clone();
            request.id = id;
            next.requests.push(request);
        }
        let mut value =
            if change.kind == ChangeKind::Added || change.kind == ChangeKind::LocalDeleted {
                endpoint.definition.clone()
            } else {
                change.current.clone()
            };
        resolve_fields(change, selection, &mut value)?;
        let preserve_folder = !new_request && value["folder"] == change.current["folder"];
        apply_definition(
            &mut next,
            &mut source,
            id,
            &value,
            &endpoint.template,
            preserve_folder,
        )?;
        if let Some(operation) = next
            .requests
            .iter_mut()
            .find(|r| r.id == id)
            .and_then(|r| r.openapi.as_mut())
        {
            operation.path = endpoint.definition["path"]
                .as_str()
                .unwrap_or_default()
                .into();
            operation.method = endpoint.template.method.to_lowercase();
            operation.directory = endpoint.definition["folder"].as_array().map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            });
        }
        let mut binding = RequestBinding {
            key: endpoint.key.clone(),
            locator: endpoint.locator.clone(),
            request_id: id,
            baseline: endpoint.definition.clone(),
            definition: value,
            remote_missing: false,
            locally_deleted: false,
            compatibility: existing
                .as_ref()
                .map(|b| b.compatibility.clone())
                .unwrap_or_default(),
        };
        if !new_request
            && change.fields.iter().any(|f| {
                f.path.contains("requestBody")
                    || f.path.contains("parameters")
                    || f.path.contains("dependencies")
                    || f.path == "/schema"
            })
        {
            let note = "接口结构已更新，本地参数与 Body 已保留，请检查字段、必填参数和媒体类型。";
            if !binding.compatibility.iter().any(|s| s == note) {
                binding.compatibility.push(note.into());
            }
        }
        if let Some(index) = change.binding {
            source.bindings[index] = binding;
        } else {
            source.bindings.push(binding);
        }
        count += 1;
    }
    if count == 0 {
        return Ok(0);
    }
    let previous_ids = undo
        .bindings
        .iter()
        .map(|b| b.request_id)
        .collect::<HashSet<_>>();
    undo.binding_ids.extend(
        source
            .bindings
            .iter()
            .filter(|b| !previous_ids.contains(&b.request_id))
            .map(|b| b.request_id),
    );
    for change in plan
        .changes
        .iter()
        .filter(|c| selection.included.contains(&c.key))
    {
        if let Some(index) = change.binding {
            undo.binding_ids.insert(undo.bindings[index].request_id);
        }
    }
    undo.bindings
        .retain(|b| undo.binding_ids.contains(&b.request_id));
    for r in &next.requests {
        let after = request_projection(r);
        let before = before_requests.get(&r.id).cloned();
        if before.as_ref() != Some(&after) {
            undo.requests.push(RequestPatch {
                id: r.id,
                before,
                after,
                creation_hash: (!before_requests.contains_key(&r.id))
                    .then(|| digest(&serde_json::to_value(r).expect("request JSON"))),
            });
        }
    }
    for f in &next.folders {
        let before = before_folders.iter().find(|b| b.id == f.id).cloned();
        if before
            .as_ref()
            .is_none_or(|b| b.name != f.name || b.parent_id != f.parent_id)
        {
            undo.folder_patches.push(FolderPatch {
                before,
                after: f.clone(),
            });
        }
    }
    source.generation += 1;
    source.runs.insert(
        0,
        SyncRun {
            id: Uuid::new_v4(),
            at: Utc::now().to_rfc3339(),
            summary: format!("已应用 {count} 项变更"),
            digest: snapshot.digest,
            generation: source.generation,
            undo: Some(undo),
        },
    );
    source.runs.truncate(10);
    next.sync_sources.insert(source_index, source);
    next.normalize()?;
    *project = next;
    Ok(count)
}
fn resolve_fields(change: &Change, selection: &Selection, value: &mut Value) -> Result<()> {
    for field in &change.fields {
        let resolution = if field.conflict {
            *selection
                .resolutions
                .get(&(change.key.clone(), field.path.clone()))
                .context("仍有冲突字段未处理")?
        } else {
            Resolution::Remote
        };
        if resolution == Resolution::Remote {
            set_pointer(value, &field.path, field.remote.clone())?;
        }
    }
    Ok(())
}

pub fn undo(project: &mut Project, source_id: Uuid, run_id: Uuid) -> Result<()> {
    let mut next = project.clone();
    let index = next
        .sync_sources
        .iter()
        .position(|s| s.id == source_id)
        .context("数据源不存在")?;
    let mut source = next.sync_sources.remove(index);
    let run = source
        .runs
        .iter()
        .find(|r| r.id == run_id)
        .context("同步记录不存在")?;
    if run.generation != source.generation {
        bail!("请先撤销后续同步批次");
    }
    let undo = run.undo.clone().context("该批次已撤销")?;
    for patch in &undo.requests {
        let pos = next.requests.iter().position(|r| r.id == patch.id);
        if let Some(before) = &patch.before {
            let r = next
                .requests
                .get_mut(pos.context("请求已删除，无法安全撤销")?)
                .unwrap();
            if request_projection(r) != patch.after {
                bail!("请求定义在同步后已被编辑，无法安全撤销本批改动");
            }
            assign_projection(r, before)?;
        } else if let Some(pos) = pos {
            if patch.creation_hash.as_ref()
                == Some(&digest(&serde_json::to_value(&next.requests[pos])?))
            {
                next.delete_request(patch.id);
            } else {
                let r = &mut next.requests[pos];
                r.source = None;
                r.openapi = None;
                r.folder_id = None;
            }
        }
    }
    for patch in undo.folder_patches.iter().rev() {
        if let Some(before) = &patch.before {
            let folder = next
                .folders
                .iter_mut()
                .find(|f| f.id == patch.after.id)
                .context("目录已删除，无法安全撤销")?;
            if folder.name != patch.after.name || folder.parent_id != patch.after.parent_id {
                bail!("目录在同步后已调整，无法安全撤销");
            }
            folder.name = before.name.clone();
            folder.parent_id = before.parent_id;
        } else if next.folder(patch.after.id).is_ok_and(|f| {
            f.name == patch.after.name
                && f.parent_id == patch.after.parent_id
                && f.auth == patch.after.auth
        }) && !next
            .requests
            .iter()
            .any(|r| r.folder_id == Some(patch.after.id))
            && !next
                .folders
                .iter()
                .any(|f| f.parent_id == Some(patch.after.id))
        {
            next.folders.retain(|f| f.id != patch.after.id);
        }
    }
    let restored_ids = undo
        .bindings
        .iter()
        .map(|b| b.request_id)
        .collect::<HashSet<_>>();
    source.bindings.retain(|b| {
        !undo.binding_ids.contains(&b.request_id) || restored_ids.contains(&b.request_id)
    });
    for binding in undo.bindings {
        if let Some(current) = source
            .bindings
            .iter_mut()
            .find(|b| b.request_id == binding.request_id)
        {
            *current = binding;
        } else {
            source.bindings.push(binding);
        }
    }
    source.folders = undo.folders;
    source.generation = undo.generation;
    if source
        .root_folder
        .is_some_and(|id| next.folder(id).is_err())
    {
        source.root_folder = None;
    }
    let run = source.runs.iter_mut().find(|r| r.id == run_id).unwrap();
    run.undo = None;
    run.summary = "已撤销本批变更".into();
    next.sync_sources.insert(index, source);
    next.normalize()?;
    *project = next;
    Ok(())
}

pub fn binding_for_request(project: &Project, id: Uuid) -> Option<(&SyncSource, &RequestBinding)> {
    project.sync_sources.iter().find_map(|s| {
        s.bindings
            .iter()
            .find(|b| b.request_id == id)
            .map(|b| (s, b))
    })
}
