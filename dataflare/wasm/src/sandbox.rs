//! WASM沙箱安全模块

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{WasmError, WasmResult};

/// 安全策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    /// 是否允许网络访问
    pub allow_network: bool,
    /// 是否允许文件系统访问
    pub allow_filesystem: bool,
    /// 是否允许环境变量访问
    pub allow_env_vars: bool,
    /// 允许的主机函数列表
    pub allowed_host_functions: Vec<String>,
    /// 禁止的主机函数列表
    pub denied_host_functions: Vec<String>,
    /// 最大内存使用 (字节)
    pub max_memory_bytes: usize,
    /// 最大执行时间 (毫秒)
    pub max_execution_time_ms: u64,
    /// 是否启用调试模式
    pub debug_mode: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            allow_network: false,
            allow_filesystem: false,
            allow_env_vars: false,
            allowed_host_functions: vec![
                "log".to_string(),
                "get_time".to_string(),
            ],
            denied_host_functions: vec![
                "exec".to_string(),
                "spawn".to_string(),
                "exit".to_string(),
            ],
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            max_execution_time_ms: 5000, // 5秒
            debug_mode: false,
        }
    }
}

/// 沙箱配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// 安全策略
    pub security_policy: SecurityPolicy,
    /// 是否启用资源监控
    pub enable_resource_monitoring: bool,
    /// 是否启用调用追踪
    pub enable_call_tracing: bool,
    /// 审计日志配置
    pub audit_config: Option<AuditConfig>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            security_policy: SecurityPolicy::default(),
            enable_resource_monitoring: true,
            enable_call_tracing: false,
            audit_config: None,
        }
    }
}

/// 审计日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// 是否启用审计
    pub enabled: bool,
    /// 日志文件路径
    pub log_file: Option<String>,
    /// 是否记录函数调用
    pub log_function_calls: bool,
    /// 是否记录内存访问
    pub log_memory_access: bool,
    /// 是否记录网络访问
    pub log_network_access: bool,
}

/// WASM沙箱
pub struct WasmSandbox {
    /// 沙箱配置
    config: SandboxConfig,
    /// 资源使用统计
    resource_stats: ResourceStats,
    /// 审计日志记录器
    audit_logger: Option<AuditLogger>,
}

