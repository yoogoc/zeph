# UI Crate - Component Architecture

This crate contains the GPUI-based UI components for the Zeph application.

## Directory Structure

Components are organized into 6 directories based on their state complexity and reusability:

| Directory | Purpose | State Characteristics |
|-----------|---------|----------------------|
| `primitives/` | Pure rendering base elements | Stateless, props only |
| `widgets/` | Reusable interactive components | Lightweight state, generic functionality |
| `containers/` | Business logic containers | Hold Entity/Subscription |
| `dialogs/` | Modal dialogs | Input validation, confirmation flows |
| `pages/` | Route-level pages | Compose multiple containers |
| `selectors/` | Selector components | Dropdown, picker interactions |

## Component Guidelines

### Primitives (`primitives/`)

- Pure display components with no internal state
- Implemented as `struct` with `RenderOnce` trait
- Examples: `Badge`, `Card`, `KeyValueRow`, `TreeItem`, `EmptyState`

```rust
#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    color: Hsla,
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement { ... }
}
```

### Widgets (`widgets/`)

- Reusable components with lightweight internal state
- Encapsulate common patterns (e.g., input handling, overlay rendering)
- Examples: `EditableInput`, `TabBar`, `OverlayDialog`

### Containers (`containers/`)

- Business logic components that manage `Entity<T>` instances
- Handle subscriptions and complex state interactions
- Examples: `Sidebar`, `WorkspaceArea`, `WorkspaceContent`, `HttpService`

### Dialogs (`dialogs/`)

- Modal dialog components for user interactions
- Handle input validation and confirmation flows
- Examples: `RenameDialog`, `ConfirmDialog`, `EnvironmentManager`

### Pages (`pages/`)

- Top-level page components that represent application routes
- Compose multiple containers and manage navigation
- Examples: `HomePage`, `TitleBar`

### Selectors (`selectors/`)

- Dropdown and picker interaction components
- Handle selection state and emit change events
- Examples: `EnvironmentSelector`, `MethodSelector`

## Naming Conventions

- Files and modules: `snake_case`
- Types and traits: `CamelCase`
- Constants: `SCREAMING_SNAKE_CASE`

## State Management

- Use `Entity<T>` for stateful components
- Use `EventEmitter<E>` for parent-child communication
- Prefer `anyhow::Result` for fallible operations
- Use `tracing` for logging

## Adding New Components

1. Determine the appropriate directory based on state complexity
2. Follow the existing patterns in that directory
3. Export the component in the directory's `mod.rs`
4. Re-export from `lib.rs` if needed for external use