use crate::error::Result;
use std::path::Path;

/// Bundled translation files embedded in the binary
pub struct BundledTranslations;

impl BundledTranslations {
    /// Get bundled translation file content by locale
    pub fn get(locale: &str) -> Option<&'static [u8]> {
        match locale {
            "en" => Some(include_bytes!("../../../locales/en.yml")),
            "zh-CN" => Some(include_bytes!("../../../locales/zh-CN.yml")),
            _ => None,
        }
    }

    /// Get all available bundled locales
    pub fn available_locales() -> Vec<&'static str> {
        vec!["en", "zh-CN"]
    }

    /// Extract all bundled translations to a directory
    pub fn extract_to(dir: &Path) -> Result<()> {
        tracing::info!("Extracting bundled translations to {}", dir.display());

        for locale in Self::available_locales() {
            if let Some(content) = Self::get(locale) {
                let file_path = dir.join(format!("{}.yml", locale));
                std::fs::write(&file_path, content)?;
                tracing::info!("Extracted bundled translation: {}", file_path.display());
            }
        }

        tracing::info!("Successfully extracted all bundled translations");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_get_bundled_en() {
        let content = BundledTranslations::get("en");
        assert!(content.is_some());
        assert!(!content.unwrap().is_empty());
    }

    #[test]
    fn test_get_bundled_zh_cn() {
        let content = BundledTranslations::get("zh-CN");
        assert!(content.is_some());
        assert!(!content.unwrap().is_empty());
    }

    #[test]
    fn test_get_bundled_unknown() {
        let content = BundledTranslations::get("fr");
        assert!(content.is_none());
    }

    #[test]
    fn test_available_locales() {
        let locales = BundledTranslations::available_locales();
        assert_eq!(locales.len(), 2);
        assert!(locales.contains(&"en"));
        assert!(locales.contains(&"zh-CN"));
    }

    #[test]
    fn test_extract_to() {
        let temp_dir = std::env::temp_dir().join("zeph_test_extract");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let result = BundledTranslations::extract_to(&temp_dir);
        assert!(result.is_ok());

        // Verify files were created
        assert!(temp_dir.join("en.yml").exists());
        assert!(temp_dir.join("zh-CN.yml").exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
