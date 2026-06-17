//! Load/save the settings document to disk.

use crate::{settings_path, Setting};
use std::io;

impl Setting {
    /// Loads settings from disk, falling back to defaults if the file is
    /// missing or unparseable.
    pub fn load() -> Self {
        let path = settings_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!(
                    "Invalid settings at {}: {e}; using defaults",
                    path.display()
                );
                Setting::default()
            }),
            Err(_) => Setting::default(),
        }
    }

    /// Writes settings to disk (creating the parent directory if needed).
    pub fn save(&self) -> io::Result<()> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(&path, text)
    }
}
