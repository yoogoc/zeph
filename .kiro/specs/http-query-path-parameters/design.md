# Design Document: HTTP Query and Path Parameters

## Overview

This design extends Zeph's HTTP request testing functionality to support query parameters and path parameters. Query parameters are key-value pairs appended to URLs as query strings (e.g., `?limit=10&page=2`), while path parameters are placeholder variables within URL paths that require substitution (e.g., `/users/{userId}/posts/{postId}`).

The implementation will follow the existing patterns established by the request headers feature, providing a consistent user experience. Parameters will be managed through dedicated UI sections, persisted to the database, and properly encoded according to RFC 3986 standards.

## Architecture

### Component Structure

The feature extends the existing HTTP service architecture with three new components:

1. **RequestQueryParams** (UI Component): Manages query parameter key-value pairs with enable/disable toggles
2. **RequestPathParams** (UI Component): Manages path parameter values detected from URL placeholders
3. **ParameterResolver** (Logic Module): Handles parameter detection, substitution, and URL building

### Data Flow

```
User Input (URL) → Path Parameter Detection → UI Update (Path Params Section)
User Input (Query Params) → Parameter Validation → URL Building → Final URL
Folder Common Params → Parameter Merging → Final URL
Service Params (override) → Parameter Merging → Final URL
```

### Integration Points

- **RequestPanel**: Extended with new tabs for Query and Path parameters
- **HttpService**: Updated to resolve parameters and build final URLs
- **Storage Layer**: Extended to persist query and path parameter configurations
- **URL Utils**: Enhanced with path parameter substitution logic

## Components and Interfaces

### 1. Data Models (models/src/lib.rs)

Extend existing models to support query and path parameters:

```rust
// Extend HttpConfig to include query and path parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub method: HttpMethod,
    pub path: SharedString,
    pub parameters: Vec<Parameter>,  // Existing field (from OpenAPI)
    pub query_params: Vec<QueryParameter>,  // New field
    pub path_params: Vec<PathParameter>,  // New field
    pub request_body: Option<RequestBody>,
    pub responses: Vec<Response>,
    pub security: Vec<SharedString>,
    pub use_base_url: bool,
}

// New: Query parameter with enable/disable state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParameter {
    pub key: SharedString,
    pub value: SharedString,
    pub enabled: bool,
    pub description: Option<SharedString>,
}

impl QueryParameter {
    pub fn new(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
            description: None,
        }
    }
}

// New: Path parameter value storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathParameter {
    pub name: SharedString,
    pub value: SharedString,
    pub description: Option<SharedString>,
}

impl PathParameter {
    pub fn new(name: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            description: None,
        }
    }
}
```

### 2. Request Query Parameters UI (ui/src/http_service/request_query_params.rs)

New component for managing query parameters:

```rust
pub enum RequestQueryParamsEvent {
    Remove(usize),
    Changed(Vec<QueryParameter>),
    ToggleEnabled(usize),
}

struct QueryParamRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
    enabled: bool,
}

pub struct RequestQueryParams {
    rows: Vec<QueryParamRow>,
    folder_params: Vec<QueryParameter>,  // Inherited from folder
}

impl RequestQueryParams {
    pub fn new(
        params: Vec<QueryParameter>,
        folder_params: Vec<QueryParameter>,
        window: &mut Window,
        cx: &mut Context<Self>
    ) -> Self;
    
    pub fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>);
    
    pub fn params(&self, cx: &App) -> Vec<QueryParameter>;
    
    pub fn toggle_enabled(&mut self, index: usize, cx: &mut Context<Self>);
}
```

### 3. Request Path Parameters UI (ui/src/http_service/request_path_params.rs)

New component for managing path parameter values:

```rust
pub enum RequestPathParamsEvent {
    Changed(Vec<PathParameter>),
}

struct PathParamRow {
    name: SharedString,
    value: Entity<InputState>,
}

pub struct RequestPathParams {
    rows: Vec<PathParamRow>,
}

impl RequestPathParams {
    pub fn new(
        params: Vec<PathParameter>,
        window: &mut Window,
        cx: &mut Context<Self>
    ) -> Self;
    
    pub fn params(&self, cx: &App) -> Vec<PathParameter>;
    
    pub fn update_from_url(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>);
}
```

### 4. Parameter Resolution Logic (ui/src/http_service/param_utils.rs)

