# Implementation Plan: Runtime I18n Loading System

## Overview

This implementation plan converts the compile-time i18n system to runtime loading by creating a custom translation loader, store, and backend that integrates with rust-i18n's API. The implementation will maintain backward compatibility while enabling post-distribution extensibility.

## Tasks

- [x] 1. Split translation files by locale
  - Split `locales/app.yml` into separate files: `locales/en.yml` and `locales/zh-CN.yml`
  - Each file should contain only translations for its locale (remove the nested locale structure)
  - Preserve the `_version: 2` metadata in each file
  - Verify both files contain all the same keys with appropriate translations
  - _Requirements: 2.1, 2.3_

- [ ] 2. Implement translation data structures and error types
  - [x] 2.1 Create error types in `crates/i18n/src/error.rs`
    - Define `I18nError` enum with variants for data directory creation, parse errors, no translations available, bundle extraction, and locale not found
    - Implement `thiserror::Error` trait for all error types
    - Add context fields (file paths, locale names) to error variants
    - _Requirements: 6.1, 6.2, 6.3_
  
  - [x] 2.2 Create translation metadata types in `crates/i18n/src/types.rs`
    - Define `TranslationMetadata` struct with version field
    - Define helper types for translation storage (locale → key → value mappings)
    - _Requirements: 1.3_

- [ ] 3. Implement YAML flattening utility
  - [x] 3.1 Create `crates/i18n/src/yaml_utils.rs` with flattening logic
    - Implement function to flatten nested YAML into dot-notation keys
    - Handle `_version` metadata field specially (skip it during flattening)
    - Support arbitrary nesting depth
    - Preserve string values exactly as they appear in YAML
    - _Requirements: 1.3, 5.3_
  
  - [ ]* 3.2 Write property test for YAML flattening
    - **Property 4: YAML Round-Trip Consistency**
    - Generate random nested YAML structures
    - Verify flattening preserves all key-value pairs
    - **Validates: Requirements 1.3**

- [ ] 4. Implement TranslationLoader
  - [x] 4.1 Create `crates/i18n/src/loader.rs` with TranslationLoader struct
    - Implement `new()` constructor with data_dir and optional source_dir
    - Implement `load_locale()` to load a single locale file
    - Implement `load_all()` to discover and load all available locale files
    - Implement `has_translations()` to check if data directory has files
    - Add logging for all file operations (info for success, warn for failures)
    - _Requirements: 1.1, 1.2, 2.2, 4.1, 4.2, 4.3, 4.4, 6.1, 6.4_
  
  - [ ]* 4.2 Write unit tests for TranslationLoader
    - Test loading from data directory
    - Test loading from source directory (fallback)
    - Test handling missing files
    - Test handling malformed YAML
    - Test priority (data dir over source dir)
    - _Requirements: 4.2, 4.3, 6.1_
  
  - [ ]* 4.3 Write property test for locale file independence
    - **Property 2: Locale File Independence**
    - Generate random locale files
    - Load in different orders
    - Verify each locale's translations are unaffected by others
    - **Validates: Requirements 2.2**

- [ ] 5. Implement bundled resource management
  - [x] 5.1 Create `crates/i18n/src/bundled.rs` with BundledTranslations
    - Use `include_bytes!()` to embed `locales/en.yml` and `locales/zh-CN.yml`
    - Implement `get()` to retrieve bundled content by locale
    - Implement `available_locales()` to list bundled locales
    - Implement `extract_to()` to write bundled files to a directory
    - Add logging for extraction operations
    - _Requirements: 3.4, 8.2, 8.3, 8.4_
  
  - [x] 5.2 Update TranslationLoader to support bundled extraction
    - Implement `extract_bundled_translations()` method
    - Call bundled extraction when data directory is empty
    - Log extraction success/failure
    - _Requirements: 3.4, 8.2_
  
  - [ ]* 5.3 Write property test for extraction idempotence
    - **Property 7: File Extraction Idempotence**
    - Extract bundled files multiple times to temp directory
    - Verify file content remains identical across extractions
    - **Validates: Requirements 3.4, 8.2**

