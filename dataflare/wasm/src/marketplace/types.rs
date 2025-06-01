//! 插件市场类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// 插件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// 插件ID
    pub id: String,
    /// 插件名称
    pub name: String,
    /// 插件描述
    pub description: String,
    /// 作者信息
    pub author: String,
    /// 作者邮箱
    pub author_email: Option<String>,
    /// 主页URL
    pub homepage: Option<String>,
    /// 仓库URL
    pub repository: Option<String>,
    /// 文档URL
    pub documentation: Option<String>,
    /// 关键词
    pub keywords: Vec<String>,
    /// 分类
    pub categories: Vec<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 插件版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginVersion {
    /// 版本号
    pub version: String,
    /// 发布时间
    pub published_at: DateTime<Utc>,
    /// 下载URL
    pub download_url: String,
    /// 文件哈希
    pub checksum: String,
    /// 文件大小
    pub file_size: u64,
    /// 兼容性信息
    pub compatibility: CompatibilityInfo,
    /// 变更日志
    pub changelog: String,
    /// 是否为预发布版本
    pub prerelease: bool,
    /// 是否已弃用
    pub deprecated: bool,
}

/// 插件统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStats {
    /// 下载次数
    pub download_count: u64,
    /// 活跃安装数
    pub active_installs: u64,
    /// 评分
    pub rating: f64,
    /// 评论数
    pub review_count: u32,
    /// 最后更新时间
    pub last_updated: DateTime<Utc>,
    /// 最近30天下载量
    pub downloads_last_30_days: u64,
}

/// 兼容性信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityInfo {
    /// 最低DataFlare版本
    pub min_dataflare_version: String,
    /// 最高DataFlare版本
    pub max_dataflare_version: Option<String>,
    /// 支持的平台
    pub platforms: Vec<String>,
    /// 支持的架构
    pub architectures: Vec<String>,
}

/// 安全评级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityRating {
    Safe,       // 安全
    Low,        // 低风险
    Medium,     // 中等风险
    High,       // 高风险
    Critical,   // 严重风险
}

/// 许可证信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    /// 许可证类型
    pub license_type: String,
    /// 许可证文本
    pub license_text: Option<String>,
    /// 是否为开源许可证
    pub is_open_source: bool,
    /// 是否允许商业使用
    pub allows_commercial_use: bool,
}

/// 插件依赖
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    /// 依赖名称
    pub name: String,
    /// 版本要求
    pub version_requirement: String,
    /// 是否为可选依赖
    pub optional: bool,
    /// 依赖类型
    pub dependency_type: DependencyType,
}

/// 依赖类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    /// 运行时依赖
    Runtime,
    /// 开发依赖
    Development,
    /// 构建依赖
    Build,
}

/// 发布状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PublishStatus {
    /// 已发布
    Published,
    /// 草稿
    Draft,
    /// 已下架
    Unpublished,
    /// 已弃用
    Deprecated,
}

/// 搜索查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// 搜索关键词
    pub query: Option<String>,
    /// 分类过滤
    pub categories: Vec<String>,
    /// 标签过滤
    pub tags: Vec<String>,
    /// 作者过滤
    pub author: Option<String>,
    /// 许可证过滤
    pub license: Option<String>,
    /// 最低评分
    pub min_rating: Option<f64>,
    /// 排序方式
    pub sort_by: SortBy,
    /// 分页信息
    pub pagination: Pagination,
}

/// 排序方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortBy {
    Relevance,
    Downloads,
    Rating,
    Updated,
    Created,
    Name,
}

/// 分页信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// 页码（从0开始）
    pub page: u32,
    /// 每页大小
    pub page_size: u32,
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// 插件列表
    pub plugins: Vec<PluginSummary>,
    /// 总数量
    pub total_count: u64,
    /// 当前页
    pub current_page: u32,
    /// 总页数
    pub total_pages: u32,
    /// 搜索耗时（毫秒）
    pub search_time_ms: u64,
}

/// 插件摘要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSummary {
    /// 插件ID
    pub id: String,
    /// 插件名称
    pub name: String,
    /// 简短描述
    pub description: String,
    /// 作者
    pub author: String,
    /// 最新版本
    pub latest_version: String,
    /// 下载次数
    pub download_count: u64,
    /// 评分
    pub rating: f64,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 关键词
    pub keywords: Vec<String>,
    /// 安全评级
    pub security_rating: SecurityRating,
}

/// 插件详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDetails {
    /// 基本信息
    pub metadata: PluginMetadata,
    /// 最新版本
    pub latest_version: String,
    /// 所有版本
    pub versions: Vec<PluginVersion>,
    /// 统计信息
    pub stats: PluginStats,
    /// 质量评分
    pub quality_score: f64,
    /// 安全评级
    pub security_rating: SecurityRating,
    /// 许可证信息
    pub license: LicenseInfo,
    /// 依赖关系
    pub dependencies: Vec<PluginDependency>,
    /// README内容
    pub readme: Option<String>,
    /// 变更日志
    pub changelog: Option<String>,
}

