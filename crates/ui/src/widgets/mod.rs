//! Lightweight stateful reusable widgets
//!
//! This module contains components with minimal internal state
//! designed for reuse across different features.

mod editable_input;
mod overlay_dialog;

pub use editable_input::{EditableInput, EditableInputEvent};
pub use overlay_dialog::{OverlayDialog, OverlayDialogEvent, OverlayDialogExt};

// Re-export TabBar from gpui_component for convenience
// The gpui_component crate already provides a full-featured TabBar,
// so we re-export it here for consistency with the widget layer.
pub use gpui_component::tab::{Tab, TabBar, TabVariant};
