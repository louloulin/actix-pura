//! DataFlare Enterprise 错误处理模块
//!
//! 提供企业级错误处理和诊断能力

use std::fmt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// DataFlare Enterprise 错误类型
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum EnterpriseError {
    /// 流处理错误
    #[error("流处理错误: {message}")]
    StreamProcessing { message: String, code: String },

    /// 分布式计算错误
    #[error("分布式计算错误: {message}")]
    DistributedComputing { message: String, node_id: Option<String> },

    /// 监控系统错误
    #[error("监控系统错误: {message}")]
    Monitoring { message: String, component: String },

    /// 安全管理错误
    #[error("安全管理错误: {message}")]
    Security { message: String, security_level: SecurityLevel },

    /// 合规检查错误
    #[error("合规检查错误: {message}")]
    Compliance { message: String, requirement: String },

    /// 插件市场错误
    #[error("插件市场错误: {message}")]
    Marketplace { message: String, plugin_id: Option<String> },

    /// 集群管理错误
    #[error("集群管理错误: {message}")]
    ClusterManagement { message: String, cluster_id: String },

    /// 存储系统错误
    #[error("存储系统错误: {message}")]
    Storage { message: String, storage_type: String },

    /// 网络通信错误
    #[error("网络通信错误: {message}")]
    Network { message: String, endpoint: Option<String> },

    /// 配置错误
    #[error("配置错误: {message}")]
    Configuration { message: String },

    /// 资源不足错误
    #[error("资源不足: {message}")]
    ResourceExhausted { message: String, resource_type: String },

    /// 权限不足错误
    #[error("权限不足: {message}")]
    PermissionDenied { message: String, required_permission: String },

    /// 服务不可用错误
    #[error("服务不可用: {message}")]
    ServiceUnavailable { message: String, service_name: String },

    /// 超时错误
    #[error("操作超时: {message}")]
    Timeout { message: String, timeout_ms: u64 },

    /// 数据验证错误
    #[error("数据验证错误: {message}")]
    Validation { message: String, field: Option<String> },

    /// 序列化/反序列化错误
    #[error("序列化错误: {0}")]
    Serialization(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(String),

    /// 其他错误
    #[error("未知错误: {message}")]
    Other { message: String },
}

/// 安全级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecurityLevel {
    /// 低级别
    Low,
    /// 中级别
    Medium,
    /// 高级别
    High,
    /// 关键级别
    Critical,
}

impl fmt::Display for SecurityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecurityLevel::Low => write!(f, "低级别"),
            SecurityLevel::Medium => write!(f, "中级别"),
            SecurityLevel::High => write!(f, "高级别"),
            SecurityLevel::Critical => write!(f, "关键级别"),
        }
    }
}

/// DataFlare Enterprise 结果类型
pub type EnterpriseResult<T> = Result<T, EnterpriseError>;

impl EnterpriseError {
    /// 创建流处理错误
    pub fn stream_processing(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::StreamProcessing {
            message: message.into(),
            code: code.into(),
        }
    }

    /// 创建分布式计算错误
    pub fn distributed_computing(message: impl Into<String>, node_id: Option<String>) -> Self {
        Self::DistributedComputing {
            message: message.into(),
            node_id,
        }
    }

    /// 创建监控系统错误
    pub fn monitoring(message: impl Into<String>, component: impl Into<String>) -> Self {
        Self::Monitoring {
            message: message.into(),
            component: component.into(),
        }
    }

    /// 创建安全管理错误
    pub fn security(message: impl Into<String>, security_level: SecurityLevel) -> Self {
        Self::Security {
            message: message.into(),
            security_level,
        }
    }

    /// 创建合规检查错误
    pub fn compliance(message: impl Into<String>, requirement: impl Into<String>) -> Self {
        Self::Compliance {
            message: message.into(),
            requirement: requirement.into(),
        }
    }

    /// 创建插件市场错误
    pub fn marketplace(message: impl Into<String>, plugin_id: Option<String>) -> Self {
        Self::Marketplace {
            message: message.into(),
            plugin_id,
        }
    }

    /// 创建集群管理错误
    pub fn cluster_management(message: impl Into<String>, cluster_id: impl Into<String>) -> Self {
        Self::ClusterManagement {
            message: message.into(),
            cluster_id: cluster_id.into(),
        }
    }

