pub mod grpc;
pub mod http;
pub mod websocket;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: Uuid,
    pub request_id: Uuid,
    pub request_name: String,
    pub method: String,
    pub environment: String,
    pub started_at: DateTime<Utc>,
    pub elapsed_ms: u128,
    pub status: String,
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub trailers: Vec<(String, String)>,
    #[serde(default)]
    pub timeline: Vec<TimingMark>,
    #[serde(default)]
    pub messages: Vec<SessionMessage>,
    pub body: String,
    pub bytes: usize,
    pub truncated: bool,
    pub error: bool,
}
impl RunRecord {
    pub fn error(request: &crate::model::Request, environment: String, message: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            request_id: request.id,
            request_name: request.name.clone(),
            method: request.method.clone(),
            environment,
            started_at: Utc::now(),
            elapsed_ms: 0,
            status: "请求失败".into(),
            headers: vec![],
            trailers: vec![],
            timeline: vec![],
            messages: vec![],
            body: message,
            bytes: 0,
            truncated: false,
            error: true,
        }
    }
}

/// A bounded payload, independent of connection events and protocol metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMessage {
    pub sent: bool,
    pub elapsed_ms: u128,
    pub body: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimingMark {
    pub elapsed_ms: u128,
    pub label: String,
}

#[derive(Clone, Debug)]
pub enum SessionEvent {
    Opened,
    Sent(String),
    Received(String),
    Metadata(Vec<(String, String)>),
    Finished {
        code: String,
        trailers: Vec<(String, String)>,
    },
    Failed(String),
}
#[derive(Clone, Debug)]
pub enum SessionCommand {
    Send(String),
    SendBinary(Vec<u8>),
    HalfClose,
    Close,
}
