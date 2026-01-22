# Implementation Plan: Tab Title Update After Rename

## Overview

This implementation plan fixes the bug where tab titles don't update after renaming a service through inline tab editing. The fix involves updating the `opened_services` list in the `Workspace` struct after a rename operation to ensure all UI components display the current service name.

## Tasks

- [x] 1. Implement helper method to update opened_services
  - Create `update_opened_services_after_rename` helper method in `WorkspaceRegistry`
  - Method should update all matching `ContentType::Service` instances in `opened_services`
  - Method should also update `active_service` if it matches the renamed service
  - _Requirements: 1.2, 2.1, 2.3_

- [ ]* 1.1 Write unit tests for helper method
  - Test updating service in opened_services
  - Test updating active_service
  - Test preserving order of opened_services
  - Test handling service not in opened_services
  - _Requirements: 1.2, 2.1, 2.3, 2.5_

- [x] 2. Modify rename_service to call helper method
  - Update `WorkspaceRegistry::rename_service` method
  - After updating `workspace.services`, call helper to update `opened_services`
  - Ensure helper is called before emitting `WorkspaceEvent::Reload`
  - _Requirements: 1.1, 1.2, 2.2_

- [ ]* 2.1 Write property test for service name consistency
  - **Property 1: Service Name Consistency in opened_services**
  - **Validates: Requirements 1.2, 2.1**
  - Generate random workspace with services
  - Rename random service
  - Assert all services in opened_services match services list
  - _Requirements: 1.2, 2.1_

- [ ]* 2.2 Write property test for active service consistency
  - **Property 2: Active Service Name Consistency**
  - **Validates: Requirements 2.3**
  - Generate random workspace with active service
  - Rename active service
  - Assert active service name matches services list
  - _Requirements: 2.3_

- [ ]* 2.3 Write property test for order preservation
  - **Property 3: Rename Preserves opened_services Order**
  - **Validates: Requirements 2.5**
  - Generate random workspace with multiple opened services
  - Rename random service
  - Assert service IDs remain in same order
  - _Requirements: 2.5_

- [ ]* 2.4 Write property test for selective update
  - **Property 4: Rename Updates Only Target Service**
  - **Validates: Requirements 1.5**
  - Generate random workspace with multiple opened services
  - Rename one service
  - Assert only target service updated, others unchanged
  - _Requirements: 1.5_

- [x] 3. Checkpoint - Ensure all tests pass
  - Run all unit tests and property tests
  - Verify tab titles update correctly in manual testing
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The fix is minimal and focused on updating the `opened_services` list after rename operations