impl WasmSandbox {
    /// 创建新的沙箱
    pub fn new(config: SandboxConfig) -> WasmResult<Self> {
        let audit_logger = if let Some(ref audit_config) = config.audit_config {
            if audit_config.enabled {
                Some(AuditLogger::new(audit_config.clone())?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            config,
            resource_stats: ResourceStats::default(),
            audit_logger,
        })
    }

    /// 检查权限
    pub fn check_permission(&self, operation: &str) -> bool {
        let policy = &self.config.security_policy;

        match operation {
            "network" => policy.allow_network,
            "filesystem" => policy.allow_filesystem,
            "env_vars" => policy.allow_env_vars,
            func_name if func_name.starts_with("host_") => {
                let host_func = &func_name[5..]; // 移除 "host_" 前缀

                // 检查是否在禁止列表中
                if policy.denied_host_functions.contains(&host_func.to_string()) {
                    return false;
                }

                // 检查是否在允许列表中
                policy.allowed_host_functions.contains(&host_func.to_string())
            }
            _ => false,
        }
    }

    /// 检查内存限制
    pub fn check_memory_limit(&self, current_usage: usize) -> bool {
        current_usage <= self.config.security_policy.max_memory_bytes
    }

    /// 检查执行时间限制
    pub fn check_execution_time(&self, elapsed_ms: u64) -> bool {
        elapsed_ms <= self.config.security_policy.max_execution_time_ms
    }

    /// 记录资源使用
    pub fn record_resource_usage(&mut self, memory_usage: usize, execution_time_ms: u64) {
        if self.config.enable_resource_monitoring {
            self.resource_stats.update(memory_usage, execution_time_ms);
        }
    }

    /// 记录函数调用
    pub fn log_function_call(&mut self, function_name: &str, params: &HashMap<String, serde_json::Value>) {
        if let Some(ref mut logger) = self.audit_logger {
            if self.config.audit_config.as_ref().unwrap().log_function_calls {
                logger.log_function_call(function_name, params);
            }
        }
    }

    /// 记录内存访问
    pub fn log_memory_access(&mut self, operation: &str, address: usize, size: usize) {
        if let Some(ref mut logger) = self.audit_logger {
            if self.config.audit_config.as_ref().unwrap().log_memory_access {
                logger.log_memory_access(operation, address, size);
            }
        }
    }

    /// 获取资源统计
    pub fn get_resource_stats(&self) -> &ResourceStats {
        &self.resource_stats
    }

    /// 获取配置
    pub fn get_config(&self) -> &SandboxConfig {
        &self.config
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.resource_stats = ResourceStats::default();
    }
}

/// 资源使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStats {
    /// 当前内存使用 (字节)
    pub current_memory_usage: usize,
    /// 峰值内存使用 (字节)
    pub peak_memory_usage: usize,
    /// 总执行时间 (毫秒)
    pub total_execution_time_ms: u64,
    /// 函数调用次数
    pub function_call_count: u64,
    /// 内存访问次数
    pub memory_access_count: u64,
    /// 最后更新时间
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for ResourceStats {
    fn default() -> Self {
        Self {
            current_memory_usage: 0,
            peak_memory_usage: 0,
            total_execution_time_ms: 0,
            function_call_count: 0,
            memory_access_count: 0,
            last_updated: chrono::Utc::now(),
        }
    }
}

impl ResourceStats {
    /// 更新统计信息
    pub fn update(&mut self, memory_usage: usize, execution_time_ms: u64) {
        self.current_memory_usage = memory_usage;
        self.peak_memory_usage = self.peak_memory_usage.max(memory_usage);
        self.total_execution_time_ms += execution_time_ms;
        self.function_call_count += 1;
        self.last_updated = chrono::Utc::now();
    }

    /// 记录内存访问
    pub fn record_memory_access(&mut self) {
        self.memory_access_count += 1;
        self.last_updated = chrono::Utc::now();
    }
}

/// 审计日志记录器
pub struct AuditLogger {
    /// 审计配置
    config: AuditConfig,
    /// 日志条目
    log_entries: Vec<AuditLogEntry>,
}

impl AuditLogger {
    /// 创建新的审计记录器
    pub fn new(config: AuditConfig) -> WasmResult<Self> {
        Ok(Self {
            config,
            log_entries: Vec::new(),
        })
    }

    /// 记录函数调用
    pub fn log_function_call(&mut self, function_name: &str, params: &HashMap<String, serde_json::Value>) {
        let entry = AuditLogEntry {
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::FunctionCall,
            details: serde_json::json!({
                "function_name": function_name,
                "parameters": params
            }),
        };

        self.log_entries.push(entry);
        self.flush_if_needed();
    }

    /// 记录内存访问
    pub fn log_memory_access(&mut self, operation: &str, address: usize, size: usize) {
        let entry = AuditLogEntry {
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::MemoryAccess,
            details: serde_json::json!({
                "operation": operation,
                "address": address,
                "size": size
            }),
        };

        self.log_entries.push(entry);
        self.flush_if_needed();
    }

    /// 如果需要则刷新日志
    fn flush_if_needed(&mut self) {
        if self.log_entries.len() >= 100 {
            self.flush();
        }
    }

    /// 刷新日志到文件
    fn flush(&mut self) {
        if let Some(ref log_file) = self.config.log_file {
            let log_path = std::path::Path::new(log_file);
            match self.write_logs_to_file(log_path) {
                Ok(_) => log::debug!("审计日志已写入文件: {} 条记录", self.log_entries.len()),
                Err(e) => log::error!("写入审计日志失败: {}", e),
            }
        }
        self.log_entries.clear();
    }

