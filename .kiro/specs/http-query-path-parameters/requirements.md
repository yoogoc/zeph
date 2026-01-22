# Requirements Document

## Introduction

This specification defines the requirements for adding query parameter and path parameter support to Zeph's HTTP request testing functionality. Currently, Zeph supports URL input, request headers, and request body configuration. This feature will extend the request configuration capabilities to include query string parameters and path variable substitution, enabling users to test APIs with parameterized URLs more effectively.

Query parameters are key-value pairs appended to URLs (e.g., `?limit=10&page=2`) commonly used for filtering, pagination, and search. Path parameters are placeholder values within URL paths (e.g., `/users/{userId}/posts/{postId}`) that represent resource identifiers or dynamic segments.

## Glossary

- **Query_Parameter**: A key-value pair appended to a URL after the `?` symbol, used to pass data to the server
- **Path_Parameter**: A placeholder variable within a URL path that must be substituted with an actual value before sending the request
- **Parameter_Location**: An enumeration defining where a parameter is used (Path, Query, Header, Cookie)
- **HTTP_Service**: A configured API endpoint within Zeph that can be tested
- **Folder**: A container for organizing related HTTP services with shared configuration
- **Environment**: A named configuration set containing base URLs and variable values
- **Request_Panel**: The UI component where users configure HTTP request details
- **URL_Resolution**: The process of combining base URLs, path segments, and parameters into a final request URL
- **Parameter_Substitution**: The process of replacing path parameter placeholders with actual values
- **Common_Search_Parameters**: Folder-level query parameters that apply to all services within that folder
- **Service_Parameters**: Service-level parameters that can override folder-level parameters

## Requirements

### Requirement 1: Query Parameter Management

**User Story:** As a developer, I want to add and manage query parameters for my HTTP requests, so that I can test APIs with filtering, pagination, and search functionality.

#### Acceptance Criteria

1. WHEN a user adds a query parameter with a key and value, THE System SHALL append it to the request URL as a query string
2. WHEN multiple query parameters are defined, THE System SHALL join them with `&` symbols in the query string
3. WHEN a query parameter key is empty, THE System SHALL exclude it from the final URL
4. WHEN a query parameter is disabled, THE System SHALL exclude it from the final URL
5. THE System SHALL URL-encode query parameter keys and values according to RFC 3986

### Requirement 2: Path Parameter Detection and Substitution

**User Story:** As a developer, I want to define path parameters in my URLs using placeholders, so that I can test RESTful APIs with dynamic resource identifiers.

#### Acceptance Criteria

1. WHEN a URL contains a path parameter placeholder in the format `{paramName}`, THE System SHALL detect it as a path parameter
2. WHEN a path parameter placeholder is detected, THE System SHALL provide an input field for the user to enter its value
3. WHEN a path parameter value is provided, THE System SHALL substitute the placeholder with the value in the final URL
4. WHEN a path parameter value is empty, THE System SHALL leave the placeholder in the URL unchanged
5. THE System SHALL URL-encode path parameter values according to RFC 3986

### Requirement 3: Parameter UI Organization

**User Story:** As a developer, I want a clear and organized interface for managing parameters, so that I can efficiently configure my HTTP requests.

#### Acceptance Criteria

1. THE Request_Panel SHALL display a dedicated section for query parameters
2. THE Request_Panel SHALL display a dedicated section for path parameters
3. WHEN path parameters are detected in the URL, THE System SHALL automatically populate the path parameters section
4. THE System SHALL allow users to add, edit, and remove query parameters
5. THE System SHALL allow users to enable or disable individual query parameters without deleting them

### Requirement 4: Parameter Persistence

**User Story:** As a developer, I want my parameter configurations to be saved with my HTTP services, so that I don't have to re-enter them every time.

#### Acceptance Criteria

1. WHEN a user defines query parameters for a service, THE System SHALL persist them to the database
2. WHEN a user defines path parameter values for a service, THE System SHALL persist them to the database
3. WHEN a user loads a saved service, THE System SHALL restore all query and path parameters
4. THE System SHALL maintain parameter enabled/disabled state across sessions

### Requirement 5: Folder-Level Common Parameters

**User Story:** As a developer, I want to define common query parameters at the folder level, so that I can apply shared configuration to multiple services.

#### Acceptance Criteria

1. WHEN a folder has common search parameters defined, THE System SHALL apply them to all services within that folder
2. WHEN a service defines a query parameter with the same key as a folder-level parameter, THE System SHALL use the service-level value
3. WHEN a service-level parameter is removed, THE System SHALL fall back to the folder-level parameter if it exists
4. THE System SHALL clearly indicate which parameters are inherited from the folder level

### Requirement 6: URL Preview and Validation

**User Story:** As a developer, I want to see the final resolved URL with all parameters applied, so that I can verify my request configuration before sending it.

#### Acceptance Criteria

1. THE System SHALL display a preview of the final URL with all parameters resolved
2. WHEN any parameter value changes, THE System SHALL update the URL preview immediately
3. WHEN path parameters are unresolved, THE System SHALL visually indicate incomplete substitution
4. THE System SHALL validate that all required path parameters have values before allowing request execution

### Requirement 7: Parameter Import from OpenAPI

**User Story:** As a developer, I want to import parameter definitions from OpenAPI specifications, so that I can quickly configure services based on existing API documentation.

#### Acceptance Criteria

1. WHEN importing an OpenAPI specification, THE System SHALL extract query parameter definitions
2. WHEN importing an OpenAPI specification, THE System SHALL extract path parameter definitions
3. THE System SHALL preserve parameter metadata such as required status and default values
4. THE System SHALL map OpenAPI parameter locations to the appropriate Parameter_Location enum values

### Requirement 8: Parameter Encoding and Special Characters

**User Story:** As a developer, I want special characters in my parameters to be properly encoded, so that my requests are valid and work correctly.

#### Acceptance Criteria

1. THE System SHALL encode reserved characters in query parameter keys and values
2. THE System SHALL encode reserved characters in path parameter values
3. THE System SHALL preserve already-encoded sequences (avoid double-encoding)
4. THE System SHALL handle Unicode characters correctly in parameter values
5. WHEN a user enters a space character, THE System SHALL encode it as `%20` in path parameters and `+` or `%20` in query parameters

### Requirement 9: Parameter Validation

**User Story:** As a developer, I want to be notified of invalid parameter configurations, so that I can fix issues before sending requests.

#### Acceptance Criteria

1. WHEN a required path parameter is missing a value, THE System SHALL display a validation error
2. WHEN a query parameter key contains invalid characters, THE System SHALL display a validation warning
3. THE System SHALL prevent sending requests when critical validation errors exist
4. THE System SHALL allow sending requests with validation warnings after user confirmation