/// 插件包
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPackage {
    /// 元数据
    pub metadata: PluginMetadata,
    /// 版本信息
    pub version: PluginVersion,
    /// WASM二进制文件
    pub wasm_binary: Vec<u8>,
    /// 配置文件
    pub config_file: Option<String>,
    /// README文件
    pub readme: Option<String>,
    /// 许可证文件
    pub license_file: Option<String>,
    /// 其他文件
    pub additional_files: HashMap<String, Vec<u8>>,
}

/// 认证令牌
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// 令牌值
    pub token: String,
    /// 用户ID
    pub user_id: String,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
}

/// 发布结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    /// 插件ID
    pub plugin_id: String,
    /// 版本号
    pub version: String,
    /// 发布时间
    pub published_at: DateTime<Utc>,
    /// 下载URL
    pub download_url: String,
}

/// 更新结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    /// 插件ID
    pub plugin_id: String,
    /// 新版本号
    pub new_version: String,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 插件评论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginReview {
    /// 评论ID
    pub id: String,
    /// 用户ID
    pub user_id: String,
    /// 用户名
    pub username: String,
    /// 评分 (1-5)
    pub rating: u8,
    /// 评论内容
    pub comment: String,
    /// 评论时间
    pub created_at: DateTime<Utc>,
    /// 是否已验证购买
    pub verified_purchase: bool,
}

/// 安全发现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityFinding {
    /// 危险的导入函数
    DangerousImport {
        module: String,
        function: String,
        severity: SecuritySeverity,
    },
    /// 意外的导出函数
    UnexpectedExport {
        function: String,
        severity: SecuritySeverity,
    },
    /// 依赖漏洞
    Vulnerability {
        dependency: String,
        version: String,
        cve_id: String,
        severity: SecuritySeverity,
        description: String,
    },
    /// 权限滥用
    PermissionAbuse {
        permission: String,
        reason: String,
        severity: SecuritySeverity,
    },
    /// 恶意代码
    MaliciousCode {
        pattern: String,
        location: String,
        severity: SecuritySeverity,
    },
}

/// 安全严重程度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// 质量建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityRecommendation {
    /// 建议类型
    pub recommendation_type: RecommendationType,
    /// 建议内容
    pub message: String,
    /// 优先级
    pub priority: Priority,
}

/// 建议类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    CodeQuality,
    Performance,
    Security,
    Documentation,
    Testing,
}

/// 优先级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// 推荐上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationContext {
    /// 用户当前项目类型
    pub project_type: Option<ProjectType>,
    /// 数据源类型
    pub data_sources: Vec<String>,
    /// 目标数据格式
    pub target_formats: Vec<String>,
    /// 性能要求
    pub performance_requirements: PerformanceRequirements,
    /// 预算限制
    pub budget_constraints: Option<BudgetConstraints>,
    /// 技术栈偏好
    pub tech_stack_preferences: Vec<String>,
}

/// 项目类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    DataPipeline,
    RealTimeAnalytics,
    BatchProcessing,
    MachineLearning,
    DataVisualization,
}

/// 性能要求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRequirements {
    /// 最大延迟（毫秒）
    pub max_latency_ms: Option<u64>,
    /// 最小吞吐量（记录/秒）
    pub min_throughput_rps: Option<u64>,
    /// 内存限制（MB）
    pub memory_limit_mb: Option<u64>,
}

/// 预算限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConstraints {
    /// 最大月费用
    pub max_monthly_cost: f64,
    /// 货币单位
    pub currency: String,
}

/// 插件推荐
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRecommendation {
    /// 插件ID
    pub plugin_id: String,
    /// 推荐分数 (0-1)
    pub score: f64,
    /// 推荐原因
    pub reason: RecommendationReason,
    /// 相似用户数量
    pub similar_users_count: u32,
    /// 预期收益
    pub expected_benefits: Vec<String>,
}

/// 推荐原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationReason {
    /// 相似用户也使用
    SimilarUsers,
    /// 内容相似
    ContentSimilarity,
    /// 功能互补
    Complementary,
    /// 工作流匹配
    WorkflowMatch,
    /// 性能优化
    PerformanceOptimization,
    /// 成本效益
    CostEffective,
}

/// 配置类型定义
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub storage_type: StorageType,
    pub connection_string: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone)]
pub enum StorageType {
    Memory,
    File,
    Database,
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub enable_vulnerability_scanning: bool,
    pub enable_malware_detection: bool,
    pub max_scan_time_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub max_results_per_page: u32,
    pub enable_fuzzy_search: bool,
    pub search_timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct RecommendationConfig {
    pub enable_collaborative_filtering: bool,
    pub enable_content_based: bool,
    pub max_recommendations: u32,
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub rate_limit_per_minute: u32,
    pub enable_authentication: bool,
    pub cors_origins: Vec<String>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: StorageType::Memory,
            connection_string: "memory://".to_string(),
            max_connections: 10,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_vulnerability_scanning: true,
            enable_malware_detection: true,
            max_scan_time_seconds: 300,
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results_per_page: 50,
            enable_fuzzy_search: true,
            search_timeout_ms: 5000,
        }
    }
}

impl Default for RecommendationConfig {
    fn default() -> Self {
        Self {
            enable_collaborative_filtering: true,
            enable_content_based: true,
            max_recommendations: 10,
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            rate_limit_per_minute: 100,
            enable_authentication: true,
            cors_origins: vec!["*".to_string()],
        }
    }
}
