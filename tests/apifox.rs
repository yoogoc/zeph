use serde_json::{Value, json};
use std::collections::HashSet;
use uuid::Uuid;
use zeph::{
    apifox::{self, ChangeKind, Resolution, Selection, Snapshot, SourceConfig, SyncSource},
    model::{Auth, Project},
};

fn spec() -> Value {
    json!({"openapi":"3.1.0","info":{"title":"Shop","version":"1"},"servers":[{"url":"http://localhost:8080"}],"paths":{"/orders":{"post":{"summary":"创建订单","operationId":"createOrder","x-apifox-folder":"商城/订单","tags":["订单","Public"],"parameters":[{"in":"query","name":"page","schema":{"type":"integer","default":1}}],"requestBody":{"content":{"application/json":{"schema":{"type":"object","properties":{"note":{"type":"string"}}},"example":{"note":"下单"}}}},"responses":{"200":{"description":"ok"}}}}}})
}
fn project() -> (Project, Uuid) {
    let mut p = Project::empty("Test");
    let mut source = SyncSource::new(SourceConfig {
        name: "商城 API".into(),
        project_id: "1234567".into(),
        branch_id: None,
        module_id: None,
    });
    source.received(Snapshot::parse(spec()).unwrap());
    let id = source.id;
    p.sync_sources.push(source);
    (p, id)
}
fn apply_safe(p: &mut Project, id: Uuid) -> usize {
    let plan = apifox::plan(p, id).unwrap();
    let selection = Selection::safe(&plan);
    apifox::apply(p, &plan, &selection).unwrap()
}
fn receive(p: &mut Project, doc: Value) {
    p.sync_sources[0].received(Snapshot::parse(doc).unwrap());
}

