//! WASM组件系统
//!
//! 支持DataFlare完整DSL的WASM插件组件，包括：
//! - Source连接器
//! - Destination连接器
//! - Processor处理器
//! - Transformer转换器
//! - Filter过滤器
//! - Aggregator聚合器

pub mod source;
pub mod destination;
pub mod processor;
pub mod transformer;
pub mod filter;
pub mod aggregator;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use log::{info, debug, warn, error};

use crate::{WasmError, WasmResult, WasmPluginMetadata};

/// WASM组件类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WasmComponentType {
    /// 数据源连接器
    Source,
    /// 数据目标连接器
    Destination,
    /// 数据处理器
    Processor,
    /// 数据转换器
    Transformer,
    /// 数据过滤器
    Filter,
    /// 数据聚合器
    Aggregator,
    /// 自定义组件
    Custom(String),
}

impl WasmComponentType {
    /// 从字符串解析组件类型
    pub fn from_str(s: &str) -> WasmResult<Self> {
        match s.to_lowercase().as_str() {
            "source" => Ok(WasmComponentType::Source),
            "destination" => Ok(WasmComponentType::Destination),
            "processor" => Ok(WasmComponentType::Processor),
            "transformer" => Ok(WasmComponentType::Transformer),
            "filter" => Ok(WasmComponentType::Filter),
            "aggregator" => Ok(WasmComponentType::Aggregator),
            custom => Ok(WasmComponentType::Custom(custom.to_string())),
        }
    }

    /// 转换为字符串
    pub fn to_string(&self) -> String {
        match self {
            WasmComponentType::Source => "source".to_string(),
            WasmComponentType::Destination => "destination".to_string(),
            WasmComponentType::Processor => "processor".to_string(),
            WasmComponentType::Transformer => "transformer".to_string(),
            WasmComponentType::Filter => "filter".to_string(),
            WasmComponentType::Aggregator => "aggregator".to_string(),
            WasmComponentType::Custom(name) => name.clone(),
        }
    }
}

/// WASM组件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmComponentConfig {
    /// 组件类型
    pub component_type: WasmComponentType,
    /// 组件名称
    pub name: String,
    /// WASM模块路径
    pub module_path: String,
    /// 组件配置参数
    pub config: Option<HashMap<String, Value>>,
    /// 运行时配置
    pub runtime_config: Option<HashMap<String, Value>>,
    /// 组件元数据
    pub metadata: Option<HashMap<String, String>>,
}

/// WASM组件接口
#[async_trait]
pub trait WasmComponent: Send + Sync {
    /// 获取组件类型
    fn get_component_type(&self) -> WasmComponentType;

    /// 获取组件名称
    fn get_name(&self) -> &str;

    /// 配置组件
    async fn configure(&mut self, config: &Value) -> WasmResult<()>;

    /// 初始化组件
    async fn initialize(&mut self) -> WasmResult<()>;

    /// 获取组件元数据
    fn get_metadata(&self) -> &WasmPluginMetadata;

    /// 检查组件健康状态
    async fn health_check(&self) -> WasmResult<bool>;

    /// 清理组件资源
    async fn cleanup(&mut self) -> WasmResult<()>;
}

/// WASM组件工厂
pub struct WasmComponentFactory {
    /// 已注册的组件类型
    registered_types: HashMap<String, WasmComponentType>,
}

impl WasmComponentFactory {
    /// 创建新的组件工厂
    pub fn new() -> Self {
        let mut factory = Self {
            registered_types: HashMap::new(),
        };

        // 注册默认组件类型
        factory.register_default_types();
        factory
    }

    /// 注册默认组件类型
    fn register_default_types(&mut self) {
        self.registered_types.insert("source".to_string(), WasmComponentType::Source);
        self.registered_types.insert("destination".to_string(), WasmComponentType::Destination);
        self.registered_types.insert("processor".to_string(), WasmComponentType::Processor);
        self.registered_types.insert("transformer".to_string(), WasmComponentType::Transformer);
        self.registered_types.insert("filter".to_string(), WasmComponentType::Filter);
        self.registered_types.insert("aggregator".to_string(), WasmComponentType::Aggregator);
    }

