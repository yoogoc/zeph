# Implementation Plan: HTTP Query and Path Parameters

## Overview

This implementation plan breaks down the addition of query parameter and path parameter support into incremental, testable steps. The approach follows the existing patterns established by the request headers feature, ensuring consistency with the current codebase architecture.

## Tasks

- [x] 1. Extend data models for query and path parameters
  - Add `QueryParameter` and `PathParameter` structs to `crates/models/src/lib.rs`
  - Add `query_params` and `path_params` fields to `HttpConfig`
  - Implement constructors and helper methods for new types
  - Update `Default` implementation for `HttpConfig`
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.3, 4.1, 4.2_

- [ ]* 1.1 Write property test for QueryParameter model
  - **Property 9: Query Parameter Persistence Round-Trip**
  - **Validates: Requirements 4.1, 4.4**

- [ ]* 1.2 Write property test for PathParameter model
  - **Property 10: Path Parameter Persistence Round-Trip**
  - **Validates: Requirements 4.2**

- [x] 2. Create parameter utility module
  - [x] 2.1 Create `crates/ui/src/http_service/param_utils.rs`
    - Implement `detect_path_params()` to find `{paramName}` placeholders in URLs
    - Implement `substitute_path_params()` to replace placeholders with values
    - Implement `build_url_with_query_params()` to append query string
    - Implement `merge_query_params()` for folder/service parameter merging
    - Implement `encode_param_value()` for RFC 3986 encoding
    - Implement validation functions: `is_valid_query_param_key()`, `is_valid_path_param_name()`
    - _Requirements: 1.1, 1.2, 1.3, 1.5, 2.1, 2.3, 2.4, 2.5, 5.2, 8.1, 8.2, 8.3, 8.4, 8.5_

  - [ ]* 2.2 Write property test for path parameter detection
    - **Property 4: Path Parameter Detection and Substitution**
    - **Validates: Requirements 2.1, 2.3, 2.4**

  - [ ]* 2.3 Write property test for query parameter URL formatting
    - **Property 1: Query Parameter URL Formatting**
    - **Validates: Requirements 1.1, 1.2**

  - [ ]* 2.4 Write property test for parameter filtering
    - **Property 2: Empty Query Parameter Filtering**
    - **Validates: Requirements 1.3, 1.4**

  - [ ]* 2.5 Write property test for query parameter encoding
    - **Property 3: Query Parameter Encoding**
    - **Validates: Requirements 1.5, 8.1, 8.4, 8.5**

  - [ ]* 2.6 Write property test for path parameter encoding
    - **Property 5: Path Parameter Encoding**
    - **Validates: Requirements 2.5, 8.2, 8.4, 8.5**

  - [ ]* 2.7 Write property test for encoding idempotence
    - **Property 6: Encoding Idempotence**
    - **Validates: Requirements 8.3**

  - [ ]* 2.8 Write unit tests for edge cases
    - Test malformed placeholders (unclosed braces, nested braces)
    - Test empty parameter lists
    - Test very long parameter values
    - Test special characters in keys
    - _Requirements: 1.3, 2.4, 8.1, 8.2, 9.2_

- [x] 3. Create query parameters UI component
  - [x] 3.1 Create `crates/ui/src/http_service/request_query_params.rs`
    - Define `RequestQueryParamsEvent` enum (Remove, Changed, ToggleEnabled)
    - Implement `RequestQueryParams` struct with rows and folder_params
    - Implement `new()` constructor accepting service and folder parameters
    - Implement `add_param()` to add new query parameter row
    - Implement `params()` to get current parameter list
    - Implement `toggle_enabled()` to enable/disable parameters
    - Implement `Render` trait with key/value inputs, enable checkbox, and remove button
    - Display folder-inherited parameters with visual indicator
    - _Requirements: 3.1, 3.4, 3.5, 5.4_

  - [ ]* 3.2 Write property test for query parameter CRUD operations
    - **Property 8: Query Parameter CRUD Operations**
    - **Validates: Requirements 3.4, 3.5**

  - [ ]* 3.3 Write unit tests for UI interactions
    - Test add parameter creates new row
    - Test remove parameter deletes row
    - Test toggle enabled changes state
    - Test folder parameter display with indicator
    - _Requirements: 3.4, 3.5, 5.4_

- [x] 4. Create path parameters UI component
  - [x] 4.1 Create `crates/ui/src/http_service/request_path_params.rs`
    - Define `RequestPathParamsEvent` enum (Changed)
    - Implement `RequestPathParams` struct with parameter rows
    - Implement `new()` constructor accepting path parameters
    - Implement `params()` to get current parameter values
    - Implement `update_from_url()` to sync with URL changes
    - Implement `Render` trait with read-only name labels and value inputs
    - Display validation errors for missing required parameters
    - _Requirements: 2.2, 3.2, 3.3, 6.3, 9.1_

  - [ ]* 4.2 Write property test for path parameter UI synchronization
    - **Property 7: Path Parameter UI Synchronization**
    - **Validates: Requirements 2.2, 3.3**

  - [ ]* 4.3 Write unit tests for path parameter UI
    - Test update_from_url detects new placeholders
    - Test update_from_url removes old placeholders
    - Test validation error display for empty required params
    - _Requirements: 2.2, 3.3, 6.3, 9.1_

