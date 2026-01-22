use crate::bundled::BundledTranslations;
use crate::error::{I18nError, Result};
use crate::types::{AllTranslations, LocaleTranslations};
use crate::yaml_utils::flatten_yaml;
use std::path::PathBuf;

/// Loads translation files from various sources
pub struct TranslationLoader {
    data_dir: PathBuf,
    source_dir: Option<PathBuf>,
}

impl TranslationLoader {
    /// Create a new loader with data directory and optional source directory
    pub fn new(data_dir: PathBuf, source_dir: Option<PathBuf>) -> Self {
        Self {
            data_dir,
            source_dir,
        }
    }

    /// Load a specific locale file
    pub fn load_locale(&self, locale: &str) -> Result<LocaleTranslations> {
        let file_name = format!("{}.yml", locale);

        // Try data directory first
        let data_path = self.data_dir.join(&file_name);
        if data_path.exists() {
            tracing::info!(
                "Loading locale '{}' from data directory: {}",
                locale,
                data_path.display()
            );
            return self.load_file(&data_path, locale);
        }

        // Try source directory (development mode)
        if let Some(ref source_dir) = self.source_dir {
            let source_path = source_dir.join(&file_name);
            if source_path.exists() {
                tracing::info!(
                    "Loading locale '{}' from source directory: {}",
                    locale,
                    source_path.display()
                );
                return self.load_file(&source_path, locale);
            }
        }

        tracing::warn!("Locale '{}' not found in any location", locale);
        Err(I18nError::LocaleNotFound(locale.to_string()))
    }

    /// Load all available translation files
    pub fn load_all(&self) -> Result<AllTranslations> {
        let mut translations = AllTranslations::new();
        let mut loaded_count = 0;

        // Discover locales from data directory
        let locales = self.discover_locales();

        if locales.is_empty() {
            tracing::warn!("No locale files found");
            return Err(I18nError::NoTranslationsAvailable);
        }

        for locale in locales {
            match self.load_locale(&locale) {
                Ok(locale_translations) => {
                    let count = locale_translations.len();
                    translations.insert(locale.clone(), locale_translations);
                    loaded_count += 1;
                    tracing::info!("Loaded {} translations for locale '{}'", count, locale);
                }
                Err(e) => {
                    tracing::warn!("Failed to load locale '{}': {}", locale, e);
                }
            }
        }

        if loaded_count == 0 {
            return Err(I18nError::NoTranslationsAvailable);
        }

        tracing::info!("Successfully loaded {} locales", loaded_count);
        Ok(translations)
    }

    /// Extract bundled translations to data directory
    pub fn extract_bundled_translations(&self) -> Result<()> {
        BundledTranslations::extract_to(&self.data_dir)
    }

    /// Check if data directory has translation files
    pub fn has_translations(&self) -> bool {
        !self.discover_locales().is_empty()
    }

