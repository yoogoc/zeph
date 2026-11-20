#![cfg(feature = "desktop")]
use std::{collections::BTreeMap, path::Path};
use tokio_util::sync::CancellationToken;
use zeph::{
    model::{BodyKind, Parameter, Protocol, Request},
    operations::{Kind, Operation, OperationContext, Outcome, Stage},
    scripting::{self, ScriptInput, ScriptOutput, ScriptRequest, ScriptResponse},
    transport::{RunRecord, SessionMessage},
};

fn worker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_zeph"))
}
fn input(source: &str, stage: Stage) -> ScriptInput {
    let request = Request::new(Protocol::Http, "script");
    let mut record = RunRecord::error(&request, "Local".into(), String::new());
    record.error = false;
    record.status = "200 OK".into();
    record.body = r#"{"ok":true,"data":{"token":"returned-token"}}"#.into();
    record.headers = vec![("x-request-id".into(), "response-trace".into())];
    ScriptInput {
        source: source.into(),
        stage,
        variables: BTreeMap::from([("token".into(), "original".into())]),
        environment: BTreeMap::from([("token".into(), "original".into())]),
        request: ScriptRequest::from(&request),
        response: (stage == Stage::Post).then(|| ScriptResponse::from(&record)),
        check_only: false,
    }
}
async fn run(source: &str, stage: Stage) -> ScriptOutput {
    scripting::execute(
        input(source, stage),
        Some(worker()),
        tokio::time::Instant::now() + scripting::TIMEOUT,
        &CancellationToken::new(),
    )
    .await
    .unwrap()
}
fn script(source: &str) -> Operation {
    Operation {
        script: source.into(),
        ..Operation::new(Kind::JavaScript)
    }
}
fn context() -> OperationContext {
    OperationContext::new(BTreeMap::from([("token".into(), "original".into())]))
        .with_script_worker(worker())
}

#[tokio::test]
async fn boa_pre_api_supports_request_edits_ordered_variables_uuid_and_plain_logs() {
    let output = run(
        r#"
        zeph.variables.set('local', 'run-value');
        zeph.environment.set('token', 'pending');
        zeph.variables.set('token', 'local-override');
        zeph.assert(zeph.variables.get('token')==='local-override');
        zeph.assert(zeph.environment.get('token')==='pending');
        const a=zeph.uuid(), b=zeph.uuid();
        zeph.assert(a!==b && a.length===36);
        zeph.request.url='http://localhost/{{local}}';
        zeph.request.method='post';
        zeph.request.headers.set('X-Token','{{token}}');
        zeph.request.query.add('tag','one');
        zeph.request.query.add('tag','two');
        zeph.request.path.set('id','123');
        zeph.request.body.setJson({hello:'{{local}}'});
        console.log('prepared', {ok:true});
    "#,
        Stage::Pre,
    )
    .await;
    assert!(output.succeeded(), "{output:?}");
    assert_eq!(output.variables["token"], "local-override");
    assert_eq!(output.environment["token"], "pending");
    let request = output.request.unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.body_mode, BodyKind::Json);
    assert_eq!(request.query.len(), 2);
    assert!(request.body.contains("{{local}}"));
    assert_eq!(output.logs.len(), 1);
    assert!(!output.logs[0].contains("local-override"));
}

#[tokio::test]
async fn script_commits_only_after_all_checks_pass_and_rolls_back_on_failure() {
    let mut request = Request::new(Protocol::Http, "transaction");
    request.pre_operations = vec![script(
        "zeph.variables.set('token','bad');zeph.environment.set('token','bad');zeph.request.headers.set('x-bad','bad');console.log('before failure');zeph.test('fails',()=>zeph.assert(false,'expected failure'));zeph.test('still runs',()=>zeph.assert(true));",
    )];
    request.pre_operations[0].continue_on_error = true;
    request.pre_operations.push(Operation {
        key: "x-after".into(),
        value: "{{token}}".into(),
        ..Operation::new(Kind::SetHeader)
    });
    let mut ctx = context();
    assert!(ctx.pre(&mut request, &CancellationToken::new()).await);
    assert_eq!(ctx.variables["token"], "original");
    assert_eq!(request.headers.len(), 1);
    assert_eq!(request.headers[0].value, "original");
    assert_eq!(ctx.reports[0].outcome, Outcome::Failed);
    assert_eq!(ctx.reports[0].tests.len(), 2);
    assert!(ctx.reports[0].tests[1].passed);
    assert_eq!(ctx.reports[0].logs.len(), 1);
    let mut record = RunRecord::error(&request, "Local".into(), String::new());
    record.error = false;
    assert!(
        ctx.finish(&request, &mut record, true, &CancellationToken::new())
            .await
            .is_none()
    );
}

