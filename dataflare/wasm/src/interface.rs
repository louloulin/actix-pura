//! 统一的WASM插件接口
//!
//! 支持DataFlare完整DSL的统一插件接口系统

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{WasmError, WasmResult};

/// WASM函数调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmFunctionCall {
    /// 函数名称
    pub function_name: String,
    /// 函数参数
    pub parameters: HashMap<String, serde_json::Value>,
    /// 调用上下文
    pub context: Option<HashMap<String, serde_json::Value>>,
    /// 调用ID (用于追踪)
    pub call_id: Option<String>,
}

impl WasmFunctionCall {
    /// 创建新的函数调用
    pub fn new(function_name: String) -> Self {
        Self {
            function_name,
            parameters: HashMap::new(),
            context: None,
            call_id: None,
        }
    }

    /// 添加参数
    pub fn with_parameter<T: Serialize>(mut self, key: String, value: T) -> WasmResult<Self> {
        let json_value = serde_json::to_value(value)
            .map_err(|e| WasmError::Serialization(e))?;
        self.parameters.insert(key, json_value);
        Ok(self)
    }

    /// 添加上下文
    pub fn with_context(mut self, context: HashMap<String, serde_json::Value>) -> Self {
        self.context = Some(context);
        self
    }

    /// 设置调用ID
    pub fn with_call_id(mut self, call_id: String) -> Self {
        self.call_id = Some(call_id);
        self
    }

    /// 序列化为字节数组
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| WasmError::Serialization(e))
    }

    /// 从字节数组反序列化
    pub fn from_bytes(bytes: &[u8]) -> WasmResult<Self> {
        serde_json::from_slice(bytes)
            .map_err(|e| WasmError::Serialization(e))
    }
}

/// WASM函数调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmFunctionResult {
    /// 是否成功
    pub success: bool,
    /// 返回值
    pub result: Option<serde_json::Value>,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时间 (毫秒)
    pub execution_time_ms: Option<u64>,
    /// 调用ID (对应请求的call_id)
    pub call_id: Option<String>,
    /// 额外的元数据
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl WasmFunctionResult {
    /// 创建成功结果
    pub fn success<T: Serialize>(result: T) -> WasmResult<Self> {
        let json_value = serde_json::to_value(result)
            .map_err(|e| WasmError::Serialization(e))?;

        Ok(Self {
            success: true,
            result: Some(json_value),
            error: None,
            execution_time_ms: None,
            call_id: None,
            metadata: None,
        })
    }

    /// 创建错误结果
    pub fn error<T: ToString>(error: T) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(error.to_string()),
            execution_time_ms: None,
            call_id: None,
            metadata: None,
        }
    }

    /// 设置执行时间
    pub fn with_execution_time(mut self, time_ms: u64) -> Self {
        self.execution_time_ms = Some(time_ms);
        self
    }

    /// 设置调用ID
    pub fn with_call_id(mut self, call_id: String) -> Self {
        self.call_id = Some(call_id);
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// 序列化为字节数组
    pub fn to_bytes(&self) -> WasmResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| WasmError::Serialization(e))
    }

    /// 从字节数组反序列化
    pub fn from_bytes(bytes: &[u8]) -> WasmResult<Self> {
        serde_json::from_slice(bytes)
            .map_err(|e| WasmError::Serialization(e))
    }

    /// 获取结果值
    pub fn get_result<T: for<'de> Deserialize<'de>>(&self) -> WasmResult<T> {
        if !self.success {
            return Err(WasmError::PluginExecution(
                self.error.as_deref().unwrap_or("Unknown error").to_string()
            ));
        }

        let result_value = self.result.as_ref()
            .ok_or_else(|| WasmError::PluginExecution("No result value".to_string()))?;

        serde_json::from_value(result_value.clone())
            .map_err(|e| WasmError::TypeConversion(format!("Failed to deserialize result: {}", e)))
    }
}

