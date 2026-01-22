# Technology Stack

## Core Technologies

- **Language**: Rust 2021 edition
- **UI Framework**: GPUI (Zed's GPU-accelerated UI framework)
- **Component Library**: gpui-component for reusable UI elements
- **Build System**: Cargo workspace with multiple crates
- **Database**: SQLite via rusqlite (bundled)
- **HTTP Client**: reqwest with full feature set (json, cookies, compression, multipart, socks)

## Key Dependencies

- **Async Runtime**: smol, gpui_tokio for async operations
- **Serialization**: serde, serde_json
- **Internationalization**: rust-i18n with locale files in YAML
- **Error Handling**: anyhow for error propagation
- **Logging**: tracing with tracing-subscriber
- **Date/Time**: chrono

## Common Commands

### Development
```bash
# Format code
cargo fmt

# Lint with clippy
cargo clippy --workspace --all-targets

# Build all crates
cargo build --workspace

# Build release version
cargo build --release

# Run the application
cargo run -p zeph
```

### Testing
```bash
# Run all tests
cargo test --workspace

# Test specific crate
cargo test -p <crate-name>
```

### Release Build
The release profile is optimized for size and performance:
- Strip symbols enabled
- Optimization level: "z" (size)
- LTO enabled
- Single codegen unit
- Panic abort strategy

## Platform Support

- Primary target: macOS (darwin)
- Bundle identifier: `dev.yoogo.Zeph`
- URL scheme: `zeph://`
- Minimum icons: 32x32, 64x64, 128x128, 256x256
