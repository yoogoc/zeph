use std::collections::HashSet;
use uuid::Uuid;
use zeph::{
    collections::CollectionRow,
    model::{OpenApiOperation, Project, Protocol, Request},
    openapi,
    storage::{ProjectStore, save_contract},
};

#[test]
fn legacy_projects_keep_flat_requests_and_no_fake_bindings() {
    let mut value = serde_json::to_value(Project::examples()).unwrap();
    value.as_object_mut().unwrap().remove("folders");
    for request in value["requests"].as_array_mut().unwrap() {
        request.as_object_mut().unwrap().remove("folder_id");
        request.as_object_mut().unwrap().remove("openapi");
    }
    let mut project: Project = serde_json::from_value(value).unwrap();
    project.normalize().unwrap();
    assert!(project.folders.is_empty());
    assert!(
        project
            .requests
            .iter()
            .all(|r| r.folder_id.is_none() && r.openapi.is_none())
    );
    assert_eq!(project.collection_rows("").len(), project.requests.len());
}

#[test]
fn folders_preserve_identity_tabs_and_bindings_across_moves_and_saves() {
    let mut p = Project::examples();
    let id = p.requests[0].id;
    let parent = p.add_folder("商城", None).unwrap();
    let child = p.add_folder("订单", Some(parent)).unwrap();
    let other = p.add_folder("客户", None).unwrap();
    let binding = OpenApiOperation {
        contract_id: Uuid::new_v4(),
        path: "/orders/{id}".into(),
        method: "get".into(),
        operation_id: Some("getOrder".into()),
        tags: vec!["Orders".into(), "Public".into()],
    };
    p.requests[0].openapi = Some(binding.clone());
    let url = p.requests[0].url.clone();
    let query = serde_json::to_value(&p.requests[0].query).unwrap();
    p.move_request_to_folder(id, Some(child)).unwrap();
    p.rename_folder(child, "查询订单").unwrap();
    p.move_folder(child, Some(other)).unwrap();
    assert_eq!(p.folder_path(child), "客户 / 查询订单");
    assert_eq!(p.requests[0].openapi, Some(binding));
    assert_eq!(p.requests[0].url, url);
    assert_eq!(serde_json::to_value(&p.requests[0].query).unwrap(), query);
    assert_eq!(p.tabs(), &[id]);
    let directory = tempfile::tempdir().unwrap();
    let store = ProjectStore::new(directory.path());
    store.save(&p).unwrap();
    let restored = store.load().unwrap();
    assert_eq!(restored.requests[0].folder_id, Some(child));
    assert_eq!(restored.folder_path(child), "客户 / 查询订单");
    assert_eq!(restored.tabs(), &[id]);
}

#[test]
fn cycles_duplicate_names_and_invalid_targets_are_rejected_atomically() {
    let mut p = Project::empty("Test");
    let a = p.add_folder("A", None).unwrap();
    let b = p.add_folder("B", Some(a)).unwrap();
    let c = p.add_folder("B", None).unwrap();
    assert!(p.move_folder(a, Some(b)).is_err());
    assert!(p.move_folder(a, Some(a)).is_err());
    assert!(p.move_folder(b, None).is_err());
    assert!(p.rename_folder(a, " B ").is_err());
    assert!(p.add_folder(" ", None).is_err());
    assert!(p.add_folder("A", None).is_err());
    assert!(p.move_folder(c, Some(Uuid::new_v4())).is_err());
    assert_eq!(p.folder(a).unwrap().parent_id, None);
    assert_eq!(p.folder(b).unwrap().parent_id, Some(a));
    p.normalize().unwrap();
}

#[test]
fn deleting_a_folder_promotes_content_and_undo_does_not_rebind_later_moves() {
    let mut p = Project::examples();
    let id = p.requests[0].id;
    let a = p.add_folder("A", None).unwrap();
    let b = p.add_folder("A", Some(a)).unwrap();
    let other = p.add_folder("Other", None).unwrap();
    p.move_request_to_folder(id, Some(a)).unwrap();
    let removed = p.remove_folder(a).unwrap();
    assert_eq!(p.folder(b).unwrap().parent_id, None);
    assert_eq!(p.requests[0].folder_id, None);
    assert_eq!(p.tabs(), &[id]);
    p.move_request_to_folder(id, Some(other)).unwrap();
    p.restore_folder(removed).unwrap();
    assert_eq!(p.folder(b).unwrap().parent_id, Some(a));
    assert_eq!(p.requests[0].folder_id, Some(other));
    let removed = p.remove_folder(a).unwrap();
    p.restore_folder(removed).unwrap();
    p.normalize().unwrap();
}

#[test]
fn folder_delete_name_collisions_leave_everything_untouched() {
    let mut p = Project::empty("Test");
    let a = p.add_folder("A", None).unwrap();
    let b = p.add_folder("B", Some(a)).unwrap();
    p.add_folder("B", None).unwrap();
    let before = serde_json::to_value(&p).unwrap();
    assert!(p.remove_folder(a).is_err());
    assert_eq!(serde_json::to_value(&p).unwrap(), before);
    assert_eq!(p.folder(b).unwrap().parent_id, Some(a));
}

