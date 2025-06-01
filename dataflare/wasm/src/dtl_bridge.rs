//! DTL-WASM桥接模块
//!
//! 实现DTL (DataFlare Transform Language) 与WASM插件的深度集成

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, debug, warn, error};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::DataRecord,
};

use crate::{
    WasmPlugin, WasmRuntime,
    interface::{WasmFunctionCall, WasmPluginInterface},
    error::{WasmError, WasmResult},
};

/// DTL-WASM桥接器
///
/// 提供DTL脚本中调用WASM函数的能力
pub struct DTLWasmBridge {
    /// WASM运行时
    runtime: Arc<RwLock<WasmRuntime>>,
    /// 已注册的WASM函数
    registered_functions: HashMap<String, WasmFunctionInfo>,
    /// 函数调用统计
    call_stats: HashMap<String, DTLWasmCallStats>,
}

/// WASM函数信息
#[derive(Debug, Clone)]
pub struct WasmFunctionInfo {
    /// 插件ID
    pub plugin_id: String,
    /// 函数名称
    pub function_name: String,
    /// 函数描述
    pub description: Option<String>,
    /// 参数类型
    pub parameter_types: Vec<String>,
    /// 返回类型
    pub return_type: String,
    /// 是否异步
    pub is_async: bool,
}

/// DTL-WASM调用统计
#[derive(Debug, Clone, Default)]
pub struct DTLWasmCallStats {
    /// 调用次数
    pub call_count: u64,
    /// 总执行时间 (毫秒)
    pub total_execution_time_ms: u64,
    /// 成功次数
    pub success_count: u64,
    /// 错误次数
    pub error_count: u64,
    /// 平均执行时间 (毫秒)
    pub avg_execution_time_ms: f64,
}

impl DTLWasmBridge {
    /// 创建新的DTL-WASM桥接器
    pub fn new(runtime: Arc<RwLock<WasmRuntime>>) -> Self {
        Self {
            runtime,
            registered_functions: HashMap::new(),
            call_stats: HashMap::new(),
        }
    }

    /// 注册WASM函数到DTL环境
    pub async fn register_wasm_function(
        &mut self,
        function_alias: String,
        plugin_id: String,
        function_name: String,
        description: Option<String>,
    ) -> WasmResult<()> {
        // 验证插件是否存在
        {
            let runtime = self.runtime.read().await;
            if runtime.get_plugin(&plugin_id).is_none() {
                return Err(WasmError::plugin_load(format!("插件不存在: {}", plugin_id)));
            }
        }

        let function_info = WasmFunctionInfo {
            plugin_id: plugin_id.clone(),
            function_name: function_name.clone(),
            description,
            parameter_types: vec!["any".to_string()], // 暂时支持任意类型
            return_type: "any".to_string(),
            is_async: true,
        };

        self.registered_functions.insert(function_alias.clone(), function_info);
        self.call_stats.insert(function_alias.clone(), DTLWasmCallStats::default());

        info!("WASM函数注册成功: {} -> {}::{}", function_alias, plugin_id, function_name);
        Ok(())
    }

    /// 从DTL调用WASM函数
    pub async fn call_wasm_function(
        &mut self,
        function_alias: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        let start_time = std::time::Instant::now();

        // 获取函数信息
        let function_info = self.registered_functions.get(function_alias)
            .ok_or_else(|| DataFlareError::Plugin(format!("WASM函数未注册: {}", function_alias)))?
            .clone();

        // 更新调用统计
        if let Some(stats) = self.call_stats.get_mut(function_alias) {
            stats.call_count += 1;
        }

        // 调用WASM函数
        let result = self.execute_wasm_function(&function_info, args).await;

        // 更新执行时间统计
        let execution_time = start_time.elapsed().as_millis() as u64;
        if let Some(stats) = self.call_stats.get_mut(function_alias) {
            stats.total_execution_time_ms += execution_time;
            match &result {
                Ok(_) => stats.success_count += 1,
                Err(_) => stats.error_count += 1,
            }
            stats.avg_execution_time_ms = stats.total_execution_time_ms as f64 / stats.call_count as f64;
        }

        result
    }

    /// 执行WASM函数
    async fn execute_wasm_function(
        &self,
        function_info: &WasmFunctionInfo,
        args: Vec<Value>,
    ) -> Result<Value> {
        let runtime = self.runtime.read().await;
        let plugin = runtime.get_plugin(&function_info.plugin_id)
            .ok_or_else(|| DataFlareError::Plugin(format!("插件不存在: {}", function_info.plugin_id)))?;

        // 创建函数调用
        let mut call = WasmFunctionCall::new(function_info.function_name.clone());

        // 添加参数
        for (i, arg) in args.into_iter().enumerate() {
            call = call.with_parameter(format!("arg_{}", i), arg)
                .map_err(|e| DataFlareError::Plugin(format!("添加参数失败: {}", e)))?;
        }

        // 调用函数
        let result = plugin.call_function(call).await
            .map_err(|e| DataFlareError::Plugin(format!("WASM函数调用失败: {}", e)))?;

        // 处理结果
        if result.success {
            Ok(result.result.unwrap_or(Value::Null))
        } else {
            Err(DataFlareError::Plugin(format!("WASM函数执行失败: {}",
                result.error.unwrap_or_else(|| "未知错误".to_string()))))
        }
    }

