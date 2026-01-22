use crate::types::AllTranslations;
use std::sync::RwLock;

/// Thread-safe store for translations with locale switching support
pub struct TranslationStore {
    translations: RwLock<AllTranslations>,
    current_locale: RwLock<String>,
    default_locale: String,
}

impl TranslationStore {
    /// Create a new store with translations
    pub fn new(translations: AllTranslations, default_locale: String) -> Self {
        let current_locale = default_locale.clone();

        Self {
            translations: RwLock::new(translations),
            current_locale: RwLock::new(current_locale),
            default_locale,
        }
    }

    /// Get translation for key in current locale
    ///
    /// Fallback chain: current locale → default locale → key itself
    pub fn get(&self, key: &str) -> String {
        let current_locale = self.current_locale.read().unwrap().clone();
        self.get_locale(&current_locale, key)
    }

    /// Get translation for key in specific locale
    ///
    /// Fallback chain: specified locale → default locale → key itself
    pub fn get_locale(&self, locale: &str, key: &str) -> String {
        let translations = self.translations.read().unwrap();

        // Try current/specified locale
        if let Some(locale_translations) = translations.get(locale) {
            if let Some(value) = locale_translations.get(key) {
                return value.clone();
            }
        }

        // Fallback to default locale
        if locale != self.default_locale {
            if let Some(default_translations) = translations.get(&self.default_locale) {
                if let Some(value) = default_translations.get(key) {
                    tracing::debug!(
                        "Key '{}' not found in locale '{}', using default locale '{}'",
                        key,
                        locale,
                        self.default_locale
                    );
                    return value.clone();
                }
            }
        }

        // Fallback to key itself
        tracing::debug!(
            "Key '{}' not found in any locale, returning key as fallback",
            key
        );
        key.to_string()
    }

    /// Set current locale
    pub fn set_locale(&self, locale: &str) {
        let translations = self.translations.read().unwrap();

        if translations.contains_key(locale) {
            let mut current = self.current_locale.write().unwrap();
            *current = locale.to_string();
            tracing::info!("Switched locale to '{}'", locale);
        } else {
            tracing::warn!(
                "Attempted to switch to unknown locale '{}', keeping current locale",
                locale
            );
        }
    }

    /// Get current locale
    pub fn current_locale(&self) -> String {
        self.current_locale.read().unwrap().clone()
    }

    /// Get all available locales
    pub fn available_locales(&self) -> Vec<String> {
        let translations = self.translations.read().unwrap();
        translations.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_store() -> TranslationStore {
        let mut translations = AllTranslations::new();

        let mut en = HashMap::new();
        en.insert("common.save".to_string(), "Save".to_string());
        en.insert("common.cancel".to_string(), "Cancel".to_string());
        translations.insert("en".to_string(), en);

        let mut zh_cn = HashMap::new();
        zh_cn.insert("common.save".to_string(), "保存".to_string());
        // Note: common.cancel is missing in zh-CN to test fallback
        translations.insert("zh-CN".to_string(), zh_cn);

        TranslationStore::new(translations, "en".to_string())
    }

    #[test]
    fn test_get_current_locale() {
        let store = create_test_store();
        let value = store.get("common.save");
        assert_eq!(value, "Save");
    }

    #[test]
    fn test_get_specific_locale() {
        let store = create_test_store();
        let value = store.get_locale("zh-CN", "common.save");
        assert_eq!(value, "保存");
    }

    #[test]
    fn test_fallback_to_default_locale() {
        let store = create_test_store();
        // common.cancel doesn't exist in zh-CN, should fallback to en
        let value = store.get_locale("zh-CN", "common.cancel");
        assert_eq!(value, "Cancel");
    }

    #[test]
    fn test_fallback_to_key() {
        let store = create_test_store();
        // Key doesn't exist in any locale
        let value = store.get("nonexistent.key");
        assert_eq!(value, "nonexistent.key");
    }

    #[test]
    fn test_set_locale() {
        let store = create_test_store();
        assert_eq!(store.current_locale(), "en");

        store.set_locale("zh-CN");
        assert_eq!(store.current_locale(), "zh-CN");

        let value = store.get("common.save");
        assert_eq!(value, "保存");
    }

    #[test]
    fn test_set_invalid_locale() {
        let store = create_test_store();
        assert_eq!(store.current_locale(), "en");

        store.set_locale("fr"); // Doesn't exist
        assert_eq!(store.current_locale(), "en"); // Should remain unchanged
    }

    #[test]
    fn test_available_locales() {
        let store = create_test_store();
        let mut locales = store.available_locales();
        locales.sort();

        assert_eq!(locales, vec!["en", "zh-CN"]);
    }

    #[test]
    fn test_locale_switching_idempotence() {
        let store = create_test_store();

        store.set_locale("zh-CN");
        let value1 = store.get("common.save");

        store.set_locale("zh-CN");
        let value2 = store.get("common.save");

        assert_eq!(value1, value2);
        assert_eq!(value1, "保存");
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(create_test_store());
        let mut handles = vec![];

        // Spawn multiple threads reading translations
        for _ in 0..10 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let _ = store_clone.get("common.save");
                }
            });
            handles.push(handle);
        }

        // Spawn threads switching locales
        for _ in 0..5 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    if i % 2 == 0 {
                        store_clone.set_locale("en");
                    } else {
                        store_clone.set_locale("zh-CN");
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify store is still functional
        assert!(!store.get("common.save").is_empty());
    }
}
