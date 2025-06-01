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

// New 3-layer architecture modules
pub mod interface;
pub mod backend;
pub mod pool;

// Core modules
pub mod core;
pub mod runtime;

// Performance optimization modules
pub mod wasm_pool;
pub mod memory_pool;
pub mod batch;
pub mod manager;

// Legacy compatibility modules
pub mod adapter;

// Legacy modules (deprecated)
#[deprecated(note = "Use core module instead")]
pub mod plugin;
#[deprecated(note = "Use core::PluginRecord instead")]
pub mod record;
#[deprecated(note = "Use core::PluginError instead")]
pub mod error;

#[cfg(test)]
pub mod test_utils;

// Re-export main types for new 3-layer architecture
pub use interface::{
    DataFlarePlugin, PluginRecord, PluginResult, PluginError, PluginType, PluginInfo, PluginState,
};
pub use backend::{
    PluginBackend, PluginBackendConfig, BackendType, NativeBackend, WasmBackend,
    SecurityPolicy, ResourceLimits, BackendStats,
};
pub use pool::{
    PluginPool, PluginPoolConfig, PluginInstanceHandle, PoolStats,
};

// Re-export core types for compatibility
pub use core::{
    API_VERSION,
};
pub use runtime::{PluginRuntime, PluginRuntimeConfig, RuntimeSummary};

// Performance optimization re-exports
pub use wasm_pool::{WasmInstancePool, WasmPoolConfig};
pub use memory_pool::{MemoryPool, MemoryPoolConfig, BufferSize};
pub use batch::{BatchPlugin, BatchConfig, BatchResult, BatchStats, BatchAdapter};
pub use manager::{PluginManager, PluginManagerConfig, PluginMetrics, FailureRecoveryConfig};

// Note: For advanced WASM features (marketplace, distributed, AI integration),
// use the separate `dataflare-wasm` crate which provides enterprise-grade WASM capabilities.

// Legacy compatibility re-exports
pub use adapter::{SmartPluginAdapter, OwnedPluginRecord};

// Legacy re-exports (deprecated)
#[deprecated(note = "Use DataFlarePlugin instead")]
pub use plugin::{SmartPlugin};
#[deprecated(note = "Use core::PluginRecord instead")]
pub use record::{OwnedPluginRecord as LegacyOwnedPluginRecord};

/// Plugin system version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
