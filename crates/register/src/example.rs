use models::HttpMethod;
use std::sync::LazyLock;

pub static SERVICE_SIMPLES: LazyLock<Vec<(String, HttpMethod, String)>> = LazyLock::new(|| {
    vec![
        (
            "Get Users".to_string(),
            HttpMethod::Get,
            "https://api.example.com/users".to_string(),
        ),
        (
            "Create User".to_string(),
            HttpMethod::Post,
            "https://api.example.com/users".to_string(),
        ),
        (
            "httpbin json".to_string(),
            HttpMethod::Get,
            "https://httpbin.org/json".to_string(),
        ),
        (
            "httpbin stream".to_string(),
            HttpMethod::Get,
            "https://httpbin.org/stream/5".to_string(),
        ),
    ]
});
