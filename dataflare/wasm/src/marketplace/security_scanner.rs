//! 插件安全扫描器

use super::types::*;
use crate::{WasmError, WasmResult};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use log::{info, debug, warn, error};

/// 插件安全扫描器
pub struct PluginSecurityScanner {
    /// 漏洞数据库
    vulnerability_db: VulnerabilityDatabase,
    /// 恶意代码检测器
    malware_detector: MalwareDetector,
    /// 权限分析器
    permission_analyzer: PermissionAnalyzer,
    /// 配置
    config: SecurityConfig,
}

/// 漏洞数据库
#[derive(Debug, Clone, Default)]
pub struct VulnerabilityDatabase {
    /// 已知漏洞
    vulnerabilities: HashMap<String, Vec<Vulnerability>>,
}

/// 漏洞信息
#[derive(Debug, Clone)]
pub struct Vulnerability {
    /// CVE ID
    pub cve_id: String,
    /// 严重程度
    pub severity: SecuritySeverity,
    /// 描述
    pub description: String,
    /// 影响的版本范围
    pub affected_versions: String,
}

/// 恶意代码检测器
#[derive(Debug, Clone, Default)]
pub struct MalwareDetector {
    /// 恶意模式
    malicious_patterns: Vec<MaliciousPattern>,
}

/// 恶意模式
#[derive(Debug, Clone)]
pub struct MaliciousPattern {
    /// 模式名称
    pub name: String,
    /// 模式描述
    pub description: String,
    /// 检测规则
    pub rule: String,
    /// 严重程度
    pub severity: SecuritySeverity,
}

/// 权限分析器
#[derive(Debug, Clone, Default)]
pub struct PermissionAnalyzer {
    /// 危险权限列表
    dangerous_permissions: Vec<String>,
}

/// 安全扫描结果
#[derive(Debug, Clone)]
pub struct SecurityScanResult {
    /// 扫描时间
    pub scan_time: DateTime<Utc>,
    /// 总体安全评级
    pub security_rating: SecurityRating,
    /// 发现的安全问题
    pub findings: Vec<SecurityFinding>,
    /// 扫描统计
    pub stats: SecurityScanStats,
}

/// 安全扫描统计
#[derive(Debug, Clone)]
pub struct SecurityScanStats {
    /// 扫描耗时（毫秒）
    pub scan_duration_ms: u64,
    /// 扫描的文件数
    pub files_scanned: u32,
    /// 发现的问题数
    pub issues_found: u32,
    /// 高危问题数
    pub critical_issues: u32,
}

impl PluginSecurityScanner {
    /// 创建新的安全扫描器
    pub async fn new(config: SecurityConfig) -> WasmResult<Self> {
        let mut scanner = Self {
            vulnerability_db: VulnerabilityDatabase::default(),
            malware_detector: MalwareDetector::default(),
            permission_analyzer: PermissionAnalyzer::default(),
            config,
        };

        // 初始化扫描器
        scanner.initialize().await?;

        Ok(scanner)
    }

    /// 初始化扫描器
    async fn initialize(&mut self) -> WasmResult<()> {
        info!("初始化安全扫描器");

        // 加载漏洞数据库
        self.load_vulnerability_database().await?;

        // 加载恶意代码模式
        self.load_malware_patterns().await?;

        // 初始化权限分析器
        self.initialize_permission_analyzer().await?;

        info!("安全扫描器初始化完成");
        Ok(())
    }

    /// 执行完整安全扫描
    pub async fn scan_plugin(&self, plugin_package: &PluginPackage) -> WasmResult<SecurityScanResult> {
        let start_time = std::time::Instant::now();
        let scan_time = Utc::now();

        info!("开始安全扫描: {}", plugin_package.metadata.name);

        let mut findings = Vec::new();
        let mut files_scanned = 0;

        // 1. 静态代码分析
        if self.config.enable_vulnerability_scanning {
            let static_findings = self.analyze_static_code(&plugin_package.wasm_binary).await?;
            findings.extend(static_findings);
            files_scanned += 1;
        }

        // 2. 依赖漏洞扫描
        if self.config.enable_vulnerability_scanning {
            let dependency_findings = self.scan_dependencies(&plugin_package.metadata.id).await?;
            findings.extend(dependency_findings);
        }

        // 3. 权限审计
        let permission_findings = self.audit_permissions(&plugin_package.metadata).await?;
        findings.extend(permission_findings);

        // 4. 恶意代码检测
        if self.config.enable_malware_detection {
            let malware_findings = self.detect_malware(&plugin_package.wasm_binary).await?;
            findings.extend(malware_findings);
        }

        // 5. 网络行为分析
        let network_findings = self.analyze_network_behavior(&plugin_package.metadata).await?;
        findings.extend(network_findings);

        // 计算安全评级
        let security_rating = self.calculate_security_rating(&findings);

        // 统计信息
        let critical_issues = findings.iter()
            .filter(|f| matches!(self.get_finding_severity(f), SecuritySeverity::Critical))
            .count() as u32;

        let stats = SecurityScanStats {
            scan_duration_ms: start_time.elapsed().as_millis() as u64,
            files_scanned,
            issues_found: findings.len() as u32,
            critical_issues,
        };

        let result = SecurityScanResult {
            scan_time,
            security_rating,
            findings,
            stats,
        };

        info!("安全扫描完成: {} 个问题，评级: {:?}",
              result.stats.issues_found, result.security_rating);

        Ok(result)
    }

