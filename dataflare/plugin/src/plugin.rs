//! Core plugin traits and types
//!
//! Defines the SmartPlugin trait and related types that form the foundation
//! of the DataFlare plugin system.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::{PluginRecord, PluginError, Result};

/// Plugin processing result
#[derive(Debug, Clone)]
pub enum PluginResult {
    /// Filter result - whether to keep the record
    Filtered(bool),

    /// Map result - transformed data
    Mapped(Vec<u8>),

    /// Aggregate result - accumulated data
    Aggregated(Vec<u8>),

    /// Multiple outputs (for split operations)
    Multiple(Vec<Vec<u8>>),

    /// No output (record consumed)
    None,
}

impl PluginResult {
    /// Check if the result indicates the record should be kept
    pub fn should_keep(&self) -> bool {
        match self {
            Self::Filtered(keep) => *keep,
            Self::Mapped(_) | Self::Aggregated(_) | Self::Multiple(_) => true,
            Self::None => false,
        }
    }

    /// Get the output data if available
    pub fn get_data(&self) -> Option<&[u8]> {
        match self {
            Self::Mapped(data) | Self::Aggregated(data) => Some(data),
            _ => None,
        }
    }

    /// Get multiple outputs if available
    pub fn get_multiple(&self) -> Option<&[Vec<u8>]> {
        match self {
            Self::Multiple(outputs) => Some(outputs),
            _ => None,
        }
    }

    /// Convert to a single output, combining multiple if necessary
    pub fn to_single_output(&self) -> Option<Vec<u8>> {
        match self {
            Self::Mapped(data) | Self::Aggregated(data) => Some(data.clone()),
            Self::Multiple(outputs) => {
                if outputs.len() == 1 {
                    Some(outputs[0].clone())
                } else {
                    // Combine multiple outputs as JSON array
                    let combined: Vec<serde_json::Value> = outputs
                        .iter()
                        .filter_map(|data| {
                            std::str::from_utf8(data)
                                .ok()
                                .and_then(|s| serde_json::from_str(s).ok())
                        })
                        .collect();
                    serde_json::to_vec(&combined).ok()
                }
            }
            _ => None,
        }
    }
}

/// Plugin information and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin name
    pub name: String,

    /// Plugin version
    pub version: String,

    /// Plugin description
    pub description: String,

    /// Plugin author
    pub author: String,

    /// Plugin type
    pub plugin_type: PluginType,

    /// Supported API version
    pub api_version: String,

    /// Plugin tags for categorization
    pub tags: Vec<String>,

    /// Plugin configuration schema (optional)
    pub config_schema: Option<serde_json::Value>,
}

impl PluginInfo {
    /// Create new plugin info
    pub fn new(name: String, version: String, plugin_type: PluginType) -> Self {
        Self {
            name,
            version,
            description: String::new(),
            author: String::new(),
            plugin_type,
            api_version: crate::API_VERSION.to_string(),
            tags: Vec::new(),
            config_schema: None,
        }
    }

    /// Check if this plugin is compatible with the given API version
    pub fn is_compatible(&self, api_version: &str) -> bool {
        // Simple version compatibility check
        // In a real implementation, this would use semantic versioning
        self.api_version == api_version
    }
}

/// Plugin type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginType {
    /// Filter plugin - filters records based on criteria
    Filter,

    /// Map plugin - transforms records
    Map,

    /// Aggregate plugin - accumulates data across records
    Aggregate,

    /// Split plugin - splits one record into multiple
    Split,

    /// Custom plugin type
    Custom(String),
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Filter => write!(f, "filter"),
            Self::Map => write!(f, "map"),
            Self::Aggregate => write!(f, "aggregate"),
            Self::Split => write!(f, "split"),
            Self::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

impl std::str::FromStr for PluginType {
    type Err = PluginError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "filter" => Ok(Self::Filter),
            "map" => Ok(Self::Map),
            "aggregate" => Ok(Self::Aggregate),
            "split" => Ok(Self::Split),
            custom if custom.starts_with("custom:") => {
                Ok(Self::Custom(custom[7..].to_string()))
            }
            _ => Err(PluginError::Configuration(format!("Unknown plugin type: {}", s))),
        }
    }
}