#[test]
fn apifox_directories_have_escaped_names_and_tags_are_not_paths() {
    let path = zeph::openapi::parse_apifox_directory(r"商城/订单\/售后/反\\斜线").unwrap();
    assert_eq!(path, vec!["商城", "订单/售后", r"反\斜线"]);
    assert!(zeph::openapi::parse_apifox_directory("A//B").is_err());
    assert!(zeph::openapi::parse_apifox_directory("A\\x").is_err());
    let imported = zeph::openapi::import_value(spec()).unwrap();
    let mut p = Project::empty("Test");
    p.merge_collection(imported.folders, imported.requests)
        .unwrap();
    assert_eq!(
        p.folder_path(p.requests[0].folder_id.unwrap()),
        "Shop / 商城 / 订单"
    );
    assert_eq!(
        p.requests[0].openapi.as_ref().unwrap().tags,
        vec!["订单", "Public"]
    );
}
#[test]
fn first_apply_repeated_checks_and_roundtrip_keep_resource_identity() {
    let (mut p, id) = project();
    assert_eq!(apply_safe(&mut p, id), 1);
    let request = p.requests[0].id;
    assert_eq!(
        p.folder_path(p.requests[0].folder_id.unwrap()),
        "商城 API / 商城 / 订单"
    );
    receive(&mut p, spec());
    assert!(apifox::plan(&p, id).unwrap().changes.is_empty());
    assert_eq!(p.requests[0].id, request);
    let temp = tempfile::tempdir().unwrap();
    let store = zeph::storage::ProjectStore::new(temp.path());
    store.save(&p).unwrap();
    let restored = store.load().unwrap();
    assert!(apifox::plan(&restored, id).unwrap().changes.is_empty());
    assert!(
        !std::fs::read_to_string(temp.path().join("zeph.project.json"))
            .unwrap()
            .contains("Bearer")
    );
}
#[test]
fn upstream_schema_changes_preserve_runtime_values_auth_files_tabs_and_environment() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let request = p.requests[0].id;
    p.open_request(request);
    p.requests[0].query[0].value = "42".into();
    p.requests[0].body.text = r#"{"note":"local"}"#.into();
    p.requests[0].body.file_path = "/tmp/private.bin".into();
    p.requests[0].auth = Auth::Bearer("local-secret".into());
    p.environments[0]
        .variables
        .insert("token".into(), "local".into());
    let mut remote = spec();
    remote["paths"]["/orders"]["post"]["parameters"][0]["schema"]["default"] = json!(20);
    remote["paths"]["/orders"]["post"]["requestBody"]["content"]["application/json"]["example"]["note"] =
        json!("upstream");
    receive(&mut p, remote);
    let plan = apifox::plan(&p, id).unwrap();
    assert!(plan.changes.iter().all(|c| c.kind != ChangeKind::Conflict));
    apply_safe(&mut p, id);
    assert_eq!(p.requests[0].query[0].value, "42");
    assert_eq!(p.requests[0].body.text, r#"{"note":"local"}"#);
    assert_eq!(p.requests[0].body.file_path, "/tmp/private.bin");
    assert!(matches!(&p.requests[0].auth,Auth::Bearer(s) if s=="local-secret"));
    assert_eq!(p.tabs(), &[request]);
    assert_eq!(p.environments[0].variables["token"], "local");
    assert!(!p.sync_sources[0].bindings[0].compatibility.is_empty());
}
#[test]
fn conflict_resolution_advances_baseline_and_skipped_resources_remain_pending() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    p.requests[0].name = "本地名称".into();
    let mut remote = spec();
    remote["paths"]["/orders"]["post"]["summary"] = json!("远端名称");
    receive(&mut p, remote);
    let plan = apifox::plan(&p, id).unwrap();
    assert_eq!(plan.changes[0].kind, ChangeKind::Conflict);
    assert!(Selection::safe(&plan).included.is_empty());
    assert_eq!(
        apifox::apply(&mut p, &plan, &Selection::default()).unwrap(),
        0
    );
    assert!(!apifox::plan(&p, id).unwrap().changes.is_empty());
    let plan = apifox::plan(&p, id).unwrap();
    let c = &plan.changes[0];
    let mut selection = Selection::default();
    selection.included.insert(c.key.clone());
    assert!(apifox::apply(&mut p, &plan, &selection).is_err());
    for f in c.fields.iter().filter(|f| f.conflict) {
        selection
            .resolutions
            .insert((c.key.clone(), f.path.clone()), Resolution::Local);
    }
    apifox::apply(&mut p, &plan, &selection).unwrap();
    assert_eq!(p.requests[0].name, "本地名称");
    assert!(apifox::plan(&p, id).unwrap().changes.is_empty());
}
#[test]
fn local_edits_after_preview_reject_the_whole_apply() {
    let (mut p, id) = project();
    let plan = apifox::plan(&p, id).unwrap();
    let selection = Selection::safe(&plan);
    p.name = "Changed".into();
    let before = serde_json::to_value(&p).unwrap();
    assert!(apifox::apply(&mut p, &plan, &selection).is_err());
    assert_eq!(serde_json::to_value(&p).unwrap(), before);
}
#[test]
fn remote_removal_keeps_local_request_and_reappearance_reuses_id() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let request = p.requests[0].id;
    let mut remote = spec();
    remote["paths"] = json!({});
    receive(&mut p, remote);
    let plan = apifox::plan(&p, id).unwrap();
    assert_eq!(plan.changes[0].kind, ChangeKind::Removed);
    assert!(Selection::safe(&plan).included.is_empty());
    let mut selection = Selection::default();
    selection.included.insert(plan.changes[0].key.clone());
    apifox::apply(&mut p, &plan, &selection).unwrap();
    assert_eq!(p.requests[0].id, request);
    assert!(p.sync_sources[0].bindings[0].remote_missing);
    receive(&mut p, spec());
    apply_safe(&mut p, id);
    assert_eq!(p.requests.len(), 1);
    assert_eq!(p.requests[0].id, request);
    assert!(!p.sync_sources[0].bindings[0].remote_missing);
}
#[test]
fn locally_deleted_requests_require_explicit_restore() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let request = p.requests[0].id;
    p.delete_request(request);
    let plan = apifox::plan(&p, id).unwrap();
    assert_eq!(plan.changes[0].kind, ChangeKind::LocalDeleted);
    assert!(Selection::safe(&plan).included.is_empty());
    let mut selection = Selection::default();
    let key = plan.changes[0].key.clone();
    selection.included.insert(key.clone());
    selection
        .resolutions
        .insert((key, "$".into()), Resolution::Local);
    apifox::apply(&mut p, &plan, &selection).unwrap();
    assert!(p.requests.is_empty());
    assert!(apifox::plan(&p, id).unwrap().changes.is_empty());
}
#[test]
fn path_changes_use_unique_operation_id_and_folder_rename_preserves_folder_id() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let request = p.requests[0].id;
    let folder = p.requests[0].folder_id;
    let mut remote = spec();
    let mut op = remote["paths"]["/orders"].take();
    op["post"]["x-apifox-folder"] = json!("商城/订单管理");
    remote["paths"] = json!({"/v2/orders":op});
    receive(&mut p, remote);
    apply_safe(&mut p, id);
    assert_eq!(p.requests[0].id, request);
    assert_eq!(p.requests[0].folder_id, folder);
    assert!(p.requests[0].url.ends_with("/v2/orders"));
    assert_eq!(p.folder(folder.unwrap()).unwrap().name, "订单管理");
}
#[test]
fn undo_preserves_later_runtime_edits_and_rejects_later_definition_edits() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let mut remote = spec();
    remote["paths"]["/orders"]["post"]["summary"] = json!("New name");
    receive(&mut p, remote);
    apply_safe(&mut p, id);
    let run = p.sync_sources[0].runs[0].id;
    p.requests[0].body.text = "later draft".into();
    apifox::undo(&mut p, id, run).unwrap();
    assert_eq!(p.requests[0].name, "创建订单");
    assert_eq!(p.requests[0].body.text, "later draft");
    apply_safe(&mut p, id);
    let run = p.sync_sources[0].runs[0].id;
    p.requests[0].name = "later rename".into();
    let before = serde_json::to_value(&p).unwrap();
    assert!(apifox::undo(&mut p, id, run).is_err());
    assert_eq!(serde_json::to_value(&p).unwrap(), before);
}
#[test]
fn undo_new_requests_keeps_subsequent_drafts_as_local_requests() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let run = p.sync_sources[0].runs[0].id;
    p.requests[0].body.text = "my draft".into();
    apifox::undo(&mut p, id, run).unwrap();
    assert_eq!(p.requests.len(), 1);
    assert_eq!(p.requests[0].body.text, "my draft");
    assert!(p.requests[0].source.is_none());
    assert!(p.sync_sources[0].bindings.is_empty());
}
#[test]
fn ambiguous_operation_ids_fall_back_to_method_path_and_sources_stay_isolated() {
    let mut doc = spec();
    doc["paths"]["/other"] = doc["paths"]["/orders"].clone();
    let snapshot = Snapshot::parse(doc).unwrap();
    assert!(
        snapshot
            .endpoints
            .iter()
            .all(|e| e.key.starts_with("endpoint:"))
    );
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let mut second = p.sync_sources[0].clone();
    second.id = Uuid::new_v4();
    second.root_folder = None;
    second.bindings.clear();
    second.folders.clear();
    second.runs.clear();
    let other = second.id;
    p.sync_sources.push(second);
    apply_safe(&mut p, other);
    assert_eq!(p.requests.len(), 2);
    assert_ne!(p.requests[0].folder_id, p.requests[1].folder_id);
}
#[test]
fn incomplete_remote_snapshots_and_network_references_are_rejected() {
    assert!(Snapshot::parse(json!({"openapi":"3.1.0"})).is_err());
    let mut doc = spec();
    doc["components"] = json!({"schemas":{"Unsafe":{"$ref":"file:///tmp/private.json"}}});
    assert!(Snapshot::parse(doc).is_err());
    let mut doc = spec();
    doc["components"] = json!({"schemas":{"Missing":{"$ref":"#/components/schemas/nope"}}});
    assert!(Snapshot::parse(doc).is_err());
}
#[test]
fn hash_ignores_object_key_order_and_error_backoff_keeps_snapshot() {
    assert_eq!(
        apifox::digest(&json!({"a":1,"b":2})),
        apifox::digest(&json!({"b":2,"a":1}))
    );
    let (mut p, _) = project();
    let digest = p.sync_sources[0].latest.as_ref().unwrap().digest.clone();
    p.sync_sources[0].check_minutes = 5;
    p.sync_sources[0].failed(&apifox::ApiFailure {
        kind: apifox::FailureKind::Authorization,
        message: "expired".into(),
        retry_after: None,
    });
    assert!(!p.sync_sources[0].due(i64::MAX));
    assert_eq!(p.sync_sources[0].latest.as_ref().unwrap().digest, digest);
}

