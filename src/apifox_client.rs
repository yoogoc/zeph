use super::{Snapshot, SourceConfig};
use anyhow::{Result, bail};
use reqwest::{
    StatusCode,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroizing;

#[cfg(feature = "credentials")]
const SERVICE: &str = "dev.zeph.apifox";
const ORIGIN: &str = "https://api.apifox.com";
const LIMIT: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Authorization,
    RateLimit,
    Network,
    Invalid,
}
#[derive(Clone, Debug)]
pub struct ApiFailure {
    pub kind: FailureKind,
    pub message: String,
    pub retry_after: Option<u64>,
}
impl ApiFailure {
    pub fn credentials() -> Self {
        Self {
            kind: FailureKind::Authorization,
            message: "无法读取访问令牌，请编辑连接并重新保存令牌".into(),
            retry_after: None,
        }
    }
    fn invalid(message: &str) -> Self {
        Self {
            kind: FailureKind::Invalid,
            message: message.into(),
            retry_after: None,
        }
    }
}

#[cfg(feature = "credentials")]
pub fn save_token(id: Uuid, token: &str) -> Result<()> {
    if token.trim().is_empty() {
        bail!("请输入 Apifox 访问令牌");
    }
    let entry = keyring::Entry::new(SERVICE, &id.to_string())
        .map_err(|_| anyhow::anyhow!("系统凭据存储不可用"))?;
    entry
        .set_password(token.trim())
        .map_err(|_| anyhow::anyhow!("访问令牌无法保存到系统凭据存储"))
}
#[cfg(feature = "credentials")]
pub fn read_token(id: Uuid) -> Result<Zeroizing<String>> {
    let entry = keyring::Entry::new(SERVICE, &id.to_string())
        .map_err(|_| anyhow::anyhow!("系统凭据存储不可用"))?;
    entry
        .get_password()
        .map(Zeroizing::new)
        .map_err(|_| anyhow::anyhow!("访问令牌不可用，请重新保存连接"))
}
#[cfg(feature = "credentials")]
pub fn remove_token(id: Uuid) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, &id.to_string())
        .map_err(|_| anyhow::anyhow!("系统凭据存储不可用"))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => bail!("系统凭据无法删除"),
    }
}

#[cfg(not(feature = "credentials"))]
pub fn save_token(_id: Uuid, _token: &str) -> Result<()> {
    bail!("此构建未启用系统凭据存储")
}
#[cfg(not(feature = "credentials"))]
pub fn read_token(_id: Uuid) -> Result<Zeroizing<String>> {
    bail!("此构建未启用系统凭据存储")
}
#[cfg(not(feature = "credentials"))]
pub fn remove_token(_id: Uuid) -> Result<()> {
    bail!("此构建未启用系统凭据存储")
}

