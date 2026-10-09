mod support;
use futures::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let shutdown = tokio_util::sync::CancellationToken::new();
    let grpc = TcpListener::bind("127.0.0.1:50051").await?;
    let http = TcpListener::bind("127.0.0.1:8080").await?;
    println!(
        "HTTP / WebSocket: 127.0.0.1:8080\ngRPC + Reflection: 127.0.0.1:50051\nCtrl-C stops the local demo. All data is temporary."
    );
    let grpc_task = tokio::spawn(support::serve(grpc, shutdown.clone()));
    let token = shutdown.clone();
    let http_task = tokio::spawn(async move {
        loop {
            let result = tokio::select! {_=token.cancelled()=>break,result=http.accept()=>result};
            let (socket, _) = result?;
            tokio::spawn(async move {
                if let Err(e) = handle(socket).await {
                    eprintln!("demo: {e}");
                }
            });
        }
        Ok::<_, anyhow::Error>(())
    });
    tokio::signal::ctrl_c().await?;
    shutdown.cancel();
    grpc_task.await??;
    http_task.await??;
    Ok(())
}
async fn handle(mut socket: tokio::net::TcpStream) -> anyhow::Result<()> {
    let mut peek = [0; 8192];
    let n = socket.peek(&mut peek).await?;
    if String::from_utf8_lossy(&peek[..n])
        .to_lowercase()
        .contains("upgrade: websocket")
    {
        let mut ws = tokio_tungstenite::accept_async(socket).await?;
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"event":"connected","channel":"orders"}"#.into(),
        ))
        .await?;
        while let Some(message) = ws.next().await {
            let message = message?;
            if message.is_close() {
                let _ = ws.flush().await;
                break;
            }
            if message.is_text() || message.is_binary() {
                ws.send(message).await?;
            }
        }
        return Ok(());
    }
    let mut bytes = vec![];
    let end = loop {
        bytes.push(socket.read_u8().await?);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
        if bytes.len() > 65536 {
            anyhow::bail!("request header too large");
        }
    };
    let headers = String::from_utf8_lossy(&bytes);
    let line = headers.lines().next().unwrap_or("");
    let method = line.split_whitespace().next().unwrap_or("").to_owned();
    let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
    let length = headers
        .lines()
        .find_map(|l| {
            l.to_lowercase()
                .strip_prefix("content-length:")
                .and_then(|v| v.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    if length > 8 * 1024 * 1024 {
        anyhow::bail!("demo request body exceeds 8 MiB");
    }
    bytes.resize(end + length, 0);
    socket.read_exact(&mut bytes[end..]).await?;
    let received = String::from_utf8_lossy(&bytes[end..]);
    let body = serde_json::to_string_pretty(
        &serde_json::json!({"ok":true,"method":method,"path":path,"receivedBody":received,"orders":[{"orderId":"ord_1024","status":"paid","totalAmount":129900,"currency":"CNY"}]}),
    )?;
    socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAllow: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, TRACE\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),if method=="HEAD"{""}else{&body}).as_bytes()).await?;
    Ok(())
}