#[tokio::test]
async fn post_script_reads_response_extracts_environment_and_preserves_transport_status() {
    let mut request = Request::new(Protocol::Http, "response");
    request.post_operations = vec![script(
        "zeph.test('status',()=>zeph.assert(zeph.response.code===200));zeph.test('json',()=>zeph.assert(zeph.response.json().ok));zeph.environment.set('token',zeph.response.json().data.token);zeph.variables.set('trace',zeph.response.headers.get('X-REQUEST-ID'));console.info('validated');",
    )];
    let mut record = RunRecord::error(&request, "Local".into(), String::new());
    record.error = false;
    record.status = "200 OK".into();
    record.body = input("", Stage::Post).response.unwrap().body;
    record.headers = vec![("x-request-id".into(), "response-trace".into())];
    let patch = context()
        .finish(&request, &mut record, true, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(patch.writes.len(), 1);
    assert_eq!(patch.writes[0].value, "returned-token");
    assert_eq!(record.status, "200 OK");
    assert_eq!(record.operations[0].tests.len(), 2);
    let serialized = serde_json::to_string(&record.operations).unwrap();
    assert!(!serialized.contains("returned-token"));
    let roundtrip: RunRecord =
        serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
    assert_eq!(roundtrip.operations[0].logs, record.operations[0].logs);
}

#[tokio::test]
async fn thrown_errors_keep_logs_and_script_locations_without_applying_changes() {
    let output = run(
        "console.warn('before error');\nthrow new Error('broken script');",
        Stage::Pre,
    )
    .await;
    assert!(!output.succeeded());
    assert_eq!(output.logs.len(), 1);
    let error = output.error.unwrap();
    assert!(error.contains("broken script"), "{error}");
    assert!(error.contains("pre-request.js"), "{error}");
}

#[tokio::test]
async fn syntax_check_does_not_execute_and_invalid_code_reports_position() {
    let mut check = input("throw new Error('must not execute')", Stage::Pre);
    check.check_only = true;
    let output = scripting::execute(
        check,
        Some(worker()),
        tokio::time::Instant::now() + scripting::TIMEOUT,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(output.succeeded());
    let mut check = input("const broken = ;", Stage::Pre);
    check.check_only = true;
    let output = scripting::execute(
        check,
        Some(worker()),
        tokio::time::Instant::now() + scripting::TIMEOUT,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(output.error.unwrap().contains("SyntaxError"));
}

#[tokio::test]
async fn deadlines_and_cancellation_terminate_unbounded_scripts_and_worker_remains_usable() {
    // The nested loops reset each loop counter and exercise the parent's wall-clock guard.
    let endless = "for(;;){for(let i=0;i<1000000;i++)Math.sqrt(i);}";
    let start = std::time::Instant::now();
    let error = scripting::execute(
        input(endless, Stage::Pre),
        Some(worker()),
        tokio::time::Instant::now() + std::time::Duration::from_millis(250),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("超时"), "{error}");
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        token.cancel();
    });
    let error = scripting::execute(
        input(endless, Stage::Pre),
        Some(worker()),
        tokio::time::Instant::now() + scripting::TIMEOUT,
        &cancel,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("取消"));
    assert!(run("zeph.assert(2+3===5)", Stage::Pre).await.succeeded());
}

#[tokio::test]
async fn script_contexts_are_isolated_and_host_system_apis_are_absent() {
    assert!(run("globalThis.leftover=42;", Stage::Pre).await.succeeded());
    let output=run("zeph.assert(typeof leftover==='undefined');zeph.assert(typeof fetch==='undefined');zeph.assert(typeof require==='undefined');zeph.assert(typeof process==='undefined');zeph.assert(typeof setTimeout==='undefined');",Stage::Pre).await;
    assert!(output.succeeded(), "{output:?}");
    assert!(
        !run(
            "Promise.resolve().then(()=>zeph.variables.set('late','bad')); 0;",
            Stage::Pre
        )
        .await
        .succeeded()
    );
    assert!(!run("(async()=>42)()", Stage::Pre).await.succeeded());
    assert!(
        !run("zeph.request.headers.set('x','bad');", Stage::Post)
            .await
            .succeeded()
    );
}

#[tokio::test]
async fn stream_response_uses_last_received_message_trailers_and_rejects_truncated_json() {
    let mut data = input(
        "zeph.assert(zeph.response.code===0);zeph.assert(zeph.response.json().id===9);zeph.assert(zeph.response.trailers.get('X-TRACE')==='t');zeph.assert(zeph.response.messages().length===2);",
        Stage::Post,
    );
    let r = data.response.as_mut().unwrap();
    r.status = "0 OK".into();
    r.code = Some(0);
    r.body = "{\"id\":9}".into();
    r.trailers = vec![("x-trace".into(), "t".into())];
    r.messages = vec![
        SessionMessage {
            sent: false,
            body: r.body.clone(),
            elapsed_ms: 0,
        },
        SessionMessage {
            sent: true,
            body: "{\"id\":10}".into(),
            elapsed_ms: 1,
        },
    ];
    let output = scripting::execute(
        data,
        Some(worker()),
        tokio::time::Instant::now() + scripting::TIMEOUT,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(output.succeeded(), "{output:?}");
    let mut data = input("zeph.response.json()", Stage::Post);
    data.response.as_mut().unwrap().truncated = true;
    assert!(
        !scripting::execute(
            data,
            Some(worker()),
            tokio::time::Instant::now() + scripting::TIMEOUT,
            &CancellationToken::new()
        )
        .await
        .unwrap()
        .succeeded()
    );
}

#[tokio::test]
async fn limits_and_invalid_headers_fail_atomically_and_console_is_bounded() {
    let output = run(
        "for(let i=0;i<200;i++)console.log('x'.repeat(4096));",
        Stage::Pre,
    )
    .await;
    assert!(output.succeeded());
    assert_eq!(output.logs.len(), 50);
    assert!(output.logs.iter().all(|s| s.len() <= 2048));
    let unicode = run("console.log('🦀'.repeat(3000));", Stage::Pre).await;
    assert!(unicode.succeeded());
    assert!(unicode.logs[0].chars().count() <= 2048);
    assert!(!unicode.logs[0].is_empty());
    let mut request = Request::new(Protocol::Http, "invalid");
    request.headers = vec![Parameter::new("x-original", "ok")];
    request.pre_operations = vec![script(
        "zeph.variables.set('token','bad');zeph.request.headers.set('bad header','value');",
    )];
    let mut ctx = context();
    assert!(!ctx.pre(&mut request, &CancellationToken::new()).await);
    assert_eq!(ctx.variables["token"], "original");
    assert_eq!(request.headers.len(), 1);
    let mut data = input("", Stage::Pre);
    data.source = " ".repeat(scripting::MAX_SOURCE + 1);
    assert!(
        scripting::execute(
            data,
            Some(worker()),
            tokio::time::Instant::now() + scripting::TIMEOUT,
            &CancellationToken::new()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn scripts_prepare_real_http_payload_and_validate_then_extract_the_response() {
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(socket);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        assert!(
            line.starts_with("POST /scripted?tag=one&tag=two "),
            "{line}"
        );
        let mut length = 0;
        let mut headers = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse::<usize>().unwrap();
            }
            headers.push_str(&line.to_lowercase());
        }
        assert!(headers.contains("authorization: bearer scripted-token"));
        assert!(headers.contains("x-script: prepared"));
        let mut body = vec![0; length];
        reader.read_exact(&mut body).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"value":"from-script"})
        );
        let response = r#"{"ok":true,"token":"next-token"}"#;
        reader
            .get_mut()
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                    response.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    });
    let mut request = Request::new(Protocol::Http, "wire");
    request.url = format!("http://{address}/{{{{route}}}}");
    request.auth = zeph::model::Auth::Bearer("{{token}}".into());
    request.auth_inherit = Some(false);
    request.pre_operations = vec![script(
        "zeph.variables.set('route','scripted');zeph.variables.set('token','scripted-token');zeph.variables.set('payload','from-script');zeph.request.method='POST';zeph.request.headers.set('x-script','prepared');zeph.request.query.add('tag','one');zeph.request.query.add('tag','two');zeph.request.body.setJson({value:'{{payload}}'});",
    )];
    request.post_operations = vec![script(
        "zeph.test('response',()=>zeph.assert(zeph.response.json().ok));zeph.environment.set('token',zeph.response.json().token);",
    )];
    let original = request.clone();
    let mut project = zeph::model::Project::empty("script cascade");
    let parent = project.add_folder("Root", None).unwrap();
    let child = project.add_folder("Child", Some(parent)).unwrap();
    project.folders[0].operations.pre = vec![script("zeph.variables.set('from_parent','root');")];
    project.folders[1].operations.pre = vec![script(
        "zeph.assert(zeph.variables.get('from_parent')==='root');",
    )];
    project.folders[0].operations.post = vec![script(
        "zeph.test('parent sees extracted token',()=>zeph.assert(zeph.environment.get('token')==='next-token'));",
    )];
    request.folder_id = Some(child);
    let mut request = project.prepare_request(&request).unwrap();
    let cancel = CancellationToken::new();
    let mut ctx = context();
    assert!(ctx.pre(&mut request, &cancel).await);
    let mut record = zeph::transport::http::execute(
        request.clone(),
        ctx.variables.clone(),
        "Local".into(),
        cancel.clone(),
    )
    .await
    .unwrap();
    let patch = ctx
        .finish(&request, &mut record, true, &cancel)
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(patch.writes[0].value, "next-token");
    assert!(
        record
            .operations
            .iter()
            .all(|r| r.outcome == Outcome::Passed)
    );
    assert_eq!(original.method, "GET");
    assert_eq!(original.body.kind, BodyKind::None);
}