    /// Discover available locales from data and source directories
    fn discover_locales(&self) -> Vec<String> {
        let mut locales = Vec::new();

        // Check data directory
        if let Ok(entries) = std::fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                if let Some(locale) = self.extract_locale_from_path(&entry.path()) {
                    if !locales.contains(&locale) {
                        locales.push(locale);
                    }
                }
            }
        }

        // Check source directory (development mode)
        if let Some(ref source_dir) = self.source_dir {
            if let Ok(entries) = std::fs::read_dir(source_dir) {
                for entry in entries.flatten() {
                    if let Some(locale) = self.extract_locale_from_path(&entry.path()) {
                        if !locales.contains(&locale) {
                            locales.push(locale);
                        }
                    }
                }
            }
        }

        locales
    }

    /// Extract locale name from file path (e.g., "en.yml" -> "en")
    fn extract_locale_from_path(&self, path: &std::path::Path) -> Option<String> {
        if path.extension()? != "yml" {
            return None;
        }

        path.file_stem()?.to_str().map(|s| s.to_string())
    }

    /// Load and parse a translation file
    fn load_file(&self, path: &std::path::Path, locale: &str) -> Result<LocaleTranslations> {
        let content = std::fs::read_to_string(path)?;

        let value: serde_yaml::Value =
            serde_yaml::from_str(&content).map_err(|source| I18nError::ParseError {
                file: path.display().to_string(),
                source,
            })?;

        let flattened = flatten_yaml(&value);

        if flattened.is_empty() {
            tracing::warn!("Translation file for locale '{}' is empty", locale);
        }

        Ok(flattened)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_locale_file(dir: &std::path::Path, locale: &str, content: &str) {
        let file_path = dir.join(format!("{}.yml", locale));
        fs::write(file_path, content).unwrap();
    }

    #[test]
    fn test_load_locale_from_data_dir() {
        let temp_dir = std::env::temp_dir().join("zeph_test_loader_data");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        create_test_locale_file(&temp_dir, "en", "_version: 2\ncommon:\n  save: \"Save\"\n");

        let loader = TranslationLoader::new(temp_dir.clone(), None);
        let result = loader.load_locale("en");

        assert!(result.is_ok());
        let translations = result.unwrap();
        assert_eq!(translations.get("common.save"), Some(&"Save".to_string()));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_locale_from_source_dir() {
        let data_dir = std::env::temp_dir().join("zeph_test_loader_data2");
        let source_dir = std::env::temp_dir().join("zeph_test_loader_source");
        let _ = fs::remove_dir_all(&data_dir);
        let _ = fs::remove_dir_all(&source_dir);
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&source_dir).unwrap();

        create_test_locale_file(
            &source_dir,
            "en",
            "_version: 2\ncommon:\n  cancel: \"Cancel\"\n",
        );

        let loader = TranslationLoader::new(data_dir.clone(), Some(source_dir.clone()));
        let result = loader.load_locale("en");

        assert!(result.is_ok());
        let translations = result.unwrap();
        assert_eq!(
            translations.get("common.cancel"),
            Some(&"Cancel".to_string())
        );

        let _ = fs::remove_dir_all(&data_dir);
        let _ = fs::remove_dir_all(&source_dir);
    }

    #[test]
    fn test_load_locale_priority() {
        let data_dir = std::env::temp_dir().join("zeph_test_loader_priority_data");
        let source_dir = std::env::temp_dir().join("zeph_test_loader_priority_source");
        let _ = fs::remove_dir_all(&data_dir);
        let _ = fs::remove_dir_all(&source_dir);
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&source_dir).unwrap();

        // Create different content in both directories
        create_test_locale_file(
            &data_dir,
            "en",
            "_version: 2\ncommon:\n  save: \"Save from data\"\n",
        );
        create_test_locale_file(
            &source_dir,
            "en",
            "_version: 2\ncommon:\n  save: \"Save from source\"\n",
        );

        let loader = TranslationLoader::new(data_dir.clone(), Some(source_dir.clone()));
        let result = loader.load_locale("en");

        assert!(result.is_ok());
        let translations = result.unwrap();
        // Should load from data directory (higher priority)
        assert_eq!(
            translations.get("common.save"),
            Some(&"Save from data".to_string())
        );

        let _ = fs::remove_dir_all(&data_dir);
        let _ = fs::remove_dir_all(&source_dir);
    }

    #[test]
    fn test_load_locale_not_found() {
        let temp_dir = std::env::temp_dir().join("zeph_test_loader_notfound");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let loader = TranslationLoader::new(temp_dir.clone(), None);
        let result = loader.load_locale("fr");

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), I18nError::LocaleNotFound(_)));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_has_translations() {
        let temp_dir = std::env::temp_dir().join("zeph_test_loader_has");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let loader = TranslationLoader::new(temp_dir.clone(), None);
        assert!(!loader.has_translations());

        create_test_locale_file(&temp_dir, "en", "_version: 2\ncommon:\n  save: \"Save\"\n");
        assert!(loader.has_translations());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