    /// 静态代码分析
    async fn analyze_static_code(&self, wasm_binary: &[u8]) -> WasmResult<Vec<SecurityFinding>> {
        let mut findings = Vec::new();

        // 简化实现：检查WASM二进制的基本结构
        if wasm_binary.len() < 8 {
            findings.push(SecurityFinding::MaliciousCode {
                pattern: "invalid_wasm_header".to_string(),
                location: "binary_header".to_string(),
                severity: SecuritySeverity::High,
            });
        }

        // 检查WASM魔数
        if wasm_binary.len() >= 4 {
            let magic = &wasm_binary[0..4];
            if magic != b"\0asm" {
                findings.push(SecurityFinding::MaliciousCode {
                    pattern: "invalid_wasm_magic".to_string(),
                    location: "binary_header".to_string(),
                    severity: SecuritySeverity::Critical,
                });
            }
        }

        // TODO: 实现更详细的WASM分析
        // - 检查导入/导出函数
        // - 分析内存使用模式
        // - 检查可疑的指令序列

        Ok(findings)
    }

    /// 依赖漏洞扫描
    async fn scan_dependencies(&self, plugin_id: &str) -> WasmResult<Vec<SecurityFinding>> {
        let mut findings = Vec::new();

        // 简化实现：检查已知的危险依赖
        let dangerous_deps = ["malicious-lib", "unsafe-crate"];

        for dep in &dangerous_deps {
            if plugin_id.contains(dep) {
                findings.push(SecurityFinding::Vulnerability {
                    dependency: dep.to_string(),
                    version: "unknown".to_string(),
                    cve_id: "CVE-2024-XXXX".to_string(),
                    severity: SecuritySeverity::High,
                    description: format!("检测到危险依赖: {}", dep),
                });
            }
        }

        Ok(findings)
    }

    /// 权限审计
    async fn audit_permissions(&self, metadata: &PluginMetadata) -> WasmResult<Vec<SecurityFinding>> {
        let mut findings = Vec::new();

        // 简化实现：检查插件名称中的可疑关键词
        let suspicious_keywords = ["admin", "root", "system", "kernel"];

        for keyword in &suspicious_keywords {
            if metadata.name.to_lowercase().contains(keyword) {
                findings.push(SecurityFinding::PermissionAbuse {
                    permission: keyword.to_string(),
                    reason: format!("插件名称包含可疑关键词: {}", keyword),
                    severity: SecuritySeverity::Medium,
                });
            }
        }

        Ok(findings)
    }

    /// 恶意代码检测
    async fn detect_malware(&self, wasm_binary: &[u8]) -> WasmResult<Vec<SecurityFinding>> {
        let mut findings = Vec::new();

        // 简化实现：检查二进制中的可疑模式
        for pattern in &self.malware_detector.malicious_patterns {
            if self.binary_contains_pattern(wasm_binary, &pattern.rule) {
                findings.push(SecurityFinding::MaliciousCode {
                    pattern: pattern.name.clone(),
                    location: "wasm_binary".to_string(),
                    severity: pattern.severity.clone(),
                });
            }
        }

        Ok(findings)
    }

    /// 网络行为分析
    async fn analyze_network_behavior(&self, metadata: &PluginMetadata) -> WasmResult<Vec<SecurityFinding>> {
        let mut findings = Vec::new();

        // 简化实现：检查描述中的网络相关关键词
        let network_keywords = ["http", "tcp", "udp", "socket", "network"];

        for keyword in &network_keywords {
            if metadata.description.to_lowercase().contains(keyword) {
                findings.push(SecurityFinding::PermissionAbuse {
                    permission: "network_access".to_string(),
                    reason: format!("插件可能进行网络访问: {}", keyword),
                    severity: SecuritySeverity::Low,
                });
                break; // 只报告一次
            }
        }

        Ok(findings)
    }

