//! Boa runs in a headless invocation of this executable. Only JSON crosses the process boundary.
use crate::{
    model::{Parameter, Request},
    operations::Stage,
    transport::RunRecord,
};
use anyhow::{Context as _, Result, bail};
use boa_engine::{
    Context, NativeFunction, Script, Source, js_string, module::IdleModuleLoader,
    property::Attribute,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    rc::Rc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::Instant,
};
use tokio_util::sync::CancellationToken;

pub const TIMEOUT: Duration = Duration::from_secs(3);
pub const MAX_SOURCE: usize = 256 * 1024;
const MAX_INPUT: usize = 24 * 1024 * 1024;
const MAX_OUTPUT: usize = 2 * 1024 * 1024;
const WORKER_ARG: &str = "--script-worker";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<Parameter>,
    pub query: Vec<Parameter>,
    pub path: Vec<Parameter>,
    pub body_mode: crate::model::BodyKind,
    pub body: String,
}
impl From<&Request> for ScriptRequest {
    fn from(r: &Request) -> Self {
        Self {
            method: r.method.clone(),
            url: r.url.clone(),
            headers: r.headers.clone(),
            query: r.query.clone(),
            path: r.path.clone(),
            body_mode: r.body.kind,
            body: r.body.text.clone(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptResponse {
    pub status: String,
    pub code: Option<u16>,
    pub body: String,
    pub headers: Vec<(String, String)>,
    pub trailers: Vec<(String, String)>,
    pub elapsed_ms: u128,
    pub truncated: bool,
    pub messages: Vec<crate::transport::SessionMessage>,
}
impl From<&RunRecord> for ScriptResponse {
    fn from(r: &RunRecord) -> Self {
        Self {
            status: r.status.clone(),
            code: r
                .status
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok()),
            body: r
                .messages
                .iter()
                .rev()
                .find(|m| !m.sent)
                .map(|m| m.body.clone())
                .unwrap_or_else(|| r.body.clone()),
            headers: r.headers.clone(),
            trailers: r.trailers.clone(),
            elapsed_ms: r.elapsed_ms,
            truncated: r.truncated,
            messages: r.messages.clone(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptInput {
    pub source: String,
    pub stage: Stage,
    pub variables: BTreeMap<String, String>,
    pub environment: BTreeMap<String, String>,
    pub request: ScriptRequest,
    pub response: Option<ScriptResponse>,
    pub check_only: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptTest {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScriptOutput {
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    pub request: Option<ScriptRequest>,
    #[serde(default)]
    pub logs: Vec<String>,
    #[serde(default)]
    pub tests: Vec<ScriptTest>,
    pub error: Option<String>,
}
impl ScriptOutput {
    pub fn succeeded(&self) -> bool {
        self.error.is_none() && self.tests.iter().all(|t| t.passed)
    }
}

/// An explicit executable is useful to headless callers and integration tests.
pub async fn execute(
    input: ScriptInput,
    executable: Option<&Path>,
    deadline: Instant,
    cancel: &CancellationToken,
) -> Result<ScriptOutput> {
    if input.source.len() > MAX_SOURCE {
        bail!("脚本超过 256 KiB 上限");
    }
    if cancel.is_cancelled() {
        bail!("脚本已取消");
    }
    let bytes = serde_json::to_vec(&input)?;
    if bytes.len() > MAX_INPUT {
        bail!("脚本输入超过 24 MiB 上限");
    }
    let executable = executable
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_exe)?;
    let mut command = Command::new(executable);
    command
        .arg(WORKER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW, also for console builds.
    let mut child = command.spawn().context("无法启动 JavaScript 子进程")?;
    let mut stdin = child.stdin.take().context("脚本输入管道不可用")?;
    let stdout = child.stdout.take().context("脚本输出管道不可用")?;
    let work = async {
        let send = async {
            stdin.write_all(&bytes).await?;
            stdin.shutdown().await?;
            drop(stdin);
            Ok::<_, std::io::Error>(())
        };
        let receive = async {
            let mut data = vec![];
            stdout
                .take((MAX_OUTPUT + 1) as u64)
                .read_to_end(&mut data)
                .await?;
            Ok::<_, std::io::Error>(data)
        };
        let (_, data, status) = tokio::try_join!(send, receive, child.wait())?;
        if data.len() > MAX_OUTPUT {
            bail!("脚本输出超过 2 MiB 上限");
        }
        if !status.success() {
            bail!("JavaScript 子进程异常退出，未应用修改");
        }
        serde_json::from_slice(&data).context("JavaScript 子进程返回了无效结果")
    };
    let deadline = deadline.min(Instant::now() + TIMEOUT);
    let result = tokio::select! {
        biased;
        _=cancel.cancelled()=>Err(anyhow::anyhow!("脚本已取消")),
        result=tokio::time::timeout_at(deadline,work)=>result.unwrap_or_else(|_|Err(anyhow::anyhow!("脚本超时，已终止子进程（最多 3 秒）"))),
    };
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    result
}

pub fn worker_main() -> Option<Result<()>> {
    if std::env::args().nth(1).as_deref() != Some(WORKER_ARG) {
        return None;
    }
    Some((|| {
        let mut data = vec![];
        std::io::stdin()
            .take((MAX_INPUT + 1) as u64)
            .read_to_end(&mut data)?;
        if data.len() > MAX_INPUT {
            bail!("script input too large");
        }
        let input: ScriptInput = serde_json::from_slice(&data)?;
        let output = run_worker(input);
        let mut bytes = serde_json::to_vec(&output)?;
        if bytes.len() > MAX_OUTPUT {
            bytes = serde_json::to_vec(&ScriptOutput {
                error: Some("脚本输出超过 2 MiB 上限".into()),
                ..Default::default()
            })?;
        }
        std::io::stdout().write_all(&bytes)?;
        Ok(())
    })())
}

fn run_worker(input: ScriptInput) -> ScriptOutput {
    match evaluate(&input) {
        Ok(output) => output,
        Err(error) => ScriptOutput {
            error: Some(error.chars().take(4096).collect()),
            ..Default::default()
        },
    }
}
#[derive(Default)]
struct SyncJobs(Cell<bool>);
impl boa_engine::job::JobExecutor for SyncJobs {
    fn enqueue_job(self: Rc<Self>, _: boa_engine::job::Job, _: &mut Context) {
        self.0.set(true);
    }
    fn run_jobs(self: Rc<Self>, _: &mut Context) -> boa_engine::JsResult<()> {
        Ok(())
    }
}
fn evaluate(input: &ScriptInput) -> std::result::Result<ScriptOutput, String> {
    if input.source.len() > MAX_SOURCE {
        return Err("脚本超过 256 KiB 上限".into());
    }
    let jobs = Rc::new(SyncJobs::default());
    let mut context = Context::builder()
        .module_loader(Rc::new(IdleModuleLoader))
        .job_executor(jobs.clone())
        .can_block(false)
        .build()
        .map_err(|e| e.to_string())?;
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(10_000_000);
    context.runtime_limits_mut().set_recursion_limit(128);
    context.runtime_limits_mut().set_stack_size_limit(32_768);
    let filename = if input.stage == Stage::Pre {
        "pre-request.js"
    } else {
        "post-response.js"
    };
    let script = Script::parse(
        Source::from_bytes(&input.source).with_path(Path::new(filename)),
        None,
        &mut context,
    )
    .map_err(|e| e.to_string())?;
    if input.check_only {
        return Ok(ScriptOutput::default());
    }
    let payload = serde_json::to_string(input).map_err(|e| e.to_string())?;
    context
        .register_global_property(
            js_string!("__zephInput"),
            boa_engine::JsString::from(payload.as_str()),
            Attribute::all(),
        )
        .map_err(|e| e.to_string())?;
    context
        .register_global_builtin_callable(
            js_string!("__zephUuid"),
            0,
            NativeFunction::from_fn_ptr(|_, _, _| {
                Ok(boa_engine::JsString::from(uuid::Uuid::new_v4().to_string()).into())
            }),
        )
        .map_err(|e| e.to_string())?;
    context
        .eval(Source::from_bytes(include_str!("scripting_api.js")))
        .map_err(|e| e.to_string())?;
    let failure = match script.evaluate(&mut context) {
        Ok(value) if value.as_promise().is_some() => {
            Some("首版只支持同步脚本，请勿返回 Promise 或使用 async / await".into())
        }
        Ok(_) => None,
        Err(error) => Some(error.to_string()),
    };
    let failure = failure.or_else(|| {
        jobs.0
            .get()
            .then(|| "首版只支持同步脚本，不执行 Promise、async / await 或模块导入".into())
    });
    let output = context
        .eval(Source::from_bytes("__zephFinish()"))
        .map_err(|e| e.to_string())?;
    let text = output
        .as_string()
        .ok_or("脚本结果不是 JSON 字符串")?
        .to_std_string_escaped();
    let mut output: ScriptOutput = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    output.error = failure.or(output.error);
    Ok(output)
}

pub fn sample(stage: Stage) -> &'static str {
    match stage {
        Stage::Pre => {
            "// 变量可用于后续操作和本次请求的 {{variable}}\nconst traceId = zeph.uuid();\nzeph.variables.set('trace_id', traceId);\nzeph.request.headers.set('X-Trace-ID', traceId);\nconsole.log('已生成请求追踪 ID');\n"
        }
        Stage::Post => {
            "zeph.test('状态码正确', () => {\n  zeph.assert(zeph.response.code === 200, '预期状态码为 200');\n});\n\n// 按实际响应修改路径；全部操作成功后写回环境\n// const body = zeph.response.json();\n// zeph.environment.set('token', body.data.token);\nconsole.log('响应验证完成');\n"
        }
    }
}

pub(crate) fn apply_request(change: ScriptRequest, request: &mut Request) -> Result<()> {
    if change.url.len() > 64 * 1024 || change.body.len() > 8 * 1024 * 1024 {
        bail!("脚本返回的 URL 或 Body 超过上限");
    }
    reqwest::Method::from_bytes(change.method.as_bytes()).context("脚本返回的 Method 无效")?;
    for (field, headers) in [
        (&change.headers, true),
        (&change.query, false),
        (&change.path, false),
    ] {
        if field.len() > 256 {
            bail!("脚本返回的参数超过 256 项");
        }
        for p in field {
            if p.name.len() > 256 || p.value.len() > 64 * 1024 {
                bail!("脚本返回的参数超过上限");
            }
            if headers {
                reqwest::header::HeaderName::from_bytes(p.name.as_bytes())
                    .context("脚本返回的 Header 名称无效")?;
                reqwest::header::HeaderValue::from_str(&p.value)
                    .context("脚本返回的 Header 值无效")?;
            }
        }
    }
    use crate::model::BodyKind;
    if change.body_mode != request.body.kind
        && !matches!(
            change.body_mode,
            BodyKind::None | BodyKind::Json | BodyKind::Text | BodyKind::Xml
        )
    {
        bail!("脚本只能切换到无 Body、JSON、Text 或 XML");
    }
    request.method = change.method;
    request.url = change.url;
    request.headers = change.headers;
    request.query = change.query;
    request.path = change.path;
    request.body.kind = change.body_mode;
    request.body.text = change.body;
    Ok(())
}
