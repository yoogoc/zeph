#[path = "../examples/support/mod.rs"]
mod support;
use std::{collections::BTreeMap, time::Duration};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_util::sync::CancellationToken;
use zeph::{
    model::*,
    transport::{SessionCommand, SessionEvent, grpc},
};
#[tokio::test]
async fn reflection_and_all_four_rpc_modes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(support::serve(listener, shutdown.clone()));
    let mut request = Request::new(Protocol::Grpc, "test");
    request.url = format!("http://{address}");
    let vars = BTreeMap::new();
    let pool = grpc::discover(&request, &vars).await.unwrap();
    assert_eq!(grpc::methods(&pool).len(), 4);
    for (name, body, messages, expected) in [
        ("GetOrder", r#"{"orderId":"ord_test"}"#, vec![], 1),
        ("WatchOrders", r#"{"customerId":"cus_test"}"#, vec![], 2),
        (
            "CreateOrders",
            "{}",
            vec![
                r#"{"customerId":"cus_test","sku":"a","quantity":1}"#,
                r#"{"customerId":"cus_test","sku":"b","quantity":2}"#,
            ],
            1,
        ),
        (
            "SyncOrders",
            "{}",
            vec![
                r#"{"orderId":"o1","status":"paid","version":"9007199254740993"}"#,
                r#"{"orderId":"o2","status":"paid","version":"2"}"#,
            ],
            2,
        ),
    ] {
        let mut request = request.clone();
        request.grpc_method = format!("shop.v1.OrderService/{name}");
        request.body.text = body.into();
        let (tx, rx) = mpsc::channel(16);
        let (events, mut incoming) = mpsc::channel(32);
        let cancel = CancellationToken::new();
        for text in messages {
            tx.send(SessionCommand::Send(text.into())).await.unwrap();
        }
        if name == "CreateOrders" || name == "SyncOrders" {
            tx.send(SessionCommand::HalfClose).await.unwrap();
        }
        let task = tokio::spawn(grpc::run(
            request,
            vars.clone(),
            pool.clone(),
            rx,
            events,
            cancel,
        ));
        let mut values = vec![];
        let mut finished = false;
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(event) = incoming.recv().await {
                match event {
                    SessionEvent::Received(text) => {
                        values.push(serde_json::from_str::<serde_json::Value>(&text).unwrap())
                    }
                    SessionEvent::Finished { code, .. } => {
                        assert_eq!(code, "0 OK");
                        finished = true;
                    }
                    SessionEvent::Failed(error) => panic!("{error}"),
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        task.await.unwrap().unwrap();
        assert!(finished, "{name}");
        assert_eq!(values.len(), expected, "{name}");
        if name == "GetOrder" {
            assert_eq!(values[0]["orderId"], "ord_test");
        }
        if name == "CreateOrders" {
            assert_eq!(values[0]["createdCount"], 2);
        }
        if name == "SyncOrders" {
            assert_eq!(values[0]["version"], "9007199254740993");
        }
    }
    shutdown.cancel();
    server.await.unwrap().unwrap();
}
#[tokio::test]
async fn rpc_errors_are_not_success() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(support::serve(listener, shutdown.clone()));
    let mut r = Request::new(Protocol::Grpc, "bad");
    r.url = format!("http://{address}");
    r.grpc_method = "shop.v1.OrderService/CreateOrders".into();
    let (tx, rx) = mpsc::channel(8);
    tx.send(SessionCommand::Send(r#"{"quantity":0}"#.into()))
        .await
        .unwrap();
    tx.send(SessionCommand::HalfClose).await.unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let result = grpc::run(
        r,
        BTreeMap::new(),
        grpc::example_pool(),
        rx,
        events,
        CancellationToken::new(),
    )
    .await;
    assert!(result.unwrap_err().to_string().contains("quantity"));
    while let Some(event) = incoming.recv().await {
        assert!(!matches!(event, SessionEvent::Finished { .. }));
    }
    shutdown.cancel();
    server.await.unwrap().unwrap();
}
