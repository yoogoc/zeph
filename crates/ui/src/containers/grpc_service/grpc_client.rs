//! Dynamic gRPC client: runtime proto compilation + reflection-free unary calls.
//!
//! Compiles a `.proto` source string at runtime (via `protox`), builds a
//! `DescriptorPool` (via `prost-reflect`), and issues unary calls over `tonic`'s
//! low-level `Grpc` client using a raw-bytes codec — so no proto types need to be
//! known at compile time. Request/response messages are exchanged as JSON.

use anyhow::{anyhow, Context, Result};
use bytes::{Buf, BufMut};
use prost::Message as _;
use prost_reflect::{DescriptorPool, DynamicMessage, MethodDescriptor, SerializeOptions};
use std::io::Write;
use tonic::codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder};
use tonic::Status;

/// A compiled proto: the descriptor pool plus the discovered methods.
pub struct ProtoDescriptors {
    pool: DescriptorPool,
}

/// Identifies a single RPC method, discovered from the compiled proto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodInfo {
    /// Fully-qualified service name, e.g. `pkg.Greeter`.
    pub service: String,
    /// Method name, e.g. `SayHello`.
    pub method: String,
    /// Whether the client streams requests.
    pub client_streaming: bool,
    /// Whether the server streams responses.
    pub server_streaming: bool,
}

impl MethodInfo {
    /// The gRPC path, e.g. `/pkg.Greeter/SayHello`.
    pub fn path(&self) -> String {
        format!("/{}/{}", self.service, self.method)
    }
}

impl ProtoDescriptors {
    /// Compiles `.proto` source text into a descriptor pool.
    ///
    /// `protox` only compiles files on disk, so the source is written to a
    /// temporary file (cleaned up before returning). `include_dirs` are extra
    /// import roots for resolving `import` statements (proto library paths).
    pub fn compile(proto_source: &str, include_dirs: &[std::path::PathBuf]) -> Result<Self> {
        let tmp_dir = std::env::temp_dir();
        let file_name = format!("zeph-grpc-{}.proto", uuid::Uuid::new_v4());
        let file_path = tmp_dir.join(&file_name);

        {
            let mut f = std::fs::File::create(&file_path)
                .with_context(|| format!("creating temp proto {}", file_path.display()))?;
            f.write_all(proto_source.as_bytes())?;
        }

        // Include the temp dir itself so the file resolves, plus any user roots.
        let mut includes: Vec<std::path::PathBuf> = vec![tmp_dir.clone()];
        includes.extend(include_dirs.iter().cloned());

        let result = protox::compile([&file_path], &includes);
        let _ = std::fs::remove_file(&file_path);

        let fds = result.map_err(|e| anyhow!("proto compile error: {e}"))?;
        let pool = DescriptorPool::from_file_descriptor_set(fds)
            .map_err(|e| anyhow!("descriptor pool error: {e}"))?;
        Ok(Self { pool })
    }

    /// Lists all RPC methods across all services in the proto.
    pub fn methods(&self) -> Vec<MethodInfo> {
        let mut out = Vec::new();
        for service in self.pool.services() {
            for method in service.methods() {
                out.push(MethodInfo {
                    service: service.full_name().to_string(),
                    method: method.name().to_string(),
                    client_streaming: method.is_client_streaming(),
                    server_streaming: method.is_server_streaming(),
                });
            }
        }
        out
    }

    /// Looks up a method descriptor by service full-name and method name.
    fn method_descriptor(&self, service: &str, method: &str) -> Result<MethodDescriptor> {
        self.pool
            .services()
            .find(|s| s.full_name() == service)
            .and_then(|s| s.methods().find(|m| m.name() == method))
            .ok_or_else(|| anyhow!("method {service}/{method} not found in proto"))
    }

    /// Encodes a JSON request body into protobuf bytes for the method's input type.
    fn encode_request(&self, method: &MethodDescriptor, json: &str) -> Result<Vec<u8>> {
        let input = method.input();
        let mut de = serde_json::Deserializer::from_str(json);
        let msg = DynamicMessage::deserialize(input, &mut de)
            .map_err(|e| anyhow!("invalid request JSON for {}: {e}", method.full_name()))?;
        de.end().map_err(|e| anyhow!("trailing JSON: {e}"))?;
        Ok(msg.encode_to_vec())
    }

