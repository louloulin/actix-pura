//! 插件质量评分器

use super::types::*;
use crate::{WasmError, WasmResult};
use std::collections::HashMap;
use log::{info, debug};

/// 插件质量评分器
pub struct PluginQualityScorer {
    /// 代码质量分析器
    code_analyzer: CodeQualityAnalyzer,
    /// 性能分析器
    performance_analyzer: PerformanceAnalyzer,
    /// 文档分析器
    documentation_analyzer: DocumentationAnalyzer,
    /// 测试覆盖率分析器
    test_coverage_analyzer: TestCoverageAnalyzer,
}

/// 代码质量分析器
#[derive(Debug, Clone, Default)]
pub struct CodeQualityAnalyzer;

/// 性能分析器
#[derive(Debug, Clone, Default)]
pub struct PerformanceAnalyzer;

/// 文档分析器
#[derive(Debug, Clone, Default)]
pub struct DocumentationAnalyzer;

/// 测试覆盖率分析器
#[derive(Debug, Clone, Default)]
pub struct TestCoverageAnalyzer;

/// 质量评分结果
#[derive(Debug, Clone)]
pub struct QualityScore {
    /// 总体评分 (0-100)
    pub overall_score: f64,
    /// 各组件评分
    pub component_scores: HashMap<String, ComponentScore>,
    /// 评分等级
    pub grade: QualityGrade,
    /// 改进建议
    pub recommendations: Vec<QualityRecommendation>,
}

/// 组件评分
#[derive(Debug, Clone)]
pub struct ComponentScore {
    /// 评分
    pub score: f64,
    /// 权重
    pub weight: f64,
    /// 详细信息
    pub details: Vec<String>,
}

/// 质量等级
#[derive(Debug, Clone)]
pub enum QualityGrade {
    Excellent,  // 90-100
    Good,       // 80-89
    Fair,       // 70-79
    Poor,       // 60-69
    Failing,    // 0-59
}

impl PluginQualityScorer {
    /// 创建新的质量评分器
    pub async fn new() -> WasmResult<Self> {
        Ok(Self {
            code_analyzer: CodeQualityAnalyzer::default(),
            performance_analyzer: PerformanceAnalyzer::default(),
            documentation_analyzer: DocumentationAnalyzer::default(),
            test_coverage_analyzer: TestCoverageAnalyzer::default(),
        })
    }

    /// 计算插件质量评分
    pub async fn calculate_quality_score(&self, plugin_package: &PluginPackage) -> WasmResult<QualityScore> {
        info!("计算插件质量评分: {}", plugin_package.metadata.name);

        let mut component_scores = HashMap::new();
        let mut total_weighted_score = 0.0;
        let mut total_weight = 0.0;

        // 1. 代码质量评分 (30%)
        let code_quality = self.analyze_code_quality(plugin_package).await?;
        let code_weight = 0.30;
        component_scores.insert("code_quality".to_string(), ComponentScore {
            score: code_quality,
            weight: code_weight,
            details: vec!["代码结构良好".to_string()],
        });
        total_weighted_score += code_quality * code_weight;
        total_weight += code_weight;

        // 2. 性能评分 (25%)
        let performance = self.analyze_performance(plugin_package).await?;
        let performance_weight = 0.25;
        component_scores.insert("performance".to_string(), ComponentScore {
            score: performance,
            weight: performance_weight,
            details: vec!["性能表现良好".to_string()],
        });
        total_weighted_score += performance * performance_weight;
        total_weight += performance_weight;

        // 3. 文档质量评分 (20%)
        let documentation = self.analyze_documentation(plugin_package).await?;
        let doc_weight = 0.20;
        component_scores.insert("documentation".to_string(), ComponentScore {
            score: documentation,
            weight: doc_weight,
            details: vec!["文档完整性良好".to_string()],
        });
        total_weighted_score += documentation * doc_weight;
        total_weight += doc_weight;

        // 4. 测试覆盖率评分 (15%)
        let test_coverage = self.analyze_test_coverage(plugin_package).await?;
        let test_weight = 0.15;
        component_scores.insert("test_coverage".to_string(), ComponentScore {
            score: test_coverage,
            weight: test_weight,
            details: vec!["测试覆盖率适中".to_string()],
        });
        total_weighted_score += test_coverage * test_weight;
        total_weight += test_weight;

        // 5. 安全性评分 (10%)
        let security = self.analyze_security(plugin_package).await?;
        let security_weight = 0.10;
        component_scores.insert("security".to_string(), ComponentScore {
            score: security,
            weight: security_weight,
            details: vec!["安全性良好".to_string()],
        });
        total_weighted_score += security * security_weight;
        total_weight += security_weight;

        // 计算总分
        let overall_score = if total_weight > 0.0 {
            total_weighted_score / total_weight
        } else {
            0.0
        };

        // 确定等级
        let grade = match overall_score {
            90.0..=100.0 => QualityGrade::Excellent,
            80.0..=89.9 => QualityGrade::Good,
            70.0..=79.9 => QualityGrade::Fair,
            60.0..=69.9 => QualityGrade::Poor,
            _ => QualityGrade::Failing,
        };

        // 生成建议
        let recommendations = self.generate_recommendations(&component_scores, overall_score);

        let quality_score = QualityScore {
            overall_score,
            component_scores,
            grade,
            recommendations,
        };

        info!("质量评分完成: {:.1}/100 ({:?})", overall_score, quality_score.grade);

        Ok(quality_score)
    }

