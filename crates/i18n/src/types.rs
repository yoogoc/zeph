use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata for translation files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationMetadata {
    /// Version of the translation file format
    #[serde(rename = "_version")]
    pub version: u32,
}

/// Type alias for locale identifier (e.g., "en", "zh-CN")
pub type Locale = String;

/// Type alias for translation key (e.g., "common.save")
pub type TranslationKey = String;

/// Type alias for translation value
pub type TranslationValue = String;

/// Type alias for a single locale's translations (key → value)
pub type LocaleTranslations = HashMap<TranslationKey, TranslationValue>;

/// Type alias for all translations (locale → key → value)
pub type AllTranslations = HashMap<Locale, LocaleTranslations>;
