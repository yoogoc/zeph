# Implementation Plan: Folder Common Headers

## Overview

This implementation plan breaks down the folder common headers feature into discrete, manageable coding tasks. Each task builds incrementally on previous work, ensuring that the feature is developed systematically from data model extensions through UI implementation to HTTP service integration.

## Tasks

- [x] 1. Extend data models and database schema
  - Add common_headers and common_search_params fields to Folder struct
  - Create SearchParameter struct with key-value pairs
  - Update database schema with new columns for storing JSON data
  - Implement serialization/deserialization for new fields
  - _Requirements: 5.1, 5.2, 5.3, 5.5_

- [ ]* 1.1 Write property test for folder data model
  - **Property 5: Folder initialization**
  - **Validates: Requirements 5.3**

- [ ]* 1.2 Write property test for serialization round-trip
  - **Property 6: Serialization round-trip**
  - **Validates: Requirements 5.5**

- [x] 2. Extend storage layer for folder common settings
  - Add methods to FolderStore for updating common headers and search parameters
  - Implement database migration for existing folders
  - Add methods to retrieve folder with common settings
  - Handle JSON serialization in database operations
  - _Requirements: 5.4_

- [ ]* 2.1 Write property test for collection management operations
  - **Property 3: Collection management operations**
  - **Validates: Requirements 3.2, 3.3, 3.4, 3.5, 4.2, 4.3, 4.4, 4.5**

- [x] 3. Create folder detail page UI components
  - Create FolderDetailPage component with tab interface
  - Implement FolderDetailTab enum and tab switching logic
  - Create FolderOverviewPanel for basic folder information
  - Set up navigation and routing for folder detail pages
  - _Requirements: 1.1, 1.2, 2.1, 2.2_

- [ ]* 3.1 Write property test for tab interface functionality
  - **Property 2: Tab interface functionality**
  - **Validates: Requirements 2.3, 2.4**

- [ ]* 3.2 Write unit test for folder detail page display
  - Test that folder information is displayed correctly
  - Test placeholder message when no folder is selected
  - _Requirements: 1.2, 1.3_

- [x] 4. Implement common headers management panel
  - Create CommonHeadersPanel component
  - Implement HeaderInputRow for key-value input pairs
  - Add functionality for adding, editing, and deleting headers
  - Implement input validation for header keys and values
  - _Requirements: 3.1, 3.2, 3.5, 3.6_

- [ ]* 4.1 Write property test for collection display consistency
  - **Property 4: Collection display consistency**
  - **Validates: Requirements 3.1, 3.6, 4.1, 4.6**

- [x] 5. Implement search parameters management panel
  - Create SearchParamsPanel component
  - Implement ParamInputRow for key-value input pairs
  - Add functionality for adding, editing, and deleting parameters
  - Implement input validation for parameter keys and values
  - _Requirements: 4.1, 4.2, 4.5, 4.6_

- [x] 6. Checkpoint - Ensure folder detail page functionality works
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Integrate folder detail page with workspace navigation
  - Update WorkspaceView enum to include FolderDetail variant
  - Modify workspace sidebar to support folder detail navigation
  - Add navigation links from folder detail page back to service list
  - Implement breadcrumb navigation for folder hierarchy
  - _Requirements: 1.4, 7.1, 7.3_

- [ ]* 7.1 Write property test for folder navigation consistency
  - **Property 1: Folder navigation consistency**
  - **Validates: Requirements 1.1, 1.4, 7.1**

- [ ]* 7.2 Write property test for breadcrumb navigation accuracy
  - **Property 11: Breadcrumb navigation accuracy**
  - **Validates: Requirements 7.3**

- [x] 8. Extend HTTP service to inherit folder settings
  - Modify HttpService to query parent folder for common settings
  - Implement resolve_final_headers method to merge folder and service headers
  - Implement resolve_final_url method to merge search parameters
  - Add conflict resolution logic (service settings override folder settings)
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [ ]* 8.1 Write property test for HTTP service inheritance
  - **Property 7: HTTP service inheritance**
  - **Validates: Requirements 6.1, 6.2**

- [ ]* 8.2 Write property test for configuration conflict resolution
  - **Property 8: Configuration conflict resolution**
  - **Validates: Requirements 6.3, 6.4**

- [ ]* 8.3 Write property test for configuration merging
  - **Property 9: Configuration merging**
  - **Validates: Requirements 6.5**

- [x] 9. Add cross-folder navigation features
  - Add navigation links from service pages to parent folder details
  - Implement folder switching functionality in detail pages
  - Update workspace content to handle folder detail view transitions
  - _Requirements: 7.2, 7.4_

- [ ]* 9.1 Write property test for cross-folder navigation
  - **Property 10: Cross-folder navigation**
  - **Validates: Requirements 7.2, 7.4**

- [x] 10. Implement error handling and validation
  - Add input validation for header and parameter forms
  - Implement error states for database operations
  - Add loading states and user feedback for async operations
  - Handle edge cases like missing folders or malformed data
  - _Requirements: All error handling scenarios_

- [ ]* 10.1 Write unit tests for error handling
  - Test input validation edge cases
  - Test database error scenarios
  - Test UI error states and recovery

- [x] 11. Final integration and testing
  - Wire all components together in workspace content
  - Test end-to-end workflows from folder creation to HTTP request execution
  - Verify data persistence across application sessions
  - Test performance with large folder hierarchies
  - _Requirements: All integration requirements_

- [x] 12. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at key milestones
- Property tests validate universal correctness properties using randomized inputs
- Unit tests validate specific examples, edge cases, and error conditions
- The implementation follows Rust best practices and integrates with existing GPUI architecture