use futures::{SinkExt, StreamExt};
use std::collections::BTreeMap;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use zeph::{
    model::*,
    openapi,
    storage::ProjectStore,
    transport::{self, SessionCommand, SessionEvent},
};

#[test]
fn variables_and_ordered_parameters() {
    let vars = BTreeMap::from([
        ("base".into(), "http://localhost:8080".into()),
        ("a".into(), "{{b}}".into()),
        ("b".into(), "done".into()),
    ]);
    assert_eq!(
        interpolate(&"{{a}} ".repeat(30), &vars).unwrap(),
        "done ".repeat(30)
    );
    assert!(interpolate("{{missing}}", &vars).is_err());
    assert!(interpolate("{{a}}", &BTreeMap::from([("a".into(), "{{a}}".into())])).is_err());
    let mut r = Request::new(Protocol::Http, "query");
    r.url = "{{base}}/items/{id}?existing=1".into();
    r.path.push(Parameter::new("id", "a/b ?上海"));
    r.query = vec![Parameter::new("tag", "one"), Parameter::new("tag", "two")];
    assert_eq!(
        r.resolved_url(&vars).unwrap(),
        "http://localhost:8080/items/a%2Fb%20%3F%E4%B8%8A%E6%B5%B7?existing=1&tag=one&tag=two"
    );
    r.path.clear();
    assert!(r.resolved_url(&vars).is_err());
}
#[test]
fn local_store_is_atomic_and_rejects_stale_saves() {
    let temp = tempfile::tempdir().unwrap();
    let store = ProjectStore::new(temp.path());
    let mut p = store.load().unwrap();
    p.name = "newer".into();
    store.save_versioned(&p, 2).unwrap();
    p.name = "older".into();
    store.clone().save_versioned(&p, 1).unwrap();
    assert_eq!(store.load().unwrap().name, "newer");
    let file = temp.path().join("zeph.project.json");
    std::fs::write(&file, "broken").unwrap();
    assert!(store.load().is_err());
    assert_eq!(std::fs::read_to_string(file).unwrap(), "broken");
}
#[test]
fn openapi_versions_references_and_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    for version in ["3.0.3", "3.1.0"] {
        let path = temp.path().join(format!("{version}.json"));
        let doc = serde_json::json!({"openapi":version,"info":{"title":"shop","version":"1"},"servers":[{"url":"https://example.com/{version}","variables":{"version":{"default":"v1"}}}],"paths":{"/orders/{id}":{"parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string","default":"ord1"}}],"post":{"summary":"Create","parameters":[{"name":"tag","in":"query","schema":{"type":"array","items":{"type":"string"}}}],"requestBody":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Order"}}}}}}},"components":{"schemas":{"Order":{"type":"object","properties":{"quantity":{"type":"integer","default":3},"serverId":{"type":"string","readOnly":true}}}}}});
        std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        let imported = openapi::import(&path).unwrap();
        assert_eq!(imported.requests.len(), 1);
        let r = &imported.requests[0];
        assert_eq!(r.url, "https://example.com/v1/orders/{id}");
        assert_eq!(r.path[0].value, "ord1");
        assert!(r.schema.as_ref().unwrap().get("properties").is_some());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&r.body.text).unwrap(),
            serde_json::json!({"quantity":3})
        );
        assert!(imported.diagnostics.iter().any(|d| d.contains("序列化")));
        assert_eq!(imported.document, doc);
        zeph::storage::save_contract(&temp.path().join("contracts"), &imported).unwrap();
    }
    let path = temp.path().join("v2.yaml");
    std::fs::write(&path,"swagger: '2.0'\ninfo: {title: Shop, version: '1'}\nhost: localhost:9000\nbasePath: /api\nconsumes: [multipart/form-data]\npaths:\n  /upload:\n    post:\n      parameters:\n        - {name: caption, in: formData, type: string, default: image}\n        - {name: image, in: formData, type: file}\n").unwrap();
    let imported = openapi::import(&path).unwrap();
    assert_eq!(imported.requests[0].url, "http://localhost:9000/api/upload");
    assert_eq!(imported.requests[0].body.kind, BodyKind::FormData);
    assert!(imported.requests[0].body.fields.iter().any(|f| f.file));
    std::fs::write(
        temp.path().join("model.yaml"),
        "type: object\nproperties:\n  ok: {type: boolean, default: true}\n",
    )
    .unwrap();
    let path = temp.path().join("external.yaml");
    std::fs::write(&path,"openapi: 3.1.0\npaths:\n  /x:\n    post:\n      requestBody:\n        content:\n          application/json:\n            schema: {$ref: 'model.yaml'}\n").unwrap();
    assert!(
        openapi::import(&path).unwrap().requests[0]
            .body
            .text
            .contains("true")
    );
}
#[test]
fn grpc_methods_and_protojson() {
    let pool = transport::grpc::example_pool();
    let methods = transport::grpc::methods(&pool);
    assert_eq!(methods.len(), 4);
    assert_eq!(
        methods
            .iter()
            .map(|m| (m.client_streaming, m.server_streaming))
            .collect::<Vec<_>>(),
        vec![(false, false), (false, true), (true, false), (true, true)]
    );
    let descriptor = pool.get_message_by_name("shop.v1.Order").unwrap();
    let message = transport::grpc::parse_message(
        descriptor.clone(),
        r#"{"orderId":"o1","totalAmount":"9007199254740993"}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&message).unwrap()["totalAmount"],
        "9007199254740993"
    );
    assert!(transport::grpc::parse_message(descriptor, r#"{"unknown":true}"#).is_err());
}

async fn http_fixture(head: bool) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = vec![];
        loop {
            let mut chunk = [0; 4096];
            let n = stream.read(&mut chunk).await.unwrap();
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..n]);
            if let Some(end) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let body = if head { "" } else { r#"{"ok":true}"# };
        stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",if head{11}else{body.len()}).as_bytes()).await.unwrap();
        bytes
    });
    (format!("http://{addr}"), task)
}
#[tokio::test]
async fn http_methods_body_encodings_and_connect() {
    for method in HTTP_METHODS.into_iter().filter(|m| *m != "CONNECT") {
        let (url, task) = http_fixture(method == "HEAD").await;
        let mut r = Request::new(Protocol::Http, method);
        r.url = url;
        r.method = method.into();
        r.query = vec![Parameter::new("tag", "a"), Parameter::new("tag", "b")];
        let record =
            transport::http::execute(r, BTreeMap::new(), "test".into(), CancellationToken::new())
                .await
                .unwrap();
        assert_eq!(record.status, "200 OK");
        assert_eq!(record.timeline.len(), 3);
        assert!(
            record
                .timeline
                .windows(2)
                .all(|marks| marks[0].elapsed_ms <= marks[1].elapsed_ms)
        );
        if method == "HEAD" {
            assert_eq!(record.body, "");
        }
        let wire = String::from_utf8(task.await.unwrap()).unwrap();
        assert!(wire.starts_with(&format!("{method} /?tag=a&tag=b HTTP/1.1")));
    }
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), b"file contents").unwrap();
    for kind in [
        BodyKind::Json,
        BodyKind::UrlEncoded,
        BodyKind::FormData,
        BodyKind::Text,
        BodyKind::Xml,
        BodyKind::Binary,
    ] {
        let (url, task) = http_fixture(false).await;
        let mut r = Request::new(Protocol::Http, "body");
        r.url = url;
        r.method = "POST".into();
        r.body.kind = kind;
        r.body.text = if kind == BodyKind::Json {
            r#"{"name":"{{name}}"}"#
        } else {
            "<name>{{name}}</name>"
        }
        .into();
        r.body.file_path = temp.path().to_string_lossy().into_owned();
        r.body.fields = vec![Parameter::new("tag", "a b"), Parameter::new("tag", "c")];
        if kind == BodyKind::FormData {
            let mut file = Parameter::new("upload", &r.body.file_path);
            file.file = true;
            r.body.fields.push(file);
        }
        transport::http::execute(
            r,
            BTreeMap::from([("name".into(), "Zeph".into())]),
            "test".into(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let wire = String::from_utf8(task.await.unwrap()).unwrap();
        match kind {
            BodyKind::Json => assert!(wire.contains(r#"{"name":"Zeph"}"#)),
            BodyKind::UrlEncoded => assert!(wire.ends_with("tag=a+b&tag=c")),
            BodyKind::FormData => {
                assert!(wire.contains("boundary="));
                assert!(wire.contains("file contents"));
                assert!(wire.contains("name=\"tag\""));
            }
            BodyKind::Binary => assert!(wire.ends_with("file contents")),
            _ => assert!(wire.ends_with("<name>Zeph</name>")),
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proxy = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        loop {
            bytes.push(s.read_u8().await.unwrap());
            if bytes.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        s.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .unwrap();
        String::from_utf8(bytes).unwrap()
    });
    let mut r = Request::new(Protocol::Http, "connect");
    r.method = "CONNECT".into();
    r.url = "example.com:443".into();
    r.proxy = format!("http://{address}");
    let record =
        transport::http::execute(r, BTreeMap::new(), "test".into(), CancellationToken::new())
            .await
            .unwrap();
    assert!(!record.error);
    assert!(record.body.contains("已关闭"));
    assert!(
        proxy
            .await
            .unwrap()
            .starts_with("CONNECT example.com:443 HTTP/1.1")
    );
}
#[tokio::test]
async fn http_cancellation_and_invalid_json() {
    let (url, task) = http_fixture(false).await;
    let mut r = Request::new(Protocol::Http, "bad body");
    r.url = url;
    r.body.kind = BodyKind::Json;
    r.body.text = "invalid".into();
    assert!(
        transport::http::execute(r, BTreeMap::new(), "test".into(), CancellationToken::new())
            .await
            .is_err()
    );
    task.abort();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut r = Request::new(Protocol::Http, "cancel");
    r.url = format!("http://{}", listener.local_addr().unwrap());
    let token = CancellationToken::new();
    token.cancel();
    assert!(
        transport::http::execute(r, BTreeMap::new(), "test".into(), token)
            .await
            .unwrap_err()
            .to_string()
            .contains("取消")
    );
}
#[tokio::test]
async fn websocket_real_echo_and_close() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
        let message = ws.next().await.unwrap().unwrap();
        ws.send(message).await.unwrap();
        while let Some(Ok(msg)) = ws.next().await {
            if msg.is_close() {
                let _ = ws.flush().await;
                break;
            }
        }
    });
    let mut r = Request::new(Protocol::WebSocket, "echo");
    r.url = format!("ws://{address}");
    let (tx, rx) = mpsc::channel(8);
    let (events, mut ev) = mpsc::channel(16);
    let worker = tokio::spawn(transport::websocket::run(
        r,
        BTreeMap::new(),
        rx,
        events,
        CancellationToken::new(),
    ));
    loop {
        if matches!(ev.recv().await, Some(SessionEvent::Opened)) {
            break;
        }
    }
    tx.send(SessionCommand::Send("hello Zeph".into()))
        .await
        .unwrap();
    loop {
        if let Some(SessionEvent::Received(text)) = ev.recv().await {
            assert_eq!(text, "hello Zeph");
            break;
        }
    }
    tx.send(SessionCommand::Close).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn session_cancel_before_connect_is_visible() {
    let mut request = Request::new(Protocol::WebSocket, "cancel");
    request.url = "ws://127.0.0.1:1".into();
    let (_tx, rx) = mpsc::channel(1);
    let (events, mut incoming) = mpsc::channel(1);
    let cancel = CancellationToken::new();
    cancel.cancel();
    transport::websocket::run(request.clone(), BTreeMap::new(), rx, events, cancel)
        .await
        .unwrap();
    assert!(
        matches!(incoming.recv().await,Some(SessionEvent::Finished{code,..}) if code=="已取消")
    );
    request.protocol = Protocol::Grpc;
    request.url = "http://127.0.0.1:1".into();
    request.grpc_method = "shop.v1.OrderService/GetOrder".into();
    let (_tx, rx) = mpsc::channel(1);
    let (events, mut incoming) = mpsc::channel(1);
    let cancel = CancellationToken::new();
    cancel.cancel();
    transport::grpc::run(
        request,
        BTreeMap::new(),
        transport::grpc::example_pool(),
        rx,
        events,
        cancel,
    )
    .await
    .unwrap();
    assert!(
        matches!(incoming.recv().await,Some(SessionEvent::Finished{code,..}) if code=="1 CANCELLED")
    );
}