#[test]
fn undo_after_deepening_a_promoted_tree_is_atomic_and_can_be_retried() {
    let mut p = Project::empty("Test");
    let root = p.add_folder("Root", None).unwrap();
    let mut parent = Some(root);
    for i in 0..31 {
        parent = Some(p.add_folder(&format!("Level {i}"), parent).unwrap());
    }
    let removed = p.remove_folder(root).unwrap();
    let leaf = p.add_folder("New leaf", parent).unwrap();
    let before = serde_json::to_value(&p).unwrap();
    assert!(p.restore_folder(removed.clone()).is_err());
    assert_eq!(serde_json::to_value(&p).unwrap(), before);
    p.remove_folder(leaf).unwrap();
    p.restore_folder(removed).unwrap();
    p.normalize().unwrap();
}

#[test]
fn search_reveals_collapsed_ancestors_and_can_match_a_folder() {
    let mut p = Project::empty("Test");
    let root = p.add_folder("商城", None).unwrap();
    let child = p.add_folder("订单", Some(root)).unwrap();
    let mut r = Request::new(Protocol::Http, "查询详情");
    r.folder_id = Some(child);
    p.requests.push(r);
    p.requests.push(Request::new(Protocol::Grpc, "Other"));
    p.folders.iter_mut().for_each(|f| f.expanded = false);
    assert_eq!(p.collection_rows("").len(), 2);
    for query in ["查询详情", "订单", "商城"] {
        let rows = p.collection_rows(query);
        assert!(
            matches!(&rows[..], [CollectionRow::Folder { id:a,.. },CollectionRow::Folder { id:b,.. },CollectionRow::Request { index:0,depth:2 }] if *a==root && *b==child)
        );
    }
    assert!(p.folders.iter().all(|f| !f.expanded));
    assert!(p.collection_rows("missing").is_empty());
}

fn spec(version: &str) -> serde_json::Value {
    let mut doc = serde_json::json!({"info":{"title":"Shop","version":"1"},"tags":[{"name":"Orders"},{"name":"Customers"},{"name":"Empty"}],"paths":{"/orders/{id}":{"get":{"operationId":"getOrder","tags":["Orders","Public","Orders"],"responses":{"200":{"description":"ok"}}}},"/customers":{"post":{"tags":["Customers"],"responses":{"200":{"description":"ok"}}}},"/untagged":{"get":{"responses":{"200":{"description":"ok"}}}},"/nested":{"get":{"tags":["Orders/Returns"],"responses":{"200":{"description":"ok"}}}}}});
    doc[if version == "2.0" {
        "swagger"
    } else {
        "openapi"
    }] = version.into();
    doc
}

#[test]
fn all_openapi_versions_map_document_tag_and_operation_without_duplicate_requests() {
    let temp = tempfile::tempdir().unwrap();
    for version in ["2.0", "3.0.3", "3.1.0"] {
        let path = temp.path().join(format!("{version}.json"));
        let doc = spec(version);
        std::fs::write(&path, doc.to_string()).unwrap();
        let import = openapi::import(&path).unwrap();
        assert_eq!(import.document, doc);
        assert_eq!(import.requests.len(), 4);
        let mut p = Project::empty("Test");
        p.folders = import.folders.clone();
        p.requests = import.requests.clone();
        p.normalize().unwrap();
        let root = p
            .folders
            .iter()
            .find(|f| f.openapi.as_ref().is_some_and(|b| b.tag.is_none()))
            .unwrap();
        assert_eq!(root.name, "Shop");
        let order = p
            .requests
            .iter()
            .find(|r| r.openapi.as_ref().unwrap().path == "/orders/{id}")
            .unwrap();
        let mapping = order.openapi.as_ref().unwrap();
        assert_eq!(mapping.contract_id, import.contract_id);
        assert_eq!(mapping.tags, vec!["Orders", "Public"]);
        assert_eq!(mapping.operation_id.as_deref(), Some("getOrder"));
        assert_eq!(mapping.method, "get");
        assert_eq!(
            p.folder(order.folder_id.unwrap())
                .unwrap()
                .openapi
                .as_ref()
                .unwrap()
                .tag
                .as_deref(),
            Some("Orders")
        );
        assert_eq!(
            p.requests
                .iter()
                .find(|r| r.openapi.as_ref().unwrap().path == "/untagged")
                .unwrap()
                .folder_id,
            Some(root.id)
        );
        let slash = p
            .folders
            .iter()
            .find(|f| f.name == "Orders/Returns")
            .unwrap();
        assert_eq!(slash.parent_id, Some(root.id));
        assert!(p.folders.iter().any(|f| f.name == "Empty"));
        save_contract(&temp.path().join("contracts"), &import).unwrap();
        assert!(
            temp.path()
                .join("contracts")
                .join(import.contract_id.to_string())
                .join("original.json")
                .exists()
        );
    }
}

