//! DataFlare兼容的WASM处理器
//!
//! 实现DataFlare Processor trait，使WASM插件能够无缝集成到DataFlare的处理器系统中

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use log::{info, debug, warn, error};
use wasmtime::{Engine, Module};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    processor::{Processor, ProcessorState},
    model::Schema,
};

use crate::{
    WasmPlugin, WasmPluginConfig, WasmPluginMetadata,
    runtime::WasmRuntimeConfig,
    interface::{WasmFunctionCall, WasmPluginInterface},
    error::{WasmError, WasmResult},
};

/// DataFlare兼容的WASM处理器
///
/// 这个处理器实现了DataFlare的Processor trait，
/// 使WASM插件能够作为标准的DataFlare处理器使用
pub struct DataFlareWasmProcessor {
    /// 处理器ID
    processor_id: String,
    /// 处理器状态
    state: ProcessorState,
    /// WASM插件实例
    plugin: Option<WasmPlugin>,
    /// 配置信息
    config: Option<WasmProcessorConfig>,
    /// 插件元数据
    metadata: Option<WasmPluginMetadata>,
}

/// WASM处理器配置
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WasmProcessorConfig {
    /// WASM插件路径
    pub plugin_path: String,
    /// 插件配置
    pub plugin_config: Option<Value>,
    /// 运行时配置
    pub runtime_config: Option<WasmRuntimeConfig>,
    /// 处理函数名称（默认为"process_record"）
    pub process_function: Option<String>,
    /// 批处理函数名称（默认为"process_batch"）
    pub batch_function: Option<String>,
    /// 内存限制（字节）
    pub memory_limit: Option<usize>,
    /// 超时时间（毫秒）
    pub timeout_ms: Option<u64>,
}

impl DataFlareWasmProcessor {
    /// 创建新的WASM处理器
    pub fn new(processor_id: String) -> Self {
        Self {
            processor_id: processor_id.clone(),
            state: ProcessorState::new(&processor_id),
            plugin: None,
            config: None,
            metadata: None,
        }
    }

    /// 从配置创建WASM处理器
    pub fn from_config(config: &Value) -> Result<Self> {
        let processor_config: WasmProcessorConfig = serde_json::from_value(config.clone())
            .map_err(|e| DataFlareError::Config(format!("无效的WASM处理器配置: {}", e)))?;

        let processor_id = format!("wasm_processor_{}", uuid::Uuid::new_v4());
        let mut processor = Self::new(processor_id);
        processor.config = Some(processor_config);

        Ok(processor)
    }

    /// 初始化WASM插件
    async fn initialize_plugin(&mut self) -> Result<()> {
        let config = self.config.as_ref()
            .ok_or_else(|| DataFlareError::Config("WASM处理器配置未设置".to_string()))?;

        // 创建WASM引擎和模块
        let engine = wasmtime::Engine::default();
        let wasm_bytes = std::fs::read(&config.plugin_path)
            .map_err(|e| DataFlareError::Plugin(format!("读取WASM文件失败: {}", e)))?;
        let module = wasmtime::Module::new(&engine, &wasm_bytes)
            .map_err(|e| DataFlareError::Plugin(format!("编译WASM模块失败: {}", e)))?;

        // 创建插件配置
        let plugin_config = WasmPluginConfig {
            name: self.processor_id.clone(),
            module_path: config.plugin_path.clone(),
            config: config.plugin_config.clone().unwrap_or_default().as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_else(HashMap::new),
            security_policy: crate::sandbox::SecurityPolicy::default(),
            memory_limit: config.memory_limit
                .or_else(|| config.runtime_config.as_ref().map(|rc| rc.memory_limit))
                .unwrap_or(64 * 1024 * 1024), // 默认64MB
            timeout_ms: config.timeout_ms
                .or_else(|| config.runtime_config.as_ref().map(|rc| rc.timeout_ms))
                .unwrap_or(5000), // 默认5秒
        };

        let mut plugin = WasmPlugin::new(plugin_config, module, engine).await
            .map_err(|e| DataFlareError::Plugin(format!("创建WASM插件失败: {}", e)))?;

        // 初始化插件
        let init_config = config.plugin_config.clone().unwrap_or_default();
        let init_config_map: HashMap<String, Value> = if let Some(obj) = init_config.as_object() {
            obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            HashMap::new()
        };

        plugin.initialize(&init_config_map).await
            .map_err(|e| DataFlareError::Plugin(format!("初始化WASM插件失败: {}", e)))?;

        // 获取插件元数据
        self.metadata = Some(plugin.get_metadata().clone());

        // 存储插件
        self.plugin = Some(plugin);

        info!("WASM处理器初始化完成: {}", self.processor_id);
        Ok(())
    }

    /// 调用WASM插件处理记录
    async fn call_plugin_process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        let plugin = self.plugin.as_mut()
            .ok_or_else(|| DataFlareError::Plugin("WASM插件未初始化".to_string()))?;

