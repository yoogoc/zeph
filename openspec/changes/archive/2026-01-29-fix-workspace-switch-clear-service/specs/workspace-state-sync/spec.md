## ADDED Requirements

### Requirement: Workspace switch clears service state

When switching between workspaces, the UI SHALL synchronize the current service state to match the target workspace's active service. The system SHALL NOT display services from the previous workspace.

#### Scenario: Switch to workspace with no active service
- **WHEN** user switches to a workspace that has no active service
- **THEN** the service content area SHALL display the empty state ("Select a service to get started")
- **AND** the service tabs SHALL display the target workspace's opened services list (may be empty)

#### Scenario: Switch to workspace with active service
- **WHEN** user switches to a workspace that has an active service
- **THEN** the service content area SHALL display that workspace's active service
- **AND** the service tabs SHALL display the target workspace's opened services list

#### Scenario: Previous workspace service cache cleanup
- **WHEN** user switches away from a workspace
- **THEN** the HTTP service instances from the previous workspace SHALL be cleared from memory
- **AND** switching back SHALL recreate service instances as needed