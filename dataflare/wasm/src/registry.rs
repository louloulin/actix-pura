//! WASM插件注册表

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use crate::{WasmPlugin, WasmPluginMetadata, WasmError, WasmResult};

/// 插件注册信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRegistration {
    /// 插件ID
    pub plugin_id: String,
    /// 插件元数据
    pub metadata: WasmPluginMetadata,
    /// 注册时间
    pub registered_at: chrono::DateTime<chrono::Utc>,
    /// 是否启用
    pub enabled: bool,
    /// 使用次数
    pub usage_count: u64,
}

/// WASM插件注册表
pub struct WasmPluginRegistry {
    /// 已注册的插件
    plugins: Arc<RwLock<HashMap<String, WasmPlugin>>>,
    /// 插件注册信息
    registrations: Arc<RwLock<HashMap<String, PluginRegistration>>>,
}

impl WasmPluginRegistry {
    /// 创建新的插件注册表
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            registrations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册插件
    pub async fn register_plugin(&self, plugin_id: String, plugin: WasmPlugin) -> WasmResult<()> {
        let metadata = plugin.get_metadata().clone();

        // 存储插件
        {
            let mut plugins = self.plugins.write()
                .map_err(|_| WasmError::Registry("获取插件锁失败".to_string()))?;
            plugins.insert(plugin_id.clone(), plugin);
        }

        // 存储注册信息
        {
            let mut registrations = self.registrations.write()
                .map_err(|_| WasmError::Registry("获取注册信息锁失败".to_string()))?;

            let registration = PluginRegistration {
                plugin_id: plugin_id.clone(),
                metadata,
                registered_at: chrono::Utc::now(),
                enabled: true,
                usage_count: 0,
            };

            registrations.insert(plugin_id.clone(), registration);
        }

        log::info!("插件注册成功: {}", plugin_id);
        Ok(())
    }

    /// 获取插件
    pub fn get_plugin(&self, plugin_id: &str) -> WasmResult<Option<WasmPlugin>> {
        let plugins = self.plugins.read()
            .map_err(|_| WasmError::Registry("获取插件锁失败".to_string()))?;

        Ok(plugins.get(plugin_id).cloned())
    }

    /// 列出所有插件
    pub fn list_plugins(&self) -> Vec<String> {
        let plugins = self.plugins.read().unwrap_or_else(|_| {
            log::warn!("获取插件锁失败，返回空列表");
            return self.plugins.read().unwrap();
        });

        plugins.keys().cloned().collect()
    }

    /// 清理注册表
    pub async fn cleanup(&self) -> WasmResult<()> {
        {
            let mut plugins = self.plugins.write()
                .map_err(|_| WasmError::Registry("获取插件锁失败".to_string()))?;
            plugins.clear();
        }

        {
            let mut registrations = self.registrations.write()
                .map_err(|_| WasmError::Registry("获取注册信息锁失败".to_string()))?;
            registrations.clear();
        }

        log::info!("插件注册表清理完成");
        Ok(())
    }
}

impl Default for WasmPluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