    /// 注册自定义组件类型
    pub fn register_component_type(&mut self, name: String, component_type: WasmComponentType) {
        self.registered_types.insert(name.clone(), component_type);
        info!("注册WASM组件类型: {}", name);
    }

    /// 创建WASM组件
    pub async fn create_component(&self, config: &WasmComponentConfig) -> WasmResult<Box<dyn WasmComponent>> {
        info!("创建WASM组件: {} (类型: {:?})", config.name, config.component_type);

        match config.component_type {
            WasmComponentType::Source => {
                let source = source::WasmSourceConnector::new(config.clone()).await?;
                Ok(Box::new(source))
            },
            WasmComponentType::Destination => {
                let destination = destination::WasmDestinationConnector::new(config.clone()).await?;
                Ok(Box::new(destination))
            },
            WasmComponentType::Processor => {
                let processor = processor::WasmProcessorComponent::new(config.clone()).await?;
                Ok(Box::new(processor))
            },
            WasmComponentType::Transformer => {
                let transformer = transformer::WasmTransformerComponent::new(config.clone()).await?;
                Ok(Box::new(transformer))
            },
            WasmComponentType::Filter => {
                let filter = filter::WasmFilterComponent::new(config.clone()).await?;
                Ok(Box::new(filter))
            },
            WasmComponentType::Aggregator => {
                let aggregator = aggregator::WasmAggregatorComponent::new(config.clone()).await?;
                Ok(Box::new(aggregator))
            },
            WasmComponentType::Custom(ref name) => {
                // 对于自定义组件，使用通用处理器
                let mut config_clone = config.clone();
                config_clone.component_type = WasmComponentType::Processor;
                let processor = processor::WasmProcessorComponent::new(config_clone).await?;
                Ok(Box::new(processor))
            },
        }
    }

    /// 从配置文件创建组件
    pub async fn create_from_config(&self, config_value: &Value) -> WasmResult<Box<dyn WasmComponent>> {
        let config: WasmComponentConfig = serde_json::from_value(config_value.clone())
            .map_err(|e| WasmError::Configuration(format!("无效的WASM组件配置: {}", e)))?;

        self.create_component(&config).await
    }

    /// 列出已注册的组件类型
    pub fn list_component_types(&self) -> Vec<String> {
        self.registered_types.keys().cloned().collect()
    }

    /// 验证组件配置
    pub fn validate_config(&self, config: &WasmComponentConfig) -> WasmResult<()> {
        // 验证模块路径
        if config.module_path.is_empty() {
            return Err(WasmError::Configuration("WASM模块路径不能为空".to_string()));
        }

        // 验证文件存在
        if !std::path::Path::new(&config.module_path).exists() {
            return Err(WasmError::Configuration(format!("WASM模块文件不存在: {}", config.module_path)));
        }

        // 验证组件名称
        if config.name.is_empty() {
            return Err(WasmError::Configuration("组件名称不能为空".to_string()));
        }

        info!("WASM组件配置验证通过: {}", config.name);
        Ok(())
    }
}

impl Default for WasmComponentFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// WASM组件管理器
pub struct WasmComponentManager {
    /// 组件工厂
    factory: WasmComponentFactory,
    /// 已加载的组件
    components: HashMap<String, Box<dyn WasmComponent>>,
}

impl WasmComponentManager {
    /// 创建新的组件管理器
    pub fn new() -> Self {
        Self {
            factory: WasmComponentFactory::new(),
            components: HashMap::new(),
        }
    }

    /// 加载组件
    pub async fn load_component(&mut self, name: String, config: &WasmComponentConfig) -> WasmResult<()> {
        // 验证配置
        self.factory.validate_config(config)?;

        // 创建组件
        let mut component = self.factory.create_component(config).await?;

        // 初始化组件
        component.initialize().await?;

        // 存储组件
        self.components.insert(name.clone(), component);

        info!("WASM组件加载成功: {}", name);
        Ok(())
    }