    /// Decodes protobuf response bytes into pretty JSON for the method's output type.
    fn decode_response(&self, method: &MethodDescriptor, bytes: &[u8]) -> Result<String> {
        let output = method.output();
        let msg =
            DynamicMessage::decode(output, bytes).map_err(|e| anyhow!("decoding response: {e}"))?;
        let options = SerializeOptions::new().skip_default_fields(false);
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::pretty(&mut buf);
        msg.serialize_with_options(&mut ser, &options)
            .map_err(|e| anyhow!("serializing response: {e}"))?;
        Ok(String::from_utf8(buf)?)
    }

    /// Encodes a JSON request message into protobuf bytes for the given method.
    pub fn encode_message(&self, service: &str, method: &str, json: &str) -> Result<Vec<u8>> {
        let md = self.method_descriptor(service, method)?;
        self.encode_request(&md, json)
    }
}

/// Performs a unary gRPC call and returns the response as pretty JSON.
///
/// `endpoint` is the server URL (e.g. `http://localhost:50051`), `metadata` are
/// request headers (key/value), `request_json` is the request message as JSON.
pub async fn call_unary(
    descriptors: &ProtoDescriptors,
    endpoint: &str,
    service: &str,
    method: &str,
    metadata: &[(String, String)],
    request_json: &str,
) -> Result<String> {
    let method_desc = descriptors.method_descriptor(service, method)?;
    let info = MethodInfo {
        service: service.to_string(),
        method: method.to_string(),
        client_streaming: method_desc.is_client_streaming(),
        server_streaming: method_desc.is_server_streaming(),
    };
    let request_bytes = descriptors.encode_request(&method_desc, request_json)?;

    let channel = tonic::transport::Channel::from_shared(endpoint.to_string())
        .map_err(|e| anyhow!("invalid endpoint {endpoint}: {e}"))?
        .connect()
        .await
        .with_context(|| format!("connecting to {endpoint}"))?;

    let mut client = tonic::client::Grpc::new(channel);
    client
        .ready()
        .await
        .map_err(|e| anyhow!("service not ready: {e}"))?;

    let mut request = tonic::Request::new(request_bytes);
    for (k, v) in metadata {
        if let (Ok(key), Ok(val)) = (
            tonic::metadata::MetadataKey::from_bytes(k.as_bytes()),
            v.parse::<tonic::metadata::MetadataValue<_>>(),
        ) {
            request.metadata_mut().insert(key, val);
        }
    }

    let path: tonic::codegen::http::uri::PathAndQuery = info
        .path()
        .parse()
        .map_err(|e| anyhow!("invalid method path: {e}"))?;

    let response = client
        .unary(request, path, RawCodec)
        .await
        .map_err(|status| anyhow!("gRPC call failed: {status}"))?;

    descriptors.decode_response(&method_desc, response.get_ref())
}

/// A response message from a streaming call: decoded JSON, or an error string.
pub type StreamItem = Result<String, String>;

