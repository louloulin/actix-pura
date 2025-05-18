//! 处理器注册表
//!
//! 提供注册和获取处理器的功能。

use std::sync::Arc;
use serde_json::Value;

use crate::{
    error::Result,
    processor::{Processor, MappingProcessor, FilterProcessor, AggregateProcessor},
};

/// 注册默认处理器
pub fn register_default_processors() {
    // 注册映射处理器
    register_processor("mapping", Arc::new(|config: Value| -> Result<Box<dyn Processor>> {
        let mut processor = MappingProcessor::new();
        processor.configure(&config)?;
        Ok(Box::new(processor))
    }));

    // 注册过滤处理器
    register_processor("filter", Arc::new(|config: Value| -> Result<Box<dyn Processor>> {
        let mut processor = FilterProcessor::new();
        processor.configure(&config)?;
        Ok(Box::new(processor))
    }));

    // 注册聚合处理器
    register_processor("aggregate", Arc::new(|config: Value| -> Result<Box<dyn Processor>> {
        let processor = AggregateProcessor::from_config(&config)?;
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

    Err(crate::error::DataFlareError::Registry(format!("处理器未找到: {}", name)))
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
    }
}
