use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const HTTP_METHODS: [&str; 10] = [
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "CONNECT", "QUERY",
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiOperation {
    pub contract_id: Uuid,
    pub path: String,
    pub method: String,
    pub operation_id: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiFolder {
    pub contract_id: Uuid,
    /// None identifies the document root; Some identifies an original OpenAPI tag.
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub expanded: bool,
    #[serde(default)]
    pub openapi: Option<OpenApiFolder>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub folder_id: Option<Uuid>,
    #[serde(default)]
    pub openapi: Option<OpenApiOperation>,
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
            folder_id: None,
            openapi: None,
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
    pub fn encoding_preview(&self, vars: &BTreeMap<String, String>) -> Result<String> {
        let url = self.resolved_url(vars)?;
        let mut text = if self.protocol == Protocol::Http {
            format!("{} {url}\n", self.method)
        } else if self.protocol == Protocol::Grpc {
            format!("gRPC {url}\nMethod: {}\n", self.grpc_method)
        } else {
            format!("WebSocket {url}\n")
        };
        for header in self
            .headers
            .iter()
            .filter(|h| h.enabled && !h.name.is_empty())
        {
            let name = interpolate(&header.name, vars)?;
            let value = if ["authorization", "cookie", "proxy-authorization"]
                .iter()
                .any(|h| name.eq_ignore_ascii_case(h))
            {
                "[已隐藏凭据]".into()
            } else {
                interpolate(&header.value, vars)?
            };
            text.push_str(&format!("{name}: {value}\n"));
        }
        if !matches!(self.auth, Auth::None) {
            text.push_str("Authorization: [已隐藏凭据]\n");
        }
        if self.protocol == Protocol::Http
            && let Some(content_type) = self.body.kind.content_type()
            && !self
                .headers
                .iter()
                .any(|h| h.enabled && h.name.eq_ignore_ascii_case("content-type"))
        {
            text.push_str(&format!("Content-Type: {content_type}\n"));
        }
        text.push('\n');
        if self.protocol != Protocol::Http {
            text.push_str(&format!(
                "消息草稿{}\n{}",
                if self.protocol == Protocol::WebSocket {
                    "（连接后手动发送）"
                } else {
                    ""
                },
                interpolate(&self.body.text, vars)?
            ));
        } else {
            match self.body.kind {
                BodyKind::None => text.push_str("无 Body"),
                BodyKind::Json | BodyKind::Text | BodyKind::Xml => {
                    let body = interpolate(&self.body.text, vars)?;
                    if self.body.kind == BodyKind::Json {
                        serde_json::from_str::<serde_json::Value>(&body)?;
                    }
                    text.push_str(&body);
                }
                BodyKind::UrlEncoded => {
                    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
                    for field in self
                        .body
                        .fields
                        .iter()
                        .filter(|f| f.enabled && !f.name.is_empty())
                    {
                        serializer.append_pair(
                            &interpolate(&field.name, vars)?,
                            &interpolate(&field.value, vars)?,
                        );
                    }
                    text.push_str(&serializer.finish());
                }
                BodyKind::FormData => {
                    text.push_str(
                        "multipart/form-data 字段清单（boundary 与文件内容在发送时生成）\n",
                    );
                    for field in self
                        .body
                        .fields
                        .iter()
                        .filter(|f| f.enabled && !f.name.is_empty())
                    {
                        text.push_str(&format!(
                            "{}{}: {}\n",
                            interpolate(&field.name, vars)?,
                            if field.file { " [File]" } else { " [Text]" },
                            interpolate(&field.value, vars)?
                        ));
                    }
                }
                BodyKind::Binary => text.push_str(&format!(
                    "本地二进制文件: {}",
                    interpolate(&self.body.file_path, vars)?
                )),
            }
        }
        Ok(text)
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
#[serde(default)]
pub struct PaneLayout {
    pub sidebar: f32,
    pub schema: f32,
    pub request_height: f32,
    pub request_width: f32,
}
impl Default for PaneLayout {
    fn default() -> Self {
        Self {
            sidebar: 260.,
            schema: 258.,
            request_height: 360.,
            request_width: 520.,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub name: String,
    pub requests: Vec<Request>,
    #[serde(default)]
    pub folders: Vec<Folder>,
    pub environments: Vec<Environment>,
    pub active_environment: usize,
    pub active_request: usize,
    /// None migrates projects saved before request tabs were introduced.
    #[serde(default)]
    pub opened_requests: Option<Vec<Uuid>>,
    #[serde(default)]
    pub panes: PaneLayout,
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
                "QUERY" => "检索订单",
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
                "QUERY" => {
                    r.body.kind = BodyKind::Json;
                    r.body.text =
                        "{\n  \"filter\": {\"status\": \"paid\"},\n  \"limit\": 20\n}".into();
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
        let opened_requests = Some(vec![requests[0].id]);
        Self {
            version: 1,
            name: "商城服务".into(),
            requests,
            folders: vec![],
            environments: vec![local, staging],
            active_environment: 0,
            active_request: 0,
            opened_requests,
            panes: PaneLayout::default(),
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
    pub fn empty(name: impl Into<String>) -> Self {
        let mut project = Self::examples();
        project.name = name.into();
        project.requests.clear();
        project.opened_requests = Some(vec![]);
        project
    }
    pub fn tabs(&self) -> &[Uuid] {
        self.opened_requests.as_deref().unwrap_or_default()
    }
    pub fn open_request(&mut self, id: Uuid) {
        if let Some(index) = self.requests.iter().position(|r| r.id == id) {
            let tabs = self.opened_requests.get_or_insert_with(Vec::new);
            if !tabs.contains(&id) {
                tabs.push(id);
            }
            self.active_request = index;
        }
    }
    pub fn close_request(&mut self, id: Uuid) {
        let active = self.requests.get(self.active_request).map(|r| r.id);
        let tabs = self.opened_requests.get_or_insert_with(Vec::new);
        if let Some(index) = tabs.iter().position(|tab| *tab == id) {
            tabs.remove(index);
            if active == Some(id)
                && let Some(next) = tabs.get(index.min(tabs.len().saturating_sub(1))).copied()
                && let Some(index) = self.requests.iter().position(|r| r.id == next)
            {
                self.active_request = index;
            }
        }
    }
    pub fn delete_request(&mut self, id: Uuid) -> Option<(usize, Request)> {
        let index = self.requests.iter().position(|r| r.id == id)?;
        self.close_request(id);
        let active = self.requests.get(self.active_request).map(|r| r.id);
        let removed = self.requests.remove(index);
        self.active_request = self
            .requests
            .iter()
            .position(|r| Some(r.id) == active)
            .unwrap_or(0);
        Some((index, removed))
    }
    pub fn normalize(&mut self) -> Result<()> {
        if self.version != 1 {
            bail!("不支持的项目版本: {}", self.version);
        }
        let mut ids = std::collections::HashSet::new();
        if self.requests.iter().any(|r| !ids.insert(r.id)) {
            bail!("项目含有重复请求 ID，原文件未修改");
        }
        self.validate_folders()?;
        if self.environments.is_empty() {
            self.environments.push(Environment {
                name: "Local".into(),
                variables: BTreeMap::new(),
            });
        }
        self.active_request = self
            .active_request
            .min(self.requests.len().saturating_sub(1));
        let active = self.requests.get(self.active_request).map(|r| r.id);
        let tabs = self
            .opened_requests
            .get_or_insert_with(|| active.into_iter().collect());
        let mut seen = std::collections::HashSet::new();
        tabs.retain(|id| ids.contains(id) && seen.insert(*id));
        if !tabs.contains(&active.unwrap_or(Uuid::nil()))
            && let Some(id) = tabs.first()
        {
            self.active_request = self.requests.iter().position(|r| &r.id == id).unwrap_or(0);
        }
        self.active_environment = self.active_environment.min(self.environments.len() - 1);
        Ok(())
    }
}
