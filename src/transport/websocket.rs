use super::{SessionCommand, SessionEvent};
use crate::model::{Auth, Request, interpolate};
use anyhow::{Result, bail};
use futures::{SinkExt, StreamExt};
use std::collections::BTreeMap;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderName, HeaderValue},
    },
};
use tokio_util::sync::CancellationToken;

pub async fn run(
    request: Request,
    variables: BTreeMap<String, String>,
    commands: mpsc::Receiver<SessionCommand>,
    events: mpsc::Sender<SessionEvent>,
    cancel: CancellationToken,
) -> Result<()> {
    tokio::select! {biased; _=cancel.cancelled()=>{let _=events.send(SessionEvent::Finished{code:"已取消".into(),trailers:vec![]}).await;Ok(())},result=run_session(request,variables,commands,events.clone(),cancel.clone())=>result}
}

async fn run_session(
    request: Request,
    variables: BTreeMap<String, String>,
    mut commands: mpsc::Receiver<SessionCommand>,
    events: mpsc::Sender<SessionEvent>,
    cancel: CancellationToken,
) -> Result<()> {
    if !request.proxy.is_empty() {
        bail!("WebSocket 代理配置暂不支持");
    }
    if !request.tls_verify {
        bail!("WebSocket 当前使用系统证书校验");
    }
    let resolved = request.resolved_url(&variables)?;
    let mut handshake = resolved.into_client_request()?;
    for h in request
        .headers
        .iter()
        .filter(|h| h.enabled && !h.name.is_empty())
    {
        handshake.headers_mut().append(
            HeaderName::from_bytes(h.name.as_bytes())?,
            HeaderValue::from_str(&interpolate(&h.value, &variables)?)?,
        );
    }
    match &request.auth {
        Auth::Bearer(token) => {
            handshake.headers_mut().insert(
                "Authorization",
                HeaderValue::from_str(&format!("Bearer {}", interpolate(token, &variables)?))?,
            );
        }
        Auth::Basic { username, password } => {
            use base64::Engine;
            let text = format!(
                "{}:{}",
                interpolate(username, &variables)?,
                interpolate(password, &variables)?
            );
            handshake.headers_mut().insert(
                "Authorization",
                HeaderValue::from_str(&format!(
                    "Basic {}",
                    base64::engine::general_purpose::STANDARD.encode(text)
                ))?,
            );
        }
        Auth::None => {}
    }
    let (mut socket, response) = tokio::select! {_=cancel.cancelled()=>return Ok(()),value=tokio::time::timeout(std::time::Duration::from_secs(request.timeout_secs.max(1)),connect_async_with_config(handshake,Some(tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default().max_message_size(Some(8*1024*1024)).max_frame_size(Some(8*1024*1024))),false))=>value??};
    events
        .send(SessionEvent::Metadata(
            response
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        if k == "set-cookie" {
                            "[已隐藏 Cookie]".to_owned()
                        } else {
                            v.to_str().unwrap_or("[binary]").to_string()
                        },
                    )
                })
                .collect(),
        ))
        .await?;
    events.send(SessionEvent::Opened).await?;
    loop {
        tokio::select! {
            _=cancel.cancelled()=>{let _=socket.close(None).await;break;},
            command=commands.recv()=>match command {
                Some(SessionCommand::Send(text))=>{socket.send(Message::Text(text.clone().into())).await?;events.send(SessionEvent::Sent(text)).await?;},
                Some(SessionCommand::SendBinary(bytes))=>{let count=bytes.len();socket.send(Message::Binary(bytes.into())).await?;events.send(SessionEvent::Sent(format!("[Binary · {count} bytes]"))).await?;},
                _=>{socket.close(None).await?;break;}
            },
            message=socket.next()=>match message {
                Some(Ok(Message::Text(text)))=>{events.send(SessionEvent::Received(text.to_string())).await?;},
                Some(Ok(Message::Binary(bytes)))=>{use base64::Engine;events.send(SessionEvent::Received(format!("[Binary · {} bytes]\n{}",bytes.len(),base64::engine::general_purpose::STANDARD.encode(&bytes)))).await?;},
                Some(Ok(Message::Ping(bytes)))=>{events.send(SessionEvent::Received(format!("[Ping · {} bytes]",bytes.len()))).await?;},
                Some(Ok(Message::Pong(bytes)))=>{events.send(SessionEvent::Received(format!("[Pong · {} bytes]",bytes.len()))).await?;},
                Some(Ok(Message::Close(frame)))=>{events.send(SessionEvent::Finished{code:frame.map(|f|format!("{} {}",f.code,f.reason)).unwrap_or_else(||"连接关闭".into()),trailers:vec![]}).await?;return Ok(());},
                Some(Err(error))=>return Err(error.into()),None=>break,_=>{}
            }
        }
    }
    let _ = events
        .send(SessionEvent::Finished {
            code: "连接关闭".into(),
            trailers: vec![],
        })
        .await;
    Ok(())
}