#[test]
fn local_folder_moves_and_renames_survive_unrelated_remote_updates() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let original = p.requests[0].folder_id.unwrap();
    p.rename_folder(original, "我的订单").unwrap();
    let mut remote = spec();
    remote["paths"]["/orders"]["post"]["description"] = json!("Updated documentation");
    receive(&mut p, remote);
    apply_safe(&mut p, id);
    assert_eq!(p.requests[0].folder_id, Some(original));
    assert_eq!(p.folder(original).unwrap().name, "我的订单");
    let custom = p.add_folder("其他", None).unwrap();
    let request = p.requests[0].id;
    p.move_request_to_folder(request, Some(custom)).unwrap();
    let mut remote = spec();
    remote["paths"]["/orders"]["post"]["description"] = json!("Another update");
    receive(&mut p, remote);
    apply_safe(&mut p, id);
    assert_eq!(p.requests[0].folder_id, Some(custom));
}

#[test]
fn ancestor_rename_retargets_descendant_bindings_without_duplicate_folders() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let ids = p.folders.iter().map(|f| f.id).collect::<HashSet<_>>();
    let mut remote = spec();
    remote["paths"]["/orders"]["post"]["x-apifox-folder"] = json!("新商城/订单");
    receive(&mut p, remote);
    apply_safe(&mut p, id);
    assert_eq!(p.folders.iter().map(|f| f.id).collect::<HashSet<_>>(), ids);
    assert!(apifox::plan(&p, id).unwrap().changes.is_empty());
}

