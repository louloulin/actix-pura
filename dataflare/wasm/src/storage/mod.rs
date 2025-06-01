//! 存储模块 (占位符)

use serde::{Deserialize, Serialize};

/// 企业级存储 (占位符)
pub struct EnterpriseStorage;

/// 存储配置 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub storage_type: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self { storage_type: "memory".to_string() }
    }
}

/// 存储指标 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetrics {
    pub bytes_stored: u64,
}

impl Default for StorageMetrics {
    fn default() -> Self {
        Self { bytes_stored: 0 }
    }
}
