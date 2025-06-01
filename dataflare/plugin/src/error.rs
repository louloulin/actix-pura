//! Error handling for the DataFlare plugin system
//!
//! Provides comprehensive error types that integrate with DataFlare's existing
//! error handling while maintaining plugin-specific error information.

use thiserror::Error;

/// Plugin-specific error types
#[derive(Debug, Error)]
pub enum PluginError {
    /// Error during data processing
    #[error("Processing error: {0}")]
    Processing(String),

    /// Invalid input data
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Plugin configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Plugin initialization error
    #[error("Initialization error: {0}")]
    Initialization(String),

    /// Plugin not found
    #[error("Plugin not found: {0}")]
    NotFound(String),

    /// Plugin version incompatibility
    #[error("Version incompatibility: {0}")]
    VersionIncompatible(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// UTF-8 conversion error
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// Generic error with custom message
    #[error("Plugin error: {0}")]
    Custom(String),
}

/// Result type for plugin operations
pub type Result<T> = std::result::Result<T, PluginError>;

impl PluginError {
    /// Create a new processing error
    pub fn processing<S: Into<String>>(msg: S) -> Self {
        Self::Processing(msg.into())
    }

    /// Create a new invalid input error
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a new configuration error
    pub fn configuration<S: Into<String>>(msg: S) -> Self {
        Self::Configuration(msg.into())
    }

    /// Create a new initialization error
    pub fn initialization<S: Into<String>>(msg: S) -> Self {
        Self::Initialization(msg.into())
    }

    /// Create a new custom error
    pub fn custom<S: Into<String>>(msg: S) -> Self {
        Self::Custom(msg.into())
    }

    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Processing(_) | Self::InvalidInput(_) => true,
            Self::Configuration(_) | Self::Initialization(_) | Self::NotFound(_) => false,
            Self::VersionIncompatible(_) => false,
            Self::Io(_) | Self::Json(_) | Self::Utf8(_) => true,
            Self::Custom(_) => false,
        }
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::Processing(_) => "processing",
            Self::InvalidInput(_) => "input",
            Self::Configuration(_) => "configuration",
            Self::Initialization(_) => "initialization",
            Self::NotFound(_) => "not_found",
            Self::VersionIncompatible(_) => "version",
            Self::Io(_) => "io",
            Self::Json(_) => "json",
            Self::Utf8(_) => "utf8",
            Self::Custom(_) => "custom",
        }
    }
}

/// Convert PluginError to DataFlare's core error type
impl From<PluginError> for dataflare_core::error::DataFlareError {
    fn from(err: PluginError) -> Self {
        match err {
            PluginError::Processing(msg) => {
                dataflare_core::error::DataFlareError::Processing(format!("Plugin: {}", msg))
            }
            PluginError::InvalidInput(msg) => {
                dataflare_core::error::DataFlareError::Validation(format!("Plugin input: {}", msg))
            }
            PluginError::Configuration(msg) => {
                dataflare_core::error::DataFlareError::Config(format!("Plugin config: {}", msg))
            }
            PluginError::Initialization(msg) => {
                dataflare_core::error::DataFlareError::Plugin(format!("Plugin init: {}", msg))
            }
            PluginError::NotFound(msg) => {
                dataflare_core::error::DataFlareError::Plugin(format!("Plugin not found: {}", msg))
            }
            PluginError::VersionIncompatible(msg) => {
                dataflare_core::error::DataFlareError::Plugin(format!("Plugin version: {}", msg))
            }
            PluginError::Io(err) => {
                dataflare_core::error::DataFlareError::Io(err)
            }
            PluginError::Json(err) => {
                dataflare_core::error::DataFlareError::Serialization(err.to_string())
            }
            PluginError::Utf8(err) => {
                dataflare_core::error::DataFlareError::Serialization(format!("UTF-8 error: {}", err))
            }
            PluginError::Custom(msg) => {
                dataflare_core::error::DataFlareError::Plugin(format!("Plugin: {}", msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = PluginError::processing("test error");
        assert_eq!(err.to_string(), "Processing error: test error");
        assert!(err.is_recoverable());
        assert_eq!(err.category(), "processing");
    }

    #[test]
    fn test_error_conversion() {
        let plugin_err = PluginError::processing("test");
        let dataflare_err: dataflare_core::error::DataFlareError = plugin_err.into();
        assert!(dataflare_err.to_string().contains("Plugin: test"));
    }

    #[test]
    fn test_recoverable_errors() {
        assert!(PluginError::processing("test").is_recoverable());
        assert!(PluginError::invalid_input("test").is_recoverable());
        assert!(!PluginError::configuration("test").is_recoverable());
        assert!(!PluginError::initialization("test").is_recoverable());
    }
}