    /// 计算安全评级
    fn calculate_security_rating(&self, findings: &[SecurityFinding]) -> SecurityRating {
        let mut critical_count = 0;
        let mut high_count = 0;
        let mut medium_count = 0;
        let mut low_count = 0;

        for finding in findings {
            match self.get_finding_severity(finding) {
                SecuritySeverity::Critical => critical_count += 1,
                SecuritySeverity::High => high_count += 1,
                SecuritySeverity::Medium => medium_count += 1,
                SecuritySeverity::Low => low_count += 1,
            }
        }

        if critical_count > 0 {
            SecurityRating::Critical
        } else if high_count > 0 {
            SecurityRating::High
        } else if medium_count > 2 {
            SecurityRating::Medium
        } else if medium_count > 0 || low_count > 5 {
            SecurityRating::Low
        } else {
            SecurityRating::Safe
        }
    }

    /// 获取发现的严重程度
    fn get_finding_severity<'a>(&self, finding: &'a SecurityFinding) -> &'a SecuritySeverity {
        match finding {
            SecurityFinding::DangerousImport { severity, .. } => severity,
            SecurityFinding::UnexpectedExport { severity, .. } => severity,
            SecurityFinding::Vulnerability { severity, .. } => severity,
            SecurityFinding::PermissionAbuse { severity, .. } => severity,
            SecurityFinding::MaliciousCode { severity, .. } => severity,
        }
    }

    /// 检查二进制是否包含模式
    fn binary_contains_pattern(&self, binary: &[u8], pattern: &str) -> bool {
        // 简化实现：字符串搜索
        let binary_str = String::from_utf8_lossy(binary);
        binary_str.contains(pattern)
    }

    /// 加载漏洞数据库
    async fn load_vulnerability_database(&mut self) -> WasmResult<()> {
        debug!("加载漏洞数据库");

        // 简化实现：添加一些示例漏洞
        let vuln = Vulnerability {
            cve_id: "CVE-2024-0001".to_string(),
            severity: SecuritySeverity::High,
            description: "示例漏洞".to_string(),
            affected_versions: "*".to_string(),
        };

        self.vulnerability_db.vulnerabilities
            .entry("example-lib".to_string())
            .or_insert_with(Vec::new)
            .push(vuln);

        Ok(())
    }

    /// 加载恶意代码模式
    async fn load_malware_patterns(&mut self) -> WasmResult<()> {
        debug!("加载恶意代码模式");

        // 简化实现：添加一些示例模式
        self.malware_detector.malicious_patterns.push(MaliciousPattern {
            name: "suspicious_string".to_string(),
            description: "可疑字符串".to_string(),
            rule: "malware".to_string(),
            severity: SecuritySeverity::High,
        });

        Ok(())
    }

    /// 初始化权限分析器
    async fn initialize_permission_analyzer(&mut self) -> WasmResult<()> {
        debug!("初始化权限分析器");

        // 简化实现：添加危险权限列表
        self.permission_analyzer.dangerous_permissions = vec![
            "file_system_access".to_string(),
            "network_access".to_string(),
            "system_calls".to_string(),
        ];

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_security_scanner() {
        let config = SecurityConfig::default();
        let scanner = PluginSecurityScanner::new(config).await.unwrap();

        // 创建测试插件包
        let package = PluginPackage {
            metadata: PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test Plugin".to_string(),
                description: "A test plugin".to_string(),
                author: "Test Author".to_string(),
                author_email: None,
                homepage: None,
                repository: None,
                documentation: None,
                keywords: vec![],
                categories: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            version: PluginVersion {
                version: "1.0.0".to_string(),
                published_at: Utc::now(),
                download_url: "".to_string(),
                checksum: "".to_string(),
                file_size: 0,
                compatibility: CompatibilityInfo {
                    min_dataflare_version: "4.0.0".to_string(),
                    max_dataflare_version: None,
                    platforms: vec![],
                    architectures: vec![],
                },
                changelog: "".to_string(),
                prerelease: false,
                deprecated: false,
            },
            wasm_binary: b"\0asm\x01\x00\x00\x00".to_vec(), // 有效的WASM头
            config_file: None,
            readme: None,
            license_file: None,
            additional_files: std::collections::HashMap::new(),
        };

        // 执行安全扫描
        let result = scanner.scan_plugin(&package).await.unwrap();

        // 验证结果
        assert_eq!(result.security_rating, SecurityRating::Safe);
        assert_eq!(result.findings.len(), 0);
    }
}
