//! WASM处理器组件

use async_trait::async_trait;
use crate::{WasmError, WasmResult, WasmPluginMetadata, components::{WasmComponent, WasmComponentType, WasmComponentConfig}};

/// WASM处理器组件
pub struct WasmProcessorComponent {
    config: WasmComponentConfig,
    metadata: WasmPluginMetadata,
    initialized: bool,
}

impl WasmProcessorComponent {
    pub async fn new(config: WasmComponentConfig) -> WasmResult<Self> {
        Ok(Self {
            config,
            metadata: WasmPluginMetadata::default(),
            initialized: false,
        })
    }
}

#[async_trait]
impl WasmComponent for WasmProcessorComponent {
    fn get_component_type(&self) -> WasmComponentType {
        WasmComponentType::Processor
    }

    fn get_name(&self) -> &str {
        &self.config.name
    }

    async fn configure(&mut self, _config: &serde_json::Value) -> WasmResult<()> {
        Ok(())
    }

    async fn initialize(&mut self) -> WasmResult<()> {
        self.initialized = true;
        Ok(())
    }

    fn get_metadata(&self) -> &WasmPluginMetadata {
        &self.metadata
    }

    async fn health_check(&self) -> WasmResult<bool> {
        Ok(self.initialized)
    }

    async fn cleanup(&mut self) -> WasmResult<()> {
        self.initialized = false;
        Ok(())
    }
}
