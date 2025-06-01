//! WASM数据目标连接器

use async_trait::async_trait;
use crate::{WasmError, WasmResult, WasmPluginMetadata, components::{WasmComponent, WasmComponentType, WasmComponentConfig}};

/// WASM数据目标连接器
pub struct WasmDestinationConnector {
    config: WasmComponentConfig,
    metadata: WasmPluginMetadata,
    initialized: bool,
}

impl WasmDestinationConnector {
    pub async fn new(config: WasmComponentConfig) -> WasmResult<Self> {
        if config.component_type != WasmComponentType::Destination {
            return Err(WasmError::Component("组件类型必须是Destination".to_string()));
        }

        Ok(Self {
            config,
            metadata: WasmPluginMetadata::default(),
            initialized: false,
        })
    }
}

#[async_trait]
impl WasmComponent for WasmDestinationConnector {
    fn get_component_type(&self) -> WasmComponentType {
        WasmComponentType::Destination
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
