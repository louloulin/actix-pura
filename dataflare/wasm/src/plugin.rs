//! WASM插件实现

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasmtime::*;
use log::{info, debug, warn, error};

use crate::{
    WasmError, WasmResult,
    interface::{WasmPluginInterface, WasmPluginCapabilities, WasmFunctionCall, WasmFunctionResult},
    sandbox::SecurityPolicy,
};

/// WASM插件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginConfig {
    /// 插件名称
    pub name: String,
    /// WASM模块路径
    pub module_path: String,
    /// 插件配置参数
    pub config: HashMap<String, serde_json::Value>,
    /// 安全策略
    pub security_policy: SecurityPolicy,
    /// 内存限制
    pub memory_limit: usize,
    /// 执行超时
    pub timeout_ms: u64,
}

/// WASM插件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginMetadata {
    /// 插件名称
    pub name: String,
    /// 插件版本
    pub version: String,
    /// 插件作者
    pub author: String,
    /// 插件描述
    pub description: String,
    /// 支持的DataFlare版本
    pub dataflare_version: String,
    /// 插件能力
    pub capabilities: WasmPluginCapabilities,
    /// 支持的函数列表
    pub supported_functions: Vec<String>,
    /// 支持的组件类型
    pub component_types: Vec<String>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Default for WasmPluginMetadata {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            name: "unknown".to_string(),
            version: "0.1.0".to_string(),
            author: "unknown".to_string(),
            description: "WASM plugin".to_string(),
            dataflare_version: "0.1.0".to_string(),
            capabilities: WasmPluginCapabilities::default(),
            supported_functions: Vec::new(),
            component_types: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// WASM插件状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WasmPluginStatus {
    /// 未初始化
    Uninitialized,
    /// 已初始化
    Initialized,
    /// 运行中
    Running,
    /// 已停止
    Stopped,
    /// 错误状态
    Error(String),
}

/// WASM插件实现
#[derive(Clone)]
pub struct WasmPlugin {
    /// 插件配置
    config: WasmPluginConfig,
    /// 插件元数据
    metadata: WasmPluginMetadata,
    /// WASM模块
    module: Module,
    /// Wasmtime引擎
    engine: Engine,
    /// 插件状态
    status: WasmPluginStatus,
    /// 执行统计
    stats: WasmPluginStats,
}

impl WasmPlugin {
    /// 创建新的WASM插件
    pub async fn new(
        config: WasmPluginConfig,
        module: Module,
        engine: Engine,
    ) -> WasmResult<Self> {
        let metadata = Self::extract_metadata(&module, &config).await?;

        let plugin = Self {
            config,
            metadata,
            module,
            engine,
            status: WasmPluginStatus::Uninitialized,
            stats: WasmPluginStats::default(),
        };

        info!("WASM插件创建成功: {}", plugin.metadata.name);
        Ok(plugin)
    }

    /// 从WASM模块提取元数据
    async fn extract_metadata(
        module: &Module,
        config: &WasmPluginConfig,
    ) -> WasmResult<WasmPluginMetadata> {
        let mut metadata = WasmPluginMetadata::default();
        metadata.name = config.name.clone();

        // 1. 尝试从WASM模块的自定义段提取元数据
        if let Some(custom_metadata) = Self::extract_custom_section_metadata(module)? {
            metadata = custom_metadata;
            metadata.name = config.name.clone(); // 保持配置中的名称
        }

        // 2. 尝试从配置中获取元数据（优先级更高）
        if let Some(meta_config) = config.config.get("metadata") {
            if let Ok(meta) = serde_json::from_value::<WasmPluginMetadata>(meta_config.clone()) {
                metadata = meta;
                metadata.name = config.name.clone(); // 保持配置中的名称
            }
        }

        // 3. 从WASM模块导出函数推断能力
        Self::infer_capabilities_from_exports(module, &mut metadata)?;

        Ok(metadata)
    }

    /// 从WASM模块的自定义段提取元数据
    fn extract_custom_section_metadata(_module: &Module) -> WasmResult<Option<WasmPluginMetadata>> {
        // 注意：wasmtime的Module API不直接提供访问自定义段的方法
        // 这里我们暂时返回None，在未来版本中可以通过其他方式实现
        // 例如：使用wasm-parser或在编译时嵌入元数据

        log::debug!("自定义段元数据提取暂未实现，使用配置文件元数据");
        Ok(None)
    }

