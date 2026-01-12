# Repository Guidelines

## Project Structure & Module Organization
- Workspace root: `Cargo.toml` defines members in `crates/*`; main app lives in `crates/zeph` (`src/main.rs`, `application.rs`, `theme.rs`, `resources/` for icons/bundle files).
- UI layer: `crates/ui` hosts GPUI views (sidebar, workspace area, dialogs, selectors). Feature work that changes layout or interactions should land here.
- Domain/data: `crates/models`, `crates/workspace`, `crates/setting`, `crates/storage`, and `crates/i18n` hold shared types, state, and helpers; prefer adding new services or stores as their own modules.
- Assets: `locales/` contains translation files (default `zh-CN`, `en` available); `themes/` contains theme definitions; `target/` is build output (do not commit).

## Build, Test, and Development Commands
- `cargo fmt` — format all Rust code.
- `cargo clippy --workspace --all-targets` — lint the workspace; fix or allow with justification.
- `cargo build --workspace` (or `--release` for bundles) — compile all crates.
- `cargo run -p zeph` — launch the GPUI app locally.
- `cargo test --workspace` — run tests; for a single crate use `-p <crate>`.

## Coding Style & Naming Conventions
- Rust 2021, 4-space indent; follow `rustfmt` output and keep `use` ordering tidy.
- Modules and files in `snake_case`; types and traits in `CamelCase`; constants in `SCREAMING_SNAKE_CASE`.
- Prefer `anyhow::Result` for fallible functions, `tracing` for logs, and avoid panics in UI or IO paths.
- Keep UI logic composable: push shared view helpers into `crates/ui/utils.rs` or crate-specific submodules instead of `main.rs`.

## Testing Guidelines
- Add unit tests alongside code with `#[cfg(test)] mod tests` and descriptive `it_*`/`should_*`-style names.
- Favor deterministic tests; mock network or filesystem access where possible.
- When adding stateful logic (workspace, storage, settings), cover both success and error paths before opening a PR.

## Commit & Pull Request Guidelines
- Commits are short, present-tense summaries (e.g., `fix sidebar resize`, `调整样式`); squash noisy WIP commits before sharing.
- PRs should state scope, user-facing impact, and any i18n/theme updates; link issues/tasks when available.
- Include run results for `fmt`, `clippy`, and relevant tests in the PR description; add screenshots/GIFs for UI changes.
