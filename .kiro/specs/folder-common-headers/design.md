# Design Document: Folder Common Headers

## Overview

This design document outlines the implementation of folder common headers functionality for the Zeph API client. The feature enables users to define common HTTP headers and search parameters at the folder level, which are automatically inherited by all HTTP services within that folder. This provides a centralized way to manage shared configuration across multiple API endpoints, reducing duplication and improving maintainability.

The implementation involves extending the existing folder data model, creating a new folder detail page with tabbed interface, and integrating the common headers functionality into the HTTP service execution flow.

## Architecture

The folder common headers feature follows the existing architectural patterns in the Zeph application:

### Component Architecture
- **Data Layer**: Extended Folder model with common headers and search parameters
- **Storage Layer**: Database schema updates and new storage operations
- **UI Layer**: New FolderDetailPage component with tabbed interface
- **Integration Layer**: Modified HTTP service execution to inherit folder settings

### Data Flow
1. User defines common headers/parameters in folder detail page
2. Settings are persisted to database via storage layer
3. When HTTP service executes, it queries parent folder for common settings
4. Common settings are merged with service-specific settings
5. Final merged configuration is used for HTTP request

## Components and Interfaces

### 1. Extended Folder Data Model

The existing `Folder` struct will be extended to include common headers and search parameters:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    // ... existing fields ...
    
    // New fields for common headers functionality
    #[serde(default)]
    pub common_headers: Vec<Header>,
    #[serde(default)]
    pub common_search_params: Vec<SearchParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParameter {
    pub key: SharedString,
    pub value: SharedString,
}
```

### 2. Database Schema Extension

New database schema will include columns for storing common headers and search parameters:

```sql
-- Add columns to existing folders table
ALTER TABLE folders ADD COLUMN common_headers TEXT DEFAULT '[]';
ALTER TABLE folders ADD COLUMN common_search_params TEXT DEFAULT '[]';
```

### 3. FolderDetailPage Component

A new UI component for displaying and managing folder details:

```rust
pub struct FolderDetailPage {
    folder_id: Option<Uuid>,
    folder: Option<Folder>,
    active_tab: FolderDetailTab,
    overview_panel: Entity<FolderOverviewPanel>,
    headers_panel: Entity<CommonHeadersPanel>,
    params_panel: Entity<SearchParamsPanel>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FolderDetailTab {
    Overview,
    CommonHeaders,
    SearchParameters,
}
```

### 4. Tab Panel Components

#### CommonHeadersPanel
Manages the list of common headers with add/edit/delete functionality:

```rust
pub struct CommonHeadersPanel {
    headers: Vec<Header>,
    header_inputs: Vec<HeaderInputRow>,
}

struct HeaderInputRow {
    key_input: Entity<InputState>,
    value_input: Entity<InputState>,
}
```

#### SearchParamsPanel
Similar to CommonHeadersPanel but for search parameters:

```rust
pub struct SearchParamsPanel {
    params: Vec<SearchParameter>,
    param_inputs: Vec<ParamInputRow>,
}
```

### 5. Storage Layer Extensions

Extended storage operations for folder management:

```rust
impl<'a> FolderStore<'a> {
    pub fn update_common_headers(&self, folder_id: Uuid, headers: Vec<Header>) -> Result<()>;
    pub fn update_search_params(&self, folder_id: Uuid, params: Vec<SearchParameter>) -> Result<()>;
    pub fn get_folder_with_common_settings(&self, folder_id: Uuid) -> Result<Option<Folder>>;
}
```

### 6. HTTP Service Integration

Modified HTTP service execution to inherit folder settings:

```rust
impl HttpService {
    fn resolve_final_headers(&self, cx: &Context<Self>) -> Vec<Header> {
        let mut final_headers = Vec::new();
        
        // Add folder common headers first (lower priority)
        if let Some(folder_headers) = self.get_folder_common_headers(cx) {
            final_headers.extend(folder_headers);
        }
        
        // Add service-specific headers (higher priority, can override)
        final_headers.extend(self.get_service_headers(cx));
        
        // Remove duplicates, keeping service-level headers
        self.deduplicate_headers(final_headers)
    }
    