    /// 代码质量分析
    async fn analyze_code_quality(&self, plugin_package: &PluginPackage) -> WasmResult<f64> {
        let mut score: f64 = 100.0;

        // 检查WASM二进制大小
        let binary_size = plugin_package.wasm_binary.len();
        if binary_size > 1024 * 1024 { // 1MB
            score -= 10.0; // 文件过大扣分
        }

        // 检查元数据完整性
        if plugin_package.metadata.description.len() < 50 {
            score -= 5.0; // 描述过短扣分
        }

        if plugin_package.metadata.keywords.is_empty() {
            score -= 5.0; // 缺少关键词扣分
        }

        // 检查版本信息
        if plugin_package.version.changelog.is_empty() {
            score -= 5.0; // 缺少变更日志扣分
        }

        Ok(score.max(0.0).min(100.0))
    }

    /// 性能分析
    async fn analyze_performance(&self, plugin_package: &PluginPackage) -> WasmResult<f64> {
        let mut score: f64 = 100.0;

        // 简化实现：基于文件大小评估性能
        let binary_size = plugin_package.wasm_binary.len();

        // 文件大小评分
        match binary_size {
            0..=102400 => {}, // 100KB以下，满分
            102401..=512000 => score -= 5.0, // 100KB-500KB，扣5分
            512001..=1048576 => score -= 15.0, // 500KB-1MB，扣15分
            _ => score -= 25.0, // 1MB以上，扣25分
        }

        // 检查是否有性能优化标记
        if plugin_package.metadata.keywords.contains(&"performance".to_string()) {
            score += 5.0; // 有性能标记加分
        }

        Ok(score.max(0.0).min(100.0))
    }

    /// 文档分析
    async fn analyze_documentation(&self, plugin_package: &PluginPackage) -> WasmResult<f64> {
        let mut score: f64 = 0.0;

        // 检查README
        if let Some(readme) = &plugin_package.readme {
            score += 40.0;
            if readme.len() > 500 {
                score += 10.0; // 详细README加分
            }
        }

        // 检查描述质量
        let desc_len = plugin_package.metadata.description.len();
        match desc_len {
            0..=20 => {}, // 描述过短，不加分
            21..=100 => score += 15.0,
            101..=300 => score += 25.0,
            _ => score += 30.0, // 详细描述加分
        }

        // 检查关键词
        if !plugin_package.metadata.keywords.is_empty() {
            score += 10.0;
        }

        // 检查分类
        if !plugin_package.metadata.categories.is_empty() {
            score += 10.0;
        }

        // 检查作者信息
        if plugin_package.metadata.author_email.is_some() {
            score += 5.0;
        }

        // 检查主页或仓库链接
        if plugin_package.metadata.homepage.is_some() || plugin_package.metadata.repository.is_some() {
            score += 10.0;
        }

        Ok(score.max(0.0).min(100.0))
    }

    /// 测试覆盖率分析
    async fn analyze_test_coverage(&self, plugin_package: &PluginPackage) -> WasmResult<f64> {
        let mut score: f64 = 50.0; // 基础分数

        // 简化实现：基于关键词判断是否有测试
        if plugin_package.metadata.keywords.contains(&"tested".to_string()) {
            score += 30.0;
        }

        if plugin_package.metadata.description.to_lowercase().contains("test") {
            score += 20.0;
        }

        Ok(score.max(0.0).min(100.0))
    }

