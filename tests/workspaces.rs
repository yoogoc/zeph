use zeph::{
    model::{Project, Protocol, Request},
    storage::{ProjectStore, WorkspaceSession},
    transport::RunRecord,
};

#[test]
fn legacy_projects_migrate_and_closed_tabs_stay_closed() {
    let mut json = serde_json::to_value(Project::examples()).unwrap();
    json.as_object_mut().unwrap().remove("opened_requests");
    json.as_object_mut().unwrap().remove("panes");
    json["active_request"] = 2.into();
    let mut project: Project = serde_json::from_value(json).unwrap();
    project.normalize().unwrap();
    assert_eq!(project.panes.sidebar, 260.);
    project.panes.sidebar = 318.;
    project.panes.request_height = 278.;
    let id = project.requests[2].id;
    assert_eq!(project.tabs(), &[id]);
    project.open_request(id);
    project.open_request(id);
    assert_eq!(project.tabs().len(), 1);
    project.close_request(id);
    assert!(project.tabs().is_empty());
    assert_eq!(project.requests.len(), zeph::model::HTTP_METHODS.len() + 2);
    let dir = tempfile::tempdir().unwrap();
    let store = ProjectStore::new(dir.path());
    store.save(&project).unwrap();
    let restored = store.load().unwrap();
    assert!(restored.tabs().is_empty());
    assert_eq!(restored.panes.sidebar, 318.);
    assert_eq!(restored.panes.request_height, 278.);
}

#[test]
fn deleting_requests_keeps_active_identity_and_handles_empty_collections() {
    let mut project = Project::examples();
    let ids = project
        .requests
        .iter()
        .take(3)
        .map(|r| r.id)
        .collect::<Vec<_>>();
    for id in &ids {
        project.open_request(*id);
    }
    project.delete_request(ids[0]).unwrap();
    assert_eq!(project.requests[project.active_request].id, ids[2]);
    project.delete_request(ids[2]).unwrap();
    assert_eq!(project.requests[project.active_request].id, ids[1]);
    let removed = project.delete_request(ids[1]).unwrap();
    assert!(project.tabs().is_empty());
    project.requests.insert(removed.0, removed.1);
    project.open_request(ids[1]);
    assert_eq!(project.requests[project.active_request].id, ids[1]);
    let remaining = project.requests.iter().map(|r| r.id).collect::<Vec<_>>();
    for id in remaining {
        project.delete_request(id).unwrap();
    }
    project.normalize().unwrap();
    assert!(project.requests.is_empty());
    let dir = tempfile::tempdir().unwrap();
    let store = ProjectStore::new(dir.path());
    store.save(&project).unwrap();
    assert!(store.load().unwrap().requests.is_empty());
}

#[test]
fn invalid_tab_ids_are_pruned_and_duplicate_request_ids_are_rejected() {
    let mut project = Project::examples();
    let id = project.requests[3].id;
    project.opened_requests = Some(vec![uuid::Uuid::new_v4(), id, id]);
    project.normalize().unwrap();
    assert_eq!(project.tabs(), &[id]);
    assert_eq!(project.requests[project.active_request].id, id);
    project.requests.push(project.requests[0].clone());
    assert!(project.normalize().is_err());
}

#[test]
fn workspaces_isolate_requests_environments_and_history() {
    let root = tempfile::tempdir().unwrap();
    let a = ProjectStore::new(root.path().join("a"));
    let b = ProjectStore::new(root.path().join("b"));
    let mut pa = a.create("A").unwrap();
    let pb = b.create("B").unwrap();
    assert!(pa.requests.is_empty() && pb.requests.is_empty());
    let mut request = Request::new(Protocol::Http, "Only A");
    request.body.text = "draft A".into();
    pa.requests.push(request.clone());
    pa.open_request(request.id);
    pa.environments[0]
        .variables
        .insert("base_url".into(), "http://a.local".into());
    a.save_versioned(&pa, 2).unwrap();
    b.save_versioned(&pb, 100).unwrap();
    a.record(&RunRecord::error(&request, "A".into(), "A history".into()))
        .unwrap();
    assert_eq!(a.load().unwrap().requests[0].body.text, "draft A");
    assert!(b.load().unwrap().requests.is_empty());
    assert_ne!(a.load().unwrap().variables(), b.load().unwrap().variables());
    assert_eq!(a.history().unwrap().len(), 1);
    assert!(b.history().unwrap().is_empty());
    assert!(a.create("overwrite").is_err());
    assert_eq!(a.load().unwrap().name, "A");
    let missing = root.path().join("missing");
    assert!(ProjectStore::open_existing(&missing).is_err());
    assert!(!missing.exists());
    let (opened, _) = ProjectStore::open_existing(root.path().join("a/../a")).unwrap();
    assert_eq!(
        opened.directory,
        std::fs::canonicalize(&a.directory).unwrap()
    );
}