    fn resolve_final_url(&self, cx: &Context<Self>) -> String {
        let base_url = self.get_base_url(cx);
        let path = self.get_path(cx);
        let service_params = self.get_service_params(cx);
        let folder_params = self.get_folder_common_params(cx).unwrap_or_default();
        
        // Merge parameters (service params override folder params)
        let final_params = self.merge_parameters(folder_params, service_params);
        
        url_utils::build_url_with_params(&base_url, &path, &final_params)
    }
}
```

## Data Models

### Extended Folder Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub folder_type: FolderType,
    pub name: SharedString,
    pub description: Option<String>,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub sync: SyncMeta,
    #[serde(skip)]
    pub is_expanded: bool,
    
    // New fields for common headers functionality
    #[serde(default)]
    pub common_headers: Vec<Header>,
    #[serde(default)]
    pub common_search_params: Vec<SearchParameter>,
}
```

### SearchParameter Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParameter {
    pub key: SharedString,
    pub value: SharedString,
}

impl SearchParameter {
    pub fn new(key: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}
```

### Navigation and Routing

Extended workspace view enum to support folder details:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum WorkspaceView {
    #[default]
    Services,
    Docs,
    History,
    Settings,
    FolderDetail(Uuid), // New view for folder details
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property Reflection

After analyzing all acceptance criteria, I identified several areas where properties can be consolidated to eliminate redundancy:

- Properties 3.1-3.6 and 4.1-4.6 follow identical patterns for headers and parameters - these can be combined into generic collection management properties
- Properties 1.1, 1.4, 7.1, 7.2 all test navigation functionality - these can be consolidated into comprehensive navigation properties
- Properties 2.3 and 2.4 both test tab interface behavior - these can be combined into a single tab management property
- Properties 6.1 and 6.2 test the same inheritance pattern for different data types - these can be unified
- Properties 6.3 and 6.4 test the same conflict resolution pattern - these can be unified

### Correctness Properties

Property 1: Folder navigation consistency
*For any* folder in the workspace, clicking on it in the sidebar should display its detail page, and the detail page should provide navigation back to the service list
**Validates: Requirements 1.1, 1.4, 7.1**

Property 2: Tab interface functionality
*For any* folder detail page, clicking on any tab should switch to the corresponding content section and maintain the active tab state throughout the session
**Validates: Requirements 2.3, 2.4**

Property 3: Collection management operations
*For any* folder and any collection type (headers or search parameters), adding valid items should persist them, deleting items should remove them, and editing items should update them correctly
**Validates: Requirements 3.2, 3.3, 3.4, 3.5, 4.2, 4.3, 4.4, 4.5**

Property 4: Collection display consistency
*For any* folder with common headers or search parameters, accessing the respective tab should display all existing items in editable format
**Validates: Requirements 3.1, 3.6, 4.1, 4.6**

Property 5: Folder initialization
*For any* newly created folder, it should have empty collections for both common headers and search parameters
**Validates: Requirements 5.3**

Property 6: Serialization round-trip
*For any* folder with common headers and search parameters, serializing then deserializing should produce equivalent data
**Validates: Requirements 5.5**

Property 7: HTTP service inheritance
*For any* HTTP service with a parent folder, executing the service should automatically include both common headers and search parameters from the parent folder
**Validates: Requirements 6.1, 6.2**

Property 8: Configuration conflict resolution
*For any* HTTP service that has both folder-level and service-level settings with conflicting keys, the service-level settings should take priority in the final configuration
**Validates: Requirements 6.3, 6.4**

Property 9: Configuration merging
*For any* HTTP service with a parent folder, the final request configuration should include all non-conflicting settings from both folder and service levels
**Validates: Requirements 6.5**

Property 10: Cross-folder navigation
*For any* service with a parent folder, there should be a navigation link to view the parent folder details, and switching between different folders should update the detail page content accordingly
**Validates: Requirements 7.2, 7.4**

Property 11: Breadcrumb navigation accuracy
*For any* folder detail page, the breadcrumb navigation should accurately reflect the current location in the folder hierarchy
**Validates: Requirements 7.3**

<function_calls>
<invoke name="prework">
<parameter name="featureName">folder-common-headers

## Error Handling

### Input Validation
- **Empty Values**: Prevent adding headers or parameters with empty keys or values
- **Duplicate Keys**: Handle duplicate header/parameter keys by updating existing entries
- **Invalid Characters**: Validate header names according to HTTP specification
- **Maximum Limits**: Enforce reasonable limits on number of headers/parameters per folder

### Database Operations
- **Connection Failures**: Graceful handling of database connection issues
- **Transaction Rollback**: Ensure data consistency during multi-step operations
- **Migration Errors**: Safe handling of schema migration failures
- **Concurrent Updates**: Handle concurrent modifications to folder settings

### UI Error States
- **Loading States**: Show appropriate loading indicators during data operations
- **Network Errors**: Display user-friendly error messages for network failures
- **Validation Feedback**: Provide immediate feedback for invalid inputs
- **Recovery Actions**: Offer retry mechanisms for failed operations

### HTTP Service Integration Errors
- **Missing Folder**: Handle cases where service references non-existent folder
- **Malformed Headers**: Validate inherited headers before HTTP request
- **URL Construction**: Handle errors in URL parameter merging
- **Request Failures**: Distinguish between configuration and network errors

## Testing Strategy

### Dual Testing Approach
The testing strategy employs both unit tests and property-based tests to ensure comprehensive coverage:

- **Unit Tests**: Verify specific examples, edge cases, and error conditions
- **Property Tests**: Verify universal properties across all inputs using randomized testing
- Both approaches are complementary and necessary for complete validation

### Unit Testing Focus Areas
- **Component Integration**: Test interactions between folder detail page and storage layer
- **Edge Cases**: Empty folders, maximum limits, special characters in headers
- **Error Conditions**: Database failures, network errors, invalid inputs
- **UI Interactions**: Tab switching, form validation, navigation flows

### Property-Based Testing Configuration
- **Minimum 100 iterations** per property test to ensure thorough randomized coverage
- **Test Library**: Use `proptest` crate for Rust property-based testing
- **Data Generators**: Create smart generators for folders, headers, and parameters
- **Test Tags**: Each property test references its design document property

#### Property Test Examples

```rust
// Property 3: Collection management operations
#[proptest]
fn test_collection_management_operations(
    folder: Folder,
    headers: Vec<Header>,
    params: Vec<SearchParameter>
) {
    // Test adding, editing, and deleting operations
    // Tag: Feature: folder-common-headers, Property 3: Collection management operations
}

// Property 6: Serialization round-trip  
#[proptest]
fn test_serialization_round_trip(folder: Folder) {
    let serialized = serde_json::to_string(&folder).unwrap();
    let deserialized: Folder = serde_json::from_str(&serialized).unwrap();
    prop_assert_eq!(folder, deserialized);
    // Tag: Feature: folder-common-headers, Property 6: Serialization round-trip
}

// Property 8: Configuration conflict resolution
#[proptest]
fn test_configuration_conflict_resolution(
    folder_headers: Vec<Header>,
    service_headers: Vec<Header>
) {
    let final_config = merge_configurations(folder_headers, service_headers);
    // Verify service headers take priority for conflicting keys
    // Tag: Feature: folder-common-headers, Property 8: Configuration conflict resolution
}
```

### Integration Testing
- **End-to-End Workflows**: Test complete user journeys from folder creation to HTTP request execution
- **Database Integration**: Verify data persistence and retrieval across application restarts
- **UI Component Integration**: Test interactions between different UI components
- **Cross-Component Communication**: Verify event handling between sidebar, detail page, and HTTP service

### Performance Testing
- **Large Folder Hierarchies**: Test performance with deeply nested folder structures
- **Many Headers/Parameters**: Verify performance with folders containing many common settings
- **Concurrent Access**: Test behavior under concurrent folder modifications
- **Memory Usage**: Monitor memory consumption during folder detail page operations

### Accessibility Testing
- **Keyboard Navigation**: Ensure all functionality is accessible via keyboard
- **Screen Reader Support**: Verify proper ARIA labels and semantic markup
- **Color Contrast**: Ensure sufficient contrast for all UI elements
- **Focus Management**: Test proper focus handling in tab interface