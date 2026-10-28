use std::{collections::BTreeMap, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use zeph::{
    auth::AuthOrigin,
    model::{Auth, Project, Protocol, Request},
    storage::ProjectStore,
    transport,
};

fn inherited(p: &Project, r: &Request) -> Auth {
    p.effective_auth(r).unwrap().auth.clone()
}
#[test]
fn nearest_scope_override_and_explicit_no_auth_stop_inheritance() {
    let mut p = Project::empty("Auth");
    p.auth = Auth::Bearer("workspace".into());
    let root = p.add_folder("Root", None).unwrap();
    let child = p.add_folder("Child", Some(root)).unwrap();
    let mut r = Request::new(Protocol::Http, "Test");
    r.folder_id = Some(child);
    assert_eq!(inherited(&p, &r), p.auth);
    assert_eq!(p.effective_auth(&r).unwrap().origin, AuthOrigin::Workspace);
    p.folders.iter_mut().find(|f| f.id == root).unwrap().auth = Some(Auth::Bearer("folder".into()));
    assert_eq!(inherited(&p, &r), Auth::Bearer("folder".into()));
    assert_eq!(
        p.effective_auth(&r).unwrap().origin,
        AuthOrigin::Folder(root)
    );
    p.folders.iter_mut().find(|f| f.id == child).unwrap().auth = Some(Auth::None);
    assert_eq!(inherited(&p, &r), Auth::None);
    r.auth = Auth::Basic {
        username: "user".into(),
        password: "password".into(),
    };
    r.auth_inherit = Some(false);
    assert_eq!(inherited(&p, &r), r.auth);
    r.auth = Auth::None;
    assert_eq!(inherited(&p, &r), Auth::None);
}
#[test]
fn legacy_auth_is_preserved_and_settings_roundtrip_without_materializing_credentials() {
    let mut p = Project::empty("Legacy");
    p.auth = Auth::Bearer("workspace-secret".into());
    let folder = p.add_folder("Folder", None).unwrap();
    let mut local = Request::new(Protocol::Http, "local");
    local.auth = Auth::Bearer("local-secret".into());
    local.auth_inherit = None;
    let mut inherited_request = Request::new(Protocol::Http, "inherited");
    inherited_request.auth_inherit = None;
    inherited_request.folder_id = Some(folder);
    let mut disabled = Request::new(Protocol::Http, "disabled");
    disabled.auth_inherit = Some(false);
    p.requests = vec![local, inherited_request, disabled];
    p.normalize().unwrap();
    let t = tempfile::tempdir().unwrap();
    let store = ProjectStore::new(t.path());
    store.save(&p).unwrap();
    let p = store.load().unwrap();
    assert_eq!(
        inherited(&p, &p.requests[0]),
        Auth::Bearer("local-secret".into())
    );
    assert_eq!(inherited(&p, &p.requests[1]), p.auth);
    assert_eq!(inherited(&p, &p.requests[2]), Auth::None);
    let before = serde_json::to_value(&p.requests).unwrap();
    let prepared = p.prepare_request(&p.requests[1]).unwrap();
    assert_eq!(prepared.auth, p.auth);
    assert_eq!(serde_json::to_value(&p.requests).unwrap(), before);
    assert!(!before.to_string().contains("workspace-secret"));
    let mut old = serde_json::to_value(&p).unwrap();
    old.as_object_mut().unwrap().remove("auth");
    for f in old["folders"].as_array_mut().unwrap() {
        f.as_object_mut().unwrap().remove("auth");
    }
    let old: Project = serde_json::from_value(old).unwrap();
    assert_eq!(old.auth, Auth::None);
    assert!(old.folders[0].auth.is_none());
}
#[test]
fn moves_deletion_undo_and_runtime_snapshots_resolve_dynamically() {
    let mut p = Project::empty("Moves");
    p.auth = Auth::Bearer("workspace".into());
    let a = p.add_folder("A", None).unwrap();
    let b = p.add_folder("B", None).unwrap();
    p.folders.iter_mut().find(|f| f.id == a).unwrap().auth = Some(Auth::Bearer("A".into()));
    p.folders.iter_mut().find(|f| f.id == b).unwrap().auth = Some(Auth::Bearer("B".into()));
    let mut r = Request::new(Protocol::Http, "Test");
    r.folder_id = Some(a);
    let id = r.id;
    p.requests.push(r);
    let snapshot = p.prepare_request(&p.requests[0]).unwrap();
    p.move_request_to_folder(id, Some(b)).unwrap();
    assert_eq!(inherited(&p, &p.requests[0]), Auth::Bearer("B".into()));
    let deleted = p.remove_folder(b).unwrap();
    assert_eq!(inherited(&p, &p.requests[0]), p.auth);
    p.restore_folder(deleted).unwrap();
    assert_eq!(inherited(&p, &p.requests[0]), Auth::Bearer("B".into()));
    assert_eq!(snapshot.auth, Auth::Bearer("A".into()));
    p.folders.iter_mut().find(|f| f.id == b).unwrap().auth = None;
    assert_eq!(inherited(&p, &p.requests[0]), p.auth);
    let other = Project::empty("Other");
    assert_eq!(
        inherited(&other, &Request::new(Protocol::Http, "other")),
        Auth::None
    );
}
#[test]
fn broken_inheritance_is_rejected_and_previews_do_not_reveal_secrets() {
    let mut p = Project::empty("Invalid");
    let a = p.add_folder("A", None).unwrap();
    let mut r = Request::new(Protocol::Http, "test");
    r.url = "https://example.test".into();
    r.folder_id = Some(a);
    p.folders[0].parent_id = Some(a);
    assert!(p.prepare_request(&r).is_err());
    p.folders[0].parent_id = None;
    p.auth = Auth::Bearer("secret-for-preview".into());
    let text = p
        .prepare_request(&r)
        .unwrap()
        .encoding_preview(&BTreeMap::new())
        .unwrap();
    assert!(text.contains("Authorization: [已隐藏凭据]"));
    assert!(!text.contains("secret-for-preview"));
    r.folder_id = Some(uuid::Uuid::new_v4());
    assert!(p.prepare_request(&r).is_err());
}
#[tokio::test]
async fn http_inherited_auth_uses_the_current_environment_and_is_not_persisted_to_history() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut requests = vec![];
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![];
            loop {
                let mut chunk = [0; 1024];
                let n = socket.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") || n == 0 {
                    break;
                }
            }
            requests.push(String::from_utf8(request).unwrap().to_lowercase());
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        }
        requests
    });
    let mut p = Project::empty("HTTP");
    p.auth = Auth::Bearer("{{token}}".into());
    let mut r = Request::new(Protocol::Http, "inherited");
    r.url = format!("http://{address}");
    for token in ["env-one-secret", "env-two-secret"] {
        let record = transport::http::execute(
            p.prepare_request(&r).unwrap(),
            BTreeMap::from([("token".into(), token.into())]),
            "local".into(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(!serde_json::to_string(&record).unwrap().contains(token));
    }
    let requests = server.await.unwrap();
    assert!(requests[0].contains("authorization: bearer env-one-secret"));
    assert!(requests[1].contains("authorization: bearer env-two-secret"));
    assert!(r.inherits_auth());
    assert_eq!(r.auth, Auth::None);
}
#[tokio::test]
async fn websocket_handshake_uses_folder_basic_auth() {
    use base64::Engine;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let expected = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode("user:ws-secret")
    );
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_hdr_async(stream, CheckAuthorization(expected))
            .await
            .unwrap();
        socket.close(None).await.unwrap();
    });
    let mut p = Project::empty("WS");
    let id = p.add_folder("Protected", None).unwrap();
    p.folders[0].auth = Some(Auth::Basic {
        username: "user".into(),
        password: "{{password}}".into(),
    });
    let mut r = Request::new(Protocol::WebSocket, "stream");
    r.folder_id = Some(id);
    r.url = format!("ws://{address}");
    let (_commands, rx) = mpsc::channel(8);
    let (events, _incoming) = mpsc::channel(16);
    tokio::time::timeout(
        Duration::from_secs(3),
        transport::websocket::run(
            p.prepare_request(&r).unwrap(),
            BTreeMap::from([("password".into(), "ws-secret".into())]),
            rx,
            events,
            CancellationToken::new(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    server.await.unwrap();
}
#[test]
fn explicit_public_openapi_operations_do_not_inherit_workspace_credentials() {
    let document = serde_json::json!({"openapi":"3.1.0","info":{"title":"Security","version":"1"},"security":[{"Bearer":[]}],"paths":{"/private":{"get":{"responses":{}}},"/public":{"get":{"security":[],"responses":{}}}}});
    let imported = zeph::openapi::import_value(document).unwrap();
    let mut p = Project::empty("Security");
    p.auth = Auth::Bearer("secret".into());
    p.merge_collection(imported.folders, imported.requests)
        .unwrap();
    let public = p
        .requests
        .iter()
        .find(|r| r.name.contains("/public"))
        .unwrap();
    let private = p
        .requests
        .iter()
        .find(|r| r.name.contains("/private"))
        .unwrap();
    assert_eq!(inherited(&p, public), Auth::None);
    assert_eq!(inherited(&p, private), p.auth);
}

#[tokio::test]
async fn trace_still_rejects_inherited_credentials_before_sending() {
    let mut p = Project::empty("Trace");
    p.auth = Auth::Bearer("never-send-this".into());
    let mut r = Request::new(Protocol::Http, "trace");
    r.method = "TRACE".into();
    r.url = "http://127.0.0.1:1".into();
    let error = transport::http::execute(
        p.prepare_request(&r).unwrap(),
        BTreeMap::new(),
        "local".into(),
        CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("TRACE"));
    assert!(!error.to_string().contains("never-send-this"));
}

#[test]
fn moving_an_inheriting_folder_retargets_all_its_requests_without_rewriting_them() {
    let mut p = Project::empty("Folder move");
    let a = p.add_folder("A", None).unwrap();
    let b = p.add_folder("B", None).unwrap();
    let child = p.add_folder("Child", Some(a)).unwrap();
    p.folders.iter_mut().find(|f| f.id == a).unwrap().auth = Some(Auth::Bearer("A".into()));
    p.folders.iter_mut().find(|f| f.id == b).unwrap().auth = Some(Auth::Bearer("B".into()));
    let mut r = Request::new(Protocol::Http, "test");
    r.folder_id = Some(child);
    p.requests.push(r);
    assert_eq!(inherited(&p, &p.requests[0]), Auth::Bearer("A".into()));
    let before = serde_json::to_value(&p.requests).unwrap();
    p.move_folder(child, Some(b)).unwrap();
    assert_eq!(inherited(&p, &p.requests[0]), Auth::Bearer("B".into()));
    assert_eq!(serde_json::to_value(&p.requests).unwrap(), before);
}

struct CheckAuthorization(String);
impl tokio_tungstenite::tungstenite::handshake::server::Callback for CheckAuthorization {
    // The upstream Callback trait fixes this unboxed HTTP response error type.
    #[allow(clippy::result_large_err)]
    fn on_request(
        self,
        request: &tokio_tungstenite::tungstenite::handshake::server::Request,
        response: tokio_tungstenite::tungstenite::handshake::server::Response,
    ) -> Result<
        tokio_tungstenite::tungstenite::handshake::server::Response,
        tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
    > {
        assert_eq!(
            request.headers().get("authorization").unwrap(),
            self.0.as_str()
        );
        Ok(response)
    }
}
