use anyhow::{anyhow, Result};
use models::{
    Folder, FolderType, HttpConfig, HttpMethod, MediaContent, Parameter, ParameterLocation,
    PropertyType, RequestBody, Response, Schema, SchemaProperty, SchemaType, Service, ServiceType,
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: Option<String>,
    pub swagger: Option<String>,
    pub info: OpenApiInfo,
    #[serde(default)]
    pub paths: HashMap<String, PathItem>,
    #[serde(default)]
    pub components: Option<Components>,
    #[serde(default)]
    pub definitions: Option<HashMap<String, SchemaObject>>, // Swagger 2.0
}

#[derive(Debug, Deserialize)]
pub struct OpenApiInfo {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PathItem {
    #[serde(default)]
    pub get: Option<Operation>,
    #[serde(default)]
    pub post: Option<Operation>,
    #[serde(default)]
    pub put: Option<Operation>,
    #[serde(default)]
    pub delete: Option<Operation>,
    #[serde(default)]
    pub patch: Option<Operation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub operation_id: Option<String>,
    #[serde(default)]
    pub parameters: Vec<ParameterObject>,
    #[serde(default)]
    pub request_body: Option<RequestBodyObject>,
    #[serde(default)]
    pub responses: HashMap<String, ResponseObject>,
}

#[derive(Debug, Deserialize)]
pub struct ParameterObject {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub schema: Option<SchemaObject>,
}

#[derive(Debug, Deserialize)]
pub struct RequestBodyObject {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub content: HashMap<String, MediaTypeObject>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseObject {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub content: Option<HashMap<String, MediaTypeObject>>,
}

#[derive(Debug, Deserialize)]
pub struct MediaTypeObject {
    #[serde(default)]
    pub schema: Option<SchemaObject>,
    #[serde(default)]
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaObject {
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    #[serde(default)]
    pub properties: Option<HashMap<String, SchemaObject>>,
    #[serde(default)]
    pub required: Option<Vec<String>>,
    #[serde(default)]
    pub items: Option<Box<SchemaObject>>,
    #[serde(rename = "$ref")]
    pub ref_path: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,
    #[serde(default)]
    pub nullable: Option<bool>,
    #[serde(default)]
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct Components {
    #[serde(default)]
    pub schemas: HashMap<String, SchemaObject>,
}

pub struct ImportResult {
    pub folders: Vec<Folder>,
    pub services: Vec<Service>,
    pub schemas: Vec<Schema>,
}

pub fn parse_openapi(content: &str) -> Result<OpenApiSpec> {
    // Try JSON first, then YAML
    serde_json::from_str(content)
        .or_else(|_| serde_yaml::from_str(content))
        .map_err(|e| anyhow!("Failed to parse OpenAPI spec: {}", e))
}

pub fn import_openapi(workspace_id: Uuid, spec: &OpenApiSpec) -> Result<ImportResult> {
    let mut folders: Vec<Folder> = vec![];
    let mut services: Vec<Service> = vec![];
    let mut schemas: Vec<Schema> = vec![];
    let mut tag_folder_map: HashMap<String, Uuid> = HashMap::new();

    // Create folders from tags
    for (_path, path_item) in &spec.paths {
        for op in [&path_item.get, &path_item.post, &path_item.put, &path_item.delete, &path_item.patch].into_iter().flatten() {
            for tag in &op.tags {
                if !tag_folder_map.contains_key(tag) {
                    let folder = Folder::with_type(workspace_id, tag.clone(), None, FolderType::Service);
                    tag_folder_map.insert(tag.clone(), folder.id);
                    folders.push(folder);
                }
            }
        }
    }

    // Create services from paths
    for (path, path_item) in &spec.paths {
        let ops = [
            (&path_item.get, HttpMethod::Get),
            (&path_item.post, HttpMethod::Post),
            (&path_item.put, HttpMethod::Put),
            (&path_item.delete, HttpMethod::Delete),
            (&path_item.patch, HttpMethod::Patch),
        ];

        for (op_opt, method) in ops {
            if let Some(op) = op_opt {
                let name = op.summary.clone()
                    .or_else(|| op.operation_id.clone())
                    .unwrap_or_else(|| format!("{} {}", method.as_str(), path));

                let folder_id = op.tags.first().and_then(|t| tag_folder_map.get(t)).copied();

                let parameters: Vec<Parameter> = op.parameters.iter().map(|p| {
                    Parameter {
                        name: p.name.clone(),
                        location: match p.location.as_str() {
                            "path" => ParameterLocation::Path,
                            "query" => ParameterLocation::Query,
                            "header" => ParameterLocation::Header,
                            "cookie" => ParameterLocation::Cookie,
                            _ => ParameterLocation::Query,
                        },
                        description: p.description.clone(),
                        required: p.required,
                        schema: p.schema.as_ref().and_then(|s| s.schema_type.clone()),
                        example: p.schema.as_ref().and_then(|s| s.example.as_ref().map(|e| e.to_string())),
                    }
                }).collect();

                let request_body = op.request_body.as_ref().map(|rb| {
                    RequestBody {
                        description: rb.description.clone(),
                        required: rb.required,
                        content: rb.content.iter().map(|(media_type, mt)| {
                            MediaContent {
                                media_type: media_type.clone(),
                                schema: mt.schema.as_ref().and_then(|s| {
                                    s.ref_path.clone().or_else(|| s.schema_type.clone())
                                }),
                                example: mt.example.as_ref().map(|e| e.to_string()),
                            }
                        }).collect(),
                    }
                });

                let responses: Vec<Response> = op.responses.iter().map(|(code, resp)| {
                    Response {
                        status_code: code.clone(),
                        description: resp.description.clone().unwrap_or_default(),
                        content: resp.content.as_ref().map(|c| {
                            c.iter().map(|(media_type, mt)| {
                                MediaContent {
                                    media_type: media_type.clone(),
                                    schema: mt.schema.as_ref().and_then(|s| {
                                        s.ref_path.clone().or_else(|| s.schema_type.clone())
                                    }),
                                    example: mt.example.as_ref().map(|e| e.to_string()),
                                }
                            }).collect()
                        }).unwrap_or_default(),
                    }
                }).collect();

                let mut service = Service::new(
                    workspace_id,
                    &name,
                    ServiceType::Http(HttpConfig {
                        method,
                        path: path.clone(),
                        parameters,
                        request_body,
                        responses,
                        security: vec![],
                    }),
                );
                service.folder_id = folder_id;
                service.description = op.description.clone();
                services.push(service);
            }
        }
    }

    // Import schemas from components (OpenAPI 3.x) or definitions (Swagger 2.0)
    let schema_defs = spec.components.as_ref()
        .map(|c| &c.schemas)
        .or(spec.definitions.as_ref());

    if let Some(defs) = schema_defs {
        for (name, schema_obj) in defs {
            let schema = convert_schema_object(workspace_id, name, schema_obj);
            schemas.push(schema);
        }
    }

    Ok(ImportResult { folders, services, schemas })
}

fn convert_schema_object(workspace_id: Uuid, name: &str, obj: &SchemaObject) -> Schema {
    let schema_type = match obj.schema_type.as_deref() {
        Some("object") => SchemaType::Object,
        Some("array") => SchemaType::Array,
        Some("string") => {
            if let Some(enums) = &obj.enum_values {
                SchemaType::Enum(enums.clone())
            } else {
                SchemaType::String
            }
        }
        Some("integer") => SchemaType::Integer,
        Some("number") => SchemaType::Number,
        Some("boolean") => SchemaType::Boolean,
        _ => SchemaType::Object,
    };

    let properties: Vec<SchemaProperty> = obj.properties.as_ref()
        .map(|props| {
            props.iter().map(|(prop_name, prop_obj)| {
                SchemaProperty {
                    name: prop_name.clone(),
                    property_type: convert_property_type(prop_obj),
                    description: prop_obj.description.clone(),
                    nullable: prop_obj.nullable.unwrap_or(false),
                    default: None,
                    example: prop_obj.example.as_ref().map(|e| e.to_string()),
                }
            }).collect()
        })
        .unwrap_or_default();

    let required = obj.required.clone().unwrap_or_default();

    let mut schema = Schema::new(workspace_id, name.to_string());
    schema.description = obj.description.clone();
    schema.schema_type = schema_type;
    schema.properties = properties;
    schema.required = required;
    schema
}

fn convert_property_type(obj: &SchemaObject) -> PropertyType {
    if let Some(ref_path) = &obj.ref_path {
        let ref_name = ref_path.split('/').last().unwrap_or(ref_path).to_string();
        return PropertyType::Ref(ref_name);
    }

    match obj.schema_type.as_deref() {
        Some("string") => PropertyType::String,
        Some("integer") => PropertyType::Integer,
        Some("number") => PropertyType::Number,
        Some("boolean") => PropertyType::Boolean,
        Some("array") => {
            let item_type = obj.items.as_ref()
                .map(|i| convert_property_type(i))
                .unwrap_or(PropertyType::String);
            PropertyType::Array(Box::new(item_type))
        }
        Some("object") => PropertyType::Object,
        _ => PropertyType::String,
    }
}
