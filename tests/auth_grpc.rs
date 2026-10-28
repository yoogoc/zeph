#[allow(dead_code)]
#[path = "../examples/support/mod.rs"]
mod support;
use std::{collections::BTreeMap, time::Duration};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_util::sync::CancellationToken;
use zeph::{
    model::{Auth, Project, Protocol, Request},
    transport::{SessionEvent, grpc},
};
#[tokio::test]
async fn grpc_reflection_and_rpc_use_the_same_inherited_auth() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    let stop = shutdown.clone();
    let (metadata, mut received) = mpsc::channel(32);
    let check = move |request: tonic::Request<()>| {
        let value = request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if value != "Bearer grpc-secret" {
            return Err(tonic::Status::unauthenticated("authentication missing"));
        }
        let _ = metadata.try_send(value.to_owned());
        Ok(request)
    };
    let server = tokio::spawn(async move {
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/order.bin"
            )))
            .build_v1()
            .unwrap();
        tonic::transport::Server::builder()
            .add_service(tonic::service::interceptor::InterceptedService::new(
                reflection,
                check.clone(),
            ))
            .add_service(
                support::shop::order_service_server::OrderServiceServer::with_interceptor(
                    support::Orders,
                    check,
                ),
            )
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                stop.cancelled_owned(),
            )
            .await
            .unwrap();
    });
    let mut p = Project::empty("gRPC");
    p.auth = Auth::Bearer("{{token}}".into());
    let mut request = Request::new(Protocol::Grpc, "inherited");
    request.url = format!("http://{address}");
    request.grpc_method = "shop.v1.OrderService/GetOrder".into();
    request.body.text = r#"{"orderId":"auth-test"}"#.into();
    let vars = BTreeMap::from([("token".into(), "grpc-secret".into())]);
    let effective = p.prepare_request(&request).unwrap();
    let pool = tokio::time::timeout(Duration::from_secs(3), grpc::discover(&effective, &vars))
        .await
        .unwrap()
        .unwrap();
    let (_commands, rx) = mpsc::channel(8);
    let (events, mut incoming) = mpsc::channel(32);
    tokio::time::timeout(
        Duration::from_secs(3),
        grpc::run(effective, vars, pool, rx, events, CancellationToken::new()),
    )
    .await
    .unwrap()
    .unwrap();
    let mut succeeded = false;
    while let Some(event) = incoming.recv().await {
        if let SessionEvent::Finished { code, .. } = event {
            assert_eq!(code, "0 OK");
            succeeded = true;
        }
    }
    assert!(succeeded);
    let mut checks = 0;
    while received.try_recv().is_ok() {
        checks += 1;
    }
    assert!(checks >= 2, "both Reflection and RPC must authenticate");
    assert!(request.inherits_auth());
    assert_eq!(request.auth, Auth::None);
    shutdown.cancel();
    server.await.unwrap();
}