    /// 将日志写入文件
    fn write_logs_to_file(&self, log_file: &std::path::Path) -> std::io::Result<()> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;

        for entry in &self.log_entries {
            let log_line = format!(
                "{} [{}] {}\n",
                entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
                entry.event_type.as_str(),
                serde_json::to_string(&entry.details).unwrap_or_else(|_| "{}".to_string())
            );
            file.write_all(log_line.as_bytes())?;
        }

        file.flush()?;
        Ok(())
    }

    /// 获取日志条目
    pub fn get_log_entries(&self) -> &[AuditLogEntry] {
        &self.log_entries
    }
}

/// 审计日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 事件类型
    pub event_type: AuditEventType,
    /// 事件详情
    pub details: serde_json::Value,
}

/// 审计事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    /// 函数调用
    FunctionCall,
    /// 内存访问
    MemoryAccess,
    /// 网络访问
    NetworkAccess,
    /// 文件系统访问
    FilesystemAccess,
    /// 权限检查
    PermissionCheck,
    /// 资源限制违规
    ResourceViolation,
}

impl AuditEventType {
    /// 获取事件类型的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditEventType::FunctionCall => "FUNCTION_CALL",
            AuditEventType::MemoryAccess => "MEMORY_ACCESS",
            AuditEventType::NetworkAccess => "NETWORK_ACCESS",
            AuditEventType::FilesystemAccess => "FILESYSTEM_ACCESS",
            AuditEventType::PermissionCheck => "PERMISSION_CHECK",
            AuditEventType::ResourceViolation => "RESOURCE_VIOLATION",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_policy_default() {
        let policy = SecurityPolicy::default();
        assert!(!policy.allow_network);
        assert!(!policy.allow_filesystem);
        assert!(!policy.allow_env_vars);
        assert!(policy.allowed_host_functions.contains(&"log".to_string()));
        assert!(policy.denied_host_functions.contains(&"exec".to_string()));
    }

    #[test]
    fn test_sandbox_permission_check() {
        let config = SandboxConfig::default();
        let sandbox = WasmSandbox::new(config).unwrap();

        assert!(!sandbox.check_permission("network"));
        assert!(!sandbox.check_permission("filesystem"));
        assert!(sandbox.check_permission("host_log"));
        assert!(!sandbox.check_permission("host_exec"));
    }

    #[test]
    fn test_resource_limits() {
        let config = SandboxConfig::default();
        let sandbox = WasmSandbox::new(config).unwrap();

        assert!(sandbox.check_memory_limit(32 * 1024 * 1024)); // 32MB < 64MB
        assert!(!sandbox.check_memory_limit(128 * 1024 * 1024)); // 128MB > 64MB

        assert!(sandbox.check_execution_time(3000)); // 3s < 5s
        assert!(!sandbox.check_execution_time(10000)); // 10s > 5s
    }

    #[test]
    fn test_resource_stats() {
        let mut stats = ResourceStats::default();

        stats.update(1024, 100);
        assert_eq!(stats.current_memory_usage, 1024);
        assert_eq!(stats.peak_memory_usage, 1024);
        assert_eq!(stats.total_execution_time_ms, 100);
        assert_eq!(stats.function_call_count, 1);

        stats.update(2048, 200);
        assert_eq!(stats.current_memory_usage, 2048);
        assert_eq!(stats.peak_memory_usage, 2048);
        assert_eq!(stats.total_execution_time_ms, 300);
        assert_eq!(stats.function_call_count, 2);
    }

    #[test]
    fn test_audit_logger() {
        let config = AuditConfig {
            enabled: true,
            log_file: None,
            log_function_calls: true,
            log_memory_access: true,
            log_network_access: false,
        };

        let mut logger = AuditLogger::new(config).unwrap();

        let params = HashMap::new();
        logger.log_function_call("test_function", &params);
        logger.log_memory_access("read", 0x1000, 256);

        assert_eq!(logger.get_log_entries().len(), 2);
    }
}
