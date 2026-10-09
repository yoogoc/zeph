use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const HTTP_METHODS: [&str; 9] = [
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "CONNECT",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Protocol {
    Http,
    Grpc,
    WebSocket,
}
impl Protocol {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Grpc => "gRPC",
            Self::WebSocket => "WS",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub value: String,
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub file: bool,
}
impl Parameter {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: true,
            description: String::new(),
            file: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyKind {
    #[default]
    None,
    Json,
    FormData,
    UrlEncoded,
    Text,
    Xml,
    Binary,
}
impl BodyKind {
    pub const ALL: [Self; 7] = [
        Self::None,
        Self::Json,
        Self::FormData,
        Self::UrlEncoded,
        Self::Text,
        Self::Xml,
        Self::Binary,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "无 Body",
            Self::Json => "JSON",
            Self::FormData => "form-data",
            Self::UrlEncoded => "x-www-form-urlencoded",
            Self::Text => "Text",
            Self::Xml => "XML",
            Self::Binary => "Binary",
        }
    }
    pub fn content_type(self) -> Option<&'static str> {
        match self {
            Self::None | Self::FormData => None,
            Self::Json => Some("application/json"),
            Self::UrlEncoded => Some("application/x-www-form-urlencoded"),
            Self::Text => Some("text/plain; charset=utf-8"),
            Self::Xml => Some("application/xml"),
            Self::Binary => Some("application/octet-stream"),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RequestBody {
    pub kind: BodyKind,
    pub text: String,
    pub fields: Vec<Parameter>,
    pub file_path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum Auth {
    #[default]
    None,
    Bearer(String),
    Basic {
        username: String,
        password: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: Uuid,
    pub name: String,
    pub protocol: Protocol,
    pub method: String,
    pub url: String,
    pub query: Vec<Parameter>,
    pub path: Vec<Parameter>,
    pub headers: Vec<Parameter>,
    pub body: RequestBody,
    pub auth: Auth,
    pub timeout_secs: u64,
    pub follow_redirects: bool,
    pub proxy: String,
    pub tls_verify: bool,
    pub grpc_method: String,
    pub proto_path: String,
    pub schema: Option<serde_json::Value>,
    pub source: Option<String>,
    pub description: String,
}
impl Request {
    pub fn new(protocol: Protocol, name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            protocol,
            method: "GET".into(),
            url: "{{base_url}}".into(),
            query: vec![],
            path: vec![],
            headers: vec![],
            body: RequestBody::default(),
            auth: Auth::None,
            timeout_secs: 30,
            follow_redirects: false,
            proxy: String::new(),
            tls_verify: true,
            grpc_method: String::new(),
            proto_path: String::new(),
            schema: None,
            source: None,
            description: String::new(),
        }
    }
    pub fn resolved_url(&self, vars: &BTreeMap<String, String>) -> Result<String> {
        let mut raw = interpolate(&self.url, vars)?;
        if self.protocol == Protocol::Http && self.method == "CONNECT" {
            return Ok(raw);
        }
        for p in self.path.iter().filter(|p| p.enabled) {
            let value = interpolate(&p.value, vars)?;
            raw = raw.replace(&format!("{{{}}}", p.name), &encode_path_segment(&value));
        }
        // OpenAPI path templates use single braces; unresolved variables never reach the network.
        if raw.contains('{') || raw.contains('}') {
            bail!("路径参数尚未填写: {raw}");
        }
        if self.protocol == Protocol::Grpc {
            return Ok(raw);
        }
        let mut url = url::Url::parse(&raw)?;
        if self.query.iter().any(|p| p.enabled && !p.name.is_empty()) {
            let mut pairs = url.query_pairs_mut();
            for p in self
                .query
                .iter()
                .filter(|p| p.enabled && !p.name.is_empty())
            {
                pairs.append_pair(&interpolate(&p.name, vars)?, &interpolate(&p.value, vars)?);
            }
        }
        Ok(url.to_string())
    }
}

pub fn encode_path_segment(value: &str) -> String {
    use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
    const SEGMENT: &AsciiSet = &CONTROLS
        .add(b' ')
        .add(b'"')
        .add(b'#')
        .add(b'%')
        .add(b'/')
        .add(b':')
        .add(b';')
        .add(b'<')
        .add(b'=')
        .add(b'>')
        .add(b'?')
        .add(b'@')
        .add(b'[')
        .add(b'\\')
        .add(b']')
        .add(b'^')
        .add(b'`')
        .add(b'{')
        .add(b'|')
        .add(b'}');
    utf8_percent_encode(value, SEGMENT).to_string()
}

pub fn interpolate(input: &str, vars: &BTreeMap<String, String>) -> Result<String> {
    let mut result = input.to_owned();
    for _ in 0..16 {
        if !result.contains("{{") {
            return Ok(result);
        }
        let mut expanded = String::with_capacity(result.len());
        let mut rest = result.as_str();
        while let Some(start) = rest.find("{{") {
            expanded.push_str(&rest[..start]);
            let tail = &rest[start + 2..];
            let end = tail
                .find("}}")
                .ok_or_else(|| anyhow::anyhow!("变量缺少结束标记"))?;
            let key = tail[..end].trim();
            let value = vars
                .get(key)
                .ok_or_else(|| anyhow::anyhow!("未定义环境变量: {key}"))?;
            expanded.push_str(value);
            rest = &tail[end + 2..];
        }
        expanded.push_str(rest);
        if expanded.len() > 16 * 1024 * 1024 {
            bail!("环境变量展开超过 16 MiB");
        }
        result = expanded;
    }
    if result.contains("{{") {
        bail!("变量展开过深，可能存在循环引用");
    }
    Ok(result)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    pub name: String,
    pub variables: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub name: String,
    pub requests: Vec<Request>,
    pub environments: Vec<Environment>,
    pub active_environment: usize,
    pub active_request: usize,
    pub horizontal: bool,
    pub show_schema: bool,
}
impl Project {
    pub fn examples() -> Self {
        let mut requests = vec![];
        for method in HTTP_METHODS {
            let name = match method {
                "GET" => "查询客户订单",
                "POST" => "创建订单",
                "PUT" => "替换收货地址",
                "PATCH" => "修改订单备注",
                "DELETE" => "删除草稿订单",
                "HEAD" => "检查订单资源",
                "OPTIONS" => "检查接口能力",
                "TRACE" => "诊断请求链路",
                _ => "建立代理隧道",
            };
            let mut r = Request::new(Protocol::Http, name);
            r.method = method.into();
            r.url = "{{base_url}}/v1/orders/{order_id}".into();
            r.path = vec![Parameter::new("order_id", "ord_1024")];
            r.headers = vec![Parameter::new("Accept", "application/json")];
            match method {
                "GET" => {
                    r.url = "{{base_url}}/v1/customers/{customer_id}/orders".into();
                    r.path = vec![Parameter::new("customer_id", "cus_1024")];
                    r.query = vec![
                        Parameter::new("status", "paid"),
                        Parameter::new("page", "1"),
                        Parameter::new("tag", "priority"),
                        Parameter::new("tag", "gift"),
                    ];
                }
                "POST" => {
                    r.url = "{{base_url}}/v1/customers/{customer_id}/orders".into();
                    r.path = vec![Parameter::new("customer_id", "cus_1024")];
                    r.query.push(Parameter::new("notify", "true"));
                    r.body.kind = BodyKind::Json;
                    r.body.text = "{\n  \"items\": [{ \"sku\": \"zeph-pro\", \"quantity\": 1 }],\n  \"currency\": \"CNY\"\n}".into();
                }
                "PUT" => {
                    r.url.push_str("/shipping-address");
                    r.body.kind = BodyKind::Json;
                    r.body.text = "{\n  \"recipient\": \"张明\",\n  \"city\": \"上海\",\n  \"street\": \"南京西路 100 号\"\n}".into();
                }
                "PATCH" => {
                    r.body.kind = BodyKind::Json;
                    r.body.text = "{\n  \"note\": \"工作日配送\"\n}".into();
                    r.headers.push(Parameter::new(
                        "Content-Type",
                        "application/merge-patch+json",
                    ));
                }
                "DELETE" => r.query.push(Parameter::new("reason", "duplicate order")),
                "OPTIONS" => {
                    r.headers
                        .push(Parameter::new("Origin", "https://dashboard.example.com"));
                    r.headers
                        .push(Parameter::new("Access-Control-Request-Method", "PATCH"));
                }
                "TRACE" => r.headers.push(Parameter::new("Max-Forwards", "0")),
                "CONNECT" => {
                    r.url = "api.example.com:443".into();
                    r.path.clear();
                    r.proxy = "http://localhost:8888".into();
                }
                _ => {}
            }
            r.body.fields = vec![
                Parameter::new("currency", "CNY"),
                Parameter::new("note", "请附上发票"),
            ];
            requests.push(r);
        }
        let mut g = Request::new(Protocol::Grpc, "OrderService");
        g.url = "{{grpc_host}}".into();
        g.grpc_method = "shop.v1.OrderService/GetOrder".into();
        g.body.kind = BodyKind::Json;
        g.body.text = "{\n  \"orderId\": \"ord_1024\"\n}".into();
        g.description = "导入 .proto / descriptor set，或通过 Reflection 读取服务方法。".into();
        requests.push(g);
        let mut ws = Request::new(Protocol::WebSocket, "实时订单事件");
        ws.url = "{{ws_url}}/events".into();
        ws.body.kind = BodyKind::Json;
        ws.body.text = "{\n  \"action\": \"subscribe\",\n  \"channel\": \"orders\"\n}".into();
        requests.push(ws);
        let local = Environment {
            name: "Local".into(),
            variables: BTreeMap::from([
                ("base_url".into(), "http://localhost:8080".into()),
                ("grpc_host".into(), "http://localhost:50051".into()),
                ("ws_url".into(), "ws://localhost:8080".into()),
            ]),
        };
        let staging = Environment {
            name: "Staging".into(),
            variables: BTreeMap::from([
                ("base_url".into(), "https://api.staging.example.com".into()),
                (
                    "grpc_host".into(),
                    "https://grpc.staging.example.com".into(),
                ),
                ("ws_url".into(), "wss://api.staging.example.com".into()),
            ]),
        };
        Self {
            version: 1,
            name: "商城服务".into(),
            requests,
            environments: vec![local, staging],
            active_environment: 0,
            active_request: 0,
            horizontal: false,
            show_schema: true,
        }
    }
    pub fn variables(&self) -> BTreeMap<String, String> {
        self.environments
            .get(self.active_environment)
            .map(|e| e.variables.clone())
            .unwrap_or_default()
    }
    pub fn normalize(&mut self) -> Result<()> {
        if self.version != 1 {
            bail!("不支持的项目版本: {}", self.version);
        }
        if self.requests.is_empty() {
            self.requests.push(Request::new(Protocol::Http, "新请求"));
        }
        if self.environments.is_empty() {
            self.environments.push(Environment {
                name: "Local".into(),
                variables: BTreeMap::new(),
            });
        }
        self.active_request = self.active_request.min(self.requests.len() - 1);
        self.active_environment = self.active_environment.min(self.environments.len() - 1);
        Ok(())
    }
}
