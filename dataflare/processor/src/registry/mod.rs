//! 处理器注册表
//!
//! 提供注册和获取处理器的功能。

use std::sync::Arc;
use serde_json::Value;

use dataflare_core::{
    error::Result,
    processor::Processor,
};
use crate::{
    mapping::MappingProcessor,
    filter::FilterProcessor,
    aggregate::AggregateProcessor,
    enrichment::EnrichmentProcessor,
    join::JoinProcessor,
};

/// 注册默认处理器
pub fn register_default_processors() {
    // 注册映射处理器
    register_processor("mapping", Arc::new(|config: Value| -> Result<Box<dyn Processor>> {
        let processor = MappingProcessor::from_json(config)?;
        Ok(Box::new(processor))
    }));

    // 注册过滤处理器
    register_processor("filter", Arc::new(|config: Value| -> Result<Box<dyn Processor>> {
        let processor = FilterProcessor::from_json(config)?;
        Ok(Box::new(processor))
    }));

    // 注册聚合处理器
    register_processor("aggregate", Arc::new(|config: Value| -> Result<Box<dyn Processor>> {
        let processor = AggregateProcessor::from_config(&config)?;
        Ok(Box::new(processor))
    }));

    // 注册丰富处理器
    register_processor("enrichment", Arc::new(|config: Value| -> Result<Box<dyn Processor>> {
        let processor = EnrichmentProcessor::from_config(&config)?;
        Ok(Box::new(processor))
    }));

    // 注册连接处理器
    register_processor("join", Arc::new(|config: Value| -> Result<Box<dyn Processor>> {
        let name = config.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("join")
            .to_string();
        let processor = JoinProcessor::from_config(name, &config)?;
        Ok(Box::new(processor))
    }));
}

/// 处理器工厂类型
type ProcessorFactory = Arc<dyn Fn(Value) -> Result<Box<dyn Processor>> + Send + Sync>;

/// 处理器注册表
static mut PROCESSOR_REGISTRY: Option<std::collections::HashMap<String, ProcessorFactory>> = None;

/// 注册处理器
pub fn register_processor(name: &str, factory: ProcessorFactory) {
    unsafe {
        if PROCESSOR_REGISTRY.is_none() {
            PROCESSOR_REGISTRY = Some(std::collections::HashMap::new());
        }

        if let Some(registry) = &mut PROCESSOR_REGISTRY {
            registry.insert(name.to_string(), factory);
        }
    }
}

/// 创建处理器
pub fn create_processor(name: &str, config: Value) -> Result<Box<dyn Processor>> {
    unsafe {
        if PROCESSOR_REGISTRY.is_none() {
            register_default_processors();
        }

        if let Some(registry) = &PROCESSOR_REGISTRY {
            if let Some(factory) = registry.get(name) {
                return factory(config);
            }
        }
    }

    Err(dataflare_core::error::DataFlareError::Registry(format!("处理器未找到: {}", name)))
}

/// 获取所有注册的处理器名称
pub fn get_processor_names() -> Vec<String> {
    unsafe {
        if PROCESSOR_REGISTRY.is_none() {
            register_default_processors();
        }

        if let Some(registry) = &PROCESSOR_REGISTRY {
            return registry.keys().cloned().collect();
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_registry() {
        // 注册默认处理器
        register_default_processors();

        // 获取处理器名称
        let names = get_processor_names();
        assert!(names.contains(&"mapping".to_string()));
        assert!(names.contains(&"filter".to_string()));
        assert!(names.contains(&"aggregate".to_string()));
        assert!(names.contains(&"join".to_string()));

        // 创建映射处理器
        let config = serde_json::json!({
            "mappings": [
                {
                    "source": "name",
                    "destination": "user.name",
                    "transform": "uppercase"
                }
            ]
        });

        let processor = create_processor("mapping", config);
        assert!(processor.is_ok());

        // 创建过滤处理器
        let config = serde_json::json!({
            "condition": "email != null"
        });

        let processor = create_processor("filter", config);
        assert!(processor.is_ok());

        // 创建聚合处理器
        let config = serde_json::json!({
            "group_by": ["category"],
            "aggregations": [
                {
                    "source": "price",
                    "destination": "avg_price",
                    "function": "average"
                }
            ]
        });

        let processor = create_processor("aggregate", config);
        assert!(processor.is_ok());

        // 创建连接处理器
        let config = serde_json::json!({
            "name": "test_join",
            "left_key": "id",
            "right_key": "user_id",
            "join_type": "inner",
            "left_prefix": "order",
            "right_prefix": "user"
        });

        let processor = create_processor("join", config);
        assert!(processor.is_ok());
    }
}