    /// 卸载组件
    pub async fn unload_component(&mut self, name: &str) -> WasmResult<()> {
        if let Some(mut component) = self.components.remove(name) {
            component.cleanup().await?;
            info!("WASM组件卸载成功: {}", name);
        } else {
            warn!("WASM组件不存在: {}", name);
        }
        Ok(())
    }

    /// 获取组件
    pub fn get_component(&self, name: &str) -> Option<&dyn WasmComponent> {
        self.components.get(name).map(|c| c.as_ref())
    }

    /// 获取组件（可变引用）
    pub fn get_component_mut(&mut self, name: &str) -> Option<&mut dyn WasmComponent> {
        if let Some(component) = self.components.get_mut(name) {
            Some(component.as_mut())
        } else {
            None
        }
    }

    /// 列出所有组件
    pub fn list_components(&self) -> Vec<String> {
        self.components.keys().cloned().collect()
    }

    /// 获取组件统计信息
    pub fn get_stats(&self) -> WasmComponentStats {
        let mut stats = WasmComponentStats {
            total_components: self.components.len(),
            source_components: 0,
            destination_components: 0,
            processor_components: 0,
            transformer_components: 0,
            filter_components: 0,
            aggregator_components: 0,
            custom_components: 0,
        };

        for component in self.components.values() {
            match component.get_component_type() {
                WasmComponentType::Source => stats.source_components += 1,
                WasmComponentType::Destination => stats.destination_components += 1,
                WasmComponentType::Processor => stats.processor_components += 1,
                WasmComponentType::Transformer => stats.transformer_components += 1,
                WasmComponentType::Filter => stats.filter_components += 1,
                WasmComponentType::Aggregator => stats.aggregator_components += 1,
                WasmComponentType::Custom(_) => stats.custom_components += 1,
            }
        }

        stats
    }

    /// 清理所有组件
    pub async fn cleanup_all(&mut self) -> WasmResult<()> {
        info!("清理所有WASM组件");

        for (name, component) in &mut self.components {
            if let Err(e) = component.cleanup().await {
                error!("清理WASM组件 {} 失败: {}", name, e);
            }
        }

        self.components.clear();
        info!("所有WASM组件清理完成");
        Ok(())
    }
}

/// WASM组件统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmComponentStats {
    /// 总组件数
    pub total_components: usize,
    /// 数据源组件数
    pub source_components: usize,
    /// 数据目标组件数
    pub destination_components: usize,
    /// 处理器组件数
    pub processor_components: usize,
    /// 转换器组件数
    pub transformer_components: usize,
    /// 过滤器组件数
    pub filter_components: usize,
    /// 聚合器组件数
    pub aggregator_components: usize,
    /// 自定义组件数
    pub custom_components: usize,
}

impl Default for WasmComponentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_component_type_parsing() {
        assert_eq!(WasmComponentType::from_str("source").unwrap(), WasmComponentType::Source);
        assert_eq!(WasmComponentType::from_str("destination").unwrap(), WasmComponentType::Destination);
        assert_eq!(WasmComponentType::from_str("processor").unwrap(), WasmComponentType::Processor);

        match WasmComponentType::from_str("custom_type").unwrap() {
            WasmComponentType::Custom(name) => assert_eq!(name, "custom_type"),
            _ => panic!("Expected custom type"),
        }
    }

    #[test]
    fn test_component_factory_creation() {
        let factory = WasmComponentFactory::new();
        let types = factory.list_component_types();

        assert!(types.contains(&"source".to_string()));
        assert!(types.contains(&"destination".to_string()));
        assert!(types.contains(&"processor".to_string()));
    }

    #[tokio::test]
    async fn test_component_manager() {
        let manager = WasmComponentManager::new();
        assert_eq!(manager.list_components().len(), 0);

        let stats = manager.get_stats();
        assert_eq!(stats.total_components, 0);
    }
}
