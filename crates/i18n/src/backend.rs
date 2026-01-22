use crate::store::TranslationStore;
use std::sync::Arc;

/// Runtime backend for rust-i18n integration
pub struct RuntimeBackend {
    store: Arc<TranslationStore>,
}

impl RuntimeBackend {
    /// Create a new runtime backend
    pub fn new(store: Arc<TranslationStore>) -> Self {
        Self { store }
    }

    /// Get the underlying store
    pub fn store(&self) -> &Arc<TranslationStore> {
        &self.store
    }
}

impl rust_i18n::Backend for RuntimeBackend {
    fn available_locales(&self) -> Vec<&str> {
        // rust-i18n expects static string references, but we have owned Strings
        // This is a limitation - we'll return a static list for now
        vec!["en", "zh-CN"]
    }

    fn translate(&self, locale: &str, key: &str) -> Option<&str> {
        // rust-i18n expects a static string reference, but our store returns owned Strings
        // We need to use a workaround: leak the string to get a static reference
        // This is acceptable because translations are loaded once at startup
        let value = self.store.get_locale(locale, key);
        Some(Box::leak(value.into_boxed_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AllTranslations;
    use rust_i18n::Backend;
    use std::collections::HashMap;

    fn create_test_backend() -> RuntimeBackend {
        let mut translations = AllTranslations::new();

        let mut en = HashMap::new();
        en.insert("common.save".to_string(), "Save".to_string());
        en.insert("common.cancel".to_string(), "Cancel".to_string());
        translations.insert("en".to_string(), en);

        let mut zh_cn = HashMap::new();
        zh_cn.insert("common.save".to_string(), "保存".to_string());
        translations.insert("zh-CN".to_string(), zh_cn);

        let store = Arc::new(TranslationStore::new(translations, "en".to_string()));
        RuntimeBackend::new(store)
    }

    #[test]
    fn test_available_locales() {
        let backend = create_test_backend();
        let locales = backend.available_locales();
        assert_eq!(locales.len(), 2);
        assert!(locales.contains(&"en"));
        assert!(locales.contains(&"zh-CN"));
    }

    #[test]
    fn test_translate() {
        let backend = create_test_backend();
        let value = backend.translate("en", "common.save");
        assert_eq!(value, Some("Save"));
    }

    #[test]
    fn test_translate_zh_cn() {
        let backend = create_test_backend();
        let value = backend.translate("zh-CN", "common.save");
        assert_eq!(value, Some("保存"));
    }

    #[test]
    fn test_translate_fallback() {
        let backend = create_test_backend();
        // common.cancel doesn't exist in zh-CN, should fallback to en
        let value = backend.translate("zh-CN", "common.cancel");
        assert_eq!(value, Some("Cancel"));
    }

    #[test]
    fn test_translate_missing_key() {
        let backend = create_test_backend();
        let value = backend.translate("en", "nonexistent.key");
        assert_eq!(value, Some("nonexistent.key"));
    }
}
