use std::path::PathBuf;

/// Errors that can occur during i18n operations
#[derive(Debug, thiserror::Error)]
pub enum I18nError {
    /// Failed to create data directory
    #[error("Failed to create data directory at {path}: {source}")]
    DataDirCreation {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse translation file
    #[error("Failed to parse translation file {file}: {source}")]
    ParseError {
        file: String,
        #[source]
        source: serde_yaml::Error,
    },

    /// No translations could be loaded
    #[error("No translations could be loaded from any source")]
    NoTranslationsAvailable,

    /// Failed to extract bundled translations
    #[error("Failed to extract bundled translations: {0}")]
    BundleExtractionError(String),

    /// Requested locale not found
    #[error("Locale '{0}' not found")]
    LocaleNotFound(String),

    /// I/O error occurred
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, I18nError>;