    /// 获取已注册的函数列表
    pub fn get_registered_functions(&self) -> Vec<String> {
        self.registered_functions.keys().cloned().collect()
    }

    /// 获取函数信息
    pub fn get_function_info(&self, function_alias: &str) -> Option<&WasmFunctionInfo> {
        self.registered_functions.get(function_alias)
    }

    /// 获取调用统计
    pub fn get_call_stats(&self, function_alias: &str) -> Option<&DTLWasmCallStats> {
        self.call_stats.get(function_alias)
    }

    /// 获取所有调用统计
    pub fn get_all_call_stats(&self) -> &HashMap<String, DTLWasmCallStats> {
        &self.call_stats
    }

    /// 清除函数注册
    pub fn unregister_function(&mut self, function_alias: &str) -> bool {
        let removed = self.registered_functions.remove(function_alias).is_some();
        self.call_stats.remove(function_alias);
        if removed {
            info!("WASM函数注销成功: {}", function_alias);
        }
        removed
    }

    /// 清除所有函数注册
    pub fn clear_all_functions(&mut self) {
        let count = self.registered_functions.len();
        self.registered_functions.clear();
        self.call_stats.clear();
        info!("清除所有WASM函数注册，共{}个", count);
    }
}

/// DTL WASM函数调用接口
///
/// 这个trait定义了DTL脚本中可以调用的WASM函数接口
#[async_trait]
pub trait DTLWasmFunction: Send + Sync {
    /// 函数名称
    fn name(&self) -> &str;

    /// 函数描述
    fn description(&self) -> Option<&str> {
        None
    }

    /// 调用函数
    async fn call(&self, args: Vec<Value>) -> Result<Value>;

    /// 获取参数类型
    fn parameter_types(&self) -> Vec<String> {
        vec!["any".to_string()]
    }

    /// 获取返回类型
    fn return_type(&self) -> String {
        "any".to_string()
    }
}

/// 内置的WASM函数调用实现
pub struct BuiltinWasmFunction {
    name: String,
    bridge: Arc<RwLock<DTLWasmBridge>>,
}

impl BuiltinWasmFunction {
    /// 创建新的内置WASM函数
    pub fn new(name: String, bridge: Arc<RwLock<DTLWasmBridge>>) -> Self {
        Self { name, bridge }
    }
}

#[async_trait]
impl DTLWasmFunction for BuiltinWasmFunction {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&str> {
        Some("调用已注册的WASM函数")
    }

    async fn call(&self, args: Vec<Value>) -> Result<Value> {
        let mut bridge = self.bridge.write().await;
        bridge.call_wasm_function(&self.name, args).await
    }
}

/// DTL WASM函数注册表
pub struct DTLWasmFunctionRegistry {
    /// 已注册的函数
    functions: HashMap<String, Box<dyn DTLWasmFunction>>,
    /// 桥接器
    bridge: Arc<RwLock<DTLWasmBridge>>,
}

impl DTLWasmFunctionRegistry {
    /// 创建新的函数注册表
    pub fn new(bridge: Arc<RwLock<DTLWasmBridge>>) -> Self {
        Self {
            functions: HashMap::new(),
            bridge,
        }
    }

    /// 注册函数
    pub fn register_function(&mut self, function: Box<dyn DTLWasmFunction>) {
        let name = function.name().to_string();
        self.functions.insert(name.clone(), function);
        info!("DTL WASM函数注册成功: {}", name);
    }

    /// 获取函数
    pub fn get_function(&self, name: &str) -> Option<&dyn DTLWasmFunction> {
        self.functions.get(name).map(|f| f.as_ref())
    }

    /// 列出所有函数
    pub fn list_functions(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }

    /// 创建内置WASM调用函数
    pub fn create_builtin_function(&self, name: String) -> BuiltinWasmFunction {
        BuiltinWasmFunction::new(name, self.bridge.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::WasmRuntimeConfig;

    #[tokio::test]
    async fn test_dtl_wasm_bridge_creation() {
        let config = WasmRuntimeConfig::default();
        let runtime = WasmRuntime::new(config).unwrap();
        let runtime_arc = Arc::new(RwLock::new(runtime));

        let bridge = DTLWasmBridge::new(runtime_arc);
        assert_eq!(bridge.get_registered_functions().len(), 0);
    }

    #[tokio::test]
    async fn test_function_registration() {
        let config = WasmRuntimeConfig::default();
        let runtime = WasmRuntime::new(config).unwrap();
        let runtime_arc = Arc::new(RwLock::new(runtime));

        let mut bridge = DTLWasmBridge::new(runtime_arc);

        // 注册函数应该失败，因为插件不存在
        let result = bridge.register_wasm_function(
            "test_func".to_string(),
            "nonexistent_plugin".to_string(),
            "process".to_string(),
            Some("测试函数".to_string()),
        ).await;

        assert!(result.is_err());
    }
}