New module for parameter detection and substitution:

```rust
/// Detects path parameter placeholders in a URL
/// Supports format: {paramName}
pub fn detect_path_params(url: &str) -> Vec<String>;

/// Substitutes path parameter placeholders with values
pub fn substitute_path_params(
    url: &str,
    params: &[PathParameter]
) -> String;

/// Builds URL with query parameters
pub fn build_url_with_query_params(
    base_url: &str,
    params: &[QueryParameter]
) -> String;

/// Merges folder and service query parameters
/// Service parameters override folder parameters with same key
pub fn merge_query_params(
    folder_params: &[QueryParameter],
    service_params: &[QueryParameter]
) -> Vec<QueryParameter>;

/// Validates query parameter key
pub fn is_valid_query_param_key(key: &str) -> bool;

/// Validates path parameter name
pub fn is_valid_path_param_name(name: &str) -> bool;

/// URL-encodes a string according to RFC 3986
pub fn encode_param_value(value: &str, is_path: bool) -> String;
```

### 5. Extended Request Panel (ui/src/http_service/request_panel.rs)

Update RequestPanel to include new parameter tabs:

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestTab {
    Body,
    Headers,
    Query,    // New tab
    Path,     // New tab (shown only when path params detected)
}

pub struct RequestPanel {
    // Existing fields...
    query_params_panel: Entity<RequestQueryParams>,  // New
    path_params_panel: Entity<RequestPathParams>,    // New
    active_tab: RequestTab,
    // ...
}

impl RequestPanel {
    pub fn query_params(&self, cx: &App) -> Vec<QueryParameter>;
    
    pub fn path_params(&self, cx: &App) -> Vec<PathParameter>;
    
    pub fn add_query_param(&mut self, window: &mut Window, cx: &mut Context<Self>);
    
    pub fn update_path_params_from_url(&mut self, window: &mut Window, cx: &mut Context<Self>);
}
```

### 6. Extended HTTP Service (ui/src/http_service/http_service.rs)

Update HttpService to handle parameter resolution:

```rust
impl HttpService {
    /// Gets folder common query parameters
    fn get_folder_common_query_params(&self, cx: &Context<Self>) -> Vec<QueryParameter>;
    
    /// Resolves final query parameters by merging folder and service params
    fn resolve_final_query_params(&self, cx: &Context<Self>) -> Vec<QueryParameter>;
    
    /// Resolves final URL with path and query parameters
    fn resolve_final_url_with_params(&self, cx: &Context<Self>) -> String;
    
    /// Validates path parameters are complete
    fn validate_path_params(&self, cx: &Context<Self>) -> Result<(), String>;
}
```

## Data Models

### Database Schema Extensions

Extend the existing service storage to include query and path parameters:

```sql
-- Query parameters table
CREATE TABLE IF NOT EXISTS query_parameters (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE
);

-- Path parameters table
CREATE TABLE IF NOT EXISTS path_parameters (
    id TEXT PRIMARY KEY,
    service_id TEXT NOT NULL,
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    description TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE
);

-- Folder common query parameters table
CREATE TABLE IF NOT EXISTS folder_query_parameters (
    id TEXT PRIMARY KEY,
    folder_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
);
```

### Storage Layer Extensions (storage/src/)

Add new storage modules:

```rust
// storage/src/query_parameter.rs
pub struct QueryParameterStore;

impl QueryParameterStore {
    pub fn create(conn: &Connection, param: &QueryParameter, service_id: Uuid) -> Result<()>;
    pub fn update(conn: &Connection, param: &QueryParameter) -> Result<()>;
    pub fn delete(conn: &Connection, param_id: Uuid) -> Result<()>;
    pub fn list_by_service(conn: &Connection, service_id: Uuid) -> Result<Vec<QueryParameter>>;
}

// storage/src/path_parameter.rs
pub struct PathParameterStore;

impl PathParameterStore {
    pub fn create(conn: &Connection, param: &PathParameter, service_id: Uuid) -> Result<()>;
    pub fn update(conn: &Connection, param: &PathParameter) -> Result<()>;
    pub fn delete(conn: &Connection, param_id: Uuid) -> Result<()>;
    pub fn list_by_service(conn: &Connection, service_id: Uuid) -> Result<Vec<PathParameter>>;
}

