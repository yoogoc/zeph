# Requirements Document

## Introduction

This document specifies the requirements for fixing a bug where tab titles do not update after a service is renamed through inline tab editing. Currently, when a user renames a service by clicking on an active tab and editing the title, the database is updated correctly, but the tab title does not reflect the change until the application is restarted or the workspace is reloaded.

## Glossary

- **Service**: An API endpoint configuration (HTTP, gRPC, or WebSocket) that can be tested
- **Tab**: A UI element representing an open service in the tab bar
- **ContentType**: An enum that wraps different content types (Service, Folder, Schema)
- **WorkspaceRegistry**: The central state manager that handles workspace data and emits events
- **ServiceTabs**: The UI component that renders the tab bar
- **Workspace**: The data structure containing all opened services and active service information

## Requirements

### Requirement 1: Tab Title Synchronization

**User Story:** As a user, when I rename a service through inline tab editing, I want the tab title to update immediately, so that I can see the new name without restarting the application.

#### Acceptance Criteria

1. WHEN a user renames a service through inline tab editing THEN the tab title SHALL update immediately to show the new name
2. WHEN a service is renamed THEN the WorkspaceRegistry SHALL update the opened_services list with the new service data
3. WHEN the WorkspaceRegistry emits a Reload event THEN the ServiceTabs component SHALL receive the updated workspace with current service names
4. WHEN ServiceTabs renders tabs THEN it SHALL display the current name from the workspace's opened_services list
5. WHEN multiple tabs are open and one service is renamed THEN only the renamed service's tab SHALL update its title

### Requirement 2: Data Consistency

**User Story:** As a developer, I want the workspace state to remain consistent after rename operations, so that all UI components display accurate information.

#### Acceptance Criteria

1. WHEN a service is renamed THEN the Service object in opened_services SHALL be updated with the new name
2. WHEN the workspace is reloaded THEN all ContentType instances SHALL contain the latest service data from the database
3. WHEN a rename operation completes THEN the workspace's active_service SHALL reference the updated Service object if it was the active service
4. WHEN the WorkspaceRegistry updates a service THEN it SHALL fetch the latest service data from the database
5. WHEN opened_services is updated THEN the order of services SHALL be preserved
