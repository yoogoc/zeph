use super::RunRecord;
use crate::model::{Auth, BodyKind, Protocol, Request, interpolate};
use anyhow::{Result, bail};
use futures::StreamExt;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    multipart, redirect,
};
use std::{collections::BTreeMap, time::Instant};
use tokio_util::sync::CancellationToken;

pub const RESPONSE_LIMIT: usize = 8 * 1024 * 1024;

pub async fn execute(
    request: Request,
    variables: BTreeMap<String, String>,
    environment: String,
    cancel: CancellationToken,
) -> Result<RunRecord> {
    let started_at = chrono::Utc::now();
    let clock = Instant::now();
    if request.protocol != Protocol::Http {
        bail!("HTTP 引擎收到其他协议的请求");
    }
    if request.method == "CONNECT" {
        return connect_probe(request, variables, environment, cancel).await;
    }
    if request.method == "TRACE"
        && (request.body.kind != BodyKind::None
            || !matches!(request.auth, Auth::None)
            || request.headers.iter().any(|h| {
                h.enabled
                    && (h.name.eq_ignore_ascii_case("authorization")
                        || h.name.eq_ignore_ascii_case("cookie"))
            }))
    {
        bail!("TRACE 不发送请求体、认证或 Cookie。");
    }
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(request.timeout_secs.max(1)))
        .danger_accept_invalid_certs(!request.tls_verify)
        .redirect(if request.follow_redirects {
            redirect::Policy::limited(10)
        } else {
            redirect::Policy::none()
        });
    if !request.proxy.trim().is_empty() {
        builder = builder.proxy(reqwest::Proxy::all(interpolate(
            &request.proxy,
            &variables,
        )?)?);
    }
    let client = builder.build()?;
    let resolved = request.resolved_url(&variables)?;
    let url = url::Url::parse(&resolved)?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("HTTP 地址必须使用 http:// 或 https://");
    }
    let mut headers = HeaderMap::new();
    for h in request
        .headers
        .iter()
        .filter(|h| h.enabled && !h.name.is_empty())
    {
        headers.append(
            HeaderName::from_bytes(interpolate(&h.name, &variables)?.as_bytes())?,
            HeaderValue::from_str(&interpolate(&h.value, &variables)?)?,
        );
    }
    if request.body.kind == BodyKind::FormData && headers.contains_key("content-type") {
        bail!("form-data 的 Content-Type 和 boundary 由引擎生成，请移除手动 Content-Type。");
    }
    if let Some(mime) = request.body.kind.content_type()
        && !headers.contains_key("content-type")
    {
        headers.insert("content-type", HeaderValue::from_static(mime));
    }
    let mut outbound = client
        .request(reqwest::Method::from_bytes(request.method.as_bytes())?, url)
        .headers(headers);
    outbound = match &request.auth {
        Auth::None => outbound,
        Auth::Bearer(token) => outbound.bearer_auth(interpolate(token, &variables)?),
        Auth::Basic { username, password } => outbound.basic_auth(
            interpolate(username, &variables)?,
            Some(interpolate(password, &variables)?),
        ),
    };
    match request.body.kind {
        BodyKind::None => {}
        BodyKind::Json => {
            let text = interpolate(&request.body.text, &variables)?;
            serde_json::from_str::<serde_json::Value>(&text)?;
            outbound = outbound.body(text);
        }
        BodyKind::Text | BodyKind::Xml => {
            outbound = outbound.body(interpolate(&request.body.text, &variables)?)
        }
        BodyKind::UrlEncoded => {
            let fields = request
                .body
                .fields
                .iter()
                .filter(|p| p.enabled && !p.name.is_empty())
                .map(|p| {
                    Ok((
                        interpolate(&p.name, &variables)?,
                        interpolate(&p.value, &variables)?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            outbound = outbound.form(&fields);
        }
        BodyKind::FormData => {
            let mut form = multipart::Form::new();
            for field in request
                .body
                .fields
                .iter()
                .filter(|p| p.enabled && !p.name.is_empty())
            {
                let name = interpolate(&field.name, &variables)?;
                let value = interpolate(&field.value, &variables)?;
                if field.file {
                    let path = std::path::Path::new(&value);
                    let bytes = tokio::fs::read(path).await?;
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("upload")
                        .to_owned();
                    form = form.part(name, multipart::Part::bytes(bytes).file_name(filename));
                } else {
                    form = form.text(name, value);
                }
            }
            outbound = outbound.multipart(form);
        }
        BodyKind::Binary => {
            outbound = outbound
                .body(tokio::fs::read(interpolate(&request.body.file_path, &variables)?).await?)
        }
    }
    let response = tokio::select! { _=cancel.cancelled()=>bail!("请求已取消"), response=outbound.send()=>response? };
    let status = format!(
        "{} {}",
        response.status().as_u16(),
        response.status().canonical_reason().unwrap_or("")
    );
    let response_headers = response
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                if k == "set-cookie" {
                    "[已隐藏 Cookie]".into()
                } else {
                    v.to_str().unwrap_or("[binary]").to_owned()
                },
            )
        })
        .collect();
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    let mut truncated = false;
    loop {
        let chunk = tokio::select! { _=cancel.cancelled()=>bail!("请求已取消"), chunk=stream.next()=>chunk };
        let Some(chunk) = chunk else { break };
        let chunk = chunk?;
        let remaining = RESPONSE_LIMIT - bytes.len();
        bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        if chunk.len() > remaining {
            truncated = true;
            break;
        }
    }
    let count = bytes.len();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let body = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or(text);
    Ok(RunRecord {
        id: uuid::Uuid::new_v4(),
        request_id: request.id,
        request_name: request.name,
        method: request.method,
        environment,
        started_at,
        elapsed_ms: clock.elapsed().as_millis(),
        status,
        headers: response_headers,
        body,
        bytes: count,
        truncated,
        error: false,
    })
}