/// Drives a streaming gRPC call (client / server / bidi).
///
/// `request_rx` yields already-encoded protobuf request messages (encode them
/// via [`ProtoDescriptors::encode_message`] before sending). For server-only
/// streaming, push the single request then drop the sender. Each decoded
/// response message (as pretty JSON) is sent to `response_tx`; the call ends
/// when the server closes the response stream. Returns `Ok(())` on clean
/// completion or `Err` if the connection / call setup failed.
pub async fn call_stream(
    descriptors: std::sync::Arc<ProtoDescriptors>,
    endpoint: String,
    service: String,
    method: String,
    metadata: Vec<(String, String)>,
    request_rx: futures::channel::mpsc::Receiver<Vec<u8>>,
    response_tx: futures::channel::mpsc::Sender<StreamItem>,
) -> Result<()> {
    use futures::SinkExt;

    let method_desc = descriptors.method_descriptor(&service, &method)?;
    let path: tonic::codegen::http::uri::PathAndQuery = MethodInfo {
        service: service.clone(),
        method: method.clone(),
        client_streaming: method_desc.is_client_streaming(),
        server_streaming: method_desc.is_server_streaming(),
    }
    .path()
    .parse()
    .map_err(|e| anyhow!("invalid method path: {e}"))?;

    let channel = tonic::transport::Channel::from_shared(endpoint.clone())
        .map_err(|e| anyhow!("invalid endpoint {endpoint}: {e}"))?
        .connect()
        .await
        .with_context(|| format!("connecting to {endpoint}"))?;

    let mut client = tonic::client::Grpc::new(channel);
    client
        .ready()
        .await
        .map_err(|e| anyhow!("service not ready: {e}"))?;

    let mut request = tonic::Request::new(request_rx);
    for (k, v) in &metadata {
        if let (Ok(key), Ok(val)) = (
            tonic::metadata::MetadataKey::from_bytes(k.as_bytes()),
            v.parse::<tonic::metadata::MetadataValue<_>>(),
        ) {
            request.metadata_mut().insert(key, val);
        }
    }

    let response = client
        .streaming(request, path, RawCodec)
        .await
        .map_err(|status| anyhow!("gRPC call failed: {status}"))?;
    let mut stream = response.into_inner();

    let mut tx = response_tx;
    loop {
        match stream.message().await {
            Ok(Some(bytes)) => {
                let item = descriptors
                    .decode_response(&method_desc, &bytes)
                    .map_err(|e| e.to_string());
                if tx.send(item).await.is_err() {
                    break; // receiver dropped (cancelled)
                }
            }
            Ok(None) => break, // server closed the stream
            Err(status) => {
                let _ = tx.send(Err(format!("stream error: {status}"))).await;
                break;
            }
        }
    }
    Ok(())
}

// ===== Server reflection: discover services/methods from a running server. =====

/// Minimal gRPC server reflection (v1) proto, compiled at runtime to drive the
/// reflection RPC dynamically — same mechanism as user protos.
const REFLECTION_PROTO: &str = r#"
syntax = "proto3";
package grpc.reflection.v1;
message ServerReflectionRequest {
  string host = 1;
  oneof message_request {
    string file_by_filename = 3;
    string file_containing_symbol = 4;
    string list_services = 7;
  }
}
message ServerReflectionResponse {
  string valid_host = 1;
  ServerReflectionRequest original_request = 2;
  oneof message_response {
    FileDescriptorResponse file_descriptor_response = 4;
    ListServiceResponse list_services_response = 6;
    ErrorResponse error_response = 7;
  }
}
message FileDescriptorResponse { repeated bytes file_descriptor_proto = 1; }
message ServiceResponse { string name = 1; }
message ListServiceResponse { repeated ServiceResponse service = 1; }
message ErrorResponse { int32 error_code = 1; string error_message = 2; }
service ServerReflection {
  rpc ServerReflectionInfo (stream ServerReflectionRequest)
      returns (stream ServerReflectionResponse);
}
"#;

const REFLECTION_PATH: &str = "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo";