/// WASM插件能力描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginCapabilities {
    /// 支持的DSL组件类型
    pub supported_component_types: Vec<String>,
    /// 支持的函数列表
    pub supported_functions: Vec<WasmFunctionCapability>,
    /// 是否支持异步操作
    pub supports_async: bool,
    /// 是否支持流式处理
    pub supports_streaming: bool,
    /// 是否支持批量处理
    pub supports_batch: bool,
    /// 最大内存使用 (字节)
    pub max_memory_usage: Option<usize>,
    /// 最大执行时间 (毫秒)
    pub max_execution_time: Option<u64>,
    /// 所需权限列表
    pub required_permissions: Vec<String>,
}

impl Default for WasmPluginCapabilities {
    fn default() -> Self {
        Self {
            supported_component_types: vec!["processor".to_string()],
            supported_functions: vec![],
            supports_async: false,
            supports_streaming: false,
            supports_batch: false,
            max_memory_usage: None,
            max_execution_time: None,
            required_permissions: vec![],
        }
    }
}

/// WASM函数能力描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmFunctionCapability {
    /// 函数名称
    pub name: String,
    /// 函数描述
    pub description: String,
    /// 输入参数定义
    pub input_parameters: Vec<WasmParameterDefinition>,
    /// 输出类型定义
    pub output_type: String,
    /// 是否为异步函数
    pub is_async: bool,
    /// 是否为流式函数
    pub is_streaming: bool,
}

/// WASM参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmParameterDefinition {
    /// 参数名称
    pub name: String,
    /// 参数类型
    pub parameter_type: String,
    /// 是否必需
    pub required: bool,
    /// 默认值
    pub default_value: Option<serde_json::Value>,
    /// 参数描述
    pub description: String,
}

/// 统一的WASM插件接口
/// 支持DataFlare所有DSL组件类型：source、destination、processor、transformer、filter、aggregator
#[async_trait]
pub trait WasmPluginInterface: Send + Sync {
    /// 初始化插件
    async fn initialize(&mut self, config: &HashMap<String, serde_json::Value>) -> WasmResult<()>;

    /// 通用函数调用接口 - 支持所有DSL组件操作
    async fn call_function(&self, call: WasmFunctionCall) -> WasmResult<WasmFunctionResult>;

    /// 获取插件能力
    fn get_capabilities(&self) -> &WasmPluginCapabilities;

    /// 检查是否支持指定函数
    fn has_function(&self, function_name: &str) -> bool {
        self.get_capabilities()
            .supported_functions
            .iter()
            .any(|f| f.name == function_name)
    }

    /// 获取支持的函数列表
    fn get_supported_functions(&self) -> Vec<String> {
        self.get_capabilities()
            .supported_functions
            .iter()
            .map(|f| f.name.clone())
            .collect()
    }

    /// 清理插件资源
    async fn cleanup(&mut self) -> WasmResult<()>;

    /// 健康检查
    async fn health_check(&self) -> WasmResult<bool>;

    /// 热重载插件
    async fn reload(&mut self) -> WasmResult<()>;
}

/// DSL组件函数映射
pub struct DslFunctionMapper;

impl DslFunctionMapper {
    /// 获取Source组件的标准函数
    pub fn source_functions() -> Vec<String> {
        vec![
            "discover_schema".to_string(),
            "read_batch".to_string(),
            "read_stream".to_string(),
            "get_state".to_string(),
            "commit".to_string(),
            "seek".to_string(),
            "estimate_count".to_string(),
        ]
    }

    /// 获取Destination组件的标准函数
    pub fn destination_functions() -> Vec<String> {
        vec![
            "prepare_schema".to_string(),
            "write_batch".to_string(),
            "write_record".to_string(),
            "flush".to_string(),
            "commit".to_string(),
            "rollback".to_string(),
            "get_write_state".to_string(),
        ]
    }

