# Design Document: Tab Title Update After Rename

## Overview

This design addresses a bug where tab titles do not update immediately after a service is renamed through inline tab editing. The root cause is that the `opened_services` list in the `Workspace` struct contains `ContentType::Service` instances with stale `Service` objects. When a service is renamed, the database and `workspace.services` list are updated, but the `opened_services` list retains the old `Service` objects.

The solution is to update the `opened_services` list to reference the updated `Service` objects from `workspace.services` after a rename operation.

## Architecture

The system follows this data flow:

1. **User Action**: User edits tab title inline and presses Enter
2. **Event Emission**: `ServiceTabs` emits `ServiceTabsEvent::Rename`
3. **Event Handling**: `WorkspaceContent` calls `rename_service_from_tab`
4. **Registry Update**: `WorkspaceRegistry::rename_service` updates database and `workspace.services`
5. **Event Broadcast**: `WorkspaceRegistry` emits `WorkspaceEvent::Reload`
6. **UI Update**: `ServiceTabs` receives updated workspace and re-renders

**Current Problem**: Step 4 updates `workspace.services` but not `workspace.opened_services`, causing tabs to display stale names.

**Solution**: After updating `workspace.services`, also update any matching `Service` objects in `workspace.opened_services`.

## Components and Interfaces

### Modified Component: WorkspaceRegistry

**Location**: `crates/workspace/src/lib.rs`

**Method to Modify**: `rename_service`

**Current Behavior**:
```rust
pub fn rename_service(&mut self, service_id: Uuid, new_name: SharedString, cx: &mut Context<Self>) {
    // 1. Update database
    // 2. Update workspace.services
    // 3. Emit WorkspaceEvent::Reload
}
```

**New Behavior**:
```rust
pub fn rename_service(&mut self, service_id: Uuid, new_name: SharedString, cx: &mut Context<Self>) {
    // 1. Update database
    // 2. Update workspace.services
    // 3. Update workspace.opened_services (NEW)
    // 4. Update workspace.active_service if needed (NEW)
    // 5. Emit WorkspaceEvent::Reload
}
```

### Helper Method: update_opened_services_after_rename

**Signature**:
```rust
fn update_opened_services_after_rename(
    workspace: &mut Workspace,
    service_id: Uuid,
    updated_service: &Service
)
```

**Purpose**: Updates all `ContentType::Service` instances in `opened_services` that match the given `service_id` with the updated `Service` object.

**Algorithm**:
1. Iterate through `workspace.opened_services`
2. For each `ContentType::Service(service)` where `service.id == service_id`:
   - Replace with `ContentType::Service(updated_service.clone())`
3. If `workspace.active_service` is `Some(ContentType::Service(service))` where `service.id == service_id`:
   - Update to `Some(ContentType::Service(updated_service.clone()))`

## Data Models

### Workspace (No Changes)

```rust
pub struct Workspace {
    pub id: Uuid,
    pub name: SharedString,
    // ... other fields ...
    
    // Runtime state
    pub opened_services: Vec<ContentType>,  // Contains ContentType::Service instances
    pub active_service: Option<ContentType>, // May contain ContentType::Service
    // ... other fields ...
}
```

### ContentType (No Changes)

```rust
pub enum ContentType {
    ServiceFolder(Folder),
    SchemaFolder(Folder),
    Service(Service),  // Contains a Service object
    Schema(Schema),
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Service Name Consistency in opened_services

*For any* service that exists in both `workspace.services` and `workspace.opened_services`, the name in `opened_services` should match the name in `workspace.services`.

**Validates: Requirements 1.2, 2.1**

### Property 2: Active Service Name Consistency

*For any* service that is the active service, if it exists in `workspace.services`, then `workspace.active_service` should contain the same name as in `workspace.services`.

**Validates: Requirements 2.3**

### Property 3: Rename Preserves opened_services Order

*For any* workspace with opened services, renaming a service should preserve the order of services in `opened_services`.

**Validates: Requirements 2.5**

### Property 4: Rename Updates Only Target Service

*For any* workspace with multiple opened services, renaming one service should only update that service's name in `opened_services`, leaving all other services unchanged.

**Validates: Requirements 1.5**

### Property 5: Tab Title Reflects Current Name

*For any* service in `opened_services`, the tab title displayed by `ServiceTabs` should match the service's current name from `workspace.services`.

**Validates: Requirements 1.1, 1.4**

## Error Handling

### Database Update Failure

**Scenario**: The database update in `rename_service` fails.

**Handling**: 
- Log the error
- Do NOT update `workspace.services` or `workspace.opened_services`
- Do NOT emit `WorkspaceEvent::Reload`
- Return early from the function

**Rationale**: Maintains data consistency by ensuring in-memory state matches database state.

### Service Not Found in workspace.services

**Scenario**: After database update, the service is not found in `workspace.services`.

**Handling**:
- Log a warning
- Continue with updating `opened_services` using the database-updated service
- Emit `WorkspaceEvent::Reload`

**Rationale**: The database is the source of truth. If the service isn't in memory, the reload event will trigger a full refresh.

### Service Not in opened_services

**Scenario**: The renamed service is not in `opened_services`.

**Handling**:
- This is a valid state (service may not be open)
- No action needed for `opened_services`
- Still emit `WorkspaceEvent::Reload` for other UI components

## Testing Strategy

### Unit Tests

**Test File**: `crates/workspace/src/lib.rs` (in `#[cfg(test)] mod tests`)

