use models::{PathParameter, QueryParameter};
use regex::Regex;
use std::sync::OnceLock;

/// Detects path parameter placeholders in a URL
/// Supports format: {paramName}
///
/// # Examples
/// ```
/// use ui::param_utils::detect_path_params;
///
/// let url = "https://api.example.com/users/{userId}/posts/{postId}";
/// let params = detect_path_params(url);
/// assert_eq!(params, vec!["userId", "postId"]);
/// ```
pub fn detect_path_params(url: &str) -> Vec<String> {
    static PATH_PARAM_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = PATH_PARAM_REGEX.get_or_init(|| {
        Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\}").expect("Invalid regex pattern")
    });

    regex
        .captures_iter(url)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

/// Substitutes path parameter placeholders with values
///
/// # Examples
/// ```
/// use models::PathParameter;
/// use ui::param_utils::substitute_path_params;
///
/// let url = "https://api.example.com/users/{userId}/posts/{postId}";
/// let params = vec![
///     PathParameter::new("userId", "123"),
///     PathParameter::new("postId", "456"),
/// ];
/// let result = substitute_path_params(url, &params);
/// assert_eq!(result, "https://api.example.com/users/123/posts/456");
/// ```
pub fn substitute_path_params(url: &str, params: &[PathParameter]) -> String {
    let mut result = url.to_string();

    for param in params {
        if !param.value.is_empty() {
            let placeholder = format!("{{{}}}", param.name);
            let encoded_value = encode_param_value(&param.value, true);
            result = result.replace(&placeholder, &encoded_value);
        }
    }

    result
}

/// Builds URL with query parameters
///
/// # Examples
/// ```
/// use models::QueryParameter;
/// use ui::param_utils::build_url_with_query_params;
///
/// let base_url = "https://api.example.com/users";
/// let params = vec![
///     QueryParameter::new("limit", "10"),
///     QueryParameter::new("page", "2"),
/// ];
/// let result = build_url_with_query_params(base_url, &params);
/// assert_eq!(result, "https://api.example.com/users?limit=10&page=2");
/// ```
pub fn build_url_with_query_params(base_url: &str, params: &[QueryParameter]) -> String {
    // Filter enabled params with non-empty keys
    let enabled_params: Vec<_> = params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .collect();

    if enabled_params.is_empty() {
        return base_url.to_string();
    }

    // Build query string
    let query_string: Vec<String> = enabled_params
        .iter()
        .map(|p| {
            let encoded_key = encode_param_value(&p.key, false);
            let encoded_value = encode_param_value(&p.value, false);
            format!("{}={}", encoded_key, encoded_value)
        })
        .collect();

    // Determine separator based on whether URL already has query string
    let separator = if base_url.contains('?') { '&' } else { '?' };

    format!("{}{}{}", base_url, separator, query_string.join("&"))
}

/// Merges folder and service query parameters
/// Service parameters override folder parameters with same key
///
/// # Examples
/// ```
/// use models::QueryParameter;
/// use ui::param_utils::merge_query_params;
///
/// let folder_params = vec![
///     QueryParameter::new("limit", "10"),
///     QueryParameter::new("sort", "asc"),
/// ];
/// let service_params = vec![
///     QueryParameter::new("limit", "20"), // Override
///     QueryParameter::new("page", "1"),   // New
/// ];
/// let merged = merge_query_params(&folder_params, &service_params);
/// // Result: limit=20 (service override), sort=asc (folder), page=1 (service)
/// ```
pub fn merge_query_params(
    folder_params: &[QueryParameter],
    service_params: &[QueryParameter],
) -> Vec<QueryParameter> {
    let mut result = Vec::new();
    let mut service_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Add all service parameters first
    for param in service_params {
        result.push(param.clone());
        service_keys.insert(param.key.to_string());
    }

    // Add folder parameters that aren't overridden by service parameters
    for param in folder_params {
        if !service_keys.contains(param.key.as_ref()) {
            result.push(param.clone());
        }
    }

    result
}

