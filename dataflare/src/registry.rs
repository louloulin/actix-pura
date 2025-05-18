//! 组件注册模块
//!
//! 定义用于注册和管理组件的功能。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::any::{Any, TypeId};

use crate::error::{DataFlareError, Result};

/// 组件注册表
#[derive(Default)]
pub struct ComponentRegistry {
    /// 已注册的组件
    components: HashMap<TypeId, HashMap<String, Box<dyn Any + Send + Sync>>>,
}

impl ComponentRegistry {
    /// 创建新的组件注册表
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    /// 注册组件
    pub fn register<T: 'static + Send + Sync>(&mut self, name: &str, component: T) {
        let type_id = TypeId::of::<T>();
        let components = self.components.entry(type_id).or_insert_with(HashMap::new);
        components.insert(name.to_string(), Box::new(component));
    }

    /// 获取组件
    pub fn get<T: 'static + Send + Sync>(&self, name: &str) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        self.components.get(&type_id)
            .and_then(|components| components.get(name))
            .and_then(|component| component.downcast_ref::<T>())
    }

    /// 获取可变组件
    pub fn get_mut<T: 'static + Send + Sync>(&mut self, name: &str) -> Option<&mut T> {
        let type_id = TypeId::of::<T>();
        self.components.get_mut(&type_id)
            .and_then(|components| components.get_mut(name))
            .and_then(|component| component.downcast_mut::<T>())
    }

    /// 移除组件
    pub fn remove<T: 'static + Send + Sync>(&mut self, name: &str) -> Option<Box<T>> {
        let type_id = TypeId::of::<T>();
        self.components.get_mut(&type_id)
            .and_then(|components| components.remove(name))
            .and_then(|component| component.downcast::<T>().ok())
    }

    /// 获取特定类型的所有组件名称
    pub fn get_names<T: 'static + Send + Sync>(&self) -> Vec<String> {
        let type_id = TypeId::of::<T>();
        self.components.get(&type_id)
            .map(|components| components.keys().cloned().collect())
            .unwrap_or_default()
    }
}

/// 连接器注册表
pub struct ConnectorRegistry;

impl ConnectorRegistry {
    /// 注册源连接器
    pub fn register_source_connector<S: Into<String>>(_name: S, _factory: Box<dyn Fn() -> Result<Box<dyn crate::connector::SourceConnector>> + Send + Sync>) -> Result<()> {
        // 待实现
        Ok(())
    }

    /// 注册目标连接器
    pub fn register_destination_connector<S: Into<String>>(_name: S, _factory: Box<dyn Fn() -> Result<Box<dyn crate::connector::DestinationConnector>> + Send + Sync>) -> Result<()> {
        // 待实现
        Ok(())
    }

    /// 获取源连接器
    pub fn get_source_connector<S: AsRef<str>>(name: S) -> Result<Box<dyn crate::connector::SourceConnector>> {
        // 待实现
        Err(DataFlareError::Registry(format!("未找到源连接器: {}", name.as_ref())))
    }

    /// 获取目标连接器
    pub fn get_destination_connector<S: AsRef<str>>(name: S) -> Result<Box<dyn crate::connector::DestinationConnector>> {
        // 待实现
        Err(DataFlareError::Registry(format!("未找到目标连接器: {}", name.as_ref())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_registry() {
        let mut registry = ComponentRegistry::new();

        // 注册组件
        registry.register("component1", "Hello".to_string());
        registry.register("component2", 42i32);

        // 获取组件
        let component1 = registry.get::<String>("component1");
        let component2 = registry.get::<i32>("component2");

        assert!(component1.is_some());
        assert!(component2.is_some());
        assert_eq!(component1.unwrap(), "Hello");
        assert_eq!(component2.unwrap(), &42);

        // 获取名称
        let names = registry.get_names::<String>();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"component1".to_string()));

        // 移除组件
        let removed = registry.remove::<String>("component1");
        assert!(removed.is_some());
        assert_eq!(*removed.unwrap(), "Hello");

        // 验证已移除
        let component1 = registry.get::<String>("component1");
        assert!(component1.is_none());
    }
}