/// Connects to `endpoint`, lists all services via reflection, fetches their file
/// descriptors, and returns the compiled descriptors plus discovered methods.
pub async fn reflect(endpoint: &str) -> Result<(ProtoDescriptors, Vec<MethodInfo>)> {
    let refl = ProtoDescriptors::compile(REFLECTION_PROTO, &[])?;
    let req_msg = refl
        .pool
        .get_message_by_name("grpc.reflection.v1.ServerReflectionRequest")
        .ok_or_else(|| anyhow!("reflection request type missing"))?;
    let resp_msg = refl
        .pool
        .get_message_by_name("grpc.reflection.v1.ServerReflectionResponse")
        .ok_or_else(|| anyhow!("reflection response type missing"))?;

    let channel = tonic::transport::Channel::from_shared(endpoint.to_string())
        .map_err(|e| anyhow!("invalid endpoint {endpoint}: {e}"))?
        .connect()
        .await
        .with_context(|| format!("connecting to {endpoint}"))?;
    let path: tonic::codegen::http::uri::PathAndQuery = REFLECTION_PATH
        .parse()
        .map_err(|e| anyhow!("bad path: {e}"))?;

    // Step 1: list services.
    let list_req = encode_reflection_request(&req_msg, "list_services", "")?;
    let services = {
        let resp = stream_reflection(&channel, &path, vec![list_req]).await?;
        let mut names = Vec::new();
        for bytes in resp {
            let msg = DynamicMessage::decode(resp_msg.clone(), bytes.as_slice())?;
            if let Some(list) = field_message(&msg, "list_services_response") {
                if let Some(prost_reflect::Value::List(svcs)) =
                    list.get_field_by_name("service").as_deref()
                {
                    for s in svcs {
                        if let Some(m) = s.as_message() {
                            if let Some(name) = m
                                .get_field_by_name("name")
                                .and_then(|v| v.as_str().map(String::from))
                            {
                                names.push(name);
                            }
                        }
                    }
                }
            }
        }
        names
    };

    // Step 2: fetch the file descriptor for each service, then recursively fetch
    // any imported files the server didn't already include. Some servers only
    // return the file directly containing the symbol (not its transitive
    // dependencies), so we resolve the missing `dependency` entries explicitly
    // via `file_by_filename` until the import graph is closed. Only then can
    // prost-reflect register the batch (it requires every dependency present).
    use std::collections::{HashMap, HashSet};
    let mut files: HashMap<String, prost_types::FileDescriptorProto> = HashMap::new();
    let mut requested_files: HashSet<String> = HashSet::new();

    for svc in &services {
        if svc.starts_with("grpc.reflection.") {
            continue;
        }
        let req = encode_reflection_request(&req_msg, "file_containing_symbol", svc)?;
        let resp = stream_reflection(&channel, &path, vec![req]).await?;
        collect_file_descriptors(&resp, &resp_msg, &mut files)?;
    }

    // Resolve transitive imports that weren't bundled in the responses above.
    loop {
        let missing: Vec<String> = files
            .values()
            .flat_map(|f| f.dependency.iter().cloned())
            .filter(|dep| !files.contains_key(dep) && !requested_files.contains(dep))
            .collect();
        if missing.is_empty() {
            break;
        }
        for dep in missing {
            requested_files.insert(dep.clone());
            let req = encode_reflection_request(&req_msg, "file_by_filename", &dep)?;
            // Best-effort: a server may not know a file by name; skip on failure.
            if let Ok(resp) = stream_reflection(&channel, &path, vec![req]).await {
                collect_file_descriptors(&resp, &resp_msg, &mut files)?;
            }
        }
    }

    // Register files tolerantly, in dependency order: a single missing import
    // (e.g. a gateway/annotation proto the server doesn't expose) must not drop
    // every service. Repeatedly register files whose dependencies are all
    // already present; skip the rest. Services in skipped files won't appear,
    // but everything resolvable still does.
    let pool = register_files_tolerant(files);

    let descriptors = ProtoDescriptors { pool };
    let methods = descriptors.methods();
    if methods.is_empty() {
        return Err(anyhow!(
            "reflection returned no usable methods (unresolved proto imports, \
             or the server does not support reflection)"
        ));
    }
    Ok((descriptors, methods))
}

