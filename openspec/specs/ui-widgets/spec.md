## ADDED Requirements

### Requirement: EditableInput encapsulates InputState management
The system SHALL provide an EditableInput widget that encapsulates InputState creation and common input behaviors including placeholder, validation, and event handling.

#### Scenario: Create input with placeholder
- **WHEN** EditableInput is instantiated with placeholder text
- **THEN** Input displays placeholder when empty and hides it when focused or has content

#### Scenario: Handle input change events
- **WHEN** User types in EditableInput with on_change callback configured
- **THEN** Callback is invoked with current input value

#### Scenario: Handle input submit events
- **WHEN** User presses Enter in EditableInput with on_submit callback configured
- **THEN** Callback is invoked with current input value

#### Scenario: Display validation error
- **WHEN** EditableInput is updated with validation_error message
- **THEN** Input displays error styling and error message below the field

### Requirement: TabBar provides reusable tab navigation
The system SHALL provide a TabBar widget that renders a horizontal row of selectable tabs with active state indication.

#### Scenario: Render tabs with active selection
- **WHEN** TabBar is instantiated with tab labels and active_index
- **THEN** Component displays all tabs with active tab visually highlighted

#### Scenario: Handle tab selection
- **WHEN** User clicks on an inactive tab
- **THEN** on_select callback is invoked with the clicked tab's index

#### Scenario: Support tab close buttons
- **WHEN** TabBar is instantiated with closable=true
- **THEN** Each tab displays a close button that triggers on_close callback when clicked

### Requirement: OverlayDialog provides modal container
The system SHALL provide an OverlayDialog widget that renders a semi-transparent overlay with centered dialog content and dismiss behavior.

#### Scenario: Render dialog with overlay
- **WHEN** OverlayDialog is instantiated with dialog content
- **THEN** Component displays semi-transparent backdrop with centered dialog

#### Scenario: Dismiss on backdrop click
- **WHEN** User clicks on the backdrop area outside the dialog
- **THEN** on_dismiss callback is invoked

#### Scenario: Prevent dismiss on dialog click
- **WHEN** User clicks within the dialog content area
- **THEN** on_dismiss callback is NOT invoked