/// Core plugin trait
///
/// All plugins must implement this trait to be usable in the DataFlare system.
/// The trait uses synchronous methods for maximum performance.
pub trait SmartPlugin: Send + Sync {
    /// Process a single record
    ///
    /// This is the core method that plugins must implement.
    /// It takes a zero-copy reference to the input record and returns
    /// a processing result.
    fn process(&self, record: &PluginRecord) -> Result<PluginResult>;

    /// Get plugin name
    fn name(&self) -> &str;

    /// Get plugin version
    fn version(&self) -> &str;

    /// Get plugin information
    fn info(&self) -> PluginInfo {
        PluginInfo::new(
            self.name().to_string(),
            self.version().to_string(),
            self.plugin_type(),
        )
    }

    /// Get plugin type
    fn plugin_type(&self) -> PluginType;

    /// Initialize plugin with configuration
    ///
    /// Called once when the plugin is loaded. Default implementation does nothing.
    fn initialize(&mut self, _config: &HashMap<String, serde_json::Value>) -> Result<()> {
        Ok(())
    }

    /// Finalize plugin
    ///
    /// Called when the plugin is being unloaded. Default implementation does nothing.
    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Check if plugin supports batch processing
    ///
    /// Default implementation returns false. Plugins that can optimize
    /// batch processing should override this.
    fn supports_batch(&self) -> bool {
        false
    }

    /// Process a batch of records (optional optimization)
    ///
    /// Default implementation processes records one by one.
    /// Plugins can override this for batch optimization.
    fn process_batch(&self, records: &[PluginRecord]) -> Result<Vec<PluginResult>> {
        records.iter().map(|record| self.process(record)).collect()
    }

    /// Get plugin configuration schema
    ///
    /// Returns a JSON schema describing the plugin's configuration format.
    /// Default implementation returns None.
    fn config_schema(&self) -> Option<serde_json::Value> {
        None
    }

    /// Validate plugin configuration
    ///
    /// Called to validate configuration before initialization.
    /// Default implementation always returns Ok.
    fn validate_config(&self, _config: &HashMap<String, serde_json::Value>) -> Result<()> {
        Ok(())
    }
}

/// Plugin factory trait for creating plugin instances
pub trait PluginFactory: Send + Sync {
    /// Create a new plugin instance
    fn create(&self, config: &HashMap<String, serde_json::Value>) -> Result<Box<dyn SmartPlugin>>;

    /// Get plugin information
    fn info(&self) -> PluginInfo;

    /// Validate configuration without creating an instance
    fn validate_config(&self, config: &HashMap<String, serde_json::Value>) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Test plugin implementation
    struct TestFilterPlugin;

    impl SmartPlugin for TestFilterPlugin {
        fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
            let data = record.value_as_str()?;
            Ok(PluginResult::Filtered(data.contains("test")))
        }

        fn name(&self) -> &str {
            "test_filter"
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn plugin_type(&self) -> PluginType {
            PluginType::Filter
        }
    }

    #[test]
    fn test_plugin_result() {
        let result = PluginResult::Filtered(true);
        assert!(result.should_keep());

        let result = PluginResult::Mapped(b"test".to_vec());
        assert!(result.should_keep());
        assert_eq!(result.get_data(), Some(b"test".as_slice()));
    }

    #[test]
    fn test_plugin_type_parsing() {
        assert_eq!("filter".parse::<PluginType>().unwrap(), PluginType::Filter);
        assert_eq!("map".parse::<PluginType>().unwrap(), PluginType::Map);
        assert!("invalid".parse::<PluginType>().is_err());
    }

    #[test]
    fn test_plugin_info() {
        let info = PluginInfo::new("test".to_string(), "1.0.0".to_string(), PluginType::Filter);
        assert!(info.is_compatible(crate::API_VERSION));
        assert!(!info.is_compatible("2.0.0"));
    }

    #[test]
    fn test_smart_plugin() {
        let plugin = TestFilterPlugin;
        let metadata = HashMap::new();

        let record = PluginRecord::new(b"test data", &metadata, 0);
        let result = plugin.process(&record).unwrap();

        match result {
            PluginResult::Filtered(keep) => assert!(keep),
            _ => panic!("Expected filtered result"),
        }

        assert_eq!(plugin.name(), "test_filter");
        assert_eq!(plugin.version(), "1.0.0");
        assert_eq!(plugin.plugin_type(), PluginType::Filter);
    }
}
