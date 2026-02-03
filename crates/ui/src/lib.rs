// New component organization
pub mod containers;
pub mod dialogs;
pub mod pages;
pub mod primitives;
pub mod selectors;
pub mod widgets;

// Shared utilities
mod folder_selector;
mod global_setting;
mod import_dialog;
mod simple_import_dialog;
mod utils;

// Re-export from new modules
pub use containers::*;
pub use dialogs::*;
pub use pages::*;
pub use primitives::*;
pub use selectors::*;
pub use widgets::*;

// Re-export remaining modules
pub use folder_selector::*;
pub use import_dialog::*;
pub use simple_import_dialog::*;

// Re-export i18n macros for use in this crate
#[macro_use]
extern crate i18n;
