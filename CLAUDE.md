# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Zeph is a Rust-based API client application built with GPUI (Zed's GPU-accelerated UI framework). It supports HTTP, gRPC, and WebSocket services with OpenAPI import capabilities.

## Build & Development Commands

```bash
cargo fmt                              # Format all code
cargo clippy --workspace --all-targets # Lint (fix or allow with justification)
cargo build --workspace                # Build all crates
cargo run -p zeph                      # Run the application
cargo test --workspace                 # Run all tests
cargo test -p <crate>                  # Run tests for specific crate
```

## Architecture

### Crate Structure

```
crates/
├── zeph/       # Main app: entry point, Application struct, theme init
├── ui/         # GPUI views: sidebar, workspace_area, dialogs, http_service/
├── models/     # Domain types: Workspace, Service, Folder, Environment, HttpConfig
├── workspace/  # State management: WorkspaceRegistry (global), OpenAPI import
├── storage/    # SQLite persistence: Database, *Store structs
├── setting/    # Application settings
└── i18n/       # Runtime i18n: t!() macro, locale switching
```

### Data Flow

1. **WorkspaceRegistry** (`crates/workspace/src/lib.rs`) - Global state manager, holds `ApplicationState` with workspaces, emits `WorkspaceEvent`/`WorkspaceServiceEvent`
2. **Storage Layer** - Each entity has a `*Store` struct (WorkspaceStore, ServiceStore, FolderStore, etc.) that wraps SQLite operations
3. **UI Layer** - Components subscribe to WorkspaceRegistry events via `cx.subscribe()` and call registry methods to mutate state

### Key Patterns

- **Global Registry**: `WorkspaceRegistry::global(cx)` provides access to app state
- **Entity Model**: Views use `Entity<T>` for GPUI's reactive state management
- **Event-Driven Updates**: State changes emit events, UI components subscribe and re-render

### Service Types

Models support three service types defined in `crates/models/src/lib.rs`:
- `ServiceType::Http(HttpConfig)` - HTTP requests with method, path, params, body
- `ServiceType::Grpc(GrpcConfig)` - gRPC with proto definitions
- `ServiceType::WebSocket(WebSocketConfig)` - WebSocket connections

## I18n System

- Translations in `locales/` (zh-CN default, en available)
- Use `t!("key")` macro or `i18n::t("key")` function
- Switch locale: `i18n::set_locale("en")`
- Initialize before use: `i18n::init()`

## Database

SQLite database stored at `dirs::data_dir()/zeph/zeph.db`. Storage crate handles all persistence with Store pattern.

## Code Style

- Rust 2021, 4-space indent, follow `rustfmt`
- Prefer `anyhow::Result` for errors, `tracing` for logs
- Tests use `#[cfg(test)] mod tests` with descriptive names