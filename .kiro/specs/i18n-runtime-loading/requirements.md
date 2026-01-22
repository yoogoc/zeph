# Requirements Document

## Introduction

This specification defines the requirements for migrating Zeph's internationalization (i18n) system from compile-time embedded translations to runtime dynamic loading from the filesystem. The current implementation uses rust-i18n macros to embed all translations into the binary at compile time, which prevents post-distribution extensibility. The new system will load translation files dynamically from a data directory, enabling users and distributors to add or modify translations without recompiling.

## Glossary

- **I18n_System**: The internationalization subsystem responsible for loading and providing translations
- **Translation_File**: A YAML file containing key-value pairs for a specific locale (e.g., en.yml, zh-CN.yml)
- **Data_Directory**: The runtime directory where translation files are stored after installation
- **Locale**: A language/region identifier (e.g., "en", "zh-CN")
- **Fallback_Mechanism**: The process of loading translations from the source repository when data directory files are unavailable
- **Translation_Key**: A dot-separated string identifier for a translatable text (e.g., "app.title")

## Requirements

### Requirement 1: Runtime Translation Loading

**User Story:** As a developer, I want the application to load translations from the filesystem at runtime, so that translations can be modified or extended without recompiling the application.

#### Acceptance Criteria

1. WHEN the application starts, THE I18n_System SHALL load translation files from the Data_Directory
2. WHEN a Translation_File is modified in the Data_Directory, THE I18n_System SHALL reflect changes on next application restart
3. WHEN loading translation files, THE I18n_System SHALL parse YAML format files
4. THE I18n_System SHALL support loading multiple Translation_Files simultaneously (one per Locale)

### Requirement 2: Locale-Separated Translation Files

**User Story:** As a translator, I want each language to have its own translation file, so that I can manage translations for specific languages independently.

#### Acceptance Criteria

1. THE I18n_System SHALL store translations in separate files named by Locale (e.g., en.yml, zh-CN.yml)
2. WHEN a Locale is requested, THE I18n_System SHALL load only the corresponding Translation_File
3. THE I18n_System SHALL support at minimum the "en" and "zh-CN" locales
4. WHEN a new Translation_File is added to the Data_Directory, THE I18n_System SHALL recognize it as a valid Locale

### Requirement 3: Data Directory Management

**User Story:** As a user, I want translation files to be automatically installed to a standard location, so that I can find and modify them easily.

#### Acceptance Criteria

1. WHEN the application is installed, THE I18n_System SHALL copy Translation_Files to the Data_Directory
2. THE Data_Directory SHALL be located in a platform-appropriate location (e.g., ~/Library/Application Support/Zeph on macOS)
3. WHEN the Data_Directory does not exist, THE I18n_System SHALL create it
4. WHEN Translation_Files are missing from the Data_Directory, THE I18n_System SHALL copy them from the bundled resources

### Requirement 4: Development Fallback Mechanism

**User Story:** As a developer, I want the application to load translations from the source repository during development, so that I don't need to manually copy files during development iterations.

#### Acceptance Criteria

1. WHEN running in development mode, THE I18n_System SHALL attempt to load Translation_Files from the source repository locales/ directory
2. WHEN Translation_Files are not found in the Data_Directory, THE I18n_System SHALL fall back to the source repository location
3. WHEN Translation_Files exist in both locations, THE I18n_System SHALL prioritize the Data_Directory version
4. THE I18n_System SHALL log which location was used for loading Translation_Files

### Requirement 5: Translation Key Resolution

**User Story:** As a developer, I want to retrieve translations using simple key lookups, so that the API remains easy to use throughout the codebase.

#### Acceptance Criteria

1. THE I18n_System SHALL provide a function to retrieve translations by Translation_Key
2. WHEN a Translation_Key is not found in the current Locale, THE I18n_System SHALL return the key itself as a fallback
3. WHEN a Translation_Key contains nested structure (dot-separated), THE I18n_System SHALL resolve it correctly
4. THE I18n_System SHALL support runtime locale switching without application restart

### Requirement 6: Error Handling and Logging

**User Story:** As a developer, I want clear error messages when translation loading fails, so that I can diagnose and fix configuration issues quickly.

#### Acceptance Criteria

1. WHEN a Translation_File cannot be parsed, THE I18n_System SHALL log a descriptive error and continue with available translations
2. WHEN the Data_Directory cannot be created, THE I18n_System SHALL log an error and fall back to bundled resources
3. WHEN no Translation_Files can be loaded, THE I18n_System SHALL log a critical error and use Translation_Keys as display text
4. THE I18n_System SHALL log the successful loading of each Translation_File with its file path

### Requirement 7: Backward Compatibility

**User Story:** As a developer, I want the public API to remain similar to the current implementation, so that existing code requires minimal changes.

#### Acceptance Criteria

1. THE I18n_System SHALL provide a `t!()` macro or function with similar syntax to the current rust-i18n implementation
2. WHEN existing code calls translation functions, THE I18n_System SHALL return translated strings without requiring code changes beyond initialization
3. THE I18n_System SHALL support the same Translation_Key format as the current implementation
4. WHEN the current locale is changed, THE I18n_System SHALL update all subsequent translation calls to use the new locale

### Requirement 8: Bundle Resource Integration

**User Story:** As a release engineer, I want translation files to be included in the application bundle, so that the application works immediately after installation without network access.

#### Acceptance Criteria

1. WHEN building a release bundle, THE I18n_System SHALL include all Translation_Files in the application resources
2. THE I18n_System SHALL provide a mechanism to extract bundled Translation_Files to the Data_Directory
3. WHEN the application runs for the first time, THE I18n_System SHALL automatically extract bundled Translation_Files
4. THE bundled Translation_Files SHALL be embedded in a way that works across all supported platforms