- [x] 5. Extend request panel with parameter tabs
  - [x] 5.1 Update `crates/ui/src/http_service/request_panel.rs`
    - Add `Query` and `Path` variants to `RequestTab` enum
    - Add `query_params_panel` and `path_params_panel` fields to `RequestPanel`
    - Update `new()` to initialize parameter panels
    - Add `query_params()` and `path_params()` methods
    - Add `add_query_param()` method
    - Add `update_path_params_from_url()` method
    - Update `Render` to include Query and Path tabs
    - Show Path tab only when path parameters are detected
    - Add "Add Parameter" button for Query tab
    - Subscribe to URL changes to update path parameters
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [ ]* 5.2 Write unit tests for request panel integration
    - Test tab switching between Body, Headers, Query, Path
    - Test Path tab visibility based on URL placeholders
    - Test add query parameter button
    - Test URL change triggers path param update
    - _Requirements: 3.1, 3.2, 3.3_

- [x] 6. Checkpoint - Ensure UI components render correctly
  - Manually test the UI components in the application
  - Verify query parameters can be added, edited, removed, and toggled
  - Verify path parameters auto-populate when URL contains placeholders
  - Verify folder-inherited parameters display with indicators
  - Ask the user if questions arise

- [x] 7. Extend HTTP service with parameter resolution
  - [x] 7.1 Update `crates/ui/src/http_service/http_service.rs`
    - Add `get_folder_common_query_params()` method
    - Add `resolve_final_query_params()` method using `merge_query_params()`
    - Update `resolve_final_url()` to include path parameter substitution
    - Update `resolve_final_url()` to include query parameter appending
    - Add `validate_path_params()` method to check for missing required params
    - Update `send_request()` to use resolved parameters
    - Update `send_request()` to validate path params before sending
    - Update request info display to show final resolved URL
    - _Requirements: 2.3, 5.1, 5.2, 5.3, 6.1, 6.2, 6.4, 9.3_

  - [ ]* 7.2 Write property test for parameter merging
    - **Property 11: Folder Parameter Inheritance**
    - **Property 12: Service Parameter Override**
    - **Property 13: Parameter Fallback**
    - **Validates: Requirements 5.1, 5.2, 5.3**

  - [ ]* 7.3 Write property test for URL preview reactivity
    - **Property 14: URL Preview Reactivity**
    - **Validates: Requirements 6.1, 6.2**

  - [ ]* 7.4 Write property test for path parameter validation
    - **Property 15: Path Parameter Validation Blocking**
    - **Validates: Requirements 6.4, 9.3**

  - [ ]* 7.5 Write property test for validation warnings
    - **Property 17: Validation Warning Allowance**
    - **Validates: Requirements 9.4**

  - [ ]* 7.6 Write unit tests for HTTP service parameter resolution
    - Test folder params applied to service
    - Test service params override folder params
    - Test removing service param reveals folder param
    - Test validation blocks request with missing path params
    - Test validation allows request with warnings after confirmation
    - _Requirements: 5.1, 5.2, 5.3, 6.4, 9.3, 9.4_

- [x] 8. Extend database schema for parameter persistence
  - [x] 8.1 Update `crates/storage/src/schema.rs`
    - Add `query_parameters` table definition
    - Add `path_parameters` table definition
    - Add `folder_query_parameters` table definition
    - Add migration logic to create new tables
    - _Requirements: 4.1, 4.2, 4.4_

  - [ ]* 8.2 Write unit tests for schema migrations
    - Test migration creates tables successfully
    - Test migration is idempotent
    - Test foreign key constraints work correctly
    - _Requirements: 4.1, 4.2_

- [x] 9. Create storage layer for parameters
  - [x] 9.1 Create `crates/storage/src/query_parameter.rs`
    - Implement `QueryParameterStore` struct
    - Implement `create()` to insert query parameter
    - Implement `update()` to modify query parameter
    - Implement `delete()` to remove query parameter
    - Implement `list_by_service()` to fetch service parameters
    - Handle enabled/disabled state in persistence
    - _Requirements: 4.1, 4.4_

  - [x] 9.2 Create `crates/storage/src/path_parameter.rs`
    - Implement `PathParameterStore` struct
    - Implement `create()` to insert path parameter
    - Implement `update()` to modify path parameter
    - Implement `delete()` to remove path parameter
    - Implement `list_by_service()` to fetch service parameters
    - _Requirements: 4.2_

  - [x] 9.3 Create `crates/storage/src/folder_query_parameter.rs`
    - Implement `FolderQueryParameterStore` struct
    - Implement `create()` to insert folder query parameter
    - Implement `update()` to modify folder query parameter
    - Implement `delete()` to remove folder query parameter
    - Implement `list_by_folder()` to fetch folder parameters
    - _Requirements: 5.1_

  - [ ]* 9.4 Write unit tests for query parameter storage
    - Test create, update, delete, list operations
    - Test enabled state persistence
    - Test foreign key cascade on service deletion
    - _Requirements: 4.1, 4.4_

  - [ ]* 9.5 Write unit tests for path parameter storage
    - Test create, update, delete, list operations
    - Test foreign key cascade on service deletion
    - _Requirements: 4.2_

  - [ ]* 9.6 Write unit tests for folder query parameter storage
    - Test create, update, delete, list operations
    - Test foreign key cascade on folder deletion
    - _Requirements: 5.1_

