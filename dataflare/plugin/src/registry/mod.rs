//! Plugin registry module
//!
//! This module provides functionality for registering and retrieving plugins.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use dataflare_core::error::Result;
use dataflare_core::message::DataRecord;
use lazy_static::lazy_static;

use crate::plugin::{PluginMetadata, PluginType, ProcessorPlugin};

lazy_static! {
    static ref PLUGIN_REGISTRY: Arc<RwLock<PluginRegistry>> = Arc::new(RwLock::new(PluginRegistry::new()));
}

/// Plugin registry for managing plugins
#[derive(Debug, Default)]
pub struct PluginRegistry {
    /// Map of plugin ID to plugin metadata
    plugins: HashMap<String, PluginMetadata>,
    /// Map of plugin ID to processor plugin
    processor_plugins: HashMap<String, Box<dyn ProcessorPlugin + Send + Sync>>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            processor_plugins: HashMap::new(),
        }
    }

    /// Register a plugin
    pub fn register_plugin(&mut self, metadata: PluginMetadata, plugin: Box<dyn ProcessorPlugin + Send + Sync>) -> Result<()> {
        let plugin_id = format!("{}-{}", metadata.name.clone(), metadata.version.clone());
        self.plugins.insert(plugin_id.clone(), metadata);
        self.processor_plugins.insert(plugin_id, plugin);
        Ok(())
    }

    /// Get a plugin by ID
    pub fn get_plugin(&self, plugin_id: &str) -> Option<&PluginMetadata> {
        self.plugins.get(plugin_id)
    }

    /// Get a processor plugin by ID
    pub fn get_processor_plugin(&self, plugin_id: &str) -> Option<&Box<dyn ProcessorPlugin + Send + Sync>> {
        self.processor_plugins.get(plugin_id)
    }

    /// Get a mutable processor plugin by ID
    pub fn get_processor_plugin_mut(&mut self, plugin_id: &str) -> Option<&mut Box<dyn ProcessorPlugin + Send + Sync>> {
        self.processor_plugins.get_mut(plugin_id)
    }

    /// List all plugins
    pub fn list_plugins(&self) -> Vec<&PluginMetadata> {
        self.plugins.values().collect()
    }

    /// List plugins by type
    pub fn list_plugins_by_type(&self, plugin_type: PluginType) -> Vec<&PluginMetadata> {
        self.plugins
            .values()
            .filter(|metadata| metadata.plugin_type == plugin_type)
            .collect()
    }
}

/// Register a plugin
pub fn register_plugin(metadata: PluginMetadata, plugin: Box<dyn ProcessorPlugin + Send + Sync>) -> Result<()> {
    let mut registry = PLUGIN_REGISTRY.write().unwrap();
    registry.register_plugin(metadata, plugin)
}

/// Get a plugin by ID
pub fn get_plugin(plugin_id: &str) -> Option<PluginMetadata> {
    let registry = PLUGIN_REGISTRY.read().unwrap();
    registry.get_plugin(plugin_id).cloned()
}

/// 简单的处理器插件实现，用于测试
#[derive(Debug)]
struct DummyProcessor {
    metadata: PluginMetadata,
}

impl DummyProcessor {
    fn new(metadata: PluginMetadata) -> Self {
        Self { metadata }
    }
}

impl ProcessorPlugin for DummyProcessor {
    fn configure(&mut self, _config: serde_json::Value) -> Result<()> {
        Ok(())
    }

    fn process(&self, record: DataRecord) -> Result<DataRecord> {
        Ok(record)
    }

    fn get_metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
}

/// Get a processor plugin by ID
pub fn get_processor_plugin(plugin_id: &str) -> Option<Box<dyn ProcessorPlugin + Send + Sync>> {
    let registry = PLUGIN_REGISTRY.read().unwrap();
    // 这里我们只能返回一个克隆的插件，因为我们不能直接返回引用
    // 在实际实现中，这需要根据具体的插件类型来处理
    registry.get_plugin(plugin_id).map(|metadata| {
        // 创建一个新的插件实例
        // 注意：这是一个简化的实现，实际上应该从插件管理器获取
        let dummy_plugin = Box::new(DummyProcessor::new(metadata.clone())) as Box<dyn ProcessorPlugin + Send + Sync>;
        dummy_plugin
    })
}

/// List all plugins
pub fn list_plugins() -> Vec<PluginMetadata> {
    let registry = PLUGIN_REGISTRY.read().unwrap();
    registry.list_plugins().iter().map(|&metadata| metadata.clone()).collect()
}

/// List plugins by type
pub fn list_plugins_by_type(plugin_type: PluginType) -> Vec<PluginMetadata> {
    let registry = PLUGIN_REGISTRY.read().unwrap();
    registry.list_plugins_by_type(plugin_type).iter().map(|&metadata| metadata.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginConfig, PluginType};
    use dataflare_core::message::DataRecord;
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct MockProcessorPlugin {
        config: Mutex<PluginConfig>,
    }

    impl MockProcessorPlugin {
        fn new() -> Self {
            Self {
                config: Mutex::new(PluginConfig::default()),
            }
        }
    }

    impl ProcessorPlugin for MockProcessorPlugin {
        fn configure(&mut self, config: serde_json::Value) -> Result<()> {
            let mut guard = self.config.lock().unwrap();
            *guard = serde_json::from_value(config).unwrap_or_default();
            Ok(())
        }

        fn process(&self, record: DataRecord) -> Result<DataRecord> {
            Ok(record)
        }

        fn get_metadata(&self) -> &PluginMetadata {
            // 这只是一个测试实现，返回一个静态元数据
            static METADATA: std::sync::OnceLock<PluginMetadata> = std::sync::OnceLock::new();
            METADATA.get_or_init(|| PluginMetadata {
                name: "MockProcessor".to_string(),
                version: "1.0.0".to_string(),
                description: "Mock processor for testing".to_string(),
                author: "DataFlare".to_string(),
                plugin_type: PluginType::Processor,
                input_schema: None,
                output_schema: None,
                config_schema: None,
            })
        }
    }

    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new();

        let metadata = PluginMetadata {
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test Plugin Description".to_string(),
            author: "Test Author".to_string(),
            plugin_type: PluginType::Processor,
            input_schema: None,
            output_schema: None,
            config_schema: None,
        };

        let plugin = Box::new(MockProcessorPlugin::new()) as Box<dyn ProcessorPlugin + Send + Sync>;

        registry.register_plugin(metadata.clone(), plugin).unwrap();

        let plugin_id = "Test Plugin-1.0.0";
        let retrieved_metadata = registry.get_plugin(plugin_id).unwrap();
        assert_eq!(retrieved_metadata.name, metadata.name);

        let plugins = registry.list_plugins();
        assert_eq!(plugins.len(), 1);

        let processor_plugins = registry.list_plugins_by_type(PluginType::Processor);
        assert_eq!(processor_plugins.len(), 1);
    }
}
