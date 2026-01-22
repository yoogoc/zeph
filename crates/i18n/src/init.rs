use crate::error::Result;
use crate::loader::TranslationLoader;
use crate::paths::get_data_dir;
use crate::store::TranslationStore;
use std::path::PathBuf;
use std::sync::Arc;

// Global store for the initialized translation store
static mut GLOBAL_STORE: Option<Arc<TranslationStore>> = None;

/// Initialize the i18n system with runtime loading
///
/// This function:
/// 1. Resolves the data directory path
/// 2. Creates a TranslationLoader
/// 3. Checks if translations exist in data directory
/// 4. Extracts bundled translations if needed
/// 5. Loads all translations
/// 6. Creates TranslationStore
/// 7. Stores it globally for access
///
/// Must be called before any t!() macro usage.
pub fn init() -> Result<Arc<TranslationStore>> {
    tracing::info!("Initializing i18n system");

    let data_dir = get_data_dir()?;
    init_with_dir(data_dir)
}

/// Initialize with custom data directory
///
/// Useful for testing or custom deployment scenarios.
pub fn init_with_dir(data_dir: PathBuf) -> Result<Arc<TranslationStore>> {
    tracing::info!(
        "Initializing i18n system with data dir: {}",
        data_dir.display()
    );

    // Determine source directory (for development mode)
    let source_dir = if cfg!(debug_assertions) {
        // In development, try to use source locales directory
        let source_path = PathBuf::from("locales");
        if source_path.exists() {
            tracing::info!(
                "Development mode: using source directory {}",
                source_path.display()
            );
            Some(source_path)
        } else {
            None
        }
    } else {
        None
    };

    let loader = TranslationLoader::new(data_dir.clone(), source_dir);

    // Check if translations exist in data directory
    if !loader.has_translations() {
        tracing::info!("No translations found in data directory, extracting bundled translations");
        loader.extract_bundled_translations()?;
    } else {
        tracing::info!("Found existing translations in data directory");
    }

    // Load all translations
    let translations = loader.load_all()?;
    let locale_count = translations.len();
    tracing::info!("Loaded {} locales", locale_count);

    // Create store with English as default
    let store = Arc::new(TranslationStore::new(translations, "en".to_string()));

    // Store globally
    unsafe {
        GLOBAL_STORE = Some(Arc::clone(&store));
    }

    tracing::info!("i18n system initialized successfully");

    Ok(store)
}

/// Get the global translation store
///
/// Returns None if init() hasn't been called yet
pub fn get_store() -> Option<Arc<TranslationStore>> {
    unsafe { GLOBAL_STORE.as_ref().cloned() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_init_with_dir() {
        let temp_dir = std::env::temp_dir().join("zeph_test_init");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let result = init_with_dir(temp_dir.clone());
        assert!(result.is_ok());

        let store = result.unwrap();
        assert!(store.available_locales().len() >= 2);

        // Verify translations were extracted
        assert!(temp_dir.join("en.yml").exists());
        assert!(temp_dir.join("zh-CN.yml").exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_with_existing_translations() {
        let temp_dir = std::env::temp_dir().join("zeph_test_init_existing");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // First initialization - extracts bundled
        let result1 = init_with_dir(temp_dir.clone());
        assert!(result1.is_ok());

        // Second initialization - uses existing files
        let result2 = init_with_dir(temp_dir.clone());
        assert!(result2.is_ok());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_creates_store() {
        let temp_dir = std::env::temp_dir().join("zeph_test_init_store");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let store = init_with_dir(temp_dir.clone()).unwrap();

        // Verify store is functional
        let value = store.get("common.save");
        assert!(!value.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
