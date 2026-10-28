use serde_json::{Value, json};
use zeph::{
    apifox::{self, Selection, Snapshot, SourceConfig, SyncSource},
    model::{Auth, BodyKind, Parameter, Project, Protocol, Request},
    openapi,
    storage::{ProjectStore, save_contract},
    structure::{RequestStructure, Section},
};

fn spec() -> Value {
    json!({"openapi":"3.1.0","info":{"title":"Orders","version":"1"},"paths":{"/orders/{id}":{"parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}}],"get":{"operationId":"listOrders","parameters":[{"name":"page","in":"query","required":true,"schema":{"type":"integer","minimum":1}}],"responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Order"}}}},"404":{"description":"not found"}}}}},"components":{"schemas":{"Order":{"type":"object","properties":{"id":{"type":"string"},"amount":{"type":"number"}}}}}})
}
fn synced(doc: Value) -> Project {
    let mut p = Project::empty("Test");
    let mut source = SyncSource::new(SourceConfig {
        name: "商城 API".into(),
        project_id: "1234567".into(),
        branch_id: None,
        module_id: None,
    });
    let id = source.id;
    source.received(Snapshot::parse(doc).unwrap());
    p.sync_sources.push(source);
    let plan = apifox::plan(&p, id).unwrap();
    let selection = Selection::safe(&plan);
    apifox::apply(&mut p, &plan, &selection).unwrap();
    p
}
#[test]
fn synced_get_without_body_shows_parameters_responses_and_referenced_models() {
    let p = synced(spec());
    assert!(p.requests[0].schema.is_none());
    let view = RequestStructure::from_apifox(&p.sync_sources[0].bindings[0], "商城 API");
    assert!(view.defined);
    assert_eq!(view.default_section(), Section::Parameters);
    assert_eq!(view.parameters.len(), 2);
    assert_eq!(view.parameters[0]["in"], "path");
    assert_eq!(view.parameters[1]["schema"]["minimum"], 1);
    assert!(view.body.is_none());
    assert_eq!(view.responses.len(), 2);
    assert_eq!(
        view.responses["200"]["content"]["application/json"]["schema"]["properties"]["amount"]["type"],
        "number"
    );
    assert!(!view.models.is_empty());
}
#[test]
fn pending_snapshots_do_not_replace_the_applied_definition_in_the_side_panel() {
    let mut p = synced(spec());
    let mut remote = spec();
    remote["paths"]["/orders/{id}"]["get"]["parameters"][0]["schema"]["minimum"] = json!(99);
    p.sync_sources[0].received(Snapshot::parse(remote).unwrap());
    let view = RequestStructure::from_apifox(&p.sync_sources[0].bindings[0], "商城 API");
    assert_eq!(view.parameters[1]["schema"]["minimum"], 1);
}
#[test]
fn response_only_operations_default_to_the_response_view() {
    let mut doc = spec();
    doc["paths"]["/orders/{id}"]
        .as_object_mut()
        .unwrap()
        .remove("parameters");
    doc["paths"]["/orders/{id}"]["get"]
        .as_object_mut()
        .unwrap()
        .remove("parameters");
    let p = synced(doc);
    let view = RequestStructure::from_apifox(&p.sync_sources[0].bindings[0], "API");
    assert_eq!(view.default_section(), Section::Responses);
    assert!(view.value(Section::Responses).is_some());
}
#[test]
fn request_body_refs_and_recursive_models_are_expanded_without_recursing_forever() {
    let mut doc = spec();
    doc["components"]["schemas"]["Order"]["properties"]["parent"] =
        json!({"$ref":"#/components/schemas/Order"});
    doc["components"]["requestBodies"] = json!({"OrderInput":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Order"}}}}});
    doc["paths"]["/orders/{id}"]["get"]["requestBody"] =
        json!({"$ref":"#/components/requestBodies/OrderInput","description":"本地说明"});
    let p = synced(doc);
    let view = RequestStructure::from_apifox(&p.sync_sources[0].bindings[0], "API");
    let body = view.body.unwrap();
    assert_eq!(body["required"], true);
    assert_eq!(body["description"], "本地说明");
    assert_eq!(
        body["content"]["application/json"]["schema"]["properties"]["parent"]["$ref"],
        "#/components/schemas/Order"
    );
    assert!(view.models.len() >= 2);
}
#[test]
fn saved_openapi_versions_work_offline_after_original_files_are_deleted() {
    for version in ["2.0", "3.0.3", "3.1.0"] {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("api.json");
        let types = temp.path().join("types.json");
        let parameter = if version == "2.0" {
            json!({"name":"page","in":"query","required":true,"type":"integer","minimum":1})
        } else {
            json!({"name":"page","in":"query","required":true,"schema":{"type":"integer","minimum":1}})
        };
        std::fs::write(&types,json!({"parameters":{"Page":parameter},"schemas":{"Order":{"type":"object","properties":{"id":{"type":"string"}}}}}).to_string()).unwrap();
        let mut doc = spec();
        if version == "2.0" {
            doc.as_object_mut().unwrap().remove("openapi");
            doc["swagger"] = json!(version);
            doc["paths"]["/orders/{id}"]["get"]["responses"]["200"] =
                json!({"description":"ok","schema":{"$ref":"types.json#/schemas/Order"}});
        } else {
            doc["openapi"] = json!(version);
        }
        doc["paths"]["/orders/{id}"]["get"]["parameters"] =
            json!([{"$ref":"types.json#/parameters/Page"}]);
        std::fs::write(&source, doc.to_string()).unwrap();
        let import = openapi::import(&source).unwrap();
        let store = ProjectStore::new(temp.path().join("store"));
        save_contract(&store.directory.join("contracts"), &import).unwrap();
        let request = import.requests[0].clone();
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(types).unwrap();
        let view = store.request_structure(&request).unwrap();
        assert!(view.defined);
        assert_eq!(view.parameters.len(), 2);
        assert_eq!(view.parameters[1]["name"], "page");
        assert!(view.diagnostics.is_empty(), "{:?}", view.diagnostics);
        assert!(view.responses.contains_key("200"));
        assert!(!view.models.is_empty());
    }
}
#[test]
fn saved_swagger_body_parameters_and_operation_overrides_remain_visible() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("api.json");
    let doc = json!({"swagger":"2.0","info":{"title":"Legacy","version":"1"},"consumes":["application/json"],"definitions":{"Payload":{"type":"object","properties":{"name":{"type":"string"}}}},"paths":{"/orders":{"parameters":[{"name":"X-Version","in":"header","type":"string","description":"path level"}],"post":{"parameters":[{"name":"X-Version","in":"header","type":"string","description":"operation override"},{"name":"body","in":"body","required":true,"schema":{"$ref":"#/definitions/Payload"}}],"responses":{"200":{"description":"ok"}}}}}});
    std::fs::write(&file, doc.to_string()).unwrap();
    let import = openapi::import(&file).unwrap();
    let store = ProjectStore::new(temp.path().join("store"));
    save_contract(&store.directory.join("contracts"), &import).unwrap();
    let view = store.request_structure(&import.requests[0]).unwrap();
    assert_eq!(view.parameters.len(), 1);
    assert_eq!(view.parameters[0]["description"], "operation override");
    assert_eq!(
        view.body.unwrap()["parameters"][0]["schema"]["properties"]["name"]["type"],
        "string"
    );
}
#[test]
fn local_parameter_overview_does_not_present_debug_values_as_schema_examples() {
    let mut request = Request::new(Protocol::Http, "Local");
    request
        .query
        .push(Parameter::new("token", "private-query-value"));
    request
        .headers
        .push(Parameter::new("Authorization", "private-auth"));
    request.auth = Auth::Bearer("private-secret".into());
    request.body.kind = BodyKind::Json;
    request.body.text = r#"{"secret":"private-body"}"#.into();
    let before = serde_json::to_value(&request).unwrap();
    let view = RequestStructure::local(&request);
    let text = view.value(Section::Parameters).unwrap().to_string();
    assert!(!view.defined);
    assert!(!text.contains("private-"));
    assert!(view.body.is_none());
    assert_eq!(serde_json::to_value(&request).unwrap(), before);
}
#[test]
fn unresolved_response_refs_are_retained_and_explained_instead_of_hiding_the_panel() {
    let doc = json!({"openapi":"3.1.0","info":{"title":"Broken","version":"1"},"paths":{"/orders":{"get":{"responses":{"200":{"$ref":"#/components/responses/Missing"}}}}}});
    let mut request = Request::new(Protocol::Http, "Orders");
    request.openapi = Some(zeph::model::OpenApiOperation {
        contract_id: uuid::Uuid::new_v4(),
        path: "/orders".into(),
        method: "get".into(),
        operation_id: None,
        tags: vec![],
        directory: None,
    });
    let view = RequestStructure::from_saved(&request, doc, json!({"source":"api.json"})).unwrap();
    assert_eq!(view.default_section(), Section::Responses);
    assert_eq!(
        view.responses["200"]["$ref"],
        "#/components/responses/Missing"
    );
    assert!(!view.diagnostics.is_empty());
}

#[test]
fn suggestions_match_parameter_location_header_case_and_scalar_types() {
    let view = RequestStructure {
        parameters: vec![
            json!({"name":"state","in":"query","schema":{"type":"string","enum":["pending","running"]}}),
            json!({"name":"state","in":"path","schema":{"enum":["active"]}}),
            json!({"name":"X-Version","in":"header","schema":{"enum":[1,2]}}),
            json!({"name":"enabled","in":"query","schema":{"type":"boolean"}}),
            json!({"name":"empty","in":"query","schema":{"enum":["",null]}}),
        ],
        ..Default::default()
    };
    assert_eq!(
        view.value_suggestions("query", "state"),
        ["pending", "running"]
    );
    assert_eq!(view.value_suggestions("path", "state"), ["active"]);
    assert_eq!(view.value_suggestions("header", "x-version"), ["1", "2"]);
    assert_eq!(
        view.value_suggestions("query", "enabled"),
        ["true", "false"]
    );
    assert_eq!(view.value_suggestions("query", "empty"), ["", "null"]);
    assert!(view.value_suggestions("query", "State").is_empty());
    assert!(view.value_suggestions("query", "X-Version").is_empty());
}
#[test]
fn suggestions_follow_openapi_unions_and_intersections_without_invalid_enum_values() {
    let view = RequestStructure {
        parameters: vec![
            json!({"name":"mode","in":"query","schema":{"oneOf":[{"const":"fast"},{"const":"safe"}]}}),
            json!({"name":"state","in":"query","schema":{"allOf":[{"enum":["pending","running"]},{"enum":["running","failed"]}]}}),
            json!({"name":"none","in":"query","schema":{"allOf":[{"const":"one"},{"const":"two"}]}}),
        ],
        ..Default::default()
    };
    assert_eq!(view.value_suggestions("query", "mode"), ["fast", "safe"]);
    assert_eq!(view.value_suggestions("query", "state"), ["running"]);
    assert!(view.value_suggestions("query", "none").is_empty());
}
#[test]
fn suggestions_resolve_applied_parameter_refs_and_form_body_properties() {
    let mut doc = spec();
    doc["components"]["parameters"]["State"] =
        json!({"name":"state","in":"query","schema":{"enum":["pending","running"]}});
    doc["paths"]["/orders/{id}"]["get"]["parameters"] =
        json!([{"$ref":"#/components/parameters/State"}]);
    doc["paths"]["/orders/{id}"]["get"]["requestBody"] = json!({"content":{
        "multipart/form-data":{"schema":{"type":"object","properties":{"mode":{"enum":["fast","safe"]}}}},
        "application/x-www-form-urlencoded":{"schema":{"type":"object","properties":{"mode":{"enum":["small","large"]}}}}
    }});
    let project = synced(doc);
    let view = RequestStructure::from_apifox(&project.sync_sources[0].bindings[0], "Test");
    assert_eq!(
        view.value_suggestions("query", "state"),
        ["pending", "running"]
    );
    assert_eq!(view.value_suggestions("formData", "mode"), ["fast", "safe"]);
    assert_eq!(view.value_suggestions("form", "mode"), ["small", "large"]);
    let legacy = RequestStructure {
        body: Some(
            json!({"parameters":[{"name":"mode","in":"formData","type":"string","enum":["legacy"]}]}),
        ),
        ..Default::default()
    };
    assert_eq!(legacy.value_suggestions("formData", "mode"), ["legacy"]);
}
#[test]
fn documentation_renders_field_metadata_trees_and_media_instead_of_raw_extensions() {
    let view = RequestStructure {
        parameters: vec![
            json!({"name":"state","in":"query","required":false,"description":"按状态筛选","schema":{"type":"string","enum":["pending","running"],"default":"pending","x-apifox-folder":"hidden"}}),
        ],
        body: Some(
            json!({"required":true,"content":{"application/json":{"schema":{"type":"object","required":["orders"],"properties":{"orders":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string","minLength":1}}}}},"example":{"orders":[]}}}}}),
        ),
        responses: std::collections::BTreeMap::from([(
            "200".into(),
            json!({"description":"成功","content":{"application/json":{"schema":{"type":"string"}}},"headers":{"X-Count":{"schema":{"type":"integer"},"description":"总数"}}}),
        )]),
        ..Default::default()
    };
    let parameters = view.documentation(Section::Parameters);
    assert_eq!(parameters[0].name, "state");
    assert_eq!(parameters[0].location.as_deref(), Some("Query"));
    assert_eq!(parameters[0].required, Some(false));
    assert_eq!(parameters[0].values, ["pending", "running"]);
    assert_eq!(parameters[0].description, "按状态筛选");
    assert_eq!(parameters[0].details, [("默认值".into(), "pending".into())]);
    let body = view.documentation(Section::Body);
    assert_eq!(body[0].name, "JSON");
    assert_eq!(body[0].required, Some(true));
    assert_eq!(body[0].children[0].required, Some(true));
    assert_eq!(body[0].children[0].children[0].children[0].name, "id");
    assert!(
        body[0]
            .details
            .contains(&("示例".into(), "对象 · 1 个字段".into()))
    );
    let response = view.documentation(Section::Responses);
    assert_eq!(response[0].name, "200");
    assert_eq!(response[0].description, "成功");
    assert_eq!(response[0].children[1].children[0].name, "X-Count");
    assert_eq!(response[0].children[1].children[0].description, "总数");
}
#[test]
fn literal_examples_are_not_resolved_as_references_and_schema_fields_named_example_work() {
    let doc = json!({"openapi":"3.1.0","info":{"title":"Literal","version":"1"},"components":{"schemas":{"Name":{"type":"string"}}},
        "paths":{"/orders":{"get":{"parameters":[{"name":"filter","in":"query","schema":{"type":"object","properties":{"example":{"$ref":"#/components/schemas/Name"}},"example":{"$ref":"https://literal.invalid/value"},"default":{"$ref":"literal"},"enum":[{"$ref":"literal"}]}}],"responses":{}}}}});
    let mut request = Request::new(Protocol::Http, "Literal");
    request.openapi = Some(zeph::model::OpenApiOperation {
        contract_id: uuid::Uuid::new_v4(),
        path: "/orders".into(),
        method: "get".into(),
        operation_id: None,
        tags: vec![],
        directory: None,
    });
    let view =
        RequestStructure::from_saved(&request, doc, json!({"source":"literal.json"})).unwrap();
    assert_eq!(
        view.parameters[0]["schema"]["properties"]["example"]["type"],
        "string"
    );
    assert_eq!(
        view.parameters[0]["schema"]["example"]["$ref"],
        "https://literal.invalid/value"
    );
    assert_eq!(view.parameters[0]["schema"]["default"]["$ref"], "literal");
    assert!(view.diagnostics.is_empty());
    assert!(view.value_suggestions("query", "filter").is_empty());
}
