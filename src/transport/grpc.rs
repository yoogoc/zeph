use super::{SessionCommand, SessionEvent};
use crate::model::{Auth, Request, interpolate};
use anyhow::{Context, Result, bail};
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor, MethodDescriptor};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tonic::{
    Status,
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
    transport::{Channel, ClientTlsConfig, Endpoint},
};

pub fn example_pool() -> DescriptorPool {
    DescriptorPool::decode(include_bytes!(concat!(env!("OUT_DIR"), "/order.bin")).as_slice())
        .expect("bundled descriptor")
}

#[derive(Clone, Debug)]
pub struct RpcMethod {
    pub path: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    pub input: String,
    pub output: String,
}
impl RpcMethod {
    pub fn kind(&self) -> &'static str {
        match (self.client_streaming, self.server_streaming) {
            (false, false) => "Unary",
            (false, true) => "服务端流",
            (true, false) => "客户端流",
            (true, true) => "双向流",
        }
    }
}
pub fn methods(pool: &DescriptorPool) -> Vec<RpcMethod> {
    pool.services()
        .flat_map(|s| {
            s.methods()
                .map(|m| RpcMethod {
                    path: format!("{}/{}", s.full_name(), m.name()),
                    client_streaming: m.is_client_streaming(),
                    server_streaming: m.is_server_streaming(),
                    input: m.input().full_name().into(),
                    output: m.output().full_name().into(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
pub fn load_proto(path: &Path) -> Result<DescriptorPool> {
    if path.extension().is_some_and(|ext| ext == "proto") {
        let parent = path.parent().unwrap_or(Path::new("."));
        let set = protox::compile([path], [parent])?;
        Ok(DescriptorPool::from_file_descriptor_set(set)?)
    } else {
        Ok(DescriptorPool::decode(std::fs::read(path)?.as_slice())?)
    }
}
pub fn method(pool: &DescriptorPool, path: &str) -> Result<MethodDescriptor> {
    let (service, name) = path
        .trim_start_matches('/')
        .rsplit_once('/')
        .context("方法应为 package.Service/Method")?;
    pool.get_service_by_name(service)
        .and_then(|s| s.methods().find(|m| m.name() == name))
        .context("接口定义中没有这个 gRPC method")
}
pub fn parse_message(descriptor: MessageDescriptor, text: &str) -> Result<DynamicMessage> {
    let mut decoder = serde_json::Deserializer::from_str(text);
    let message = DynamicMessage::deserialize(descriptor, &mut decoder)?;
    decoder.end()?;
    Ok(message)
}
pub fn message_template(descriptor: MessageDescriptor) -> String {
    let mut object = serde_json::Map::new();
    for field in descriptor.fields() {
        let value = if field.is_list() {
            serde_json::json!([])
        } else if field.is_map() {
            serde_json::json!({})
        } else {
            match field.kind() {
                prost_reflect::Kind::Bool => serde_json::json!(false),
                prost_reflect::Kind::String | prost_reflect::Kind::Bytes => serde_json::json!(""),
                prost_reflect::Kind::Message(_) => serde_json::json!({}),
                prost_reflect::Kind::Enum(e) => serde_json::json!(
                    e.values()
                        .next()
                        .map(|v| v.name().to_owned())
                        .unwrap_or_default()
                ),
                prost_reflect::Kind::Int64
                | prost_reflect::Kind::Uint64
                | prost_reflect::Kind::Sint64
                | prost_reflect::Kind::Fixed64
                | prost_reflect::Kind::Sfixed64 => serde_json::json!("0"),
                _ => serde_json::json!(0),
            }
        };
        if field.containing_oneof().is_none() {
            object.insert(field.json_name().to_owned(), value);
        }
    }
    serde_json::to_string_pretty(&object).unwrap_or_else(|_| "{}".into())
}
/// Local example values only apply to the bundled demo package.
pub fn request_template(descriptor: MessageDescriptor) -> String {
    let example = match descriptor.full_name() {
        "shop.v1.GetOrderRequest" => {
            Some(serde_json::json!({"orderId":"ord_1024","includeItems":true}))
        }
        "shop.v1.WatchOrdersRequest" => {
            Some(serde_json::json!({"customerId":"cus_1024","includeHistory":false}))
        }
        "shop.v1.CreateOrderRequest" => {
            Some(serde_json::json!({"customerId":"cus_1024","sku":"zeph-pro","quantity":1}))
        }
        "shop.v1.SyncOrderRequest" => {
            Some(serde_json::json!({"orderId":"ord_1024","status":"paid","version":"1"}))
        }
        _ => None,
    };
    example
        .map(|v| serde_json::to_string_pretty(&v).expect("example JSON"))
        .unwrap_or_else(|| message_template(descriptor))
}
async fn channel(request: &Request, vars: &BTreeMap<String, String>) -> Result<Channel> {
    if !request.proxy.is_empty() {
        bail!("gRPC 代理尚未实现");
    }
    if !request.tls_verify {
        bail!("gRPC 当前使用系统证书校验");
    }
    let raw = request.resolved_url(vars)?;
    let endpoint = if raw.contains("://") {
        raw
    } else {
        format!("http://{raw}")
    };
    let secure = endpoint.starts_with("https://");
    let mut endpoint = Endpoint::from_shared(endpoint)?
        .connect_timeout(std::time::Duration::from_secs(request.timeout_secs.max(1)));
    if secure {
        endpoint = endpoint.tls_config(ClientTlsConfig::new().with_native_roots())?;
    }
    Ok(endpoint.connect().await?)
}
fn metadata<T>(
    request: &Request,
    vars: &BTreeMap<String, String>,
    value: T,
) -> Result<tonic::Request<T>> {
    use tonic::metadata::{Ascii, Binary, MetadataKey, MetadataValue};
    let mut out = tonic::Request::new(value);
    out.set_timeout(std::time::Duration::from_secs(request.timeout_secs.max(1)));
    for header in request
        .headers
        .iter()
        .filter(|h| h.enabled && !h.name.is_empty())
    {
        let value = interpolate(&header.value, vars)?;
        if header.name.ends_with("-bin") {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD.decode(value)?;
            out.metadata_mut().append_bin(
                MetadataKey::<Binary>::from_bytes(header.name.as_bytes())?,
                MetadataValue::<Binary>::from_bytes(&bytes),
            );
        } else {
            out.metadata_mut().append(
                MetadataKey::<Ascii>::from_bytes(header.name.as_bytes())?,
                MetadataValue::try_from(value.as_str())?,
            );
        }
    }
    let authorization = match &request.auth {
        Auth::None => None,
        Auth::Bearer(token) => Some(format!("Bearer {}", interpolate(token, vars)?)),
        Auth::Basic { username, password } => {
            use base64::Engine;
            Some(format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!(
                    "{}:{}",
                    interpolate(username, vars)?,
                    interpolate(password, vars)?
                ))
            ))
        }
    };
    if let Some(value) = authorization {
        out.metadata_mut()
            .insert("authorization", MetadataValue::try_from(value.as_str())?);
    }
    Ok(out)
}
fn metadata_pairs(map: &tonic::metadata::MetadataMap) -> Vec<(String, String)> {
    use tonic::metadata::KeyAndValueRef;
    map.iter()
        .map(|entry| match entry {
            KeyAndValueRef::Ascii(k, v) => {
                (k.to_string(), v.to_str().unwrap_or("[binary]").to_owned())
            }
            KeyAndValueRef::Binary(k, v) => (
                k.to_string(),
                format!("[binary · {} bytes]", v.as_encoded_bytes().len()),
            ),
        })
        .collect()
}

macro_rules! discover_version {
    ($module:path,$channel:expr,$request:expr,$vars:expr) => {{
        use pb::{
            server_reflection_request::MessageRequest, server_reflection_response::MessageResponse,
        };
        use $module as pb;
        let mut client = pb::server_reflection_client::ServerReflectionClient::new($channel);
        let query = pb::ServerReflectionRequest {
            host: String::new(),
            message_request: Some(MessageRequest::ListServices(String::new())),
        };
        let mut stream = client
            .server_reflection_info(metadata($request, $vars, futures::stream::iter([query]))?)
            .await?
            .into_inner();
        let response = stream
            .message()
            .await?
            .context("Reflection 未返回服务列表")?;
        let services = match response.message_response {
            Some(MessageResponse::ListServicesResponse(r)) => r.service,
            Some(MessageResponse::ErrorResponse(e)) => bail!("Reflection: {}", e.error_message),
            _ => bail!("Reflection 响应格式不正确"),
        };
        let mut set = prost_reflect::prost_types::FileDescriptorSet::default();
        let mut seen = HashSet::new();
        for service in services
            .into_iter()
            .filter(|s| !s.name.starts_with("grpc.reflection."))
        {
            let query = pb::ServerReflectionRequest {
                host: String::new(),
                message_request: Some(MessageRequest::FileContainingSymbol(service.name)),
            };
            let mut stream = client
                .server_reflection_info(metadata($request, $vars, futures::stream::iter([query]))?)
                .await?
                .into_inner();
            let response = stream
                .message()
                .await?
                .context("Reflection 缺少 descriptor")?;
            match response.message_response {
                Some(MessageResponse::FileDescriptorResponse(r)) => {
                    for file in r.file_descriptor_proto {
                        let file = prost_reflect::prost_types::FileDescriptorProto::decode(
                            file.as_slice(),
                        )?;
                        if seen.insert(file.name.clone()) {
                            set.file.push(file);
                        }
                    }
                }
                Some(MessageResponse::ErrorResponse(e)) => bail!("Reflection: {}", e.error_message),
                _ => bail!("Reflection 未返回文件定义"),
            }
        }
        DescriptorPool::from_file_descriptor_set(set).map_err(anyhow::Error::from)
    }};
}
pub async fn discover(
    request: &Request,
    vars: &BTreeMap<String, String>,
) -> Result<DescriptorPool> {
    let connection = channel(request, vars).await?;
    let current: Result<DescriptorPool> =
        async { discover_version!(tonic_reflection::pb::v1, connection.clone(), request, vars) }
            .await;
    match current {
        Ok(pool) => Ok(pool),
        Err(first) => {
            let legacy: Result<DescriptorPool> = async {
                discover_version!(tonic_reflection::pb::v1alpha, connection, request, vars)
            }
            .await;
            legacy.with_context(|| format!("Reflection v1 失败: {first}; v1alpha 也无法读取"))
        }
    }
}

#[derive(Clone)]
struct DynamicCodec {
    output: MessageDescriptor,
}
struct DynamicEncoder;
struct DynamicDecoder(MessageDescriptor);
impl Codec for DynamicCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = DynamicEncoder;
    type Decoder = DynamicDecoder;
    fn encoder(&mut self) -> Self::Encoder {
        DynamicEncoder
    }
    fn decoder(&mut self) -> Self::Decoder {
        DynamicDecoder(self.output.clone())
    }
}
impl Encoder for DynamicEncoder {
    type Item = DynamicMessage;
    type Error = Status;
    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut EncodeBuf<'_>,
    ) -> std::result::Result<(), Status> {
        item.encode(dst)
            .map_err(|e| Status::internal(e.to_string()))
    }
}
impl Decoder for DynamicDecoder {
    type Item = DynamicMessage;
    type Error = Status;
    fn decode(
        &mut self,
        src: &mut DecodeBuf<'_>,
    ) -> std::result::Result<Option<Self::Item>, Status> {
        DynamicMessage::decode(self.0.clone(), src)
            .map(Some)
            .map_err(|e| Status::internal(e.to_string()))
    }
}

pub async fn run(
    request: Request,
    vars: BTreeMap<String, String>,
    pool: DescriptorPool,
    commands: mpsc::Receiver<SessionCommand>,
    events: mpsc::Sender<SessionEvent>,
    cancel: CancellationToken,
) -> Result<()> {
    let deadline = std::time::Duration::from_secs(request.timeout_secs.max(1));
    tokio::select! {biased; _=cancel.cancelled()=>{let _=events.send(SessionEvent::Finished{code:"1 CANCELLED".into(),trailers:vec![]}).await;Ok(())},result=tokio::time::timeout(deadline,run_session(request,vars,pool,commands,events.clone(),cancel.clone()))=>result.map_err(|_|anyhow::anyhow!("4 DEADLINE_EXCEEDED · 本地 gRPC Deadline 已超时"))?}
}

async fn run_session(
    request: Request,
    vars: BTreeMap<String, String>,
    pool: DescriptorPool,
    commands: mpsc::Receiver<SessionCommand>,
    events: mpsc::Sender<SessionEvent>,
    cancel: CancellationToken,
) -> Result<()> {
    let descriptor = method(&pool, &request.grpc_method)?;
    let input = descriptor.input();
    let connection = tokio::select! {_=cancel.cancelled()=>return Ok(()),result=channel(&request,&vars)=>result?};
    let mut client =
        tonic::client::Grpc::new(connection).max_decoding_message_size(8 * 1024 * 1024);
    client.ready().await?;
    let path = tonic::codegen::http::uri::PathAndQuery::from_maybe_shared(format!(
        "/{}",
        request.grpc_method.trim_start_matches('/')
    ))?;
    let codec = DynamicCodec {
        output: descriptor.output(),
    };
    events.send(SessionEvent::Opened).await?;
    let work = async {
        if !descriptor.is_client_streaming() {
            let text = interpolate(&request.body.text, &vars)?;
            let message = parse_message(input, &text)?;
            if !descriptor.is_server_streaming() {
                let response = client
                    .unary(metadata(&request, &vars, message)?, path, codec)
                    .await?;
                events
                    .send(SessionEvent::Metadata(metadata_pairs(response.metadata())))
                    .await?;
                events
                    .send(SessionEvent::Received(serde_json::to_string_pretty(
                        response.get_ref(),
                    )?))
                    .await?;
                events
                    .send(SessionEvent::Finished {
                        code: "0 OK".into(),
                        trailers: vec![],
                    })
                    .await?;
            } else {
                let response = client
                    .server_streaming(metadata(&request, &vars, message)?, path, codec)
                    .await?;
                events
                    .send(SessionEvent::Metadata(metadata_pairs(response.metadata())))
                    .await?;
                receive(response.into_inner(), &events).await?;
            }
        } else {
            let sender_events = events.clone();
            let stream = futures::stream::unfold(
                (commands, input, sender_events, vars.clone()),
                |(mut commands, input, events, vars)| async move {
                    loop {
                        match commands.recv().await {
                            Some(SessionCommand::Send(text)) => {
                                match interpolate(&text, &vars)
                                    .and_then(|t| parse_message(input.clone(), &t))
                                {
                                    Ok(message) => {
                                        let _ = events.send(SessionEvent::Sent(text)).await;
                                        return Some((message, (commands, input, events, vars)));
                                    }
                                    Err(error) => {
                                        let _ = events
                                            .send(SessionEvent::Failed(format!(
                                                "消息未发送: {error}"
                                            )))
                                            .await;
                                    }
                                }
                            }
                            _ => return None,
                        }
                    }
                },
            );
            if descriptor.is_server_streaming() {
                let response = client
                    .streaming(metadata(&request, &vars, stream)?, path, codec)
                    .await?;
                events
                    .send(SessionEvent::Metadata(metadata_pairs(response.metadata())))
                    .await?;
                receive(response.into_inner(), &events).await?;
            } else {
                let response = client
                    .client_streaming(metadata(&request, &vars, stream)?, path, codec)
                    .await?;
                events
                    .send(SessionEvent::Metadata(metadata_pairs(response.metadata())))
                    .await?;
                events
                    .send(SessionEvent::Received(serde_json::to_string_pretty(
                        response.get_ref(),
                    )?))
                    .await?;
                events
                    .send(SessionEvent::Finished {
                        code: "0 OK".into(),
                        trailers: vec![],
                    })
                    .await?;
            }
        }
        Ok(())
    };
    tokio::select! {_=cancel.cancelled()=>{let _=events.send(SessionEvent::Finished{code:"1 CANCELLED".into(),trailers:vec![]}).await;Ok(())},result=work=>result}
}
async fn receive(
    mut stream: tonic::Streaming<DynamicMessage>,
    events: &mpsc::Sender<SessionEvent>,
) -> Result<()> {
    while let Some(message) = stream.message().await? {
        events
            .send(SessionEvent::Received(serde_json::to_string_pretty(
                &message,
            )?))
            .await?;
    }
    let trailers = stream
        .trailers()
        .await?
        .map(|m| metadata_pairs(&m))
        .unwrap_or_default();
    events
        .send(SessionEvent::Finished {
            code: "0 OK".into(),
            trailers,
        })
        .await?;
    Ok(())
}
