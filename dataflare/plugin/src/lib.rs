//! DataFlare Plugin System
//!
//! A high-performance, zero-copy plugin system for DataFlare data processing.
//! This system provides a unified interface for extending DataFlare's capabilities
//! through custom plugins while maintaining excellent performance characteristics.
//!
//! ## New 3-Layer Architecture
//!
//! The plugin system has been redesigned with a simplified 3-layer architecture:
//! - **Plugin Interface**: Unified `DataFlarePlugin` trait
//! - **Plugin Runtime**: Centralized runtime management
//! - **Native/WASM Backend**: Support for both native and WASM plugins

#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod core;
pub mod runtime;
pub mod adapter;
pub mod wasm_pool;
pub mod memory_pool;

// Legacy modules (deprecated)
#[deprecated(note = "Use core module instead")]
pub mod plugin;
#[deprecated(note = "Use core::PluginRecord instead")]
pub mod record;
#[deprecated(note = "Use core::PluginError instead")]
pub mod error;

#[cfg(test)]
pub mod test_utils;

// Re-export main types for convenience
pub use core::{
    DataFlarePlugin, PluginType, PluginInfo, PluginRecord, PluginResult, PluginError, Result,
    BatchPlugin, BatchStats, API_VERSION,
};
pub use runtime::{PluginRuntime, PluginRuntimeConfig, PluginMetrics, RuntimeSummary};
pub use adapter::{SmartPluginAdapter, OwnedPluginRecord};
pub use wasm_pool::{WasmInstancePool, WasmPoolConfig};
pub use memory_pool::{MemoryPool, MemoryPoolConfig, BufferSize};

// Legacy re-exports (deprecated)
#[deprecated(note = "Use DataFlarePlugin instead")]
pub use plugin::{SmartPlugin};
#[deprecated(note = "Use core::PluginRecord instead")]
pub use record::{OwnedPluginRecord as LegacyOwnedPluginRecord};

/// Plugin system version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