    /// 创建存储系统错误
    pub fn storage(message: impl Into<String>, storage_type: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
            storage_type: storage_type.into(),
        }
    }

    /// 创建网络通信错误
    pub fn network(message: impl Into<String>, endpoint: Option<String>) -> Self {
        Self::Network {
            message: message.into(),
            endpoint,
        }
    }

    /// 创建配置错误
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// 创建资源不足错误
    pub fn resource_exhausted(message: impl Into<String>, resource_type: impl Into<String>) -> Self {
        Self::ResourceExhausted {
            message: message.into(),
            resource_type: resource_type.into(),
        }
    }

    /// 创建权限不足错误
    pub fn permission_denied(message: impl Into<String>, required_permission: impl Into<String>) -> Self {
        Self::PermissionDenied {
            message: message.into(),
            required_permission: required_permission.into(),
        }
    }

    /// 创建服务不可用错误
    pub fn service_unavailable(message: impl Into<String>, service_name: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            message: message.into(),
            service_name: service_name.into(),
        }
    }

    /// 创建超时错误
    pub fn timeout(message: impl Into<String>, timeout_ms: u64) -> Self {
        Self::Timeout {
            message: message.into(),
            timeout_ms,
        }
    }

    /// 创建数据验证错误
    pub fn validation(message: impl Into<String>, field: Option<String>) -> Self {
        Self::Validation {
            message: message.into(),
            field,
        }
    }

    /// 创建其他错误
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }

    /// 获取错误代码
    pub fn error_code(&self) -> &'static str {
        match self {
            EnterpriseError::StreamProcessing { .. } => "STREAM_PROCESSING",
            EnterpriseError::DistributedComputing { .. } => "DISTRIBUTED_COMPUTING",
            EnterpriseError::Monitoring { .. } => "MONITORING",
            EnterpriseError::Security { .. } => "SECURITY",
            EnterpriseError::Compliance { .. } => "COMPLIANCE",
            EnterpriseError::Marketplace { .. } => "MARKETPLACE",
            EnterpriseError::ClusterManagement { .. } => "CLUSTER_MANAGEMENT",
            EnterpriseError::Storage { .. } => "STORAGE",
            EnterpriseError::Network { .. } => "NETWORK",
            EnterpriseError::Configuration { .. } => "CONFIGURATION",
            EnterpriseError::ResourceExhausted { .. } => "RESOURCE_EXHAUSTED",
            EnterpriseError::PermissionDenied { .. } => "PERMISSION_DENIED",
            EnterpriseError::ServiceUnavailable { .. } => "SERVICE_UNAVAILABLE",
            EnterpriseError::Timeout { .. } => "TIMEOUT",
            EnterpriseError::Validation { .. } => "VALIDATION",
            EnterpriseError::Serialization(_) => "SERIALIZATION",
            EnterpriseError::Io(_) => "IO",
            EnterpriseError::Other { .. } => "OTHER",
        }
    }

    /// 判断是否为可重试错误
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            EnterpriseError::Network { .. }
                | EnterpriseError::ServiceUnavailable { .. }
                | EnterpriseError::Timeout { .. }
                | EnterpriseError::ResourceExhausted { .. }
        )
    }

    /// 判断是否为安全相关错误
    pub fn is_security_related(&self) -> bool {
        matches!(
            self,
            EnterpriseError::Security { .. }
                | EnterpriseError::PermissionDenied { .. }
                | EnterpriseError::Compliance { .. }
        )
    }
}

/// 将 EnterpriseError 转换为 DataFlareError
impl From<EnterpriseError> for dataflare_core::error::DataFlareError {
    fn from(err: EnterpriseError) -> Self {
        match err {
            EnterpriseError::StreamProcessing { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::DistributedComputing { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Monitoring { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Security { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Compliance { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Marketplace { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::ClusterManagement { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Storage { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Network { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Configuration { message } => dataflare_core::error::DataFlareError::Config(message),
            EnterpriseError::ResourceExhausted { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::PermissionDenied { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::ServiceUnavailable { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Timeout { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Validation { message, .. } => dataflare_core::error::DataFlareError::Processing(message),
            EnterpriseError::Serialization(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            EnterpriseError::Io(msg) => dataflare_core::error::DataFlareError::Processing(msg),
            EnterpriseError::Other { message } => dataflare_core::error::DataFlareError::Processing(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enterprise_error_creation() {
        let error = EnterpriseError::stream_processing("测试错误", "TEST_001");
        assert_eq!(error.error_code(), "STREAM_PROCESSING");
        assert!(!error.is_retryable());
        assert!(!error.is_security_related());
    }

    #[test]
    fn test_security_error() {
        let error = EnterpriseError::security("权限不足", SecurityLevel::High);
        assert!(error.is_security_related());
        assert_eq!(error.error_code(), "SECURITY");
    }

    #[test]
    fn test_retryable_error() {
        let error = EnterpriseError::network("网络超时", Some("http://example.com".to_string()));
        assert!(error.is_retryable());
    }

    #[test]
    fn test_error_conversion() {
        let enterprise_err = EnterpriseError::configuration("test config error");
        let dataflare_err: dataflare_core::error::DataFlareError = enterprise_err.into();

        match dataflare_err {
            dataflare_core::error::DataFlareError::Config(msg) => {
                assert_eq!(msg, "test config error");
            }
            _ => panic!("Expected Config error"),
        }
    }

    #[test]
    fn test_security_level_display() {
        assert_eq!(SecurityLevel::Low.to_string(), "低级别");
        assert_eq!(SecurityLevel::Medium.to_string(), "中级别");
        assert_eq!(SecurityLevel::High.to_string(), "高级别");
        assert_eq!(SecurityLevel::Critical.to_string(), "关键级别");
    }

    #[test]
    fn test_error_serialization() {
        let error = EnterpriseError::monitoring("监控错误", "stream_processor");
        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: EnterpriseError = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            EnterpriseError::Monitoring { message, component } => {
                assert_eq!(message, "监控错误");
                assert_eq!(component, "stream_processor");
            }
            _ => panic!("Expected Monitoring error"),
        }
    }
}
