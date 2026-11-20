use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;
use zeph::{
    model::{Project, Protocol, Request},
    operations::{Kind, Operation, OperationContext, Outcome, Stage},
    transport::RunRecord,
};

fn step(name: &str, value: &str) -> Operation {
    Operation {
        key: "order".into(),
        value: value.into(),
        name: name.into(),
        ..Operation::new(Kind::SetVariable)
    }
}
fn fixture() -> (Project, uuid::Uuid, uuid::Uuid, Request) {
    let mut project = Project::empty("inheritance");
    let root = project.add_folder("Root", None).unwrap();
    let child = project.add_folder("Child", Some(root)).unwrap();
    let mut request = Request::new(Protocol::Http, "Request");
    request.folder_id = Some(child);
    project.folders[0].operations.pre = vec![step("root-pre", "{{order}}R")];
    project.folders[0].operations.post = vec![step("root-post", "{{order}}r")];
    project.folders[1].operations.pre = vec![step("child-pre", "{{order}}C")];
    project.folders[1].operations.post = vec![step("child-post", "{{order}}c")];
    request.pre_operations = vec![step("request-pre", "{{order}}Q")];
    request.post_operations = vec![step("request-post", "{{order}}q")];
    (project, root, child, request)
}

#[tokio::test]
async fn nested_operations_wrap_request_share_variables_and_keep_origin_in_history() {
    let (project, root, child, request) = fixture();
    let mut snapshot = project.prepare_request(&request).unwrap();
    assert_eq!(
        snapshot
            .pre_operations
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>(),
        ["root-pre", "child-pre", "request-pre"]
    );
    assert_eq!(
        snapshot
            .post_operations
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>(),
        ["request-post", "child-post", "root-post"]
    );
    assert_eq!(request.pre_operations.len(), 1);
    let mut ctx = OperationContext::new(BTreeMap::from([("order".into(), String::new())]));
    let cancel = CancellationToken::new();
    assert!(ctx.pre(&mut snapshot, &cancel).await);
    assert_eq!(ctx.variables["order"], "RCQ");
    snapshot.post_operations.last_mut().unwrap().scope = zeph::operations::Scope::Environment;
    let mut record = RunRecord::error(&snapshot, "Local".into(), String::new());
    record.error = false;
    let patch = ctx
        .finish(&snapshot, &mut record, true, &cancel)
        .await
        .unwrap();
    assert_eq!(patch.writes[0].value, "RCQqcr");
    let origins = record
        .operations
        .iter()
        .map(|r| r.origin.as_ref().unwrap().folder_id)
        .collect::<Vec<_>>();
    assert_eq!(
        origins,
        [Some(root), Some(child), None, None, Some(child), Some(root)]
    );
    assert!(
        record
            .operations
            .iter()
            .all(|r| r.outcome == Outcome::Passed)
    );
    let history: RunRecord = serde_json::from_value(serde_json::to_value(record).unwrap()).unwrap();
    assert!(
        history.operations[1]
            .origin
            .as_ref()
            .unwrap()
            .label
            .contains("Root / Child")
    );
}

#[test]
fn request_and_folder_can_stop_each_stage_independently_without_skipping_local_steps() {
    let (mut project, _, _, mut request) = fixture();
    request.inherit_pre_operations = false;
    let plan = project.operation_plan(&request).unwrap();
    assert_eq!(plan.pre.len(), 1);
    assert_eq!(plan.post.len(), 3);
    request.inherit_pre_operations = true;
    request.inherit_post_operations = false;
    project.folders[1].operations.inherit_pre = false;
    let plan = project.operation_plan(&request).unwrap();
    assert_eq!(
        plan.pre.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
        ["child-pre", "request-pre"]
    );
    assert_eq!(plan.post.len(), 1);
    request.inherit_post_operations = true;
    project.folders[1].operations.inherit_post = false;
    let plan = project.operation_plan(&request).unwrap();
    assert_eq!(
        plan.post
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>(),
        ["request-post", "child-post"]
    );
    project.folders[1].operations.pre.clear();
    assert_eq!(project.operation_plan(&request).unwrap().pre.len(), 1);
}

#[test]
fn preparation_is_idempotent_and_moving_request_uses_new_folder_on_next_send() {
    let (mut project, _, _, mut request) = fixture();
    let snapshot = project.prepare_request(&request).unwrap();
    assert_eq!(
        project
            .prepare_request(&snapshot)
            .unwrap()
            .pre_operations
            .len(),
        3
    );
    let other = project.add_folder("Other", None).unwrap();
    project.folders.last_mut().unwrap().operations.pre = vec![step("other", "new")];
    request.folder_id = Some(other);
    assert_eq!(
        project.prepare_request(&request).unwrap().pre_operations[0].name,
        "other"
    );
    assert_eq!(snapshot.pre_operations[0].name, "root-pre");
    assert_eq!(request.pre_operations.len(), 1);
}