// CONNECT is a tunnel handshake. This workbench probes the tunnel and closes it;
// it does not send a nested HTTP request or leave an invisible tunnel running.
async fn connect_probe(
    request: Request,
    variables: BTreeMap<String, String>,
    environment: String,
    cancel: CancellationToken,
) -> Result<RunRecord> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let started_at = chrono::Utc::now();
    let clock = Instant::now();
    if request.body.kind != BodyKind::None {
        bail!("CONNECT 握手不发送 Body");
    }
    let target = interpolate(&request.url, &variables)?;
    if target.contains(['/', '?', '#', '\r', '\n', ' ']) {
        bail!("CONNECT 地址应为 host:port");
    }
    let (host, port) = target
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("CONNECT 地址应为 host:port"))?;
    let _: u16 = port.parse()?;
    if host.is_empty() {
        bail!("CONNECT 缺少 host");
    }
    let proxy = url::Url::parse(&interpolate(&request.proxy, &variables)?)?;
    if proxy.scheme() != "http" {
        bail!("CONNECT 探测目前需要 http:// 代理");
    }
    let proxy_host = proxy
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("代理缺少 host"))?
        .to_owned();
    let proxy_port = proxy.port_or_known_default().unwrap_or(80);
    let mut wire = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n");
    for h in request
        .headers
        .iter()
        .filter(|h| h.enabled && !h.name.is_empty())
    {
        let name = HeaderName::from_bytes(interpolate(&h.name, &variables)?.as_bytes())?;
        let value = HeaderValue::from_str(&interpolate(&h.value, &variables)?)?;
        if name == "host" || name == "content-length" || name == "transfer-encoding" {
            continue;
        }
        wire.push_str(&format!("{name}: {}\r\n", value.to_str()?));
    }
    use base64::Engine;
    let auth = match &request.auth {
        Auth::None if !proxy.username().is_empty() => Some(format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!(
                "{}:{}",
                proxy.username(),
                proxy.password().unwrap_or("")
            ))
        )),
        Auth::None => None,
        Auth::Bearer(token) => Some(format!("Bearer {}", interpolate(token, &variables)?)),
        Auth::Basic { username, password } => Some(format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!(
                "{}:{}",
                interpolate(username, &variables)?,
                interpolate(password, &variables)?
            ))
        )),
    };
    if let Some(auth) = auth {
        HeaderValue::from_str(&auth)?;
        wire.push_str(&format!("Proxy-Authorization: {auth}\r\n"));
    }
    wire.push_str("\r\n");
    let work = async {
        let mut stream = tokio::net::TcpStream::connect((proxy_host.as_str(), proxy_port)).await?;
        stream.write_all(wire.as_bytes()).await?;
        let mut header = Vec::new();
        loop {
            let byte = stream.read_u8().await?;
            header.push(byte);
            if header.ends_with(b"\r\n\r\n") {
                break;
            }
            if header.len() > 64 * 1024 {
                bail!("代理响应头超过 64 KiB");
            }
        }
        let text = String::from_utf8(header)?;
        let mut lines = text.split("\r\n");
        let status_line = lines.next().unwrap_or("");
        let code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("代理返回无效 HTTP 状态"))?
            .parse()?;
        let status = status_line
            .split_once(' ')
            .map(|(_, s)| s.to_owned())
            .unwrap_or_default();
        let headers = lines
            .filter_map(|line| {
                line.split_once(':')
                    .map(|(k, v)| (k.to_owned(), v.trim().to_owned()))
            })
            .collect();
        stream.shutdown().await?;
        Ok::<_, anyhow::Error>((code, status, headers, text.len()))
    };
    let (code, status, headers, bytes) = tokio::select! {_=cancel.cancelled()=>bail!("CONNECT 已取消"),result=tokio::time::timeout(std::time::Duration::from_secs(request.timeout_secs.max(1)),work)=>result??};
    let success = (200..300).contains(&code);
    Ok(RunRecord {
        id: uuid::Uuid::new_v4(),
        request_id: request.id,
        request_name: request.name,
        method: request.method,
        environment,
        started_at,
        elapsed_ms: clock.elapsed().as_millis(),
        status,
        headers,
        body: if success {
            format!("CONNECT {target}\n代理已建立隧道；探测完成后已关闭连接。")
        } else {
            format!("代理拒绝建立到 {target} 的隧道。")
        },
        bytes,
        truncated: false,
        error: !success,
    })
}
