//! 网络通信模块 (占位符)

use serde::{Deserialize, Serialize};

/// 网络管理器 (占位符)
pub struct NetworkManager;

/// 网络配置 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub enabled: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// 网络指标 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}