/// URL-encodes a string according to RFC 3986
///
/// # Arguments
/// * `value` - The string to encode
/// * `is_path` - If true, encodes for path segments; if false, encodes for query parameters
///
/// # Examples
/// ```
/// use ui::param_utils::encode_param_value;
///
/// let encoded = encode_param_value("hello world", false);
/// assert_eq!(encoded, "hello%20world");
///
/// let encoded = encode_param_value("user@example.com", true);
/// assert_eq!(encoded, "user%40example.com");
/// ```
pub fn encode_param_value(value: &str, is_path: bool) -> String {
    // Check if already encoded to avoid double-encoding
    if is_already_encoded(value) {
        return value.to_string();
    }

    if is_path {
        // For path parameters, encode all reserved characters except unreserved ones
        // Unreserved: A-Z a-z 0-9 - _ . ~
        percent_encoding::utf8_percent_encode(value, PATH_ENCODE_SET).to_string()
    } else {
        // For query parameters, encode reserved characters
        // Allow unreserved characters and some safe characters
        percent_encoding::utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
    }
}

/// Validates query parameter key
///
/// # Examples
/// ```
/// use ui::param_utils::is_valid_query_param_key;
///
/// assert!(is_valid_query_param_key("limit"));
/// assert!(is_valid_query_param_key("user_id"));
/// assert!(is_valid_query_param_key("filter[name]"));
/// assert!(!is_valid_query_param_key(""));
/// ```
pub fn is_valid_query_param_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }

    // Allow alphanumeric, underscore, hyphen, dot, and brackets (for array notation)
    static KEY_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = KEY_REGEX
        .get_or_init(|| Regex::new(r"^[a-zA-Z0-9_\-.\[\]]+$").expect("Invalid regex pattern"));

    regex.is_match(key)
}

/// Validates path parameter name
///
/// # Examples
/// ```
/// use ui::param_utils::is_valid_path_param_name;
///
/// assert!(is_valid_path_param_name("userId"));
/// assert!(is_valid_path_param_name("post_id"));
/// assert!(is_valid_path_param_name("_id"));
/// assert!(!is_valid_path_param_name("123invalid"));
/// assert!(!is_valid_path_param_name(""));
/// ```
pub fn is_valid_path_param_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // Must start with letter or underscore, followed by alphanumeric or underscore
    static NAME_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = NAME_REGEX
        .get_or_init(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").expect("Invalid regex pattern"));

    regex.is_match(name)
}

/// Checks if a string is already percent-encoded
fn is_already_encoded(value: &str) -> bool {
    static ENCODED_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = ENCODED_REGEX
        .get_or_init(|| Regex::new(r"%[0-9A-Fa-f]{2}").expect("Invalid regex pattern"));

    // If it contains valid percent-encoded sequences and decoding changes it, it's encoded
    if regex.is_match(value) {
        if let Ok(decoded) = percent_encoding::percent_decode_str(value).decode_utf8() {
            return decoded.as_ref() != value;
        }
    }

    false
}

// Define custom encode sets for RFC 3986 compliance
use percent_encoding::{AsciiSet, CONTROLS};

/// Characters to encode in path segments
/// Encode everything except: A-Z a-z 0-9 - _ . ~
const PATH_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b']');

