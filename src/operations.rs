//! Request-local operations. All mutations apply to an execution snapshot.
use crate::{
    model::{Parameter, Request, interpolate},
    transport::RunRecord,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    Pre,
    Post,
}
impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pre => "前置操作",
            Self::Post => "后置操作",
        }
    }
}
pub fn inherit_by_default() -> bool {
    true
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationSet {
    #[serde(default)]
    pub pre: Vec<Operation>,
    #[serde(default)]
    pub post: Vec<Operation>,
    #[serde(default = "inherit_by_default")]
    pub inherit_pre: bool,
    #[serde(default = "inherit_by_default")]
    pub inherit_post: bool,
}
impl Default for OperationSet {
    fn default() -> Self {
        Self {
            pre: vec![],
            post: vec![],
            inherit_pre: true,
            inherit_post: true,
        }
    }
}
impl OperationSet {
    pub fn list(&self, stage: Stage) -> &[Operation] {
        match stage {
            Stage::Pre => &self.pre,
            Stage::Post => &self.post,
        }
    }
    pub fn list_mut(&mut self, stage: Stage) -> &mut Vec<Operation> {
        match stage {
            Stage::Pre => &mut self.pre,
            Stage::Post => &mut self.post,
        }
    }
    pub fn inherits(&self, stage: Stage) -> bool {
        match stage {
            Stage::Pre => self.inherit_pre,
            Stage::Post => self.inherit_post,
        }
    }
    pub fn set_inherits(&mut self, stage: Stage, enabled: bool) {
        match stage {
            Stage::Pre => self.inherit_pre = enabled,
            Stage::Post => self.inherit_post = enabled,
        }
    }
    pub fn is_default(&self) -> bool {
        self.pre.is_empty() && self.post.is_empty() && self.inherit_pre && self.inherit_post
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationOrigin {
    pub folder_id: Option<Uuid>,
    pub label: String,
}
pub struct OperationPlan {
    pub pre: Vec<Operation>,
    pub post: Vec<Operation>,
}
impl crate::model::Project {
    pub fn inherited_operations(
        &self,
        folder: Option<Uuid>,
        stage: Stage,
    ) -> Result<Vec<Operation>> {
        let mut cursor = folder;
        let mut visited = std::collections::HashSet::new();
        let mut groups = vec![];
        while let Some(id) = cursor {
            if !visited.insert(id) || visited.len() > 32 {
                bail!("操作继承链存在循环或超过 32 层");
            }
            let folder = self.folder(id)?;
            let origin = OperationOrigin {
                folder_id: Some(id),
                label: format!("文件夹 · {}", self.folder_path(id)),
            };
            let group = folder
                .operations
                .list(stage)
                .iter()
                .cloned()
                .map(|mut op| {
                    op.origin = Some(origin.clone());
                    op
                })
                .collect::<Vec<_>>();
            groups.push(group);
            if !folder.operations.inherits(stage) {
                break;
            }
            cursor = folder.parent_id;
        }
        if stage == Stage::Pre {
            groups.reverse();
        }
        Ok(groups.into_iter().flatten().collect())
    }
    pub fn operation_plan(&self, request: &Request) -> Result<OperationPlan> {
        if request.operations_resolved {
            return Ok(OperationPlan {
                pre: request.pre_operations.clone(),
                post: request.post_operations.clone(),
            });
        }
        let resolve =
            |stage: Stage, enabled: bool, local: &[Operation]| -> Result<Vec<Operation>> {
                let mut inherited = if enabled {
                    self.inherited_operations(request.folder_id, stage)?
                } else {
                    vec![]
                };
                let mut local = local
                    .iter()
                    .cloned()
                    .map(|mut op| {
                        op.origin = Some(OperationOrigin {
                            folder_id: None,
                            label: format!("请求 · {}", request.name),
                        });
                        op
                    })
                    .collect::<Vec<_>>();
                let result = if stage == Stage::Pre {
                    inherited.append(&mut local);
                    inherited
                } else {
                    local.append(&mut inherited);
                    local
                };
                if result.len() > 64 {
                    bail!("{}级联后超过 64 项，请减少操作或关闭继承", stage.label());
                }
                Ok(result)
            };
        Ok(OperationPlan {
            pre: resolve(
                Stage::Pre,
                request.inherit_pre_operations,
                &request.pre_operations,
            )?,
            post: resolve(
                Stage::Post,
                request.inherit_post_operations,
                &request.post_operations,
            )?,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    SetVariable,
    Uuid,
    Timestamp,
    SetHeader,
    Wait,
    ExtractJson,
    ExtractHeader,
    AssertStatus,
    AssertJson,
    AssertElapsed,
    JavaScript,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::SetVariable => "设置变量",
            Self::Uuid => "生成 UUID",
            Self::Timestamp => "生成时间戳",
            Self::SetHeader => "设置 Header / Metadata",
            Self::Wait => "等待",
            Self::ExtractJson => "提取 JSON 变量",
            Self::ExtractHeader => "提取 Header / Metadata 变量",
            Self::AssertStatus => "断言状态码",
            Self::AssertJson => "断言 JSON 字段",
            Self::AssertElapsed => "断言响应耗时",
            Self::JavaScript => "JavaScript 脚本",
        }
    }
    pub fn choices(stage: Stage) -> Vec<Self> {
        match stage {
            Stage::Pre => vec![
                Self::SetVariable,
                Self::Uuid,
                Self::Timestamp,
                Self::SetHeader,
                Self::Wait,
                Self::JavaScript,
            ],
            Stage::Post => vec![
                Self::ExtractJson,
                Self::ExtractHeader,
                Self::AssertStatus,
                Self::AssertJson,
                Self::AssertElapsed,
                Self::SetVariable,
                Self::JavaScript,
            ],
        }
    }
    pub fn has_variable(self) -> bool {
        matches!(
            self,
            Self::SetVariable
                | Self::Uuid
                | Self::Timestamp
                | Self::ExtractJson
                | Self::ExtractHeader
        )
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    #[default]
    Run,
    Environment,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub id: Uuid,
    pub enabled: bool,
    #[serde(default)]
    pub continue_on_error: bool,
    pub kind: Kind,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub script: String,
    #[serde(skip)]
    pub origin: Option<OperationOrigin>,
}
impl Operation {
    pub fn new(kind: Kind) -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: true,
            continue_on_error: false,
            kind,
            key: String::new(),
            value: match kind {
                Kind::Wait => "100",
                Kind::AssertElapsed => "2000",
                Kind::AssertStatus => "200",
                _ => "",
            }
            .into(),
            source: if kind == Kind::ExtractHeader {
                "x-request-id"
            } else {
                "$.data.token"
            }
            .into(),
            scope: Scope::Run,
            name: String::new(),
            script: String::new(),
            origin: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    Skipped,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationReport {
    pub id: Uuid,
    pub stage: Stage,
    pub label: String,
    pub outcome: Outcome,
    pub detail: String,
    pub elapsed_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<OperationOrigin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<crate::scripting::ScriptTest>,
}
#[derive(Clone, Debug)]
pub struct VariableWrite {
    pub key: String,
    pub previous: Option<String>,
    pub value: String,
}
#[derive(Clone, Debug)]
pub struct EnvironmentPatch {
    pub name: String,
    pub writes: Vec<VariableWrite>,
}
impl EnvironmentPatch {
    pub fn apply(&self, project: &mut crate::model::Project) -> Result<()> {
        let env = project
            .environments
            .iter_mut()
            .find(|e| e.name == self.name)
            .context("目标环境已删除或重命名，未写回变量")?;
        if self
            .writes
            .iter()
            .any(|w| env.variables.get(&w.key) != w.previous.as_ref())
        {
            bail!("环境变量已被其他操作修改，未写回本次变量");
        }
        for w in &self.writes {
            env.variables.insert(w.key.clone(), w.value.clone());
        }
        Ok(())
    }
}
pub struct OperationContext {
    pub variables: BTreeMap<String, String>,
    baseline: BTreeMap<String, String>,
    writes: BTreeMap<String, String>,
    pub reports: Vec<OperationReport>,
    script_output: Option<crate::scripting::ScriptOutput>,
    worker_executable: Option<std::path::PathBuf>,
}
impl OperationContext {
    pub fn new(variables: BTreeMap<String, String>) -> Self {
        Self {
            baseline: variables.clone(),
            variables,
            writes: BTreeMap::new(),
            reports: vec![],
            script_output: None,
            worker_executable: None,
        }
    }
    pub fn with_script_worker(mut self, executable: impl Into<std::path::PathBuf>) -> Self {
        self.worker_executable = Some(executable.into());
        self
    }
    fn store(&mut self, op: &Operation, value: String) -> Result<()> {
        let key = op.key.trim();
        if key.is_empty() || key.contains(['{', '}']) {
            bail!("变量名不能为空或包含花括号");
        }
        if value.len() > 64 * 1024 {
            bail!("变量值超过 64 KiB 上限");
        }
        self.variables.insert(key.into(), value.clone());
        if op.scope == Scope::Environment {
            self.writes.insert(key.into(), value);
        }
        Ok(())
    }
    fn report(
        &mut self,
        op: &Operation,
        stage: Stage,
        outcome: Outcome,
        detail: impl Into<String>,
        elapsed_ms: u128,
    ) {
        let output = self.script_output.take().unwrap_or_default();
        let detail = if op.kind == Kind::JavaScript && outcome == Outcome::Passed {
            format!("脚本执行完成 · {} 项测试", output.tests.len())
        } else {
            detail.into()
        };
        self.reports.push(OperationReport {
            id: op.id,
            stage,
            label: if op.name.trim().is_empty() {
                op.kind.label().into()
            } else {
                op.name.clone()
            },
            outcome,
            detail,
            elapsed_ms,
            origin: op.origin.clone(),
            logs: output.logs,
            tests: output.tests,
        });
    }
    fn run_one(
        &mut self,
        op: &Operation,
        request: &mut Request,
        response: Option<&RunRecord>,
    ) -> Result<()> {
        match op.kind {
            Kind::SetVariable => self.store(op, interpolate(&op.value, &self.variables)?),
            Kind::Uuid => self.store(op, Uuid::new_v4().to_string()),
            Kind::Timestamp => self.store(op, chrono::Utc::now().timestamp_millis().to_string()),
            Kind::SetHeader => {
                let key = interpolate(&op.key, &self.variables)?;
                let value = interpolate(&op.value, &self.variables)?;
                reqwest::header::HeaderName::from_bytes(key.as_bytes())
                    .context("Header 名称无效")?;
                reqwest::header::HeaderValue::from_str(&value).context("Header 值无效")?;
                if let Some(h) = request
                    .headers
                    .iter_mut()
                    .find(|h| h.name.eq_ignore_ascii_case(&key))
                {
                    h.value = value;
                    h.enabled = true;
                } else {
                    request.headers.push(Parameter::new(&key, &value));
                }
                Ok(())
            }
            Kind::ExtractJson | Kind::AssertJson => {
                let r = response.context("此操作需要响应")?;
                if r.truncated {
                    bail!("响应已截断，无法可靠处理 JSON");
                }
                let body = r
                    .messages
                    .iter()
                    .rev()
                    .find(|m| !m.sent)
                    .map(|m| m.body.as_str())
                    .unwrap_or(&r.body);
                let json: Value = serde_json::from_str(body).context("响应不是完整 JSON")?;
                let value = select_json(&json, &op.source)?;
                if op.kind == Kind::ExtractJson {
                    self.store(
                        op,
                        value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    )
                } else {
                    let text = interpolate(&op.value, &self.variables)?;
                    let expected =
                        serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
                    if value != &expected {
                        bail!("JSON 字段与预期值不一致（值不写入日志）");
                    }
                    Ok(())
                }
            }
            Kind::ExtractHeader => {
                let r = response.context("此操作需要响应")?;
                let name = interpolate(&op.source, &self.variables)?;
                let value = r
                    .headers
                    .iter()
                    .chain(&r.trailers)
                    .find(|(key, _)| key.eq_ignore_ascii_case(&name))
                    .context("没有找到指定 Header / Metadata / Trailer")?
                    .1
                    .clone();
                self.store(op, value)
            }
            Kind::AssertStatus => {
                let r = response.context("此操作需要响应")?;
                let expected = interpolate(&op.value, &self.variables)?;
                if r.status.split_whitespace().next() != Some(expected.trim()) {
                    bail!("响应状态码与预期值不一致");
                }
                Ok(())
            }
            Kind::AssertElapsed => {
                let limit = interpolate(&op.value, &self.variables)?
                    .parse::<u128>()
                    .context("耗时上限应为毫秒整数")?;
                if response.context("此操作需要响应")?.elapsed_ms > limit {
                    bail!("响应耗时超过上限");
                }
                Ok(())
            }
            Kind::Wait => bail!("等待只能用于前置操作"),
            Kind::JavaScript => bail!("JavaScript 需要独立进程执行"),
        }
    }
    async fn run_script(
        &mut self,
        op: &Operation,
        stage: Stage,
        request: &mut Request,
        response: Option<&RunRecord>,
        deadline: tokio::time::Instant,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let mut environment = self.baseline.clone();
        environment.extend(self.writes.clone());
        let input = crate::scripting::ScriptInput {
            source: op.script.clone(),
            stage,
            variables: self.variables.clone(),
            environment,
            request: crate::scripting::ScriptRequest::from(&*request),
            response: response.map(crate::scripting::ScriptResponse::from),
            check_only: false,
        };
        let output =
            crate::scripting::execute(input, self.worker_executable.as_deref(), deadline, cancel)
                .await?;
        let succeeded = output.succeeded();
        let failure = output
            .error
            .clone()
            .unwrap_or_else(|| "脚本中的测试未全部通过".into());
        self.script_output = Some(output.clone());
        if !succeeded {
            bail!("{failure}");
        }
        if output.variables.len() + output.environment.len() > 256 {
            bail!("脚本变量数量超过上限");
        }
        // Validate everything against copies, then commit the script's changes atomically.
        let mut variables = self.variables.clone();
        let mut writes = self.writes.clone();
        for (is_environment, values) in [(true, &output.environment), (false, &output.variables)] {
            for (key, value) in values {
                if key.trim().is_empty()
                    || key.len() > 256
                    || key.contains(['{', '}'])
                    || value.len() > 64 * 1024
                {
                    bail!("脚本返回了无效变量");
                }
                variables.insert(key.clone(), value.clone());
                if is_environment {
                    writes.insert(key.clone(), value.clone());
                }
            }
        }
        if stage == Stage::Pre
            && let Some(change) = output.request
        {
            crate::scripting::apply_request(change, request)?;
        }
        self.variables = variables;
        self.writes = writes;
        Ok(())
    }
    pub async fn pre(&mut self, request: &mut Request, cancel: &CancellationToken) -> bool {
        let ops = request
            .pre_operations
            .iter()
            .take(65)
            .cloned()
            .collect::<Vec<_>>();
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(request.timeout_secs.max(1));
        let mut allowed = true;
        for (index, op) in ops.iter().enumerate() {
            if index >= 64 {
                self.report(op, Stage::Pre, Outcome::Failed, "每个阶段最多 64 个操作", 0);
                return false;
            }
            if !op.enabled || !allowed {
                self.report(
                    op,
                    Stage::Pre,
                    Outcome::Skipped,
                    if !op.enabled {
                        "已禁用"
                    } else {
                        "前序操作失败"
                    },
                    0,
                );
                continue;
            }
            let start = Instant::now();
            let work = async {
                if !Kind::choices(Stage::Pre).contains(&op.kind) {
                    bail!("此操作不能放在前置阶段");
                }
                if op.kind == Kind::JavaScript {
                    self.run_script(op, Stage::Pre, request, None, deadline, cancel)
                        .await
                } else if op.kind == Kind::Wait {
                    let ms = interpolate(&op.value, &self.variables)?
                        .parse::<u64>()
                        .context("等待时间应为毫秒整数")?;
                    if ms > 30_000 {
                        bail!("单次等待最多 30000 毫秒");
                    }
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    Ok(())
                } else {
                    self.run_one(op, request, None)
                }
            };
            let result = tokio::select! {biased; _=cancel.cancelled()=>Err(anyhow::anyhow!("操作已取消")), result=tokio::time::timeout_at(deadline,work)=>result.unwrap_or_else(|_|Err(anyhow::anyhow!("前置操作超时")))};
            let failed = result.is_err();
            self.report(
                op,
                Stage::Pre,
                if failed {
                    Outcome::Failed
                } else {
                    Outcome::Passed
                },
                result
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "操作完成（值不写入日志）".into()),
                start.elapsed().as_millis(),
            );
            if failed
                && (!op.continue_on_error
                    || cancel.is_cancelled()
                    || tokio::time::Instant::now() >= deadline)
            {
                allowed = false;
            }
        }
        allowed && !cancel.is_cancelled()
    }
    pub async fn finish(
        mut self,
        request: &Request,
        record: &mut RunRecord,
        received: bool,
        cancel: &CancellationToken,
    ) -> Option<EnvironmentPatch> {
        let mut allowed = received && !cancel.is_cancelled();
        let mut snapshot = request.clone();
        for (index, op) in request.post_operations.iter().enumerate() {
            if index >= 64 {
                self.report(
                    op,
                    Stage::Post,
                    Outcome::Failed,
                    "每个阶段最多 64 个操作",
                    0,
                );
                break;
            }
            if !op.enabled || !allowed {
                self.report(
                    op,
                    Stage::Post,
                    Outcome::Skipped,
                    if !op.enabled {
                        "已禁用"
                    } else {
                        "前序失败、传输失败或已取消"
                    },
                    0,
                );
                continue;
            }
            let start = Instant::now();
            let result = if op.kind == Kind::JavaScript {
                self.run_script(
                    op,
                    Stage::Post,
                    &mut snapshot,
                    Some(record),
                    tokio::time::Instant::now() + crate::scripting::TIMEOUT,
                    cancel,
                )
                .await
            } else if !Kind::choices(Stage::Post).contains(&op.kind) {
                Err(anyhow::anyhow!("此操作不能放在后置阶段"))
            } else {
                self.run_one(op, &mut snapshot, Some(record))
            };
            let failed = result.is_err();
            self.report(
                op,
                Stage::Post,
                if failed {
                    Outcome::Failed
                } else {
                    Outcome::Passed
                },
                result
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "操作完成（值不写入日志）".into()),
                start.elapsed().as_millis(),
            );
            if failed && (!op.continue_on_error || cancel.is_cancelled()) {
                allowed = false;
            }
        }
        let failed = self.reports.iter().any(|r| r.outcome == Outcome::Failed);
        let patch = if received
            && !cancel.is_cancelled()
            && !record.error
            && !failed
            && !self.writes.is_empty()
        {
            Some(EnvironmentPatch {
                name: record.environment.clone(),
                writes: self
                    .writes
                    .into_iter()
                    .map(|(key, value)| VariableWrite {
                        previous: self.baseline.get(&key).cloned(),
                        key,
                        value,
                    })
                    .collect(),
            })
        } else {
            None
        };
        record.operations = self.reports;
        patch
    }
}
/// JSON Pointer or a deliberately small JSONPath subset: $.name.items[0].id.
pub fn select_json<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
    let path = path.trim();
    if path == "$" || path.is_empty() {
        return Ok(value);
    }
    if path.starts_with('/') {
        return value.pointer(path).context("JSON Pointer 未匹配字段");
    }
    let mut rest = path
        .strip_prefix('$')
        .context("路径需使用 $.field、[索引] 或 JSON Pointer")?;
    let mut current = value;
    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix('.') {
            let end = tail.find(['.', '[']).unwrap_or(tail.len());
            let key = &tail[..end];
            if key.is_empty() {
                bail!("JSON 路径字段不能为空");
            }
            current = current.get(key).context("JSON 路径未匹配字段")?;
            rest = &tail[end..];
        } else if let Some(tail) = rest.strip_prefix('[') {
            let end = tail.find(']').context("JSON 路径缺少 ]")?;
            let index = tail[..end].parse::<usize>().context("仅支持数组整数索引")?;
            current = current.get(index).context("JSON 数组索引不存在")?;
            rest = &tail[end + 1..];
        } else {
            bail!("不支持此 JSON 路径语法，请改用 JSON Pointer");
        }
    }
    Ok(current)
}
pub async fn execute_http(
    mut request: Request,
    vars: BTreeMap<String, String>,
    environment: String,
    cancel: CancellationToken,
) -> (RunRecord, Option<EnvironmentPatch>) {
    let started = Instant::now();
    let started_at = chrono::Utc::now();
    let mut ctx = OperationContext::new(vars);
    let prepared = ctx.pre(&mut request, &cancel).await;
    let result = if prepared {
        crate::transport::http::execute(
            request.clone(),
            ctx.variables.clone(),
            environment.clone(),
            cancel.clone(),
        )
        .await
    } else {
        Err(anyhow::anyhow!("前置操作失败或已取消，未发送请求"))
    };
    let received = result.is_ok();
    let mut record = result.unwrap_or_else(|e| {
        let mut r = RunRecord::error(&request, environment, format!("{e:#}"));
        r.started_at = started_at;
        r.elapsed_ms = started.elapsed().as_millis();
        r.timeline.push(crate::transport::TimingMark {
            elapsed_ms: r.elapsed_ms,
            label: "执行失败或取消".into(),
        });
        r
    });
    let patch = ctx.finish(&request, &mut record, received, &cancel).await;
    (record, patch)
}
