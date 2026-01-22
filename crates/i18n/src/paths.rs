use crate::error::{I18nError, Result};
use std::path::PathBuf;

/// Get the platform-specific data directory for translations
///
/// Returns:
/// - macOS: `~/Library/Application Support/Zeph/locales/`
/// - Linux: `~/.local/share/zeph/locales/`
/// - Windows: `%APPDATA%\Zeph\locales\`
///
/// Creates the directory if it doesn't exist.
pub fn get_data_dir() -> Result<PathBuf> {
    let base_dir = dirs::data_dir().ok_or_else(|| {
        I18nError::BundleExtractionError("Could not determine data directory".to_string())
    })?;

    let data_dir = base_dir.join("Zeph").join("locales");

    tracing::info!("Resolved data directory: {}", data_dir.display());

    // Create directory if it doesn't exist
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir).map_err(|source| I18nError::DataDirCreation {
            path: data_dir.clone(),
            source,
        })?;
        tracing::info!("Created data directory: {}", data_dir.display());
    }

    Ok(data_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_data_dir() {
        let result = get_data_dir();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("Zeph"));
        assert!(path.to_string_lossy().contains("locales"));

        // Verify directory was created
        assert!(path.exists());
        assert!(path.is_dir());
    }

    #[test]
    fn test_get_data_dir_idempotent() {
        // Call twice to ensure it's idempotent
        let path1 = get_data_dir().unwrap();
        let path2 = get_data_dir().unwrap();

        assert_eq!(path1, path2);
    }
}
