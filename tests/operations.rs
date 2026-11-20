use serde_json::json;
use std::{collections::BTreeMap, time::Duration};
use tokio_util::sync::CancellationToken;
use zeph::{
    model::{Auth, Project, Protocol, Request},
    operations::{self, Kind, Operation, OperationContext, Outcome, Scope},
    transport::{RunRecord, SessionMessage},
};
fn op(kind: Kind, key: &str, value: &str) -> Operation {
    Operation {
        key: key.into(),
        value: value.into(),
        ..Operation::new(kind)
    }
}
fn response(request: &Request) -> RunRecord {
    let mut r = RunRecord::error(request, "Local".into(), String::new());
    r.error = false;
    r.status = "200 OK".into();
    r.body = r#"{"data":{"token":"secret-token","items":[{"id":42}]}}"#.into();
    r.headers = vec![("x-request-id".into(), "req-1".into())];
    r
}
#[tokio::test]
async fn pre_order_updates_only_snapshot_and_post_extracts_for_later_assertions() {
    let mut request = Request::new(Protocol::Http, "ordered");
    request.pre_operations = vec![
        op(Kind::SetVariable, "prefix", "Bearer"),
        op(Kind::SetHeader, "Authorization", "{{prefix}} {{token}}"),
    ];
    request.post_operations = vec![
        Operation {
            source: "$.data.items[0].id".into(),
            ..op(Kind::ExtractJson, "id", "")
        },
        Operation {
            source: "$.data.items[0].id".into(),
            ..op(Kind::AssertJson, "", "{{id}}")
        },
        op(Kind::AssertStatus, "", "200"),
    ];
    let original = request.clone();
    let token = CancellationToken::new();
    let mut context =
        OperationContext::new(BTreeMap::from([("token".into(), "secret-token".into())]));
    assert!(context.pre(&mut request, &token).await);
    assert_eq!(request.headers[0].value, "Bearer secret-token");
    assert!(original.headers.is_empty());
    let mut record = response(&request);
    assert!(
        context
            .finish(&request, &mut record, true, &token)
            .await
            .is_none()
    );
    assert_eq!(record.operations.len(), 5);
    assert!(
        record
            .operations
            .iter()
            .all(|r| r.outcome == Outcome::Passed)
    );
    assert!(
        !serde_json::to_string(&record.operations)
            .unwrap()
            .contains("secret-token")
    );
}
#[tokio::test]
async fn failed_prevents_sending_and_skip_disabled_or_continue_are_explicit() {
    let mut request = Request::new(Protocol::Http, "fail");
    request.url = "http://127.0.0.1:1/".into();
    request.pre_operations = vec![
        op(Kind::SetVariable, "value", "{{missing}}"),
        op(Kind::SetVariable, "later", "never"),
    ];
    request.post_operations = vec![op(Kind::AssertStatus, "", "200")];
    let (record, patch) = operations::execute_http(
        request.clone(),
        BTreeMap::new(),
        "Local".into(),
        CancellationToken::new(),
    )
    .await;
    assert!(record.error);
    assert!(patch.is_none());
    assert!(record.body.contains("未发送请求"));
    assert_eq!(
        record
            .operations
            .iter()
            .map(|r| r.outcome)
            .collect::<Vec<_>>(),
        vec![Outcome::Failed, Outcome::Skipped, Outcome::Skipped]
    );
    request.pre_operations[0].continue_on_error = true;
    let mut context = OperationContext::new(BTreeMap::new());
    assert!(context.pre(&mut request, &CancellationToken::new()).await);
    assert_eq!(context.variables["later"], "never");
    request.pre_operations[0].enabled = false;
    let mut context = OperationContext::new(BTreeMap::new());
    assert!(context.pre(&mut request, &CancellationToken::new()).await);
    assert_eq!(context.reports[0].outcome, Outcome::Skipped);
}
#[tokio::test]
async fn wait_is_cancellable_and_has_a_stage_deadline() {
    let mut request = Request::new(Protocol::Http, "wait");
    request.pre_operations = vec![op(Kind::Wait, "", "30000")];
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        stop.cancel();
    });
    let mut context = OperationContext::new(BTreeMap::new());
    assert!(
        !tokio::time::timeout(Duration::from_secs(1), context.pre(&mut request, &cancel))
            .await
            .unwrap()
    );
    assert_eq!(context.reports[0].outcome, Outcome::Failed);
    request.timeout_secs = 1;
    let mut context = OperationContext::new(BTreeMap::new());
    assert!(!context.pre(&mut request, &CancellationToken::new()).await);
    assert!(context.reports[0].detail.contains("超时"));
}
#[tokio::test]
async fn environment_writes_are_transactional_and_conflicts_do_not_overwrite() {
    let mut project = Project::examples();
    let vars = project.variables();
    let mut request = project.requests[0].clone();
    request.pre_operations = vec![Operation {
        scope: Scope::Environment,
        ..op(Kind::SetVariable, "token", "new-secret")
    }];
    let cancel = CancellationToken::new();
    let mut ctx = OperationContext::new(vars.clone());
    assert!(ctx.pre(&mut request, &cancel).await);
    let mut record = response(&request);
    record.environment = project.environments[0].name.clone();
    let patch = ctx
        .finish(&request, &mut record, true, &cancel)
        .await
        .unwrap();
    project.environments[0]
        .variables
        .insert("token".into(), "concurrent-secret".into());
    assert!(patch.apply(&mut project).is_err());
    assert_eq!(
        project.environments[0].variables["token"],
        "concurrent-secret"
    );
    project.environments[0].variables = vars.clone();
    patch.apply(&mut project).unwrap();
    assert_eq!(project.environments[0].variables["token"], "new-secret");
    request.post_operations = vec![op(Kind::AssertStatus, "", "500")];
    let mut ctx = OperationContext::new(vars);
    assert!(ctx.pre(&mut request, &cancel).await);
    let mut record = response(&request);
    assert!(
        ctx.finish(&request, &mut record, true, &cancel)
            .await
            .is_none()
    );
    assert_eq!(record.status, "200 OK");
    assert_eq!(record.operations.last().unwrap().outcome, Outcome::Failed);
}
#[tokio::test]
async fn stream_post_uses_last_received_json_and_metadata_and_skips_on_cancel() {
    let mut request = Request::new(Protocol::Grpc, "stream");
    request.post_operations = vec![
        Operation {
            source: "$.id".into(),
            ..op(Kind::ExtractJson, "id", "")
        },
        Operation {
            source: "$.id".into(),
            ..op(Kind::AssertJson, "", "9")
        },
        Operation {
            source: "X-Request-ID".into(),
            ..op(Kind::ExtractHeader, "trace", "")
        },
        op(Kind::AssertStatus, "", "0"),
    ];
    let mut record = response(&request);
    record.status = "0 OK".into();
    record.messages = vec![
        SessionMessage {
            sent: false,
            body: r#"{"id":9}"#.into(),
            elapsed_ms: 0,
        },
        SessionMessage {
            sent: true,
            body: r#"{"id":10}"#.into(),
            elapsed_ms: 1,
        },
    ];
    let cancel = CancellationToken::new();
    OperationContext::new(BTreeMap::new())
        .finish(&request, &mut record, true, &cancel)
        .await;
    assert!(
        record
            .operations
            .iter()
            .all(|r| r.outcome == Outcome::Passed)
    );
    cancel.cancel();
    OperationContext::new(BTreeMap::new())
        .finish(&request, &mut record, true, &cancel)
        .await;
    assert!(
        record
            .operations
            .iter()
            .all(|r| r.outcome == Outcome::Skipped)
    );
}
#[tokio::test]
async fn json_selectors_and_migration_handle_old_records_and_truncated_responses() {
    let json = json!({"items":[{"a/b":{"~":3}}],"empty":null});
    assert_eq!(
        operations::select_json(&json, "/items/0/a~1b/~0").unwrap(),
        &json!(3)
    );
    assert!(operations::select_json(&json, "$.items[99]").is_err());
    assert!(operations::select_json(&json, "$.items[*]").is_err());
    let mut value = serde_json::to_value(Request::new(Protocol::Http, "old")).unwrap();
    value.as_object_mut().unwrap().remove("pre_operations");
    value.as_object_mut().unwrap().remove("post_operations");
    let mut request: Request = serde_json::from_value(value).unwrap();
    assert!(request.pre_operations.is_empty());
    request.post_operations = vec![Operation {
        source: "$.data".into(),
        ..op(Kind::ExtractJson, "secret", "")
    }];
    let mut record = response(&request);
    record.truncated = true;
    OperationContext::new(BTreeMap::new())
        .finish(&request, &mut record, true, &CancellationToken::new())
        .await;
    assert_eq!(record.operations[0].outcome, Outcome::Failed);
    let mut old = serde_json::to_value(record).unwrap();
    old.as_object_mut().unwrap().remove("operations");
    assert!(
        serde_json::from_value::<RunRecord>(old)
            .unwrap()
            .operations
            .is_empty()
    );
}
#[tokio::test]
async fn http_pre_variables_feed_url_headers_auth_and_post_can_extract_response() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0; 4096];
        let n = socket.read(&mut buf).await.unwrap();
        let request = String::from_utf8_lossy(&buf[..n]).to_lowercase();
        assert!(request.starts_with("get /from-pre "));
        assert!(request.contains("authorization: bearer generated-secret"));
        assert!(request.contains("x-pre: prepared"));
        let body = r#"{"data":{"token":"returned-secret"}}"#;
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    });
    let mut request = Request::new(Protocol::Http, "wire");
    request.url = format!("http://{address}/{{{{path}}}}");
    request.auth = Auth::Bearer("{{token}}".into());
    request.pre_operations = vec![
        op(Kind::SetVariable, "path", "from-pre"),
        op(Kind::SetVariable, "token", "generated-secret"),
        op(Kind::SetHeader, "x-pre", "prepared"),
    ];
    request.post_operations = vec![
        Operation {
            scope: Scope::Environment,
            source: "$.data.token".into(),
            ..op(Kind::ExtractJson, "token", "")
        },
        op(Kind::AssertStatus, "", "200"),
    ];
    let original = request.clone();
    let (record, patch) = operations::execute_http(
        request,
        BTreeMap::new(),
        "Local".into(),
        CancellationToken::new(),
    )
    .await;
    server.await.unwrap();
    assert!(!record.error);
    assert!(
        record
            .operations
            .iter()
            .all(|r| r.outcome == Outcome::Passed)
    );
    assert_eq!(patch.unwrap().writes[0].value, "returned-secret");
    assert!(original.headers.is_empty());
    assert_eq!(original.url, format!("http://{address}/{{{{path}}}}"));
    assert!(
        !serde_json::to_string(&record.operations)
            .unwrap()
            .contains("secret")
    );
}