#[test]
fn workspace_session_restores_paths_and_preserves_corrupt_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("session.json");
    let a = root.path().join("a");
    let b = root.path().join("b");
    WorkspaceSession {
        directories: vec![a.clone(), b.clone(), a.clone()],
        active_directory: Some(b.clone()),
    }
    .save(&path)
    .unwrap();
    let restored = WorkspaceSession::load(&path).unwrap();
    assert_eq!(restored.directories, vec![a, b.clone()]);
    assert_eq!(restored.active_directory, Some(b));
    WorkspaceSession::default().save(&path).unwrap();
    assert!(
        WorkspaceSession::load(&path)
            .unwrap()
            .directories
            .is_empty()
    );
    std::fs::write(&path, "broken").unwrap();
    assert!(WorkspaceSession::load(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken");
}

#[test]
fn old_history_without_trailers_and_timeline_remains_readable() {
    let request = Request::new(Protocol::Grpc, "old");
    let mut json =
        serde_json::to_value(RunRecord::error(&request, "Local".into(), "".into())).unwrap();
    json.as_object_mut().unwrap().remove("trailers");
    json.as_object_mut().unwrap().remove("messages");
    json.as_object_mut().unwrap().remove("timeline");
    let record: RunRecord = serde_json::from_value(json).unwrap();
    assert!(record.trailers.is_empty() && record.timeline.is_empty() && record.messages.is_empty());
}

#[test]
fn encoding_preview_expands_paths_and_keeps_duplicate_form_fields() {
    use std::collections::BTreeMap;
    use zeph::model::{BodyKind, Parameter};
    let mut request = Request::new(Protocol::Http, "preview");
    request.method = "POST".into();
    request.url = "{{base}}/items/{id}".into();
    request.path.push(Parameter::new("id", "a/b"));
    request.body.kind = BodyKind::UrlEncoded;
    request.body.fields = vec![
        Parameter::new("tag", "a b"),
        Parameter::new("tag", "{{tag}}"),
    ];
    request
        .headers
        .push(Parameter::new("Authorization", "secret"));
    let vars = BTreeMap::from([
        ("base".into(), "http://localhost".into()),
        ("tag".into(), "c".into()),
    ]);
    let preview = request.encoding_preview(&vars).unwrap();
    assert!(preview.starts_with("POST http://localhost/items/a%2Fb"));
    assert!(preview.ends_with("tag=a+b&tag=c"));
    assert!(!preview.contains("secret"));
    request.body.kind = BodyKind::FormData;
    request.body.fields[1].file = true;
    assert!(
        request
            .encoding_preview(&vars)
            .unwrap()
            .contains("tag [File]: c")
    );
    request.body.kind = BodyKind::Json;
    request.body.text = "bad json".into();
    assert!(request.encoding_preview(&vars).is_err());
}

#[test]
fn logical_workspaces_have_stable_identity_and_remain_available_after_closing() {
    use zeph::storage::WorkspaceCatalog;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workspaces.json");
    let mut catalog = WorkspaceCatalog::default();
    // Names are labels; even identical names must not identify the workspace.
    let (a, mut project) = catalog.create(&path, "  商城服务  ").unwrap();
    let (b, _) = catalog.create(&path, "商城服务").unwrap();
    assert_ne!(a.id, b.id);
    assert_eq!(a.name, "商城服务");
    assert_eq!(
        a.store().directory,
        root.path().join("workspaces").join(a.id.to_string())
    );
    project
        .requests
        .push(Request::new(Protocol::Http, "A draft"));
    project.name = "订单服务".into();
    a.store().save(&project).unwrap();
    catalog.workspaces[0].name = project.name.clone();
    catalog.open(a.id).unwrap();
    catalog.open(a.id).unwrap();
    assert_eq!(catalog.opened, vec![a.id, b.id]);
    catalog.close(a.id);
    assert_eq!(catalog.active, Some(b.id));
    catalog.close(b.id);
    catalog.save(&path).unwrap();
    let mut restored = WorkspaceCatalog::load(&path).unwrap();
    assert!(restored.opened.is_empty());
    assert_eq!(restored.workspaces.len(), 2);
    assert_eq!(restored.entry(a.id).unwrap().name, "订单服务");
    assert_eq!(
        restored.entry(a.id).unwrap().store().directory,
        a.store().directory
    );
    restored.open(a.id).unwrap();
    assert_eq!(restored.active, Some(a.id));
    assert_eq!(
        restored
            .entry(a.id)
            .unwrap()
            .store()
            .load()
            .unwrap()
            .requests[0]
            .name,
        "A draft"
    );
    assert!(restored.create(&path, "   ").is_err());
    assert!(restored.open(uuid::Uuid::new_v4()).is_err());
}

#[test]
fn legacy_directory_sessions_migrate_without_rewriting_data() {
    use zeph::storage::WorkspaceCatalog;
    let root = tempfile::tempdir().unwrap();
    let a = ProjectStore::new(root.path().join("legacy-a"));
    let b = ProjectStore::new(root.path().join("legacy-b"));
    let mut pa = a.create("A").unwrap();
    b.create("B").unwrap();
    let request = Request::new(Protocol::Http, "saved request");
    pa.requests.push(request.clone());
    pa.open_request(request.id);
    a.save(&pa).unwrap();
    a.record(&RunRecord::error(
        &request,
        "Local".into(),
        "saved history".into(),
    ))
    .unwrap();
    let before = std::fs::read(a.directory.join("zeph.project.json")).unwrap();
    let missing = root.path().join("temporarily-missing");
    let catalog = WorkspaceCatalog::migrate_session(WorkspaceSession {
        directories: vec![
            a.directory.clone(),
            b.directory.clone(),
            a.directory.join("../legacy-a"),
            missing.clone(),
        ],
        active_directory: Some(a.directory.clone()),
    })
    .unwrap();
    assert_eq!(catalog.workspaces.len(), 3);
    assert_eq!(catalog.opened.len(), 3);
    assert_eq!(catalog.active, Some(catalog.workspaces[0].id));
    assert!(!missing.exists());
    assert_eq!(catalog.workspaces[0].store().history().unwrap().len(), 1);
    assert_eq!(
        std::fs::read(a.directory.join("zeph.project.json")).unwrap(),
        before
    );
    let path = root.path().join("workspaces.json");
    catalog.save(&path).unwrap();
    let restored = WorkspaceCatalog::load(&path).unwrap();
    assert_eq!(restored.workspaces[0].id, catalog.workspaces[0].id);
    assert!(
        restored.workspaces[0]
            .store()
            .load()
            .unwrap()
            .tabs()
            .contains(&request.id)
    );
    let empty = WorkspaceCatalog::migrate_session(WorkspaceSession::default()).unwrap();
    assert!(empty.workspaces.is_empty() && empty.opened.is_empty());
}

#[test]
fn workspace_catalog_prunes_invalid_tabs_and_preserves_invalid_files() {
    use zeph::storage::WorkspaceCatalog;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("workspaces.json");
    let mut catalog = WorkspaceCatalog::default();
    let (entry, _) = catalog.create(&path, "A").unwrap();
    catalog.opened.extend([entry.id, uuid::Uuid::new_v4()]);
    catalog.active = Some(uuid::Uuid::new_v4());
    catalog.save(&path).unwrap();
    let restored = WorkspaceCatalog::load(&path).unwrap();
    assert_eq!(restored.opened, vec![entry.id]);
    assert_eq!(restored.active, Some(entry.id));
    catalog.workspaces.push(entry);
    catalog.save(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(WorkspaceCatalog::load(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::write(&path, "broken").unwrap();
    assert!(WorkspaceCatalog::load(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken");
}

#[test]
fn catalog_bootstrap_preserves_closed_legacy_workspace_and_never_reseeds_existing_catalog() {
    use zeph::storage::WorkspaceCatalog;
    let root = tempfile::tempdir().unwrap();
    let legacy = ProjectStore::new(root.path().join("old-default"));
    legacy.create("保留的工作区").unwrap();
    let path = root.path().join("workspaces.json");
    let catalog =
        WorkspaceCatalog::initialize(&path, &legacy, Some(WorkspaceSession::default())).unwrap();
    assert_eq!(catalog.workspaces.len(), 1);
    assert!(catalog.opened.is_empty());
    let id = catalog.workspaces[0].id;
    let restored = WorkspaceCatalog::initialize(&path, &legacy, None).unwrap();
    assert!(restored.opened.is_empty());
    assert_eq!(restored.workspaces[0].id, id);
    WorkspaceCatalog::default().save(&path).unwrap();
    let empty = WorkspaceCatalog::initialize(&path, &legacy, None).unwrap();
    assert!(empty.workspaces.is_empty());
    let pristine = root.path().join("pristine/workspaces.json");
    let initialized = WorkspaceCatalog::initialize(
        &pristine,
        &ProjectStore::new(root.path().join("missing")),
        None,
    )
    .unwrap();
    assert_eq!(initialized.workspaces.len(), 1);
    assert!(
        !initialized.workspaces[0]
            .store()
            .load()
            .unwrap()
            .requests
            .is_empty()
    );
    assert!(!root.path().join("missing").exists());
}