- [ ] 6. Implement TranslationStore
  - [x] 6.1 Create `crates/i18n/src/store.rs` with TranslationStore struct
    - Use `RwLock<HashMap<String, HashMap<String, String>>>` for translations
    - Use `RwLock<String>` for current locale
    - Implement `new()` constructor
    - Implement `get()` for current locale lookup with fallback chain
    - Implement `get_locale()` for specific locale lookup
    - Implement `set_locale()` and `current_locale()`
    - Implement `available_locales()`
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  
  - [ ]* 6.2 Write unit tests for TranslationStore
    - Test key lookup in current locale
    - Test fallback to default locale
    - Test fallback to key itself when not found
    - Test locale switching
    - Test nested key resolution
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  
  - [ ]* 6.3 Write property test for key fallback chain
    - **Property 6: Key Fallback Chain**
    - Generate random keys (some missing from locales)
    - Verify system never returns None or panics
    - Verify fallback chain: current locale → default locale → key itself
    - **Validates: Requirements 5.2**
  
  - [ ]* 6.4 Write property test for locale switching idempotence
    - **Property 5: Locale Switching Idempotence**
    - Generate random locale switches
    - Verify repeated switches to same locale produce identical results
    - **Validates: Requirements 5.4**
  
  - [ ]* 6.5 Write property test for concurrent access safety
    - **Property 8: Concurrent Access Safety**
    - Generate random concurrent translation lookups and locale switches
    - Verify no data races or panics occur
    - **Validates: Requirements 5.1, 5.4**