#[test]
fn separate_documents_with_the_same_title_and_tag_do_not_merge() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("api.json");
    std::fs::write(&path, spec("3.1.0").to_string()).unwrap();
    let a = openapi::import(&path).unwrap();
    let b = openapi::import(&path).unwrap();
    assert_ne!(a.contract_id, b.contract_id);
    let mut p = Project::empty("Test");
    p.merge_collection(a.folders, a.requests).unwrap();
    p.merge_collection(b.folders, b.requests).unwrap();
    p.normalize().unwrap();
    let roots = p
        .folders
        .iter()
        .filter(|f| f.parent_id.is_none())
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    assert_ne!(roots[0].name, roots[1].name);
    assert_eq!(roots.iter().map(|f| f.id).collect::<HashSet<_>>().len(), 2);
    assert_eq!(p.requests.len(), 8);
}

#[test]
fn legacy_imported_requests_migrate_from_local_contracts_once() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("api.json");
    std::fs::write(&source, spec("3.1.0").to_string()).unwrap();
    let import = openapi::import(&source).unwrap();
    let store = ProjectStore::new(temp.path().join("workspace"));
    save_contract(&store.directory.join("contracts"), &import).unwrap();
    let mut p = Project::empty("Legacy");
    p.requests = import.requests;
    for r in &mut p.requests {
        r.folder_id = None;
        r.openapi = None;
    }
    let ids = p.requests.iter().map(|r| r.id).collect::<Vec<_>>();
    store.save(&p).unwrap();
    std::fs::remove_file(source).unwrap();
    let restored = store.load().unwrap();
    assert_eq!(
        restored.requests.iter().map(|r| r.id).collect::<Vec<_>>(),
        ids
    );
    assert!(
        restored.requests.iter().all(|r| r.folder_id.is_some()
            && r.openapi.as_ref().unwrap().contract_id == import.contract_id)
    );
    let snapshot = std::fs::read(store.directory.join("zeph.project.json")).unwrap();
    let again = store.load().unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(again).unwrap()
    );
    assert_eq!(
        std::fs::read(store.directory.join("zeph.project.json")).unwrap(),
        snapshot
    );
}

#[test]
fn corrupt_folder_trees_are_rejected_without_overwriting_the_project() {
    let temp = tempfile::tempdir().unwrap();
    let store = ProjectStore::new(temp.path());
    let mut p = Project::examples();
    let a = p.add_folder("A", None).unwrap();
    let b = p.add_folder("B", Some(a)).unwrap();
    p.folders[0].parent_id = Some(b);
    store.save(&p).unwrap();
    let path = temp.path().join("zeph.project.json");
    let bytes = std::fs::read(&path).unwrap();
    assert!(store.load().is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn legacy_ambiguous_snapshots_are_not_bound_to_an_arbitrary_document() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("api.json");
    std::fs::write(&path, spec("3.1.0").to_string()).unwrap();
    let a = openapi::import(&path).unwrap();
    let b = openapi::import(&path).unwrap();
    let store = ProjectStore::new(temp.path().join("workspace"));
    save_contract(&store.directory.join("contracts"), &a).unwrap();
    save_contract(&store.directory.join("contracts"), &b).unwrap();
    let mut p = Project::empty("Legacy");
    p.requests = a.requests;
    for r in &mut p.requests {
        r.openapi = None;
        r.folder_id = None;
    }
    store.save(&p).unwrap();
    let restored = store.load().unwrap();
    assert!(restored.folders.is_empty());
    assert!(restored.requests.iter().all(|r| r.openapi.is_none()));
}

#[test]
fn referenced_path_items_keep_root_operation_identity_and_tag_mapping() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("api.yaml");
    std::fs::write(temp.path().join("routes.yaml"),"item:\n  get:\n    tags: [Orders]\n    operationId: listOrders\n    responses: {'200': {description: ok}}\n").unwrap();
    std::fs::write(&path,"openapi: 3.1.0\ninfo: {title: Referenced, version: '1'}\npaths:\n  /orders:\n    $ref: './routes.yaml#/item'\n").unwrap();
    let import = openapi::import(&path).unwrap();
    let r = &import.requests[0];
    let mapping = r.openapi.as_ref().unwrap();
    assert_eq!(mapping.path, "/orders");
    assert_eq!(mapping.operation_id.as_deref(), Some("listOrders"));
    assert_eq!(mapping.tags, vec!["Orders"]);
    let folder = import
        .folders
        .iter()
        .find(|f| Some(f.id) == r.folder_id)
        .unwrap();
    assert_eq!(
        folder.openapi.as_ref().unwrap().contract_id,
        import.contract_id
    );
    assert_eq!(import.documents.len(), 2);
}

#[test]
fn failed_collection_merge_is_atomic() {
    let mut p = Project::examples();
    let before = serde_json::to_value(&p).unwrap();
    let duplicate = p.requests[0].clone();
    assert!(p.merge_collection(vec![], vec![duplicate]).is_err());
    assert_eq!(serde_json::to_value(p).unwrap(), before);
}