/// Characters to encode in query parameters
/// Encode everything except: A-Z a-z 0-9 - _ . ~ and some safe characters
const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'&')
    .add(b'=')
    .add(b'+');

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_path_params_single() {
        let url = "https://api.example.com/users/{userId}";
        let params = detect_path_params(url);
        assert_eq!(params, vec!["userId"]);
    }

    #[test]
    fn test_detect_path_params_multiple() {
        let url = "https://api.example.com/users/{userId}/posts/{postId}";
        let params = detect_path_params(url);
        assert_eq!(params, vec!["userId", "postId"]);
    }

    #[test]
    fn test_detect_path_params_none() {
        let url = "https://api.example.com/users";
        let params = detect_path_params(url);
        assert!(params.is_empty());
    }

    #[test]
    fn test_detect_path_params_with_underscore() {
        let url = "https://api.example.com/users/{user_id}/posts/{post_id}";
        let params = detect_path_params(url);
        assert_eq!(params, vec!["user_id", "post_id"]);
    }

    #[test]
    fn test_detect_path_params_malformed() {
        let url = "https://api.example.com/users/{}/posts/{incomplete";
        let params = detect_path_params(url);
        assert!(params.is_empty());
    }

    #[test]
    fn test_substitute_path_params_single() {
        let url = "https://api.example.com/users/{userId}";
        let params = vec![PathParameter::new("userId", "123")];
        let result = substitute_path_params(url, &params);
        assert_eq!(result, "https://api.example.com/users/123");
    }

    #[test]
    fn test_substitute_path_params_multiple() {
        let url = "https://api.example.com/users/{userId}/posts/{postId}";
        let params = vec![
            PathParameter::new("userId", "123"),
            PathParameter::new("postId", "456"),
        ];
        let result = substitute_path_params(url, &params);
        assert_eq!(result, "https://api.example.com/users/123/posts/456");
    }

    #[test]
    fn test_substitute_path_params_empty_value() {
        let url = "https://api.example.com/users/{userId}";
        let params = vec![PathParameter::new("userId", "")];
        let result = substitute_path_params(url, &params);
        assert_eq!(result, "https://api.example.com/users/{userId}");
    }

    #[test]
    fn test_substitute_path_params_partial() {
        let url = "https://api.example.com/users/{userId}/posts/{postId}";
        let params = vec![PathParameter::new("userId", "123")];
        let result = substitute_path_params(url, &params);
        assert_eq!(result, "https://api.example.com/users/123/posts/{postId}");
    }

    #[test]
    fn test_substitute_path_params_with_special_chars() {
        let url = "https://api.example.com/users/{userId}";
        let params = vec![PathParameter::new("userId", "user@123")];
        let result = substitute_path_params(url, &params);
        assert!(result.contains("user%40123"));
    }

    #[test]
    fn test_build_url_with_query_params_empty() {
        let url = "https://api.example.com/users";
        let params = vec![];
        let result = build_url_with_query_params(url, &params);
        assert_eq!(result, "https://api.example.com/users");
    }

    #[test]
    fn test_build_url_with_query_params_single() {
        let url = "https://api.example.com/users";
        let params = vec![QueryParameter::new("limit", "10")];
        let result = build_url_with_query_params(url, &params);
        assert_eq!(result, "https://api.example.com/users?limit=10");
    }

    #[test]
    fn test_build_url_with_query_params_multiple() {
        let url = "https://api.example.com/users";
        let params = vec![
            QueryParameter::new("limit", "10"),
            QueryParameter::new("page", "2"),
        ];
        let result = build_url_with_query_params(url, &params);
        assert_eq!(result, "https://api.example.com/users?limit=10&page=2");
    }

    #[test]
    fn test_build_url_with_query_params_existing_query() {
        let url = "https://api.example.com/users?sort=asc";
        let params = vec![QueryParameter::new("limit", "10")];
        let result = build_url_with_query_params(url, &params);
        assert_eq!(result, "https://api.example.com/users?sort=asc&limit=10");
    }

    #[test]
    fn test_build_url_with_query_params_disabled() {
        let url = "https://api.example.com/users";
        let params = vec![
            QueryParameter::new("limit", "10"),
            QueryParameter::new("page", "2").with_enabled(false),
        ];
        let result = build_url_with_query_params(url, &params);
        assert_eq!(result, "https://api.example.com/users?limit=10");
    }

    #[test]
    fn test_build_url_with_query_params_empty_key() {
        let url = "https://api.example.com/users";
        let params = vec![
            QueryParameter::new("limit", "10"),
            QueryParameter::new("", "value"),
        ];
        let result = build_url_with_query_params(url, &params);
        assert_eq!(result, "https://api.example.com/users?limit=10");
    }

    #[test]
    fn test_build_url_with_query_params_special_chars() {
        let url = "https://api.example.com/users";
        let params = vec![QueryParameter::new("name", "John Doe")];
        let result = build_url_with_query_params(url, &params);
        assert!(result.contains("name=John%20Doe"));
    }

    #[test]
    fn test_merge_query_params_no_overlap() {
        let folder_params = vec![QueryParameter::new("limit", "10")];
        let service_params = vec![QueryParameter::new("page", "2")];
        let merged = merge_query_params(&folder_params, &service_params);

        assert_eq!(merged.len(), 2);
        assert!(merged.iter().any(|p| p.key.as_ref() == "limit"));
        assert!(merged.iter().any(|p| p.key.as_ref() == "page"));
    }

    #[test]
    fn test_merge_query_params_with_override() {
        let folder_params = vec![
            QueryParameter::new("limit", "10"),
            QueryParameter::new("sort", "asc"),
        ];
        let service_params = vec![QueryParameter::new("limit", "20")];
        let merged = merge_query_params(&folder_params, &service_params);

        assert_eq!(merged.len(), 2);
        let limit_param = merged.iter().find(|p| p.key.as_ref() == "limit").unwrap();
        assert_eq!(limit_param.value.as_ref(), "20");
        assert!(merged.iter().any(|p| p.key.as_ref() == "sort"));
    }

    #[test]
    fn test_merge_query_params_empty_folder() {
        let folder_params = vec![];
        let service_params = vec![QueryParameter::new("page", "2")];
        let merged = merge_query_params(&folder_params, &service_params);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].key.as_ref(), "page");
    }

    #[test]
    fn test_merge_query_params_empty_service() {
        let folder_params = vec![QueryParameter::new("limit", "10")];
        let service_params = vec![];
        let merged = merge_query_params(&folder_params, &service_params);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].key.as_ref(), "limit");
    }

    #[test]
    fn test_encode_param_value_spaces() {
        let encoded = encode_param_value("hello world", false);
        assert_eq!(encoded, "hello%20world");
    }

    #[test]
    fn test_encode_param_value_special_chars_query() {
        let encoded = encode_param_value("user@example.com", false);
        assert!(encoded.contains("user@example.com") || encoded.contains("%40"));
    }

    #[test]
    fn test_encode_param_value_special_chars_path() {
        let encoded = encode_param_value("user@example.com", true);
        assert!(encoded.contains("%40"));
    }

    #[test]
    fn test_encode_param_value_already_encoded() {
        let encoded = encode_param_value("hello%20world", false);
        assert_eq!(encoded, "hello%20world");
    }

    #[test]
    fn test_encode_param_value_unicode() {
        let encoded = encode_param_value("你好", false);
        assert!(encoded.contains("%"));
        assert!(encoded.len() > 2);
    }

    #[test]
    fn test_is_valid_query_param_key_valid() {
        assert!(is_valid_query_param_key("limit"));
        assert!(is_valid_query_param_key("user_id"));
        assert!(is_valid_query_param_key("filter-name"));
        assert!(is_valid_query_param_key("filter.name"));
        assert!(is_valid_query_param_key("filter[name]"));
    }

    #[test]
    fn test_is_valid_query_param_key_invalid() {
        assert!(!is_valid_query_param_key(""));
        assert!(!is_valid_query_param_key("filter name"));
        assert!(!is_valid_query_param_key("filter=name"));
        assert!(!is_valid_query_param_key("filter&name"));
    }

    #[test]
    fn test_is_valid_path_param_name_valid() {
        assert!(is_valid_path_param_name("userId"));
        assert!(is_valid_path_param_name("user_id"));
        assert!(is_valid_path_param_name("_id"));
        assert!(is_valid_path_param_name("ID123"));
    }

    #[test]
    fn test_is_valid_path_param_name_invalid() {
        assert!(!is_valid_path_param_name(""));
        assert!(!is_valid_path_param_name("123invalid"));
        assert!(!is_valid_path_param_name("user-id"));
        assert!(!is_valid_path_param_name("user.id"));
        assert!(!is_valid_path_param_name("user id"));
    }

    #[test]
    fn test_is_already_encoded_true() {
        assert!(is_already_encoded("hello%20world"));
        assert!(is_already_encoded("user%40example.com"));
    }

    #[test]
    fn test_is_already_encoded_false() {
        assert!(!is_already_encoded("hello world"));
        assert!(!is_already_encoded("user@example.com"));
        assert!(!is_already_encoded("normal_text"));
    }

    #[test]
    fn test_encoding_idempotence() {
        let value = "hello world";
        let encoded_once = encode_param_value(value, false);
        let encoded_twice = encode_param_value(&encoded_once, false);
        assert_eq!(encoded_once, encoded_twice);
    }

    #[test]
    fn test_path_params_with_numbers() {
        let url = "https://api.example.com/v1/users/{userId}/items/{item123}";
        let params = detect_path_params(url);
        assert_eq!(params, vec!["userId", "item123"]);
    }

    #[test]
    fn test_query_params_preserve_enabled_state() {
        let folder_params = vec![QueryParameter::new("limit", "10").with_enabled(false)];
        let service_params = vec![QueryParameter::new("page", "2")];
        let merged = merge_query_params(&folder_params, &service_params);

        let limit_param = merged.iter().find(|p| p.key.as_ref() == "limit").unwrap();
        assert!(!limit_param.enabled);
    }
}