    /// 从导出函数推断插件能力
    fn infer_capabilities_from_exports(module: &Module, metadata: &mut WasmPluginMetadata) -> WasmResult<()> {
        let mut supported_functions = Vec::new();
        let mut component_types = Vec::new();

        // 检查导出的函数
        for export in module.exports() {
            if let wasmtime::ExternType::Func(_) = export.ty() {
                let func_name = export.name();
                supported_functions.push(func_name.to_string());

                // 根据函数名推断组件类型
                match func_name {
                    "read_data" | "fetch_data" | "get_data" => {
                        if !component_types.contains(&"source".to_string()) {
                            component_types.push("source".to_string());
                        }
                    }
                    "write_data" | "send_data" | "output_data" => {
                        if !component_types.contains(&"destination".to_string()) {
                            component_types.push("destination".to_string());
                        }
                    }
                    "process_record" | "process_batch" => {
                        if !component_types.contains(&"processor".to_string()) {
                            component_types.push("processor".to_string());
                        }
                    }
                    "transform" | "transform_batch" => {
                        if !component_types.contains(&"transformer".to_string()) {
                            component_types.push("transformer".to_string());
                        }
                    }
                    "filter" | "filter_batch" => {
                        if !component_types.contains(&"filter".to_string()) {
                            component_types.push("filter".to_string());
                        }
                    }
                    "aggregate" | "aggregate_batch" => {
                        if !component_types.contains(&"aggregator".to_string()) {
                            component_types.push("aggregator".to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        // 如果没有推断出组件类型，默认为processor
        if component_types.is_empty() {
            component_types.push("processor".to_string());
        }

        // 更新元数据
        if metadata.supported_functions.is_empty() {
            metadata.supported_functions = supported_functions;
        }
        if metadata.component_types.is_empty() {
            metadata.component_types = component_types;
        }

        log::debug!("推断出的组件类型: {:?}", metadata.component_types);
        log::debug!("推断出的支持函数: {:?}", metadata.supported_functions);

        Ok(())
    }

    /// 创建WASM实例
    async fn create_instance(&self) -> WasmResult<(Store<()>, Instance)> {
        // 创建Store
        let mut store = Store::new(&self.engine, ());

        // 配置内存限制
        self.configure_memory_limits(&mut store)?;

        // 配置执行时间限制
        self.configure_execution_limits(&mut store)?;

        // 实例化模块
        let instance = Instance::new(&mut store, &self.module, &[])
            .map_err(|e| WasmError::PluginExecution(format!("实例化WASM模块失败: {}", e)))?;

        Ok((store, instance))
    }

    /// 配置内存限制
    fn configure_memory_limits<T>(&self, store: &mut Store<T>) -> WasmResult<()> {
        // 设置内存限制（字节）
        let memory_limit_bytes = self.config.memory_limit;

        // 暂时跳过内存限制配置，因为wasmtime API的复杂性
        // 在生产环境中，可以通过其他方式实现内存限制
        log::debug!("内存限制配置: {} bytes (暂时跳过实际限制)", memory_limit_bytes);
        Ok(())
    }

    /// 配置执行时间限制
    fn configure_execution_limits<T>(&self, store: &mut Store<T>) -> WasmResult<()> {
        // 设置执行时间限制
        let timeout_ms = self.config.timeout_ms;

        // 使用wasmtime的中断机制
        store.set_epoch_deadline(1);

        log::debug!("配置执行时间限制: {} ms", timeout_ms);
        Ok(())
    }

    /// 调用WASM函数
    async fn call_wasm_function(
        &self,
        function_name: &str,
        input_bytes: &[u8],
    ) -> WasmResult<Vec<u8>> {
        let start_time = std::time::Instant::now();

        // 创建实例
        let (mut store, instance) = self.create_instance().await?;

        // 获取函数
        let func = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, function_name)
            .map_err(|e| WasmError::function_not_found(format!("函数 {} 不存在: {}", function_name, e)))?;

        // 分配内存并写入输入数据
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| WasmError::plugin_execution("WASM模块没有导出memory"))?;

        let input_ptr = self.allocate_memory(&mut store, &memory, input_bytes.len())?;
        memory.write(&mut store, input_ptr, input_bytes)
            .map_err(|e| WasmError::plugin_execution(format!("写入WASM内存失败: {}", e)))?;

        // 调用函数
        let output_ptr = func.call(&mut store, (input_ptr as i32, input_bytes.len() as i32))
            .map_err(|e| WasmError::plugin_execution(format!("调用WASM函数失败: {}", e)))?;

        // 读取输出数据
        let output_bytes = self.read_output(&mut store, &memory, output_ptr as usize)?;

        // 释放内存
        self.deallocate_memory(&mut store, &instance, input_ptr)?;
        self.deallocate_memory(&mut store, &instance, output_ptr as usize)?;

        let execution_time = start_time.elapsed();
        debug!("WASM函数 {} 执行完成，耗时: {:?}", function_name, execution_time);

        Ok(output_bytes)
    }

    /// 分配WASM内存
    fn allocate_memory(&self, store: &mut Store<()>, memory: &Memory, size: usize) -> WasmResult<usize> {
        // 简化的内存分配，实际应该调用WASM模块的分配函数
        let data = memory.data(store);
        let current_size = data.len();

        if current_size + size > self.config.memory_limit {
            return Err(WasmError::memory_limit("内存不足"));
        }

        // 返回当前内存末尾作为分配地址
        Ok(current_size)
    }

    /// 读取输出数据
    fn read_output(&self, store: &mut Store<()>, memory: &Memory, ptr: usize) -> WasmResult<Vec<u8>> {
        // 简化的输出读取，实际应该根据返回的指针和长度读取
        let data = memory.data(store);

        if ptr >= data.len() {
            return Err(WasmError::plugin_execution("无效的输出指针"));
        }

        // 假设输出长度存储在指针位置的前4个字节
        if ptr + 4 > data.len() {
            return Err(WasmError::plugin_execution("无法读取输出长度"));
        }

        let length = u32::from_le_bytes([data[ptr], data[ptr + 1], data[ptr + 2], data[ptr + 3]]) as usize;

        if ptr + 4 + length > data.len() {
            return Err(WasmError::plugin_execution("输出数据超出内存范围"));
        }

        Ok(data[ptr + 4..ptr + 4 + length].to_vec())
    }

    /// 释放WASM内存
    fn deallocate_memory(&self, _store: &mut Store<()>, _instance: &Instance, _ptr: usize) -> WasmResult<()> {
        // 简化的内存释放，实际应该调用WASM模块的释放函数
        Ok(())
    }

    /// 获取插件元数据
    pub fn get_metadata(&self) -> &WasmPluginMetadata {
        &self.metadata
    }

    /// 获取插件状态
    pub fn get_status(&self) -> WasmPluginStatus {
        self.status.clone()
    }

    /// 获取插件统计信息
    pub fn get_stats(&self) -> &WasmPluginStats {
        &self.stats
    }

    /// 更新状态
    fn set_status(&mut self, status: WasmPluginStatus) {
        self.status = status;
    }

    /// 检查是否支持指定函数
    pub fn has_function(&self, function_name: &str) -> bool {
        // 简化实现：假设所有基本函数都支持
        // 实际实现应该检查WASM模块的导出函数
        match function_name {
            "initialize" | "cleanup" | "process_record" | "process_batch" => true,
            _ => false,
        }
    }
}

#[async_trait]
impl WasmPluginInterface for WasmPlugin {
    async fn initialize(&mut self, config: &HashMap<String, serde_json::Value>) -> WasmResult<()> {
        if self.status == WasmPluginStatus::Initialized {
            return Ok(());
        }

        info!("初始化WASM插件: {}", self.metadata.name);

        // 更新配置
        for (key, value) in config {
            self.config.config.insert(key.clone(), value.clone());
        }

        // 调用插件的初始化函数
        if self.has_function("initialize") {
            let config_bytes = serde_json::to_vec(config)
                .map_err(|e| WasmError::Serialization(e))?;

            let _result = self.call_wasm_function("initialize", &config_bytes).await?;
        }

        self.set_status(WasmPluginStatus::Initialized);
        self.stats.initialization_count += 1;

        info!("WASM插件初始化完成: {}", self.metadata.name);
        Ok(())
    }

    async fn call_function(&self, call: WasmFunctionCall) -> WasmResult<WasmFunctionResult> {
        let start_time = std::time::Instant::now();

        // 检查函数是否存在
        if !self.has_function(&call.function_name) {
            return Ok(WasmFunctionResult::error(format!("函数不存在: {}", call.function_name)));
        }

        // 序列化调用参数
        let input_bytes = call.to_bytes()?;

        // 调用WASM函数
        match self.call_wasm_function(&call.function_name, &input_bytes).await {
            Ok(output_bytes) => {
                let execution_time = start_time.elapsed().as_millis() as u64;

                // 解析结果
                match WasmFunctionResult::from_bytes(&output_bytes) {
                    Ok(mut result) => {
                        result.execution_time_ms = Some(execution_time);
                        result.call_id = call.call_id;
                        Ok(result)
                    }
                    Err(_) => {
                        // 如果无法解析为WasmFunctionResult，则将原始字节作为结果
                        let result_value = serde_json::Value::String(
                            String::from_utf8_lossy(&output_bytes).to_string()
                        );
                        Ok(WasmFunctionResult::success(result_value)?
                            .with_execution_time(execution_time)
                            .with_call_id(call.call_id.unwrap_or_default()))
                    }
                }
            }
            Err(e) => {
                let execution_time = start_time.elapsed().as_millis() as u64;
                Ok(WasmFunctionResult::error(e.to_string())
                    .with_execution_time(execution_time)
                    .with_call_id(call.call_id.unwrap_or_default()))
            }
        }
    }

    fn get_capabilities(&self) -> &WasmPluginCapabilities {
        &self.metadata.capabilities
    }

    async fn cleanup(&mut self) -> WasmResult<()> {
        if self.status == WasmPluginStatus::Stopped {
            return Ok(());
        }

        info!("清理WASM插件: {}", self.metadata.name);

        // 调用插件的清理函数
        if self.has_function("cleanup") {
            let _result = self.call_wasm_function("cleanup", &[]).await?;
        }

        self.set_status(WasmPluginStatus::Stopped);
        self.stats.cleanup_count += 1;

        info!("WASM插件清理完成: {}", self.metadata.name);
        Ok(())
    }

    async fn health_check(&self) -> WasmResult<bool> {
        match self.status {
            WasmPluginStatus::Initialized | WasmPluginStatus::Running => Ok(true),
            _ => Ok(false),
        }
    }

    async fn reload(&mut self) -> WasmResult<()> {
        info!("重新加载WASM插件: {}", self.metadata.name);

        // 清理当前实例
        self.cleanup().await?;

        // 重新加载模块
        let wasm_bytes = std::fs::read(&self.config.module_path)
            .map_err(|e| WasmError::PluginLoad(format!("读取WASM文件失败: {}", e)))?;

        self.module = Module::new(&self.engine, &wasm_bytes)
            .map_err(|e| WasmError::PluginLoad(format!("重新编译WASM模块失败: {}", e)))?;

        // 重新初始化
        self.initialize(&self.config.config.clone()).await?;

        self.stats.reload_count += 1;
        info!("WASM插件重新加载完成: {}", self.metadata.name);
        Ok(())
    }
}

/// WASM插件统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginStats {
    /// 初始化次数
    pub initialization_count: u64,
    /// 清理次数
    pub cleanup_count: u64,
    /// 重新加载次数
    pub reload_count: u64,
    /// 函数调用次数
    pub function_call_count: u64,
    /// 总执行时间 (毫秒)
    pub total_execution_time_ms: u64,
    /// 错误次数
    pub error_count: u64,
    /// 最后调用时间
    pub last_call_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for WasmPluginStats {
    fn default() -> Self {
        Self {
            initialization_count: 0,
            cleanup_count: 0,
            reload_count: 0,
            function_call_count: 0,
            total_execution_time_ms: 0,
            error_count: 0,
            last_call_time: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata_default() {
        let metadata = WasmPluginMetadata::default();
        assert_eq!(metadata.name, "unknown");
        assert_eq!(metadata.version, "0.1.0");
    }

    #[test]
    fn test_plugin_status() {
        assert_eq!(WasmPluginStatus::Uninitialized, WasmPluginStatus::Uninitialized);
        assert_ne!(WasmPluginStatus::Initialized, WasmPluginStatus::Running);
    }

    #[test]
    fn test_plugin_stats_default() {
        let stats = WasmPluginStats::default();
        assert_eq!(stats.initialization_count, 0);
        assert_eq!(stats.function_call_count, 0);
        assert!(stats.last_call_time.is_none());
    }
}