/// Registers file descriptors in dependency order, tolerating unresolved imports.
///
/// First adds every file whose dependencies are all present. For the remainder
/// (files importing something the server never provided — commonly gateway or
/// annotation protos), it strips the missing `dependency` entries and retries,
/// so the owning services still register. Returns the resulting pool.
fn register_files_tolerant(
    mut files: std::collections::HashMap<String, prost_types::FileDescriptorProto>,
) -> DescriptorPool {
    use std::collections::HashSet;
    let mut pool = DescriptorPool::new();
    let mut added: HashSet<String> = HashSet::new();
    let available: HashSet<String> = files.keys().cloned().collect();

    loop {
        // Register files whose dependencies are all already added.
        let ready: Vec<String> = files
            .iter()
            .filter(|(_, f)| f.dependency.iter().all(|d| added.contains(d)))
            .map(|(name, _)| name.clone())
            .collect();

        if ready.is_empty() {
            // No more directly-resolvable files. Strip dependencies that the
            // server never provided at all, so files blocked only by truly
            // missing imports can still register, then continue.
            let mut progressed = false;
            for f in files.values_mut() {
                let before = f.dependency.len();
                f.dependency.retain(|d| available.contains(d));
                if f.dependency.len() != before {
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
            continue;
        }

        for name in ready {
            if let Some(fdp) = files.remove(&name) {
                if pool.add_file_descriptor_proto(fdp).is_ok() {
                    added.insert(name);
                }
            }
        }
    }
    pool
}

/// Parses `FileDescriptorResponse` messages from reflection responses, decoding
/// each contained `FileDescriptorProto` and inserting it (de-duplicated by name).
fn collect_file_descriptors(
    responses: &[Vec<u8>],
    resp_msg: &prost_reflect::MessageDescriptor,
    files: &mut std::collections::HashMap<String, prost_types::FileDescriptorProto>,
) -> Result<()> {
    for bytes in responses {
        let msg = DynamicMessage::decode(resp_msg.clone(), bytes.as_slice())?;
        if let Some(fdr) = field_message(&msg, "file_descriptor_response") {
            if let Some(prost_reflect::Value::List(protos)) =
                fdr.get_field_by_name("file_descriptor_proto").as_deref()
            {
                for f in protos {
                    if let Some(b) = f.as_bytes() {
                        if let Ok(fdp) = prost_types::FileDescriptorProto::decode(b.as_ref()) {
                            files.entry(fdp.name().to_string()).or_insert(fdp);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn encode_reflection_request(
    req_msg: &prost_reflect::MessageDescriptor,
    field: &str,
    value: &str,
) -> Result<Vec<u8>> {
    let mut msg = DynamicMessage::new(req_msg.clone());
    let fd = req_msg
        .get_field_by_name(field)
        .ok_or_else(|| anyhow!("reflection field {field} missing"))?;
    msg.set_field(&fd, prost_reflect::Value::String(value.to_string()));
    Ok(msg.encode_to_vec())
}

/// Returns the nested message of a oneof field, if set.
fn field_message(msg: &DynamicMessage, field: &str) -> Option<DynamicMessage> {
    msg.get_field_by_name(field)
        .and_then(|v| v.as_message().cloned())
}

/// Opens a bidi reflection stream, sends `requests`, collects all response bytes.
///
/// Each call uses a fresh `Grpc` client on a clone of the channel and re-runs
/// `ready()` — reusing one client across multiple low-level `streaming` calls
/// panics in tonic's tower buffer (`send_item` without `poll_reserve`). The
/// request items are delivered through an mpsc channel so the request stream is
/// driven lazily by the transport rather than being already-complete on send.
async fn stream_reflection(
    channel: &tonic::transport::Channel,
    path: &tonic::codegen::http::uri::PathAndQuery,
    requests: Vec<Vec<u8>>,
) -> Result<Vec<Vec<u8>>> {
    use futures::StreamExt;

    let mut client = tonic::client::Grpc::new(channel.clone());
    client
        .ready()
        .await
        .map_err(|e| anyhow!("reflection service not ready: {e}"))?;

    use futures::SinkExt;
    let (mut tx, rx) = futures::channel::mpsc::channel::<Vec<u8>>(requests.len().max(1));
    for req in requests {
        // Capacity reserved above; sending cannot fail here.
        let _ = tx.send(req).await;
    }
    drop(tx); // close the request stream so the server sees end-of-input
    let req_stream = rx;

    let response = client
        .streaming(tonic::Request::new(req_stream), path.clone(), RawCodec)
        .await
        .map_err(|s| anyhow!("reflection call failed: {s}"))?;
    let mut stream = response.into_inner();
    let mut out = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(bytes) => out.push(bytes),
            Err(s) => return Err(anyhow!("reflection stream error: {s}")),
        }
    }
    Ok(out)
}

// ===== Raw-bytes codec: ships pre-encoded protobuf without compile-time types. =====

#[derive(Clone)]
struct RawCodec;

impl Codec for RawCodec {
    type Encode = Vec<u8>;
    type Decode = Vec<u8>;
    type Encoder = RawCodec;
    type Decoder = RawCodec;

    fn encoder(&mut self) -> Self::Encoder {
        RawCodec
    }
    fn decoder(&mut self) -> Self::Decoder {
        RawCodec
    }
}

impl Encoder for RawCodec {
    type Item = Vec<u8>;
    type Error = Status;

    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Status> {
        dst.put_slice(&item);
        Ok(())
    }
}

impl Decoder for RawCodec {
    type Item = Vec<u8>;
    type Error = Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Status> {
        let len = src.remaining();
        let mut buf = vec![0u8; len];
        src.copy_to_slice(&mut buf);
        Ok(Some(buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROTO: &str = r#"
        syntax = "proto3";
        package greet;
        message HelloRequest { string name = 1; int32 count = 2; }
        message HelloReply { string message = 1; }
        service Greeter {
            rpc SayHello (HelloRequest) returns (HelloReply);
            rpc Stream (HelloRequest) returns (stream HelloReply);
        }
    "#;

    #[test]
    fn compiles_and_lists_methods() {
        let d = ProtoDescriptors::compile(PROTO, &[]).expect("compile");
        let methods = d.methods();
        assert_eq!(methods.len(), 2);

        let say = methods.iter().find(|m| m.method == "SayHello").unwrap();
        assert_eq!(say.service, "greet.Greeter");
        assert_eq!(say.path(), "/greet.Greeter/SayHello");
        assert!(!say.client_streaming && !say.server_streaming);

        let stream = methods.iter().find(|m| m.method == "Stream").unwrap();
        assert!(stream.server_streaming);
    }

    #[test]
    fn json_request_round_trips_through_protobuf() {
        let d = ProtoDescriptors::compile(PROTO, &[]).expect("compile");
        let md = d.method_descriptor("greet.Greeter", "SayHello").unwrap();

        // JSON -> protobuf bytes -> back to JSON via the response path (same type
        // shape) proves DynamicMessage encode/decode + serde both work.
        let bytes = d
            .encode_request(&md, r#"{"name":"ada","count":3}"#)
            .expect("encode");
        assert!(!bytes.is_empty());

        // Decode the request bytes as its own message type to verify fidelity.
        let input = md.input();
        let msg = DynamicMessage::decode(input, bytes.as_slice()).expect("decode");
        let options = SerializeOptions::new().skip_default_fields(false);
        let mut out = Vec::new();
        let mut ser = serde_json::Serializer::new(&mut out);
        msg.serialize_with_options(&mut ser, &options).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(json["name"], "ada");
        assert_eq!(json["count"], 3);
    }

    #[test]
    fn invalid_proto_errors() {
        let err = ProtoDescriptors::compile("this is not valid proto", &[]);
        assert!(err.is_err());
    }

    #[test]
    fn reflection_proto_compiles() {
        // The embedded reflection proto must compile and expose its message types.
        let d = ProtoDescriptors::compile(REFLECTION_PROTO, &[]).expect("reflection proto");
        assert!(d
            .pool
            .get_message_by_name("grpc.reflection.v1.ServerReflectionRequest")
            .is_some());
        assert!(d
            .pool
            .get_message_by_name("grpc.reflection.v1.ServerReflectionResponse")
            .is_some());
    }

    #[test]
    fn tolerant_registration_keeps_service_despite_missing_import() {
        use prost_types::{
            DescriptorProto, FieldDescriptorProto, FileDescriptorProto, MethodDescriptorProto,
            ServiceDescriptorProto,
        };
        use std::collections::HashMap;

        // A self-contained service file that *imports* a proto the "server"
        // never provided. Tolerant registration should strip the dead import
        // and still register the service.
        let msg = |name: &str| DescriptorProto {
            name: Some(name.to_string()),
            field: vec![FieldDescriptorProto {
                name: Some("value".to_string()),
                number: Some(1),
                r#type: Some(field_descriptor_proto_type_string()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let file = FileDescriptorProto {
            name: Some("svc.proto".to_string()),
            package: Some("demo".to_string()),
            dependency: vec!["google/gateway/annotations.proto".to_string()],
            message_type: vec![msg("Req"), msg("Res")],
            service: vec![ServiceDescriptorProto {
                name: Some("Demo".to_string()),
                method: vec![MethodDescriptorProto {
                    name: Some("Do".to_string()),
                    input_type: Some(".demo.Req".to_string()),
                    output_type: Some(".demo.Res".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            syntax: Some("proto3".to_string()),
            ..Default::default()
        };

        let mut files = HashMap::new();
        files.insert("svc.proto".to_string(), file);

        let pool = register_files_tolerant(files);
        let descriptors = ProtoDescriptors { pool };
        let methods = descriptors.methods();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].service, "demo.Demo");
        assert_eq!(methods[0].method, "Do");
    }

    /// `FieldDescriptorProto.Type::String` numeric value (9), avoiding the enum
    /// import in test code.
    fn field_descriptor_proto_type_string() -> i32 {
        prost_types::field_descriptor_proto::Type::String as i32
    }
}
