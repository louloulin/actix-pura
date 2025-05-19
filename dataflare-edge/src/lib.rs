//! # DataFlare Edge
//!
//! Edge mode support for the DataFlare data integration framework.
//! This crate provides optimizations for resource-constrained environments.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod runtime;
pub mod cache;
pub mod sync;
pub mod resource;

// Re-exports for convenience
pub use runtime::EdgeRuntime;
pub use cache::OfflineCache;
pub use sync::SyncManager;
pub use resource::ResourceMonitor;

/// Version of the DataFlare Edge module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Edge runtime configuration
#[derive(Debug, Clone)]
pub struct EdgeRuntimeConfig {
    /// Maximum memory usage in MB
    pub max_memory_mb: usize,
    /// Maximum CPU usage (0.0 - 1.0)
    pub max_cpu_usage: f32,
    /// Whether to enable offline mode
    pub offline_mode: bool,
    /// Sync interval in seconds
    pub sync_interval: u64,
}

impl Default for EdgeRuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 512,
            max_cpu_usage: 0.7,
            offline_mode: false,
            sync_interval: 300,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
