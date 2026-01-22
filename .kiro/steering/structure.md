# Project Structure

## Workspace Organization

Zeph uses a Cargo workspace with multiple crates under `crates/`:

```
crates/
├── zeph/          # Main application entry point
├── ui/            # GPUI views and UI components
├── models/        # Shared data models and types
├── workspace/     # Workspace and OpenAPI logic
├── storage/       # SQLite database layer
├── setting/       # Application settings
└── i18n/          # Internationalization utilities
```

## Crate Responsibilities

### `crates/zeph` (Main Application)
- Entry point: `src/main.rs`
- Application initialization: `src/application.rs`
- Theme management: `src/theme.rs`
- Bundle resources: `resources/` (app icons)
- Handles window creation and GPUI setup

### `crates/ui` (UI Layer)
All GPUI views and interactive components:
- `sidebar.rs`, `ws_sidebar/` - Navigation and workspace sidebar
- `workspace_area.rs`, `workspace_content.rs` - Main content area
- `http_service/` - HTTP request/response UI (method selector, headers, body, URL utils)
- `folder_detail.rs`, `folder_selector.rs` - Folder management
- `environment_selector.rs` - Environment switching
- `service_tabs.rs` - Tab management for services
- `import_dialog.rs`, `simple_import_dialog.rs` - Import workflows
- `global_setting/` - Settings UI (language, menu)
- `home_page.rs`, `title_bar.rs` - App chrome
- `utils.rs` - Shared UI helpers

### `crates/models`
Shared domain types and runtime state

### `crates/workspace`
Workspace logic and OpenAPI handling (`openapi.rs`)

### `crates/storage`
SQLite persistence layer:
- `db.rs` - Database connection
- `schema.rs`, `schema_store.rs` - Schema management
- `workspace.rs`, `folder.rs`, `environment.rs` - Entity stores
- `service.rs` - Service data access

### `crates/setting`
Application settings and preferences

### `crates/i18n`
Internationalization utilities (wraps rust-i18n)

## Asset Directories

- `locales/` - Translation files (YAML format, default: zh-CN)
- `themes/` - Theme definitions (currently empty)
- `target/` - Build output (gitignored)

## Configuration Files

- Root `Cargo.toml` - Workspace definition and shared dependencies
- `.kiro/` - Kiro IDE configuration (specs, steering)
- `.vscode/` - VS Code settings
- `.gitignore` - Git exclusions

## Module Naming Conventions

- Files and modules: `snake_case`
- Types and traits: `CamelCase`
- Constants: `SCREAMING_SNAKE_CASE`
- Keep related UI components in submodules (e.g., `http_service/`, `ws_sidebar/`, `global_setting/`)