// storage/src/folder_query_parameter.rs
pub struct FolderQueryParameterStore;

impl FolderQueryParameterStore {
    pub fn create(conn: &Connection, param: &QueryParameter, folder_id: Uuid) -> Result<()>;
    pub fn update(conn: &Connection, param: &QueryParameter) -> Result<()>;
    pub fn delete(conn: &Connection, param_id: Uuid) -> Result<()>;
    pub fn list_by_folder(conn: &Connection, folder_id: Uuid) -> Result<Vec<QueryParameter>>;
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Query Parameter URL Formatting

*For any* list of enabled query parameters with non-empty keys, when building a URL, the resulting query string should contain all parameters joined with `&` symbols and properly formatted with `?` or `&` separator based on existing query string.

**Validates: Requirements 1.1, 1.2**

### Property 2: Empty Query Parameter Filtering

*For any* list of query parameters, when building a URL, parameters with empty keys or disabled state should be excluded from the final URL.

**Validates: Requirements 1.3, 1.4**

### Property 3: Query Parameter Encoding

*For any* query parameter with special characters (spaces, reserved characters, Unicode), the encoded parameter should be RFC 3986 compliant and decodable back to the original value.

**Validates: Requirements 1.5, 8.1, 8.4, 8.5**

### Property 4: Path Parameter Detection and Substitution

*For any* URL containing `{paramName}` placeholders, the system should detect all placeholders, and when values are provided, substitute them correctly in the final URL, leaving empty-valued placeholders unchanged.

**Validates: Requirements 2.1, 2.3, 2.4**

### Property 5: Path Parameter Encoding

*For any* path parameter value with special characters, the encoded value should be RFC 3986 compliant for path segments and decodable back to the original value.

**Validates: Requirements 2.5, 8.2, 8.4, 8.5**

### Property 6: Encoding Idempotence

*For any* already-encoded parameter value, encoding it again should produce the same result (no double-encoding).

**Validates: Requirements 8.3**

### Property 7: Path Parameter UI Synchronization

*For any* URL change that adds or removes path parameter placeholders, the path parameters UI section should automatically update to reflect the current set of placeholders.

**Validates: Requirements 2.2, 3.3**

### Property 8: Query Parameter CRUD Operations

*For any* sequence of add, edit, remove, and toggle operations on query parameters, the final parameter list should correctly reflect all operations performed.

**Validates: Requirements 3.4, 3.5**

### Property 9: Query Parameter Persistence Round-Trip

*For any* service with query parameters, saving the service and then loading it should restore all query parameters with identical keys, values, and enabled states.

**Validates: Requirements 4.1, 4.4**

### Property 10: Path Parameter Persistence Round-Trip

*For any* service with path parameter values, saving the service and then loading it should restore all path parameter values with identical names and values.

**Validates: Requirements 4.2**

### Property 11: Folder Parameter Inheritance

*For any* folder with common query parameters, all services within that folder should inherit those parameters in their final URL unless overridden by service-level parameters.

**Validates: Requirements 5.1**

### Property 12: Service Parameter Override

*For any* query parameter key that exists at both folder and service level, the service-level value should be used in the final URL.

**Validates: Requirements 5.2**

### Property 13: Parameter Fallback

*For any* service-level query parameter that is removed, if a folder-level parameter with the same key exists, the folder-level parameter should appear in the final URL.

**Validates: Requirements 5.3**

### Property 14: URL Preview Reactivity

*For any* change to query parameters, path parameters, or the base URL, the URL preview should immediately update to reflect the new final URL with all parameters resolved.

**Validates: Requirements 6.1, 6.2**

### Property 15: Path Parameter Validation Blocking

*For any* service with unresolved required path parameters (placeholders without values), the system should prevent request execution until all required parameters have values.

**Validates: Requirements 6.4, 9.3**

### Property 16: OpenAPI Parameter Extraction

*For any* valid OpenAPI specification containing parameter definitions, importing it should correctly extract all query and path parameters with their metadata (required status, default values, descriptions) and map locations to the correct Parameter_Location enum values.

**Validates: Requirements 7.1, 7.2, 7.3, 7.4**

### Property 17: Validation Warning Allowance

*For any* request with validation warnings (non-critical issues like unusual characters), the system should allow sending the request after user confirmation.

**Validates: Requirements 9.4**

## Error Handling

### Validation Errors

1. **Missing Required Path Parameters**: Display error message, highlight missing parameters, block request execution
2. **Invalid Query Parameter Keys**: Display warning for keys with invalid characters, allow user to fix or proceed
3. **Parameter Value Too Long**: Display warning for values exceeding reasonable limits (2048 chars for query, 256 for path)
4. **Encoding Failures**: Log error, display user-friendly message, fall back to unencoded value with warning

### Edge Cases

1. **Empty Parameter Lists**: Handle gracefully, return original URL unchanged
2. **Duplicate Parameter Keys**: Service-level parameters override folder-level, last occurrence wins for same-level duplicates
3. **Malformed URL Placeholders**: Detect and warn about unclosed braces or invalid placeholder formats
4. **Special Characters in Keys**: Validate and warn, but allow if user confirms
5. **Very Long URLs**: Warn when final URL exceeds 2048 characters (common browser limit)

### Error Recovery

1. **Database Errors**: Log error, display user message, maintain in-memory state
2. **Encoding Errors**: Fall back to unencoded value with warning
3. **UI State Corruption**: Reset to last known good state from database
4. **Invalid Persisted Data**: Filter out invalid entries, log warning, continue with valid data

## Testing Strategy

### Dual Testing Approach

This feature requires both unit tests and property-based tests for comprehensive coverage:

- **Unit tests**: Verify specific examples, edge cases, and error conditions
- **Property tests**: Verify universal properties across all inputs
- Together they provide comprehensive coverage: unit tests catch concrete bugs, property tests verify general correctness

### Property-Based Testing

Use the `proptest` crate for Rust property-based testing. Each property test should:
- Run minimum 100 iterations
- Reference its design document property in a comment
- Tag format: `// Feature: http-query-path-parameters, Property {number}: {property_text}`

**Example property test structure:**

```rust
use proptest::prelude::*;

// Feature: http-query-path-parameters, Property 1: Query Parameter URL Formatting
proptest! {
    #[test]
    fn test_query_param_url_formatting(
        params in prop::collection::vec(
            (prop::string::string_regex("[a-zA-Z0-9_]+").unwrap(),
             prop::string::string_regex(".{1,100}").unwrap()),
            0..10
        )
    ) {
        let query_params: Vec<QueryParameter> = params
            .into_iter()
            .map(|(k, v)| QueryParameter::new(k, v))
            .collect();
        
        let url = build_url_with_query_params("https://api.example.com/test", &query_params);
        
        // Verify all enabled params appear in URL
        for param in &query_params {
            if param.enabled && !param.key.is_empty() {
                assert!(url.contains(&format!("{}=", param.key)));
            }
        }
        
        // Verify proper separator
        if !query_params.is_empty() {
            assert!(url.contains('?'));
        }
    }
}
```

### Unit Testing Focus Areas

1. **Path Parameter Detection**:
   - Test various placeholder formats: `{id}`, `{userId}`, `{post_id}`
   - Test edge cases: nested braces, malformed placeholders
   - Test URLs with no placeholders

2. **Parameter Merging**:
   - Test folder + service parameter merging
   - Test override behavior
   - Test fallback when service param removed

3. **URL Building**:
   - Test query string appending to URLs with/without existing query strings
   - Test path parameter substitution
   - Test combined path + query parameters

4. **Encoding**:
   - Test special characters: spaces, `&`, `=`, `?`, `#`, `/`
   - Test Unicode characters
   - Test already-encoded strings (idempotence)

5. **Persistence**:
   - Test save/load round-trips
   - Test enabled/disabled state persistence
   - Test parameter ordering preservation

6. **UI Interactions**:
   - Test add/remove/edit operations
   - Test enable/disable toggle
   - Test URL change triggering path param detection

### Integration Testing

1. **End-to-End Request Flow**:
   - Create service with parameters
   - Build final URL
   - Send request
   - Verify correct URL was used

2. **Folder Inheritance**:
   - Create folder with common params
   - Create service in folder
   - Verify params inherited
   - Override at service level
   - Verify override works

3. **OpenAPI Import**:
   - Import spec with parameters
   - Verify parameters extracted
   - Verify metadata preserved

### Test Coverage Goals

- Minimum 80% code coverage for new modules
- 100% coverage for critical paths (URL building, encoding, persistence)
- All 17 correctness properties implemented as property tests
- Edge cases covered by unit tests