    /// 安全性分析
    async fn analyze_security(&self, plugin_package: &PluginPackage) -> WasmResult<f64> {
        let mut score: f64 = 100.0;

        // 检查WASM二进制有效性
        if plugin_package.wasm_binary.len() < 8 {
            score -= 50.0; // 无效二进制严重扣分
        } else if plugin_package.wasm_binary.len() >= 4 {
            let magic = &plugin_package.wasm_binary[0..4];
            if magic != b"\0asm" {
                score -= 30.0; // 无效WASM魔数扣分
            }
        }

        // 检查许可证
        if plugin_package.license_file.is_some() {
            score += 5.0; // 有许可证文件加分
        }

        // 检查作者信息完整性
        if plugin_package.metadata.author_email.is_some() {
            score += 5.0; // 有作者邮箱加分
        }

        Ok(score.max(0.0).min(100.0))
    }

    /// 生成改进建议
    fn generate_recommendations(&self, component_scores: &HashMap<String, ComponentScore>, overall_score: f64) -> Vec<QualityRecommendation> {
        let mut recommendations = Vec::new();

        // 基于总分给出建议
        if overall_score < 70.0 {
            recommendations.push(QualityRecommendation {
                recommendation_type: RecommendationType::CodeQuality,
                message: "建议改进代码质量，添加更详细的文档和测试".to_string(),
                priority: Priority::High,
            });
        }

        // 基于各组件分数给出具体建议
        for (component, score) in component_scores {
            if score.score < 70.0 {
                let message = match component.as_str() {
                    "code_quality" => "建议优化代码结构，减少复杂度",
                    "performance" => "建议优化性能，减少资源使用",
                    "documentation" => "建议完善文档，添加使用示例",
                    "test_coverage" => "建议增加测试覆盖率",
                    "security" => "建议加强安全措施",
                    _ => "建议改进此组件",
                };

                recommendations.push(QualityRecommendation {
                    recommendation_type: self.get_recommendation_type(component),
                    message: message.to_string(),
                    priority: if score.score < 50.0 { Priority::High } else { Priority::Medium },
                });
            }
        }

        recommendations
    }

    /// 获取建议类型
    fn get_recommendation_type(&self, component: &str) -> RecommendationType {
        match component {
            "code_quality" => RecommendationType::CodeQuality,
            "performance" => RecommendationType::Performance,
            "documentation" => RecommendationType::Documentation,
            "test_coverage" => RecommendationType::Testing,
            "security" => RecommendationType::Security,
            _ => RecommendationType::CodeQuality,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_quality_scorer() {
        let scorer = PluginQualityScorer::new().await.unwrap();

        // 创建测试插件包
        let package = PluginPackage {
            metadata: PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test Plugin".to_string(),
                description: "A comprehensive test plugin with detailed description".to_string(),
                author: "Test Author".to_string(),
                author_email: Some("test@example.com".to_string()),
                homepage: Some("https://example.com".to_string()),
                repository: Some("https://github.com/test/plugin".to_string()),
                documentation: None,
                keywords: vec!["test".to_string(), "performance".to_string()],
                categories: vec!["testing".to_string()],
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
                changelog: "Initial release".to_string(),
                prerelease: false,
                deprecated: false,
            },
            wasm_binary: b"\0asm\x01\x00\x00\x00test_binary_content".to_vec(),
            config_file: None,
            readme: Some("# Test Plugin\n\nThis is a test plugin with comprehensive documentation.".to_string()),
            license_file: Some("MIT License".to_string()),
            additional_files: HashMap::new(),
        };

        // 计算质量评分
        let score = scorer.calculate_quality_score(&package).await.unwrap();

        // 验证结果
        println!("Overall score: {:.1}, Grade: {:?}", score.overall_score, score.grade);
        for (component, component_score) in &score.component_scores {
            println!("{}: {:.1}", component, component_score.score);
        }

        assert!(score.overall_score > 60.0); // 应该是合理的评分
        assert!(matches!(score.grade, QualityGrade::Fair | QualityGrade::Good | QualityGrade::Excellent));
        assert_eq!(score.component_scores.len(), 5); // 应该有5个组件评分
    }
}
