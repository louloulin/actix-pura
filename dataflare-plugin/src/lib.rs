//! # DataFlare Plugin
//!
//! Plugin system for the DataFlare data integration framework.
//! This crate provides WebAssembly (WASM) based plugin functionality.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod plugin;
pub mod wasm;
pub mod registry;
pub mod sandbox;

use std::path::PathBuf;
use dataflare_core::error::Result;

// Re-exports for convenience
pub use plugin::{PluginManager, PluginConfig, PluginMetadata, PluginType, ProcessorPlugin};
pub use wasm::WasmProcessor;
pub use registry::{register_plugin, get_plugin, PluginRegistry};

/// Version of the DataFlare Plugin module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the plugin system
pub fn init_plugin_system(plugin_dir: PathBuf) -> Result<()> {
    log::info!("Initializing plugin system from directory: {:?}", plugin_dir);
    // Implementation will be moved from the original code
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
