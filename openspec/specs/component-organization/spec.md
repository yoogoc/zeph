## ADDED Requirements

### Requirement: Directory structure follows state-based classification
The system SHALL organize UI components into directories based on their state complexity: primitives/, widgets/, containers/, dialogs/, pages/, and selectors/.

#### Scenario: Primitives directory contains stateless components
- **WHEN** A component has no internal state and only renders based on props
- **THEN** Component file SHALL be placed in primitives/ directory

#### Scenario: Widgets directory contains lightweight stateful components
- **WHEN** A component has minimal internal state and is designed for reuse across features
- **THEN** Component file SHALL be placed in widgets/ directory

#### Scenario: Containers directory contains business logic components
- **WHEN** A component manages Entity instances, Subscriptions, or complex business state
- **THEN** Component file SHALL be placed in containers/ directory

#### Scenario: Dialogs directory contains modal components
- **WHEN** A component renders as a modal overlay with user input or confirmation
- **THEN** Component file SHALL be placed in dialogs/ directory

#### Scenario: Pages directory contains route-level components
- **WHEN** A component represents a full page that composes multiple containers
- **THEN** Component file SHALL be placed in pages/ directory

### Requirement: Naming conventions reflect component type
The system SHALL use consistent naming suffixes to indicate component classification.

#### Scenario: Primitives use noun or View suffix
- **WHEN** A primitive component is created
- **THEN** Component name SHALL be a noun (e.g., Badge, Card) or end with View suffix (e.g., TreeItemView)

#### Scenario: Containers use business name or Container suffix
- **WHEN** A container component is created
- **THEN** Component name SHALL use the business domain name (e.g., HttpService) or end with Container suffix

#### Scenario: Dialogs use Dialog suffix
- **WHEN** A dialog component is created
- **THEN** Component name SHALL end with Dialog suffix (e.g., RenameDialog, ConfirmDialog)

#### Scenario: Pages use Page suffix
- **WHEN** A page component is created
- **THEN** Component name SHALL end with Page suffix (e.g., HomePage, SettingsPage)

### Requirement: Module exports follow directory structure
The system SHALL update lib.rs to export modules according to the new directory structure.

#### Scenario: Each directory has mod.rs
- **WHEN** A component directory is created (e.g., primitives/)
- **THEN** Directory SHALL contain a mod.rs that re-exports all components

#### Scenario: lib.rs exports all directories
- **WHEN** lib.rs is updated
- **THEN** All component directories SHALL be declared as public modules