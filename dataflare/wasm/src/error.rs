//! WASM插件系统错误处理

use std::fmt;
use thiserror::Error;

/// WASM插件系统错误类型
#[derive(Error, Debug)]
pub enum WasmError {
    /// 运行时错误
    #[error("WASM运行时错误: {0}")]
    Runtime(String),

    /// 插件加载错误
    #[error("插件加载失败: {0}")]
    PluginLoad(String),

    /// 插件执行错误
    #[error("插件执行失败: {0}")]
    PluginExecution(String),

    /// 配置错误
    #[error("配置错误: {0}")]
    Configuration(String),

    /// 序列化错误
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO错误
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    /// 沙箱错误
    #[error("沙箱错误: {0}")]
    Sandbox(String),

    /// 权限错误
    #[error("权限错误: {0}")]
    Permission(String),

    /// 超时错误
    #[error("操作超时: {0}")]
    Timeout(String),

    /// 内存限制错误
    #[error("内存限制错误: {0}")]
    MemoryLimit(String),

    /// 函数不存在错误
    #[error("函数不存在: {0}")]
    FunctionNotFound(String),

    /// 类型转换错误
    #[error("类型转换错误: {0}")]
    TypeConversion(String),

    /// 组件错误
    #[error("组件错误: {0}")]
    Component(String),

    /// 注册表错误
    #[error("注册表错误: {0}")]
    Registry(String),

    /// Wasmtime错误
    #[error("Wasmtime错误: {0}")]
    Wasmtime(#[from] wasmtime::Error),

    /// 通用错误
    #[error("通用错误: {0}")]
    Generic(String),
}

impl WasmError {
    /// 创建运行时错误
    pub fn runtime<T: fmt::Display>(msg: T) -> Self {
        WasmError::Runtime(msg.to_string())
    }

    /// 创建插件加载错误
    pub fn plugin_load<T: fmt::Display>(msg: T) -> Self {
        WasmError::PluginLoad(msg.to_string())
    }

    /// 创建插件执行错误
    pub fn plugin_execution<T: fmt::Display>(msg: T) -> Self {
        WasmError::PluginExecution(msg.to_string())
    }

    /// 创建配置错误
    pub fn configuration<T: fmt::Display>(msg: T) -> Self {
        WasmError::Configuration(msg.to_string())
    }

    /// 创建沙箱错误
    pub fn sandbox<T: fmt::Display>(msg: T) -> Self {
        WasmError::Sandbox(msg.to_string())
    }

    /// 创建权限错误
    pub fn permission<T: fmt::Display>(msg: T) -> Self {
        WasmError::Permission(msg.to_string())
    }

    /// 创建超时错误
    pub fn timeout<T: fmt::Display>(msg: T) -> Self {
        WasmError::Timeout(msg.to_string())
    }

    /// 创建内存限制错误
    pub fn memory_limit<T: fmt::Display>(msg: T) -> Self {
        WasmError::MemoryLimit(msg.to_string())
    }

    /// 创建函数不存在错误
    pub fn function_not_found<T: fmt::Display>(msg: T) -> Self {
        WasmError::FunctionNotFound(msg.to_string())
    }

    /// 创建类型转换错误
    pub fn type_conversion<T: fmt::Display>(msg: T) -> Self {
        WasmError::TypeConversion(msg.to_string())
    }

    /// 创建组件错误
    pub fn component<T: fmt::Display>(msg: T) -> Self {
        WasmError::Component(msg.to_string())
    }

    /// 创建注册表错误
    pub fn registry<T: fmt::Display>(msg: T) -> Self {
        WasmError::Registry(msg.to_string())
    }

    /// 创建通用错误
    pub fn generic<T: fmt::Display>(msg: T) -> Self {
        WasmError::Generic(msg.to_string())
    }
}

/// WASM插件系统结果类型
pub type WasmResult<T> = Result<T, WasmError>;

/// 将WasmError转换为DataFlareError
impl From<WasmError> for dataflare_core::error::DataFlareError {
    fn from(err: WasmError) -> Self {
        match err {
            WasmError::Runtime(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::PluginLoad(msg) => dataflare_core::error::DataFlareError::Config(msg),
            WasmError::PluginExecution(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::Configuration(msg) => dataflare_core::error::DataFlareError::Config(msg),
            WasmError::Serialization(err) => dataflare_core::error::DataFlareError::Processing(err.to_string()),
            WasmError::Io(err) => dataflare_core::error::DataFlareError::Io(err),
            WasmError::Sandbox(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::Permission(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::Timeout(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::MemoryLimit(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::FunctionNotFound(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::TypeConversion(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::Component(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::Registry(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            WasmError::Wasmtime(err) => dataflare_core::error::DataFlareError::Processing(err.to_string()),
            WasmError::Generic(msg) => dataflare_core::error::DataFlareError::Processing(msg),
        }
    }
}

/// 错误上下文扩展
pub trait WasmErrorContext<T> {
    /// 添加上下文信息
    fn with_context<F>(self, f: F) -> WasmResult<T>
    where
        F: FnOnce() -> String;

    /// 添加简单上下文
    fn context(self, msg: &str) -> WasmResult<T>;
}

impl<T> WasmErrorContext<T> for WasmResult<T> {
    fn with_context<F>(self, f: F) -> WasmResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|err| WasmError::Generic(format!("{}: {}", f(), err)))
    }

    fn context(self, msg: &str) -> WasmResult<T> {
        self.with_context(|| msg.to_string())
    }
}

// 移除冲突的实现，只保留WasmResult的实现

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = WasmError::runtime("test runtime error");
        assert!(matches!(err, WasmError::Runtime(_)));
        assert_eq!(err.to_string(), "WASM运行时错误: test runtime error");
    }

    #[test]
    fn test_error_conversion() {
        let wasm_err = WasmError::configuration("test config error");
        let dataflare_err: dataflare_core::error::DataFlareError = wasm_err.into();

        match dataflare_err {
            dataflare_core::error::DataFlareError::Config(msg) => {
                assert_eq!(msg, "test config error");
            }
            _ => panic!("Expected Config error"),
        }
    }

    #[test]
    fn test_error_display() {
        let err = WasmError::Configuration("test config error".to_string());
        assert!(err.to_string().contains("test config error"));

        let err = WasmError::Runtime("test runtime error".to_string());
        assert!(err.to_string().contains("test runtime error"));
    }

    #[test]
    fn test_error_with_context() {
        let result: WasmResult<()> = Err(WasmError::runtime("original error"));

        let contextual_result = result.with_context(|| "Additional context".to_string());
        assert!(contextual_result.is_err());

        let err = contextual_result.unwrap_err();
        assert!(err.to_string().contains("Additional context"));
        assert!(err.to_string().contains("original error"));
    }
}