- [x] 10. Integrate parameter loading and saving
  - [x] 10.1 Update service loading in `crates/storage/src/service.rs`
    - Load query parameters when loading service
    - Load path parameters when loading service
    - Populate `HttpConfig` with loaded parameters
    - _Requirements: 4.3_

  - [x] 10.2 Update service saving in `crates/storage/src/service.rs`
    - Save query parameters when saving service
    - Save path parameters when saving service
    - Handle parameter updates (delete old, insert new)
    - _Requirements: 4.1, 4.2_

  - [x] 10.3 Update folder loading in `crates/storage/src/folder.rs`
    - Load common query parameters when loading folder
    - Populate `Folder` with loaded parameters
    - _Requirements: 5.1_

  - [x] 10.4 Update folder saving in `crates/storage/src/folder.rs`
    - Save common query parameters when saving folder
    - Handle parameter updates (delete old, insert new)
    - _Requirements: 5.1_

  - [ ]* 10.5 Write integration tests for parameter persistence
    - Test save service with parameters, reload, verify parameters match
    - Test save folder with parameters, reload, verify parameters match
    - Test service inherits folder parameters after reload
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 5.1_

- [x] 11. Checkpoint - Ensure persistence works correctly
  - Manually test saving and loading services with parameters
  - Verify query parameters persist with correct enabled state
  - Verify path parameters persist with correct values
  - Verify folder common parameters persist and are inherited
  - Ask the user if questions arise

- [x] 12. Add OpenAPI parameter import support
  - [x] 12.1 Update `crates/workspace/src/openapi.rs`
    - Extract query parameters from OpenAPI `parameters` array
    - Extract path parameters from OpenAPI `parameters` array
    - Map OpenAPI `in` field to `ParameterLocation` enum
    - Preserve `required`, `description`, and `example` metadata
    - Convert OpenAPI parameters to `QueryParameter` and `PathParameter`
    - Populate `HttpConfig` with extracted parameters
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

  - [ ]* 12.2 Write property test for OpenAPI parameter extraction
    - **Property 16: OpenAPI Parameter Extraction**
    - **Validates: Requirements 7.1, 7.2, 7.3, 7.4**

  - [ ]* 12.3 Write unit tests for OpenAPI import
    - Test import spec with query parameters
    - Test import spec with path parameters
    - Test import spec with mixed parameter types
    - Test metadata preservation (required, description, example)
    - Test parameter location mapping
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

- [x] 13. Add internationalization strings
  - [x] 13.1 Update `locales/en.yml`
    - Add strings for query parameters section
    - Add strings for path parameters section
    - Add strings for parameter validation errors
    - Add strings for parameter tooltips and help text
    - _Requirements: 3.1, 3.2, 6.3, 9.1, 9.2_

  - [x] 13.2 Update `locales/zh-CN.yml`
    - Add Chinese translations for all new strings
    - _Requirements: 3.1, 3.2, 6.3, 9.1, 9.2_

- [x] 14. Final integration and testing
  - [x] 14.1 Wire all components together
    - Ensure RequestPanel creates and manages parameter panels
    - Ensure HttpService uses parameter resolution
    - Ensure storage layer is called on save/load
    - Ensure OpenAPI import populates parameters
    - _Requirements: All_

  - [ ]* 14.2 Write end-to-end integration tests
    - Test complete flow: create service → add parameters → save → reload → send request
    - Test folder inheritance flow: create folder with params → create service → verify inheritance → override → verify override
    - Test OpenAPI import flow: import spec → verify parameters → save → reload → verify persistence
    - _Requirements: All_

  - [x] 14.3 Manual testing and validation
    - Test all UI interactions work smoothly
    - Test parameter encoding with various special characters
    - Test folder parameter inheritance and override
    - Test OpenAPI import with real-world specs
    - Test persistence across app restarts
    - Verify URL preview updates correctly
    - Verify validation errors display appropriately

- [x] 15. Final checkpoint - Complete feature validation
  - Run `cargo fmt` to format all code
  - Run `cargo clippy --workspace --all-targets` and fix any warnings
  - Run `cargo test --workspace` and ensure all tests pass
  - Manually test the complete feature in the application
  - Verify all requirements are met
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation follows existing patterns from request headers feature
- All new code should follow Rust 2021 conventions and pass `cargo fmt` and `cargo clippy`
