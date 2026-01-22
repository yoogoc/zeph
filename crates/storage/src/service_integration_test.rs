// Integration tests for service parameter persistence
#[cfg(test)]
mod tests {
    use crate::{Database, FolderQueryParameterStore, FolderStore, ServiceStore};
    use models::{Folder, HttpConfig, PathParameter, QueryParameter, Service, ServiceType};
    use uuid::Uuid;

    #[test]
    fn test_service_query_params_persistence_round_trip() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a service with query parameters
        let mut http_config = HttpConfig::default();
        http_config.query_params = vec![
            QueryParameter::new("limit", "10"),
            QueryParameter::new("page", "1"),
            QueryParameter {
                key: "sort".into(),
                value: "desc".into(),
                enabled: false,
                description: Some("Sort order".into()),
            },
        ];

        let service = Service::new(
            workspace_id,
            "Test Service with Query Params",
            ServiceType::Http(http_config),
        );
        let service_id = service.id;

        // Save the service
        store.insert(&service).expect("Failed to insert service");

        // Reload the service
        let loaded_service = store
            .get(service_id)
            .expect("Failed to get service")
            .expect("Service not found");

        // Verify query parameters persisted correctly
        if let ServiceType::Http(loaded_config) = loaded_service.service_type {
            assert_eq!(loaded_config.query_params.len(), 3);

            assert_eq!(loaded_config.query_params[0].key.as_ref(), "limit");
            assert_eq!(loaded_config.query_params[0].value.as_ref(), "10");
            assert!(loaded_config.query_params[0].enabled);
            assert!(loaded_config.query_params[0].description.is_none());

            assert_eq!(loaded_config.query_params[1].key.as_ref(), "page");
            assert_eq!(loaded_config.query_params[1].value.as_ref(), "1");
            assert!(loaded_config.query_params[1].enabled);

            assert_eq!(loaded_config.query_params[2].key.as_ref(), "sort");
            assert_eq!(loaded_config.query_params[2].value.as_ref(), "desc");
            assert!(!loaded_config.query_params[2].enabled);
            assert_eq!(
                loaded_config.query_params[2]
                    .description
                    .as_ref()
                    .unwrap()
                    .as_ref(),
                "Sort order"
            );
        } else {
            panic!("Expected HTTP service type");
        }
    }

    #[test]
    fn test_service_path_params_persistence_round_trip() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a service with path parameters
        let mut http_config = HttpConfig::default();
        http_config.path = "/users/{userId}/posts/{postId}".into();
        http_config.path_params = vec![
            PathParameter::new("userId", "123"),
            PathParameter {
                name: "postId".into(),
                value: "456".into(),
                description: Some("Post identifier".into()),
            },
        ];

        let service = Service::new(
            workspace_id,
            "Test Service with Path Params",
            ServiceType::Http(http_config),
        );
        let service_id = service.id;

        // Save the service
        store.insert(&service).expect("Failed to insert service");

        // Reload the service
        let loaded_service = store
            .get(service_id)
            .expect("Failed to get service")
            .expect("Service not found");

        // Verify path parameters persisted correctly
        if let ServiceType::Http(loaded_config) = loaded_service.service_type {
            assert_eq!(loaded_config.path_params.len(), 2);

            assert_eq!(loaded_config.path_params[0].name.as_ref(), "postId");
            assert_eq!(loaded_config.path_params[0].value.as_ref(), "456");
            assert_eq!(
                loaded_config.path_params[0]
                    .description
                    .as_ref()
                    .unwrap()
                    .as_ref(),
                "Post identifier"
            );

            assert_eq!(loaded_config.path_params[1].name.as_ref(), "userId");
            assert_eq!(loaded_config.path_params[1].value.as_ref(), "123");
            assert!(loaded_config.path_params[1].description.is_none());
        } else {
            panic!("Expected HTTP service type");
        }
    }

    #[test]
    fn test_service_update_params_persistence() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a service with initial parameters
        let mut http_config = HttpConfig::default();
        http_config.query_params = vec![QueryParameter::new("limit", "10")];
        http_config.path_params = vec![PathParameter::new("id", "123")];

        let mut service = Service::new(
            workspace_id,
            "Test Service",
            ServiceType::Http(http_config.clone()),
        );
        let service_id = service.id;

        // Save the service
        store.insert(&service).expect("Failed to insert service");

        // Update the service with different parameters
        if let ServiceType::Http(ref mut config) = service.service_type {
            config.query_params = vec![
                QueryParameter::new("limit", "20"),
                QueryParameter::new("page", "2"),
            ];
            config.path_params = vec![PathParameter::new("id", "456")];
        }

        store.update(&service).expect("Failed to update service");

        // Reload and verify
        let loaded_service = store
            .get(service_id)
            .expect("Failed to get service")
            .expect("Service not found");

        if let ServiceType::Http(loaded_config) = loaded_service.service_type {
            // Verify query params were updated
            assert_eq!(loaded_config.query_params.len(), 2);
            assert_eq!(loaded_config.query_params[0].key.as_ref(), "limit");
            assert_eq!(loaded_config.query_params[0].value.as_ref(), "20");
            assert_eq!(loaded_config.query_params[1].key.as_ref(), "page");
            assert_eq!(loaded_config.query_params[1].value.as_ref(), "2");

            // Verify path params were updated
            assert_eq!(loaded_config.path_params.len(), 1);
            assert_eq!(loaded_config.path_params[0].name.as_ref(), "id");
            assert_eq!(loaded_config.path_params[0].value.as_ref(), "456");
        } else {
            panic!("Expected HTTP service type");
        }
    }

    #[test]
    fn test_service_empty_params_persistence() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a service with no parameters
        let http_config = HttpConfig::default();
        let service = Service::new(
            workspace_id,
            "Test Service No Params",
            ServiceType::Http(http_config),
        );
        let service_id = service.id;

        // Save the service
        store.insert(&service).expect("Failed to insert service");

        // Reload the service
        let loaded_service = store
            .get(service_id)
            .expect("Failed to get service")
            .expect("Service not found");

        // Verify no parameters
        if let ServiceType::Http(loaded_config) = loaded_service.service_type {
            assert_eq!(loaded_config.query_params.len(), 0);
            assert_eq!(loaded_config.path_params.len(), 0);
        } else {
            panic!("Expected HTTP service type");
        }
    }

    #[test]
    fn test_folder_common_params_persistence_round_trip() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let folder_store = FolderStore::new(db.conn());
        let param_store = FolderQueryParameterStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a folder
        let folder = Folder::new(workspace_id, "Test Folder", None);
        let folder_id = folder.id;

        folder_store
            .insert(&folder)
            .expect("Failed to insert folder");

        // Add common query parameters to the folder
        param_store
            .create(folder_id, "api_key", "secret123", true, None, 0)
            .expect("Failed to create param");
        param_store
            .create(folder_id, "version", "v2", false, Some("API version"), 1)
            .expect("Failed to create param");

        // Reload folder parameters
        let params = param_store
            .list_by_folder(folder_id)
            .expect("Failed to list params");

        // Verify parameters persisted correctly
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].key.as_ref(), "api_key");
        assert_eq!(params[0].value.as_ref(), "secret123");
        assert!(params[0].enabled);
        assert!(params[0].description.is_none());

        assert_eq!(params[1].key.as_ref(), "version");
        assert_eq!(params[1].value.as_ref(), "v2");
        assert!(!params[1].enabled);
        assert_eq!(
            params[1].description.as_ref().unwrap().as_ref(),
            "API version"
        );
    }

    #[test]
    fn test_service_with_mixed_params_persistence() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create a service with both query and path parameters
        let mut http_config = HttpConfig::default();
        http_config.path = "/api/{version}/users/{userId}".into();
        http_config.query_params = vec![
            QueryParameter::new("limit", "50"),
            QueryParameter::new("offset", "0"),
            QueryParameter {
                key: "include".into(),
                value: "profile".into(),
                enabled: false,
                description: Some("Include related data".into()),
            },
        ];
        http_config.path_params = vec![
            PathParameter::new("version", "v1"),
            PathParameter::new("userId", "42"),
        ];

        let service = Service::new(
            workspace_id,
            "Complex Service",
            ServiceType::Http(http_config),
        );
        let service_id = service.id;

        // Save the service
        store.insert(&service).expect("Failed to insert service");

        // Reload the service
        let loaded_service = store
            .get(service_id)
            .expect("Failed to get service")
            .expect("Service not found");

        // Verify all parameters persisted correctly
        if let ServiceType::Http(loaded_config) = loaded_service.service_type {
            // Verify query parameters
            assert_eq!(loaded_config.query_params.len(), 3);
            assert_eq!(loaded_config.query_params[0].key.as_ref(), "limit");
            assert_eq!(loaded_config.query_params[0].value.as_ref(), "50");
            assert!(loaded_config.query_params[0].enabled);

            assert_eq!(loaded_config.query_params[1].key.as_ref(), "offset");
            assert_eq!(loaded_config.query_params[1].value.as_ref(), "0");
            assert!(loaded_config.query_params[1].enabled);

            assert_eq!(loaded_config.query_params[2].key.as_ref(), "include");
            assert_eq!(loaded_config.query_params[2].value.as_ref(), "profile");
            assert!(!loaded_config.query_params[2].enabled);
            assert_eq!(
                loaded_config.query_params[2]
                    .description
                    .as_ref()
                    .unwrap()
                    .as_ref(),
                "Include related data"
            );

            // Verify path parameters (sorted by name)
            assert_eq!(loaded_config.path_params.len(), 2);
            assert_eq!(loaded_config.path_params[0].name.as_ref(), "userId");
            assert_eq!(loaded_config.path_params[0].value.as_ref(), "42");
            assert_eq!(loaded_config.path_params[1].name.as_ref(), "version");
            assert_eq!(loaded_config.path_params[1].value.as_ref(), "v1");
        } else {
            panic!("Expected HTTP service type");
        }
    }

    #[test]
    fn test_list_services_loads_all_params() {
        let db = Database::open_in_memory().expect("Failed to create database");
        let store = ServiceStore::new(db.conn());
        let workspace_id = Uuid::new_v4();

        // Create multiple services with parameters
        let mut http_config1 = HttpConfig::default();
        http_config1.query_params = vec![QueryParameter::new("limit", "10")];
        let service1 = Service::new(workspace_id, "Service 1", ServiceType::Http(http_config1));

        let mut http_config2 = HttpConfig::default();
        http_config2.path_params = vec![PathParameter::new("id", "123")];
        let service2 = Service::new(workspace_id, "Service 2", ServiceType::Http(http_config2));

        store.insert(&service1).expect("Failed to insert service1");
        store.insert(&service2).expect("Failed to insert service2");

        // List all services
        let services = store
            .list_by_workspace(workspace_id)
            .expect("Failed to list services");

        assert_eq!(services.len(), 2);

        // Verify first service has query params
        if let ServiceType::Http(ref config) = services[0].service_type {
            assert_eq!(config.query_params.len(), 1);
            assert_eq!(config.query_params[0].key.as_ref(), "limit");
        }

        // Verify second service has path params
        if let ServiceType::Http(ref config) = services[1].service_type {
            assert_eq!(config.path_params.len(), 1);
            assert_eq!(config.path_params[0].name.as_ref(), "id");
        }
    }
}
