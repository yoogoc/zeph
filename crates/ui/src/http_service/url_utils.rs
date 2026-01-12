/// URL utility functions for combining base URLs with paths.
///
/// This module provides functions to:
/// - Determine if a URL is absolute
/// - Combine base URLs with paths, handling slashes correctly
/// - Resolve final URLs based on the URL mode setting
/// - Build URLs with search parameters
use models::SearchParameter;

/// Checks if a URL is an absolute URL (starts with http:// or https://).
///
/// # Arguments
/// * `url` - The URL string to check
///
/// # Returns
/// `true` if the URL starts with "http://" or "https://", `false` otherwise
pub fn is_absolute_url(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

/// Combines a base URL with a path, handling trailing/leading slashes correctly.
///
/// # Arguments
/// * `base_url` - The base URL (e.g., "https://api.example.com")
/// * `path` - The path to append (e.g., "/users" or "users")
///
/// # Returns
/// The combined URL with exactly one slash between base and path
///
/// # Examples
/// - `combine_url("https://api.example.com", "/users")` -> `"https://api.example.com/users"`
/// - `combine_url("https://api.example.com/", "/users")` -> `"https://api.example.com/users"`
/// - `combine_url("https://api.example.com/", "users")` -> `"https://api.example.com/users"`
pub fn combine_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim();
    let path = path.trim();

    // If base is empty, return path as-is
    if base.is_empty() {
        return path.to_string();
    }

    // If path is empty, return base as-is
    if path.is_empty() {
        return base.to_string();
    }

    // Remove trailing slash from base and leading slash from path
    let base_trimmed = base.trim_end_matches('/');
    let path_trimmed = path.trim_start_matches('/');

    // Combine with exactly one slash
    format!("{}/{}", base_trimmed, path_trimmed)
}

/// Resolves the final URL based on the URL mode setting.
///
/// # Arguments
/// * `base_url` - The base URL from the environment
/// * `path` - The user-entered URL or path
/// * `use_base_url` - Whether to use the base URL mode
///
/// # Returns
/// The final URL to use for the request
///
/// # Logic
/// - If `use_base_url` is false, returns `path` directly
/// - If `path` is an absolute URL, returns `path` directly (ignores base_url)
/// - If `base_url` is empty, returns `path` directly
/// - Otherwise, combines `base_url` and `path`
pub fn resolve_url(base_url: &str, path: &str, use_base_url: bool) -> String {
    let path = path.trim();

    // If base_url mode is disabled, use path directly
    if !use_base_url {
        return path.to_string();
    }

    // If path is an absolute URL, use it directly
    if is_absolute_url(path) {
        return path.to_string();
    }

    // If base_url is empty, use path directly
    let base = base_url.trim();
    if base.is_empty() {
        return path.to_string();
    }

    // Combine base_url and path
    combine_url(base, path)
}

