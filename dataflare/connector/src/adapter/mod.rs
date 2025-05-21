//! Connector adapters for DataFlare
//!
//! This module provides adapter implementations that help existing connectors
//! adapt to the new batch-optimized interfaces.

use std::sync::Arc;
use dataflare_core::{
    error::Result,
    connector::{
        SourceConnector, DestinationConnector,
        BatchSourceConnector, BatchDestinationConnector
    }
};

// Import the source and destination adapters
mod source;
mod destination;

// Re-export the adapter types
pub use self::source::BatchSourceAdapter;
pub use self::destination::BatchDestinationAdapter;

// 为连接器类型添加静态类型标识函数的trait
pub trait ConnectorType {
    fn connector_type_static() -> &'static str;
}

// 为特定连接器类型实现静态类型标识
macro_rules! impl_connector_type {
    ($type:ty, $name:expr) => {
        impl ConnectorType for $type {
            fn connector_type_static() -> &'static str {
                $name
            }
        }
    };
}

/// Convert a legacy SourceConnector to a BatchSourceConnector
pub fn adapt_source<T: SourceConnector + 'static>(source: T) -> BatchSourceAdapter<T> {
    source::BatchSourceAdapter::new(source)
}

/// Convert a legacy DestinationConnector to a BatchDestinationConnector
pub fn adapt_destination<T: DestinationConnector + 'static>(dest: T) -> BatchDestinationAdapter<T> {
    destination::BatchDestinationAdapter::new(dest)
}

/// Register a legacy source connector as a batch connector
/// 
/// # Safety
/// This function uses unsafe code to downcast Box<dyn SourceConnector> to Box<T>,
/// which is necessary because Rust doesn't allow direct downcasting of trait objects.
/// This conversion is safe as long as the factory actually returns instances of type T.
pub fn register_adapted_source_connector<T: SourceConnector + 'static>(
    name: &str,
    factory: Arc<dyn Fn(serde_json::Value) -> Result<Box<dyn SourceConnector>> + Send + Sync>
) {
    let adapted_factory = move |config: serde_json::Value| -> Result<Box<dyn BatchSourceConnector>> {
        let source = factory(config.clone())?;
        
        // We need to downcast Box<dyn SourceConnector> to Box<T>
        // This is safe because we know the factory returns instances of type T
        let concrete_source = unsafe {
            let ptr = Box::into_raw(source);
            Box::from_raw(ptr as *mut T)
        };
        
        Ok(Box::new(source::BatchSourceAdapter::new(*concrete_source)))
    };
    
    crate::registry::register_connector::<dyn BatchSourceConnector>(
        name,
        Arc::new(adapted_factory),
    );
}

/// Register a legacy destination connector as a batch connector
/// 
/// # Safety
/// This function uses unsafe code to downcast Box<dyn DestinationConnector> to Box<T>,
/// which is necessary because Rust doesn't allow direct downcasting of trait objects.
/// This conversion is safe as long as the factory actually returns instances of type T.
pub fn register_adapted_destination_connector<T: DestinationConnector + 'static>(
    name: &str,
    factory: Arc<dyn Fn(serde_json::Value) -> Result<Box<dyn DestinationConnector>> + Send + Sync>
) {
    let adapted_factory = move |config: serde_json::Value| -> Result<Box<dyn BatchDestinationConnector>> {
        let dest = factory(config.clone())?;
        
        // We need to downcast Box<dyn DestinationConnector> to Box<T>
        // This is safe because we know the factory returns instances of type T
        let concrete_dest = unsafe {
            let ptr = Box::into_raw(dest);
            Box::from_raw(ptr as *mut T)
        };
        
        Ok(Box::new(destination::BatchDestinationAdapter::new(*concrete_dest)))
    };
    
    crate::registry::register_connector::<dyn BatchDestinationConnector>(
        name,
        Arc::new(adapted_factory),
    );
}

/// 提供一个完全安全的方式从源连接器获取批量适配器
pub fn create_source_adapter<T: SourceConnector + Clone + 'static>(
    source: T
) -> BatchSourceAdapter<T> {
    BatchSourceAdapter::new(source)
}

/// 提供一个完全安全的方式从目标连接器获取批量适配器
pub fn create_destination_adapter<T: DestinationConnector + Clone + 'static>(
    destination: T
) -> BatchDestinationAdapter<T> {
    BatchDestinationAdapter::new(destination)
}

/// 安全替代原始convert_source_connector
pub fn safe_convert_source<T, S>(source: S) -> Result<T>
where 
    T: SourceConnector + ConnectorType + Clone + 'static,
    S: AsRef<dyn SourceConnector> + 'static
{
    // 首先检查类型
    let source_ref = source.as_ref();
    if source_ref.connector_type() != T::connector_type_static() {
        return Err(DataFlareError::Connection(format!(
            "Connector type mismatch. Expected {}, got {}",
            T::connector_type_static(),
            source_ref.connector_type()
        )));
    }
    
    // 尝试进行类型转换
    if let Some(concrete) = (source_ref as &dyn std::any::Any).downcast_ref::<T>() {
        Ok(concrete.clone())
    } else {
        Err(DataFlareError::Connection(
            "Failed to convert source connector to the requested type".to_string()
        ))
    }
}

/// 安全替代原始convert_destination_connector
pub fn safe_convert_destination<T, D>(destination: D) -> Result<T>
where 
    T: DestinationConnector + ConnectorType + Clone + 'static,
    D: AsRef<dyn DestinationConnector> + 'static
{
    // 首先检查类型
    let dest_ref = destination.as_ref();
    if dest_ref.connector_type() != T::connector_type_static() {
        return Err(DataFlareError::Connection(format!(
            "Connector type mismatch. Expected {}, got {}",
            T::connector_type_static(),
            dest_ref.connector_type()
        )));
    }
    
    // 尝试进行类型转换
    if let Some(concrete) = (dest_ref as &dyn std::any::Any).downcast_ref::<T>() {
        Ok(concrete.clone())
    } else {
        Err(DataFlareError::Connection(
            "Failed to convert destination connector to the requested type".to_string()
        ))
    }
} 