//! Error types for DataFlare
//!
//! This module defines error types used across the DataFlare framework.

use std::fmt;
use std::error::Error;
use thiserror::Error;

/// Errors that can occur in DataFlare
#[derive(Debug, Error)]
pub enum DataFlareError {
    /// I/O errors
    #[error("I/O error: {0}")]
    IO(String),
    
    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// Actor errors
    #[error("Actor error: {0}")]
    Actor(String),
    
    /// Data errors
    #[error("Data error: {0}")]
    Data(String),
    
    /// Plugin errors
    #[error("Plugin error: {0}")]
    Plugin(String),
    
    /// Connection errors
    #[error("Connection error: {0}")]
    Connection(String),
    
    /// Workflow errors
    #[error("Workflow error: {0}")]
    Workflow(String),
    
    /// Processor errors
    #[error("Processor error: {0}")]
    Processor(String),
    
    /// Internal system errors
    #[error("Internal error: {0}")]
    Internal(String),
    
    /// External service errors
    #[error("External service error: {0}")]
    External(String),
    
    /// Configuration validation errors
    #[error("Configuration validation error: {0}")]
    Configuration(String),
    
    /// Progress reporting errors
    #[error("Progress reporting error: {0}")]
    ProgressReporting(String),
    
    /// Webhook errors
    #[error("Webhook error: {0}")]
    Webhook(String),
}

/// Result type for DataFlare operations
pub type Result<T> = std::result::Result<T, DataFlareError>;

// Implementation of conversion from std::io::Error
impl From<std::io::Error> for DataFlareError {
    fn from(error: std::io::Error) -> Self {
        DataFlareError::IO(error.to_string())
    }
}

// Implementation of conversion from serde_json::Error
impl From<serde_json::Error> for DataFlareError {
    fn from(error: serde_json::Error) -> Self {
        DataFlareError::Serialization(error.to_string())
    }
}

// Implementation of conversion from reqwest::Error
impl From<reqwest::Error> for DataFlareError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            DataFlareError::Webhook(format!("Webhook timeout: {}", error))
        } else if error.is_connect() {
            DataFlareError::Connection(format!("Connection error: {}", error))
        } else {
            DataFlareError::External(format!("Reqwest error: {}", error))
        }
    }
} 