- [ ] 7. Implement platform-specific data directory resolution
  - [x] 7.1 Add `dirs` crate dependency to `crates/i18n/Cargo.toml`
    - Add `dirs = "5.0"` to dependencies
    - _Requirements: 3.2_
  
  - [x] 7.2 Create `crates/i18n/src/paths.rs` with data directory logic
    - Implement `get_data_dir()` function using `dirs::data_dir()`
    - Return platform-specific paths:
      - macOS: `~/Library/Application Support/Zeph/locales/`
      - Linux: `~/.local/share/zeph/locales/`
      - Windows: `%APPDATA%\Zeph\locales\`
    - Create directory if it doesn't exist
    - Log the resolved path
    - _Requirements: 3.2, 3.3_
  
  - [ ]* 7.3 Write unit tests for data directory resolution
    - Test path format on current platform
    - Test directory creation
    - Test handling of permission errors
    - _Requirements: 3.2, 3.3_

- [ ] 8. Implement RuntimeBackend for rust-i18n integration
  - [x] 8.1 Create `crates/i18n/src/backend.rs` with RuntimeBackend
    - Implement struct wrapping `Arc<TranslationStore>`
    - Implement `rust_i18n::Backend` trait
    - Implement `available_locales()` method
    - Implement `translate()` method with fallback to English
    - Handle string lifetime requirements (may need internal caching)
    - _Requirements: 5.1, 7.1, 7.2_
  
  - [ ]* 8.2 Write unit tests for RuntimeBackend
    - Test Backend trait implementation
    - Test integration with rust-i18n macro system
    - Test fallback behavior
    - _Requirements: 7.1, 7.2_

- [ ] 9. Implement initialization module
  - [x] 9.1 Create `crates/i18n/src/init.rs` with initialization functions
    - Implement `init()` function that:
      - Resolves data directory path
      - Creates TranslationLoader with data dir and source dir (in dev mode)
      - Checks if translations exist in data dir
      - Extracts bundled translations if needed
      - Loads all translations
      - Creates TranslationStore
      - Creates RuntimeBackend
      - Registers backend with rust-i18n
    - Implement `init_with_dir()` for custom data directory
    - Add comprehensive logging for each step
    - Handle all errors gracefully with fallbacks
    - _Requirements: 1.1, 3.1, 3.3, 3.4, 4.1, 4.2, 6.2, 6.3, 8.3_
  
  - [ ]* 9.2 Write integration tests for initialization
    - Test first-run scenario (extract bundled → load)
    - Test subsequent runs (load from data dir)
    - Test development mode (load from source)
    - Test error handling (missing files, parse errors)
    - _Requirements: 1.1, 3.1, 3.4, 4.1, 4.2, 6.2, 6.3, 8.3_
  
  - [ ]* 9.3 Write property test for data directory fallback
    - **Property 3: Data Directory Fallback**
    - Test with empty/missing data directory
    - Verify system loads from bundled or source directory
    - Verify all translation keys are accessible
    - **Validates: Requirements 4.2**

- [x] 10. Update crates/i18n/src/lib.rs to export new API
  - Remove old `rust_i18n::i18n!()` macro invocation
  - Remove old `I18nBackend` implementation
  - Export new modules: `error`, `types`, `loader`, `store`, `backend`, `init`, `paths`, `bundled`, `yaml_utils`
  - Re-export `rust_i18n::{t, set_locale}` for backward compatibility
  - Update or remove `init!()` macro (may no longer be needed)
  - Keep `shared_t!()` macro for GPUI SharedString conversion
  - Add module documentation explaining the new runtime loading system
  - _Requirements: 7.1, 7.2, 7.3_

- [x] 11. Update application initialization in crates/zeph/src/main.rs
  - Remove old `i18n::init!()` macro call
  - Add `i18n::init()` function call before creating the Application
  - Handle initialization errors (log and continue with degraded mode)
  - Keep existing `rust_i18n::set_locale("zh-CN")` call after initialization
  - Verify `t!()` macro calls throughout the app still work
  - _Requirements: 1.1, 7.2_

- [x] 12. Add necessary dependencies to workspace Cargo.toml
  - Add `serde_yaml` to workspace dependencies
  - Add `dirs` to workspace dependencies
  - Add `thiserror` to workspace dependencies (if not already present)
  - Update `crates/i18n/Cargo.toml` to use these workspace dependencies
  - _Requirements: 1.3, 3.2_

- [x] 13. Checkpoint - Ensure all tests pass
  - Run `cargo test --workspace` to verify all tests pass
  - Run `cargo clippy --workspace --all-targets` to check for warnings
  - Run `cargo fmt` to format code
  - Run `cargo run -p zeph` to test the application manually
  - Verify translations load correctly and UI displays translated text
  - Test locale switching in the settings UI
  - Ask the user if questions arise

- [ ] 14. Update bundled resources for release builds
  - [x] 14.1 Ensure translation files are included in release bundle
    - Verify `locales/en.yml` and `locales/zh-CN.yml` are embedded via `include_bytes!()`
    - Test release build: `cargo build --release`
    - Verify bundled translations are accessible in release mode
    - _Requirements: 8.1, 8.4_
  
  - [ ]* 14.2 Write integration test for release bundle
    - Test that bundled translations work without source directory
    - Test first-run extraction in release mode
    - _Requirements: 8.1, 8.3, 8.4_

- [x] 15. Final checkpoint - End-to-end verification
  - Test complete workflow: first run → extraction → loading → usage
  - Test modifying translation files in data directory and restarting
  - Test adding a new locale file (e.g., `fr.yml`) and verifying it's recognized
  - Verify error handling: delete data directory, corrupt a YAML file, etc.
  - Verify logging output is helpful for debugging
  - Run full test suite: `cargo test --workspace`
  - Ensure all tests pass, ask the user if questions arise

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- Integration tests verify end-to-end workflows
- The implementation maintains backward compatibility with existing `t!()` macro usage
- Development mode automatically falls back to source directory for convenience
- Release builds embed translations and extract them on first run