#[tokio::test]
async fn failed_folder_pre_blocks_request_and_skips_cascaded_post() {
    let (mut project, _, _, request) = fixture();
    project.folders[0].operations.pre[0].value = "{{missing}}".into();
    let mut snapshot = project.prepare_request(&request).unwrap();
    let cancel = CancellationToken::new();
    let mut ctx = OperationContext::new(BTreeMap::new());
    assert!(!ctx.pre(&mut snapshot, &cancel).await);
    assert_eq!(ctx.reports[0].outcome, Outcome::Failed);
    assert!(
        ctx.reports[1..]
            .iter()
            .all(|r| r.outcome == Outcome::Skipped)
    );
    let mut record = RunRecord::error(&request, "Local".into(), "not sent".into());
    assert!(
        ctx.finish(&snapshot, &mut record, false, &cancel)
            .await
            .is_none()
    );
    assert!(
        record.operations[3..]
            .iter()
            .all(|r| r.outcome == Outcome::Skipped)
    );
}

#[test]
fn aggregate_limit_cycles_missing_folders_and_depth_are_rejected_before_execution() {
    let (mut project, root, _, request) = fixture();
    project.folders[0].operations.pre = (0..64).map(|_| step("limit", "x")).collect();
    assert!(
        project
            .prepare_request(&request)
            .unwrap_err()
            .to_string()
            .contains("64")
    );
    project.folders[0].operations.pre.clear();
    project.folders[0].parent_id = request.folder_id;
    assert!(
        project
            .inherited_operations(request.folder_id, Stage::Pre)
            .is_err()
    );
    project.folders[0].parent_id = Some(uuid::Uuid::new_v4());
    assert!(
        project
            .inherited_operations(request.folder_id, Stage::Pre)
            .is_err()
    );
    project.folders[0].parent_id = None;
    let mut parent = root;
    for index in 0..31 {
        parent = project
            .add_folder(&format!("Depth {index}"), Some(parent))
            .unwrap();
    }
    assert!(
        project
            .inherited_operations(Some(parent), Stage::Pre)
            .is_ok()
    );
    // Bypass add_folder's existing tree validation to inspect a malformed persisted chain.
    let mut extra = project.folders.last().unwrap().clone();
    extra.id = uuid::Uuid::new_v4();
    extra.name = "too deep".into();
    extra.parent_id = Some(parent);
    parent = extra.id;
    project.folders.push(extra);
    assert!(
        project
            .inherited_operations(Some(parent), Stage::Pre)
            .is_err()
    );
}

#[test]
fn old_projects_default_to_inheritance_and_runtime_origins_are_not_saved_into_definitions() {
    let (mut project, _, _, request) = fixture();
    project.requests.push(request);
    let mut value = serde_json::to_value(&project).unwrap();
    for folder in value["folders"].as_array_mut().unwrap() {
        folder.as_object_mut().unwrap().remove("operations");
    }
    value["requests"][0]
        .as_object_mut()
        .unwrap()
        .remove("inherit_pre_operations");
    value["requests"][0]
        .as_object_mut()
        .unwrap()
        .remove("inherit_post_operations");
    let old: Project = serde_json::from_value(value).unwrap();
    assert!(old.requests[0].inherit_pre_operations && old.requests[0].inherit_post_operations);
    assert!(old.folders[0].operations.is_default());
    let prepared = project.prepare_request(&project.requests[0]).unwrap();
    let value = serde_json::to_value(prepared).unwrap();
    assert!(value.get("operations_resolved").is_none());
    assert!(value["pre_operations"][0].get("origin").is_none());
}

#[test]
fn deleting_and_restoring_folder_preserves_its_operations_and_updates_descendant_plan() {
    let (mut project, root, child, request) = fixture();
    project.requests.push(request);
    let removed = project.remove_folder(child).unwrap();
    assert_eq!(project.requests[0].folder_id, Some(root));
    assert_eq!(
        project
            .operation_plan(&project.requests[0])
            .unwrap()
            .pre
            .len(),
        2
    );
    project.restore_folder(removed).unwrap();
    assert_eq!(
        project
            .operation_plan(&project.requests[0])
            .unwrap()
            .pre
            .len(),
        3
    );
}