#[test]
fn only_referenced_models_affect_an_endpoint_definition() {
    let (mut p, id) = project();
    let mut doc = spec();
    doc["components"] = json!({"schemas":{"Used":{"type":"object","properties":{"id":{"type":"string"}}},"Unused":{"type":"string"}}});
    doc["paths"]["/orders"]["post"]["responses"]["200"]["content"] =
        json!({"application/json":{"schema":{"$ref":"#/components/schemas/Used"}}});
    receive(&mut p, doc.clone());
    apply_safe(&mut p, id);
    assert!(
        !p.sync_sources[0].bindings[0]
            .definition
            .to_string()
            .contains("Unused")
    );
    doc["components"]["schemas"]["Unused"]["description"] = json!("unrelated update");
    receive(&mut p, doc.clone());
    assert!(apifox::plan(&p, id).unwrap().changes.is_empty());
    doc["components"]["schemas"]["Used"]["properties"]["name"] = json!({"type":"string"});
    receive(&mut p, doc);
    assert_eq!(apifox::plan(&p, id).unwrap().changes.len(), 1);
}

#[test]
fn archive_and_undo_restore_placement_without_deleting_data() {
    let (mut p, id) = project();
    apply_safe(&mut p, id);
    let folder = p.requests[0].folder_id;
    let mut doc = spec();
    doc["paths"] = json!({});
    receive(&mut p, doc);
    let plan = apifox::plan(&p, id).unwrap();
    let key = plan.changes[0].key.clone();
    let mut selection = Selection::default();
    selection.included.insert(key.clone());
    selection.archive.insert(key);
    apifox::apply(&mut p, &plan, &selection).unwrap();
    assert!(
        p.folder_path(p.requests[0].folder_id.unwrap())
            .ends_with("已移除")
    );
    let run = p.sync_sources[0].runs[0].id;
    apifox::undo(&mut p, id, run).unwrap();
    assert_eq!(p.requests.len(), 1);
    assert_eq!(p.requests[0].folder_id, folder);
    assert!(!p.sync_sources[0].bindings[0].remote_missing);
}

#[test]
fn undo_a_partial_batch_keeps_unselected_resource_bindings() {
    let (mut p, id) = project();
    let mut doc = spec();
    let mut other = doc["paths"]["/orders"].clone();
    other["post"]["operationId"] = json!("other");
    doc["paths"]["/other"] = other;
    receive(&mut p, doc.clone());
    apply_safe(&mut p, id);
    doc["paths"]["/orders"]["post"]["summary"] = json!("Changed");
    receive(&mut p, doc);
    apply_safe(&mut p, id);
    let run = p.sync_sources[0].runs[0].id;
    apifox::undo(&mut p, id, run).unwrap();
    assert_eq!(p.sync_sources[0].bindings.len(), 2);
    assert_eq!(p.requests.len(), 2);
    p.normalize().unwrap();
}

#[test]
fn malformed_path_items_never_become_a_partial_snapshot() {
    let mut doc = spec();
    doc["paths"]["/broken"] = json!({"get":null});
    assert!(Snapshot::parse(doc).is_err());
    let mut doc = spec();
    doc["paths"]["/broken"] = json!({"$ref":"#/paths/~1broken"});
    assert!(Snapshot::parse(doc).is_err());
    let mut doc = spec();
    doc["components"] = json!({"parameters":{"A":{"$ref":"#/components/parameters/B"},"B":{"$ref":"#/components/parameters/A"}}});
    doc["paths"]["/orders"]["post"]["parameters"] = json!([{"$ref":"#/components/parameters/A"}]);
    assert!(Snapshot::parse(doc).is_err());
}