pub async fn fetch(
    config: &SourceConfig,
    token: &str,
) -> std::result::Result<Snapshot, ApiFailure> {
    fetch_from(ORIGIN, config, token).await
}
async fn fetch_from(
    origin: &str,
    config: &SourceConfig,
    token: &str,
) -> std::result::Result<Snapshot, ApiFailure> {
    config
        .validate()
        .map_err(|_| ApiFailure::invalid("项目、分支或模块配置无效"))?;
    let mut headers = HeaderMap::new();
    let mut auth = HeaderValue::from_str(&format!("Bearer {}", token.trim()))
        .map_err(|_| ApiFailure::invalid("访问令牌格式无效"))?;
    auth.set_sensitive(true);
    headers.insert(AUTHORIZATION, auth);
    headers.insert(
        "X-Apifox-Api-Version",
        HeaderValue::from_static("2024-03-28"),
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .connect_timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Zeph/0.1")
        .default_headers(headers)
        .build()
        .map_err(|_| ApiFailure::invalid("无法初始化 Apifox 客户端"))?;
    let mut body = json!({"scope":{"type":"ALL"},"options":{"includeApifoxExtensionProperties":true,"addFoldersToTags":false},"oasVersion":"3.1","exportFormat":"JSON"});
    if let Some(id) = config.branch_id {
        body["branchId"] = json!(id);
    }
    if let Some(id) = config.module_id {
        body["moduleId"] = json!(id);
    }
    let mut response = client
        .post(format!(
            "{origin}/v1/projects/{}/export-openapi?locale=zh-CN",
            config.project_id
        ))
        .json(&body)
        .send()
        .await
        .map_err(|_| ApiFailure {
            kind: FailureKind::Network,
            message: "无法连接 Apifox，请检查网络后重试".into(),
            retry_after: None,
        })?;
    let status = response.status();
    if status != StatusCode::OK {
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let (kind, message) = match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (
                FailureKind::Authorization,
                "令牌失效或无项目权限，自动检查已暂停",
            ),
            StatusCode::TOO_MANY_REQUESTS => {
                (FailureKind::RateLimit, "Apifox 请求频率受限，将稍后重试")
            }
            StatusCode::NOT_FOUND => (
                FailureKind::Invalid,
                "项目、分支或模块不存在，请检查连接设置",
            ),
            StatusCode::UNPROCESSABLE_ENTITY => {
                (FailureKind::Invalid, "Apifox 不接受当前导出参数或 API 版本")
            }
            _ => (FailureKind::Network, "Apifox 导出失败，本地快照保持不变"),
        };
        // Never display untrusted response bodies: they may echo credentials or document content.
        return Err(ApiFailure {
            kind,
            message: format!("{message}（HTTP {}）", status.as_u16()),
            retry_after,
        });
    }
    if response
        .content_length()
        .is_some_and(|size| size > LIMIT as u64)
    {
        return Err(ApiFailure::invalid(
            "Apifox 导出超过 32 MiB，请缩小同步模块",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiFailure::invalid("Apifox 响应读取中断，快照未更新"))?
    {
        if bytes.len() + chunk.len() > LIMIT {
            return Err(ApiFailure::invalid(
                "Apifox 导出超过 32 MiB，请缩小同步模块",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|_| ApiFailure::invalid("Apifox 返回了无效 JSON，快照未更新"))?;
    Snapshot::parse(value)
        .map_err(|_| ApiFailure::invalid("导出文档不完整、含外部引用或版本不支持，快照未更新"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    async fn server(
        status: &str,
        body: &str,
        extra: &str,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
            body.len()
        );
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut received = vec![];
            loop {
                let mut buf = [0u8; 4096];
                let size = socket.read(&mut buf).await.unwrap();
                received.extend_from_slice(&buf[..size]);
                let text = String::from_utf8_lossy(&received);
                if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                    let len = headers
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|n| n.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if body.len() >= len {
                        break;
                    }
                }
                if size == 0 {
                    break;
                }
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(received).unwrap()
        });
        (format!("http://{address}"), task)
    }
    fn config() -> SourceConfig {
        SourceConfig {
            name: "Test".into(),
            project_id: "12345".into(),
            branch_id: Some(7),
            module_id: Some(9),
        }
    }
    #[tokio::test]
    async fn exports_with_auth_version_extensions_and_explicit_scope() {
        let (url, task) = server(
            "200 OK",
            r#"{"openapi":"3.1.0","info":{"title":"Shop","version":"1"},"paths":{}}"#,
            "",
        )
        .await;
        let snapshot = fetch_from(&url, &config(), "test-token").await.unwrap();
        assert!(snapshot.endpoints.is_empty());
        let request = task.await.unwrap();
        let lower = request.to_lowercase();
        assert!(request.starts_with("POST /v1/projects/12345/export-openapi?locale=zh-CN"));
        assert!(lower.contains("authorization: bearer test-token"));
        assert!(lower.contains("x-apifox-api-version: 2024-03-28"));
        let body: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["scope"]["type"], "ALL");
        assert_eq!(body["options"]["includeApifoxExtensionProperties"], true);
        assert_eq!(body["options"]["addFoldersToTags"], false);
        assert_eq!(body["branchId"], 7);
        assert_eq!(body["moduleId"], 9);
    }
    #[tokio::test]
    async fn failures_are_redacted_and_redirects_are_not_followed() {
        for (status, extra, expected) in [
            ("401 Unauthorized", "", FailureKind::Authorization),
            (
                "429 Too Many Requests",
                "Retry-After: 120\r\n",
                FailureKind::RateLimit,
            ),
            (
                "302 Found",
                "Location: http://127.0.0.1:1/leak\r\n",
                FailureKind::Network,
            ),
        ] {
            let (url, task) = server(status, "test-token", extra).await;
            let error = fetch_from(&url, &config(), "test-token").await.unwrap_err();
            assert_eq!(error.kind, expected);
            assert!(!error.message.contains("test-token"));
            if expected == FailureKind::RateLimit {
                assert_eq!(error.retry_after, Some(120));
            }
            task.await.unwrap();
        }
    }
}