    /// 获取Processor组件的标准函数
    pub fn processor_functions() -> Vec<String> {
        vec![
            "process_record".to_string(),
            "process_batch".to_string(),
            "get_input_schema".to_string(),
            "get_output_schema".to_string(),
            "get_state".to_string(),
        ]
    }

    /// 获取Transformer组件的标准函数
    pub fn transformer_functions() -> Vec<String> {
        vec![
            "transform".to_string(),
            "transform_batch".to_string(),
            "validate_input".to_string(),
            "get_transformation_rules".to_string(),
        ]
    }

    /// 获取Filter组件的标准函数
    pub fn filter_functions() -> Vec<String> {
        vec![
            "filter".to_string(),
            "filter_batch".to_string(),
            "validate_filter_expression".to_string(),
            "get_filter_stats".to_string(),
        ]
    }

    /// 获取Aggregator组件的标准函数
    pub fn aggregator_functions() -> Vec<String> {
        vec![
            "aggregate".to_string(),
            "aggregate_batch".to_string(),
            "reset_aggregation".to_string(),
            "get_aggregation_result".to_string(),
            "get_aggregation_stats".to_string(),
        ]
    }

    /// 根据组件类型获取标准函数
    pub fn get_functions_for_component(component_type: &str) -> Vec<String> {
        match component_type.to_lowercase().as_str() {
            "source" => Self::source_functions(),
            "destination" => Self::destination_functions(),
            "processor" => Self::processor_functions(),
            "transformer" => Self::transformer_functions(),
            "filter" => Self::filter_functions(),
            "aggregator" => Self::aggregator_functions(),
            _ => vec!["transform".to_string()], // 默认函数
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_call_creation() {
        let call = WasmFunctionCall::new("test_function".to_string())
            .with_parameter("param1".to_string(), "value1").unwrap()
            .with_call_id("test_call_123".to_string());

        assert_eq!(call.function_name, "test_function");
        assert_eq!(call.parameters.get("param1").unwrap().as_str().unwrap(), "value1");
        assert_eq!(call.call_id.as_ref().unwrap(), "test_call_123");
    }

    #[test]
    fn test_function_result_creation() {
        let result = WasmFunctionResult::success("test_result").unwrap()
            .with_execution_time(100)
            .with_call_id("test_call_123".to_string());

        assert!(result.success);
        assert_eq!(result.result.as_ref().unwrap().as_str().unwrap(), "test_result");
        assert_eq!(result.execution_time_ms.unwrap(), 100);
        assert_eq!(result.call_id.as_ref().unwrap(), "test_call_123");
    }

    #[test]
    fn test_error_result_creation() {
        let result = WasmFunctionResult::error("Test error message");

        assert!(!result.success);
        assert_eq!(result.error.as_ref().unwrap(), "Test error message");
        assert!(result.result.is_none());
    }

    #[test]
    fn test_dsl_function_mapping() {
        let source_funcs = DslFunctionMapper::source_functions();
        assert!(source_funcs.contains(&"discover_schema".to_string()));
        assert!(source_funcs.contains(&"read_batch".to_string()));

        let dest_funcs = DslFunctionMapper::destination_functions();
        assert!(dest_funcs.contains(&"write_batch".to_string()));
        assert!(dest_funcs.contains(&"commit".to_string()));

        let proc_funcs = DslFunctionMapper::processor_functions();
        assert!(proc_funcs.contains(&"process_record".to_string()));
        assert!(proc_funcs.contains(&"process_batch".to_string()));
    }

    #[test]
    fn test_serialization() {
        let call = WasmFunctionCall::new("test".to_string());
        let bytes = call.to_bytes().unwrap();
        let deserialized = WasmFunctionCall::from_bytes(&bytes).unwrap();
        assert_eq!(call.function_name, deserialized.function_name);

        let result = WasmFunctionResult::success("test").unwrap();
        let bytes = result.to_bytes().unwrap();
        let deserialized = WasmFunctionResult::from_bytes(&bytes).unwrap();
        assert_eq!(result.success, deserialized.success);
    }
}