        let function_name = self.config.as_ref()
            .and_then(|c| c.process_function.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("process_record");

        // 序列化输入记录
        let input_data = serde_json::to_value(record)
            .map_err(|e| DataFlareError::Serialization(format!("序列化输入记录失败: {}", e)))?;

        // 创建函数调用
        let call = WasmFunctionCall::new(function_name.to_string())
            .with_parameter("record".to_string(), input_data)
            .map_err(|e| DataFlareError::Plugin(format!("创建函数调用失败: {}", e)))?;

        // 调用WASM函数
        let result = plugin.call_function(call).await
            .map_err(|e| DataFlareError::Plugin(format!("调用WASM函数失败: {}", e)))?;

        // 处理结果
        match result.success {
            true => {
                // 反序列化结果
                let result_value = result.result.ok_or_else(||
                    DataFlareError::Plugin("WASM函数返回空结果".to_string()))?;
                let output_record: DataRecord = serde_json::from_value(result_value)
                    .map_err(|e| DataFlareError::Serialization(format!("反序列化输出记录失败: {}", e)))?;
                Ok(output_record)
            }
            false => {
                let error_msg = result.error.unwrap_or_else(|| "未知WASM处理错误".to_string());
                Err(DataFlareError::Plugin(format!("WASM处理失败: {}", error_msg)))
            }
        }
    }
}

#[async_trait]
impl Processor for DataFlareWasmProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        debug!("配置WASM处理器: {}", self.processor_id);

        // 解析配置
        let processor_config: WasmProcessorConfig = serde_json::from_value(config.clone())
            .map_err(|e| DataFlareError::Config(format!("无效的WASM处理器配置: {}", e)))?;

        self.config = Some(processor_config);
        // 标记为已配置（通过添加状态数据）
        self.state.add_data("configured", true);

        info!("WASM处理器配置完成: {}", self.processor_id);
        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        // 确保插件已初始化
        if self.plugin.is_none() {
            self.initialize_plugin().await?;
        }

        // 调用WASM插件处理记录
        let result = self.call_plugin_process_record(record).await?;

        // 更新状态
        self.state.records_processed += 1;

        Ok(result)
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch) -> Result<DataRecordBatch> {
        debug!("WASM处理器批量处理 {} 条记录", batch.records.len());

        // 检查是否已配置
        if self.plugin.is_none() {
            return Err(DataFlareError::Plugin("WASM处理器未配置".to_string()));
        }

        let mut processed_records = Vec::with_capacity(batch.records.len());
        let mut error_count = 0;

        // 逐个处理记录
        for record in &batch.records {
            match self.process_record(record).await {
                Ok(processed_record) => processed_records.push(processed_record),
                Err(e) => {
                    warn!("WASM处理器处理记录失败: {}", e);
                    error_count += 1;
                    // 如果所有记录都失败了，返回错误
                    if error_count == batch.records.len() {
                        return Err(DataFlareError::Plugin(format!("批量处理失败，所有记录都处理失败")));
                    }
                    // 根据错误策略决定是否继续处理
                    continue;
                }
            }
        }

        // 创建新的批次
        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        Ok(new_batch)
    }

    fn get_state(&self) -> ProcessorState {
        self.state.clone()
    }

    fn get_input_schema(&self) -> Option<Schema> {
        None // WASM插件的模式是动态的
    }

    fn get_output_schema(&self) -> Option<Schema> {
        None // WASM插件的模式是动态的
    }

    async fn initialize(&mut self) -> Result<()> {
        debug!("初始化WASM处理器: {}", self.processor_id);

        if self.config.is_some() {
            self.initialize_plugin().await?;
        }

        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        debug!("清理WASM处理器: {}", self.processor_id);

        if let Some(mut plugin) = self.plugin.take() {
            if let Err(e) = plugin.cleanup().await {
                warn!("清理WASM插件失败: {}", e);
            }
        }


        info!("WASM处理器清理完成: {}", self.processor_id);
        Ok(())
    }
}

/// 注册WASM处理器到DataFlare处理器注册表
///
/// 注意：这个函数需要在有dataflare_processor依赖的环境中调用
pub fn register_wasm_processor() -> Result<()> {
    // 这里暂时返回Ok，实际的注册逻辑需要在processor crate中实现
    info!("WASM处理器注册函数已准备就绪");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_wasm_processor_creation() {
        let config = json!({
            "plugin_path": "test.wasm",
            "plugin_config": {
                "test": "value"
            }
        });

        let processor = DataFlareWasmProcessor::from_config(&config);
        assert!(processor.is_ok());
    }

    #[tokio::test]
    async fn test_wasm_processor_configure() {
        let mut processor = DataFlareWasmProcessor::new("test".to_string());

        let config = json!({
            "plugin_path": "test.wasm",
            "plugin_config": {
                "test": "value"
            }
        });

        let result = processor.configure(&config);
        assert!(result.is_ok());
        // 检查是否已配置
        assert!(processor.state.data.get("configured").and_then(|v| v.as_bool()).unwrap_or(false));
    }
}