/// Builds a URL with search parameters appended as query string
///
/// # Arguments
/// * `base_url` - The base URL (may already contain query parameters)
/// * `params` - The search parameters to append
///
/// # Returns
/// The URL with parameters appended to the query string, or the original URL if parameter encoding fails
///
/// # Examples
/// - `build_url_with_params("https://api.example.com/users", &[SearchParameter::new("limit", "10")])`
///   -> `"https://api.example.com/users?limit=10"`
/// - `build_url_with_params("https://api.example.com/users?page=1", &[SearchParameter::new("limit", "10")])`
///   -> `"https://api.example.com/users?page=1&limit=10"`
pub fn build_url_with_params(base_url: &str, params: &[SearchParameter]) -> String {
    if params.is_empty() {
        return base_url.to_string();
    }

    let mut url = base_url.to_string();
    let separator = if url.contains('?') { "&" } else { "?" };

    let mut valid_params = Vec::new();

    // Validate and encode parameters
    for param in params {
        let key = param.key.trim();
        let value = param.value.trim();

        // Skip invalid parameters
        if key.is_empty() || value.is_empty() {
            continue;
        }

        // Skip parameters that are too long
        if key.len() > 100 || value.len() > 2048 {
            continue;
        }

        // Skip parameters with invalid characters
        if !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            continue;
        }

        // Skip parameters with newlines
        if value.contains('\n') || value.contains('\r') {
            continue;
        }

        // Encode the parameter
        match (urlencoding::encode(key), urlencoding::encode(value)) {
            (encoded_key, encoded_value) => {
                valid_params.push(format!("{}={}", encoded_key, encoded_value));
            }
        }
    }

    if !valid_params.is_empty() {
        url.push_str(separator);
        url.push_str(&valid_params.join("&"));
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_absolute_url_http() {
        assert!(is_absolute_url("http://example.com"));
        assert!(is_absolute_url("http://example.com/path"));
    }

    #[test]
    fn test_is_absolute_url_https() {
        assert!(is_absolute_url("https://example.com"));
        assert!(is_absolute_url("https://api.example.com/v1/users"));
    }

    #[test]
    fn test_is_absolute_url_relative() {
        assert!(!is_absolute_url("/users"));
        assert!(!is_absolute_url("users"));
        assert!(!is_absolute_url("/api/v1/users"));
    }

    #[test]
    fn test_is_absolute_url_with_whitespace() {
        assert!(is_absolute_url("  https://example.com  "));
        assert!(!is_absolute_url("  /users  "));
    }

    #[test]
    fn test_combine_url_basic() {
        assert_eq!(
            combine_url("https://api.example.com", "/users"),
            "https://api.example.com/users"
        );
    }

    #[test]
    fn test_combine_url_trailing_slash_on_base() {
        assert_eq!(
            combine_url("https://api.example.com/", "/users"),
            "https://api.example.com/users"
        );
    }

    #[test]
    fn test_combine_url_no_leading_slash_on_path() {
        assert_eq!(
            combine_url("https://api.example.com", "users"),
            "https://api.example.com/users"
        );
    }

    #[test]
    fn test_combine_url_both_slashes() {
        assert_eq!(
            combine_url("https://api.example.com/", "users"),
            "https://api.example.com/users"
        );
    }

    #[test]
    fn test_combine_url_empty_base() {
        assert_eq!(combine_url("", "/users"), "/users");
    }

    #[test]
    fn test_combine_url_empty_path() {
        assert_eq!(
            combine_url("https://api.example.com", ""),
            "https://api.example.com"
        );
    }

    #[test]
    fn test_resolve_url_mode_disabled() {
        assert_eq!(
            resolve_url("https://api.example.com", "/users", false),
            "/users"
        );
    }

    #[test]
    fn test_resolve_url_absolute_path() {
        assert_eq!(
            resolve_url("https://api.example.com", "https://other.com/path", true),
            "https://other.com/path"
        );
    }

    #[test]
    fn test_resolve_url_empty_base() {
        assert_eq!(resolve_url("", "/users", true), "/users");
    }

    #[test]
    fn test_resolve_url_combine() {
        assert_eq!(
            resolve_url("https://api.example.com", "/users", true),
            "https://api.example.com/users"
        );
    }

    #[test]
    fn test_build_url_with_params_empty() {
        assert_eq!(
            build_url_with_params("https://api.example.com/users", &[]),
            "https://api.example.com/users"
        );
    }

    #[test]
    fn test_build_url_with_params_single() {
        use models::SearchParameter;
        let params = vec![SearchParameter::new("limit", "10")];
        assert_eq!(
            build_url_with_params("https://api.example.com/users", &params),
            "https://api.example.com/users?limit=10"
        );
    }

    #[test]
    fn test_build_url_with_params_multiple() {
        use models::SearchParameter;
        let params = vec![
            SearchParameter::new("limit", "10"),
            SearchParameter::new("page", "2"),
        ];
        assert_eq!(
            build_url_with_params("https://api.example.com/users", &params),
            "https://api.example.com/users?limit=10&page=2"
        );
    }

    #[test]
    fn test_build_url_with_params_existing_query() {
        use models::SearchParameter;
        let params = vec![SearchParameter::new("limit", "10")];
        assert_eq!(
            build_url_with_params("https://api.example.com/users?page=1", &params),
            "https://api.example.com/users?page=1&limit=10"
        );
    }

    #[test]
    fn test_build_url_with_params_encoding() {
        use models::SearchParameter;
        let params = vec![SearchParameter::new("query", "hello world")];
        assert_eq!(
            build_url_with_params("https://api.example.com/search", &params),
            "https://api.example.com/search?query=hello%20world"
        );
    }

    #[test]
    fn test_build_url_with_params_invalid_params() {
        use models::SearchParameter;

        // Test with empty parameter name - should be skipped
        let params = vec![
            SearchParameter::new("", "value"),
            SearchParameter::new("valid", "value"),
        ];
        assert_eq!(
            build_url_with_params("https://api.example.com/search", &params),
            "https://api.example.com/search?valid=value"
        );

        // Test with empty parameter value - should be skipped
        let params = vec![
            SearchParameter::new("param", ""),
            SearchParameter::new("valid", "value"),
        ];
        assert_eq!(
            build_url_with_params("https://api.example.com/search", &params),
            "https://api.example.com/search?valid=value"
        );

        // Test with parameter containing newlines - should be skipped
        let params = vec![
            SearchParameter::new("bad", "line1\nline2"),
            SearchParameter::new("valid", "value"),
        ];
        assert_eq!(
            build_url_with_params("https://api.example.com/search", &params),
            "https://api.example.com/search?valid=value"
        );

        // Test with invalid parameter name characters - should be skipped
        let params = vec![
            SearchParameter::new("bad param", "value"), // space in name
            SearchParameter::new("valid", "value"),
        ];
        assert_eq!(
            build_url_with_params("https://api.example.com/search", &params),
            "https://api.example.com/search?valid=value"
        );
    }

    #[test]
    fn test_build_url_with_params_length_limits() {
        use models::SearchParameter;

        // Test parameter name too long - should be skipped
        let long_name = "a".repeat(101);
        let params = vec![
            SearchParameter::new(long_name, "value"),
            SearchParameter::new("valid", "value"),
        ];
        assert_eq!(
            build_url_with_params("https://api.example.com/search", &params),
            "https://api.example.com/search?valid=value"
        );

        // Test parameter value too long - should be skipped
        let long_value = "a".repeat(2049);
        let params = vec![
            SearchParameter::new("bad", long_value),
            SearchParameter::new("valid", "value"),
        ];
        assert_eq!(
            build_url_with_params("https://api.example.com/search", &params),
            "https://api.example.com/search?valid=value"
        );
    }

    #[test]
    fn test_build_url_with_params_all_invalid() {
        use models::SearchParameter;

        // Test with all invalid parameters - should return original URL
        let params = vec![
            SearchParameter::new("", "value"),
            SearchParameter::new("param", ""),
            SearchParameter::new("bad param", "value"),
        ];
        assert_eq!(
            build_url_with_params("https://api.example.com/search", &params),
            "https://api.example.com/search"
        );
    }
}