**Test Cases**:

1. **test_rename_service_updates_opened_services**
   - Setup: Create workspace with service in both `services` and `opened_services`
   - Action: Rename the service
   - Assert: Service name in `opened_services` matches new name

2. **test_rename_service_updates_active_service**
   - Setup: Create workspace with service as active service
   - Action: Rename the service
   - Assert: Active service name matches new name

3. **test_rename_service_preserves_order**
   - Setup: Create workspace with multiple services in `opened_services`
   - Action: Rename middle service
   - Assert: Order of services in `opened_services` unchanged

4. **test_rename_service_only_updates_target**
   - Setup: Create workspace with multiple services in `opened_services`
   - Action: Rename one service
   - Assert: Only target service updated, others unchanged

5. **test_rename_service_not_in_opened_services**
   - Setup: Create workspace with service in `services` but not in `opened_services`
   - Action: Rename the service
   - Assert: No error, service in `services` updated

### Property-Based Tests

Property-based tests will be implemented using the `proptest` crate for Rust.

**Configuration**: Each property test should run a minimum of 100 iterations.

**Test Cases**:

1. **Property Test: Service Name Consistency**
   - **Feature: tab-title-update-after-rename, Property 1: Service Name Consistency in opened_services**
   - Generate: Random workspace with random services
   - Action: Rename random service
   - Assert: All services in `opened_services` have names matching `services`

2. **Property Test: Active Service Consistency**
   - **Feature: tab-title-update-after-rename, Property 2: Active Service Name Consistency**
   - Generate: Random workspace with random active service
   - Action: Rename active service
   - Assert: Active service name matches `services`

3. **Property Test: Order Preservation**
   - **Feature: tab-title-update-after-rename, Property 3: Rename Preserves opened_services Order**
   - Generate: Random workspace with random opened services
   - Action: Rename random service
   - Assert: Service IDs in `opened_services` in same order

4. **Property Test: Selective Update**
   - **Feature: tab-title-update-after-rename, Property 4: Rename Updates Only Target Service**
   - Generate: Random workspace with multiple opened services
   - Action: Rename one service
   - Assert: Only target service name changed, others unchanged

### Integration Tests

**Test File**: `crates/ui/src/service_tabs.rs` (in `#[cfg(test)] mod tests`)

**Test Case**: **test_tab_title_updates_after_rename**
- Setup: Create UI with workspace and open service tabs
- Action: Trigger rename through tab editing
- Assert: Tab title displays new name after rename completes

## Implementation Notes

### Performance Considerations

- The `opened_services` list is typically small (< 20 items)
- Linear search through `opened_services` is acceptable
- Cloning `Service` objects is necessary due to Rust ownership rules
- The operation is O(n) where n is the number of opened services

### Rust-Specific Considerations

- Use `iter_mut()` to modify `opened_services` in place
- Clone the updated `Service` when creating new `ContentType::Service` instances
- Use pattern matching to identify `ContentType::Service` variants
- Ensure proper error handling with `Result` types

### Alternative Approaches Considered

**Approach 1: Reload entire workspace from database**
- Pros: Guarantees consistency
- Cons: Expensive, loses transient UI state
- Rejected: Overkill for a simple rename operation

**Approach 2: Use service ID references instead of full objects**
- Pros: Eliminates stale data problem
- Cons: Requires major refactoring of ContentType enum
- Rejected: Too invasive for this bug fix

**Approach 3: Current solution (update opened_services)**
- Pros: Minimal changes, maintains consistency, efficient
- Cons: Requires updating multiple locations
- Selected: Best balance of simplicity and correctness
