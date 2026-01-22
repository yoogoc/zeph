//! Runtime I18n Loading System
//!
//! This crate provides a runtime translation loading system that replaces the
//! compile-time embedded approach. Translations are loaded from the filesystem
//! at application startup, enabling post-distribution extensibility.
//!
//! # Usage
//!
//! ```no_run
//! use i18n;
//!
//! // Initialize the i18n system before using translations
//! i18n::init().expect("Failed to initialize i18n");
//!
//! // Use the t! macro as usual
//! let text = i18n::t!("common.save");
//!
//! // Switch locales at runtime
//! i18n::set_locale("zh-CN");
//! ```

// Export modules
pub mod backend;
pub mod bundled;
pub mod error;
pub mod init;
pub mod loader;
pub mod paths;
pub mod store;
pub mod types;
pub mod yaml_utils;

// Re-export commonly used items
pub use error::{I18nError, Result};
pub use init::{get_store, init, init_with_dir};

/// Set the current locale
pub fn set_locale(locale: &str) {
    if let Some(store) = init::get_store() {
        store.set_locale(locale);
    } else {
        tracing::warn!("Attempted to set locale before i18n initialization");
    }
}

/// Get the current locale
pub fn locale() -> String {
    if let Some(store) = init::get_store() {
        store.current_locale()
    } else {
        tracing::warn!("Attempted to get locale before i18n initialization");
        "en".to_string()
    }
}

/// Get available locales
pub fn available_locales() -> Vec<String> {
    if let Some(store) = init::get_store() {
        store.available_locales()
    } else {
        tracing::warn!("Attempted to get available locales before i18n initialization");
        vec![]
    }
}

/// Get translation for a key
pub fn t(key: &str) -> String {
    if let Some(store) = init::get_store() {
        store.get(key)
    } else {
        tracing::warn!("Attempted to get translation before i18n initialization");
        key.to_string()
    }
}

/// Get translation for a key with parameter substitution
pub fn t_with_params(key: &str, params: &[(&str, &str)]) -> String {
    let mut result = t(key);
    for (param_name, param_value) in params {
        let placeholder = format!("%{{{}}}", param_name);
        result = result.replace(&placeholder, param_value);
    }
    result
}

/// Macro for getting translations
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::t($key)
    };
    ($key:expr, $($param:ident = $value:expr),+) => {{
        let params: &[(&str, &str)] = &[
            $((stringify!($param), &$value.to_string()),)+
        ];
        $crate::t_with_params($key, params)
    }};
}

/// Macro for converting translations to GPUI SharedString
#[macro_export]
macro_rules! shared_t {
    ($key:expr) => {
        SharedString::from($crate::t!($key))
    };
    ($key:expr, $($param:ident = $value:expr),+) => {
        SharedString::from($crate::t!($key, $($param = $value),+))
    };
}
