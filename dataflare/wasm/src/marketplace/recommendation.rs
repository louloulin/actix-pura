//! 插件推荐引擎

use super::types::*;
use crate::{WasmError, WasmResult};
use std::collections::HashMap;
use log::{info, debug};

/// 插件推荐引擎
pub struct PluginRecommendationEngine {
    /// 协同过滤推荐器
    collaborative_filter: CollaborativeFilter,
    /// 内容推荐器
    content_recommender: ContentRecommender,
    /// 使用模式分析器
    usage_pattern_analyzer: UsagePatternAnalyzer,
    /// 配置
    config: RecommendationConfig,
}

/// 协同过滤推荐器
#[derive(Debug, Clone, Default)]
pub struct CollaborativeFilter {
    /// 用户-插件矩阵
    user_plugin_matrix: HashMap<String, Vec<String>>,
    /// 插件相似度矩阵
    plugin_similarity: HashMap<String, HashMap<String, f64>>,
}

/// 内容推荐器
#[derive(Debug, Clone, Default)]
pub struct ContentRecommender {
    /// 插件特征向量
    plugin_features: HashMap<String, Vec<f64>>,
    /// 分类权重
    category_weights: HashMap<String, f64>,
}

/// 使用模式分析器
#[derive(Debug, Clone, Default)]
pub struct UsagePatternAnalyzer {
    /// 使用模式
    usage_patterns: HashMap<String, UsagePattern>,
}

/// 使用模式
#[derive(Debug, Clone)]
pub struct UsagePattern {
    /// 常用插件组合
    pub common_combinations: Vec<Vec<String>>,
    /// 使用频率
    pub usage_frequency: HashMap<String, u64>,
    /// 时间模式
    pub time_patterns: HashMap<String, Vec<chrono::DateTime<chrono::Utc>>>,
}

impl PluginRecommendationEngine {
    /// 创建新的推荐引擎
    pub async fn new(config: RecommendationConfig) -> WasmResult<Self> {
        Ok(Self {
            collaborative_filter: CollaborativeFilter::default(),
            content_recommender: ContentRecommender::default(),
            usage_pattern_analyzer: UsagePatternAnalyzer::default(),
            config,
        })
    }

    /// 初始化推荐引擎
    pub async fn initialize(&self) -> WasmResult<()> {
        info!("初始化插件推荐引擎");
        Ok(())
    }

    /// 为用户推荐插件
    pub async fn recommend_plugins(&self, user_id: &str, context: RecommendationContext) -> WasmResult<Vec<PluginRecommendation>> {
        info!("为用户 {} 生成插件推荐", user_id);

        let mut recommendations = Vec::new();

        // 1. 基于协同过滤的推荐
        if self.config.enable_collaborative_filtering {
            let collaborative_recs = self.collaborative_filter.recommend(user_id, &context).await?;
            recommendations.extend(collaborative_recs);
        }

        // 2. 基于内容的推荐
        if self.config.enable_content_based {
            let content_recs = self.content_recommender.recommend(user_id, &context).await?;
            recommendations.extend(content_recs);
        }

        // 3. 基于使用模式的推荐
        let pattern_recs = self.usage_pattern_analyzer.recommend(user_id, &context).await?;
        recommendations.extend(pattern_recs);

        // 4. 合并和排序推荐结果
        let final_recommendations = self.merge_and_rank_recommendations(recommendations).await?;

        // 5. 限制推荐数量
        let limited_recommendations: Vec<PluginRecommendation> = final_recommendations
            .into_iter()
            .take(self.config.max_recommendations as usize)
            .collect();

        info!("生成了 {} 个推荐", limited_recommendations.len());
        Ok(limited_recommendations)
    }

    /// 推荐互补插件
    pub async fn recommend_complementary_plugins(&self, installed_plugins: &[String]) -> WasmResult<Vec<PluginRecommendation>> {
        let mut recommendations = Vec::new();

        for plugin_id in installed_plugins {
            // 查找与当前插件互补的插件
            let complementary = self.find_complementary_plugins(plugin_id).await?;
            recommendations.extend(complementary);
        }

        // 去重和排序
        let deduplicated = self.deduplicate_and_sort(recommendations).await?;
        Ok(deduplicated)
    }

    /// 推荐工作流模板
    pub async fn recommend_workflow_templates(&self, user_requirements: &WorkflowRequirements) -> WasmResult<Vec<WorkflowTemplate>> {
        // 基于用户需求推荐预构建的工作流模板
        self.match_workflow_templates(user_requirements).await
    }

    /// 合并和排序推荐结果
    async fn merge_and_rank_recommendations(&self, recommendations: Vec<PluginRecommendation>) -> WasmResult<Vec<PluginRecommendation>> {
        let mut plugin_scores: HashMap<String, (f64, PluginRecommendation)> = HashMap::new();

        // 合并相同插件的评分
        for rec in recommendations {
            let entry = plugin_scores.entry(rec.plugin_id.clone()).or_insert((0.0, rec.clone()));
            entry.0 = entry.0.max(rec.score); // 取最高分
            entry.1 = rec; // 更新推荐信息
        }

        // 排序
        let mut sorted_recommendations: Vec<PluginRecommendation> = plugin_scores
            .into_values()
            .map(|(_, rec)| rec)
            .collect();

        sorted_recommendations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(sorted_recommendations)
    }

    /// 查找互补插件
    async fn find_complementary_plugins(&self, plugin_id: &str) -> WasmResult<Vec<PluginRecommendation>> {
        let mut recommendations = Vec::new();

        // 简化实现：基于插件类型推荐互补插件
        let complementary_plugins = match plugin_id {
            id if id.contains("source") => vec!["processor-plugin", "destination-plugin"],
            id if id.contains("processor") => vec!["source-plugin", "destination-plugin"],
            id if id.contains("destination") => vec!["source-plugin", "processor-plugin"],
            _ => vec!["utility-plugin"],
        };

        for comp_plugin in complementary_plugins {
            recommendations.push(PluginRecommendation {
                plugin_id: comp_plugin.to_string(),
                score: 0.8,
                reason: RecommendationReason::Complementary,
                similar_users_count: 10,
                expected_benefits: vec!["完善工作流".to_string()],
            });
        }

        Ok(recommendations)
    }

    /// 去重和排序
    async fn deduplicate_and_sort(&self, recommendations: Vec<PluginRecommendation>) -> WasmResult<Vec<PluginRecommendation>> {
        let mut seen = std::collections::HashSet::new();
        let mut deduplicated = Vec::new();

        for rec in recommendations {
            if seen.insert(rec.plugin_id.clone()) {
                deduplicated.push(rec);
            }
        }

        // 按评分排序
        deduplicated.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(deduplicated)
    }

    /// 匹配工作流模板
    async fn match_workflow_templates(&self, requirements: &WorkflowRequirements) -> WasmResult<Vec<WorkflowTemplate>> {
        let mut templates = Vec::new();

        // 简化实现：基于项目类型推荐模板
        match requirements.project_type {
            Some(ProjectType::DataPipeline) => {
                templates.push(WorkflowTemplate {
                    id: "etl-pipeline".to_string(),
                    name: "ETL数据管道".to_string(),
                    description: "标准的ETL数据处理管道".to_string(),
                    components: vec!["csv-source".to_string(), "transform-processor".to_string(), "db-destination".to_string()],
                    estimated_setup_time: 30,
                });
            },
            Some(ProjectType::RealTimeAnalytics) => {
                templates.push(WorkflowTemplate {
                    id: "realtime-analytics".to_string(),
                    name: "实时分析".to_string(),
                    description: "实时数据分析工作流".to_string(),
                    components: vec!["stream-source".to_string(), "analytics-processor".to_string(), "dashboard-destination".to_string()],
                    estimated_setup_time: 45,
                });
            },
            _ => {
                templates.push(WorkflowTemplate {
                    id: "basic-workflow".to_string(),
                    name: "基础工作流".to_string(),
                    description: "通用的数据处理工作流".to_string(),
                    components: vec!["file-source".to_string(), "basic-processor".to_string(), "file-destination".to_string()],
                    estimated_setup_time: 15,
                });
            }
        }

        Ok(templates)
    }
}

impl CollaborativeFilter {
    /// 协同过滤推荐
    async fn recommend(&self, user_id: &str, context: &RecommendationContext) -> WasmResult<Vec<PluginRecommendation>> {
        let mut recommendations = Vec::new();

        // 简化实现：基于相似用户推荐
        if let Some(user_plugins) = self.user_plugin_matrix.get(user_id) {
            // 查找相似用户
            let similar_users = self.find_similar_users(user_id, user_plugins)?;

            // 推荐相似用户使用的插件
            for (similar_user, similarity) in similar_users {
                if let Some(similar_user_plugins) = self.user_plugin_matrix.get(&similar_user) {
                    for plugin_id in similar_user_plugins {
                        if !user_plugins.contains(plugin_id) {
                            recommendations.push(PluginRecommendation {
                                plugin_id: plugin_id.clone(),
                                score: similarity * 0.8,
                                reason: RecommendationReason::SimilarUsers,
                                similar_users_count: 1,
                                expected_benefits: vec!["相似用户推荐".to_string()],
                            });
                        }
                    }
                }
            }
        }

        Ok(recommendations)
    }

    /// 查找相似用户
    fn find_similar_users(&self, user_id: &str, user_plugins: &[String]) -> WasmResult<Vec<(String, f64)>> {
        let mut similar_users = Vec::new();

        for (other_user_id, other_plugins) in &self.user_plugin_matrix {
            if other_user_id != user_id {
                let similarity = self.calculate_user_similarity(user_plugins, other_plugins);
                if similarity > 0.5 {
                    similar_users.push((other_user_id.clone(), similarity));
                }
            }
        }

        // 按相似度排序
        similar_users.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(similar_users)
    }

    /// 计算用户相似度
    fn calculate_user_similarity(&self, plugins1: &[String], plugins2: &[String]) -> f64 {
        let set1: std::collections::HashSet<_> = plugins1.iter().collect();
        let set2: std::collections::HashSet<_> = plugins2.iter().collect();

        let intersection = set1.intersection(&set2).count();
        let union = set1.union(&set2).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }
}

impl ContentRecommender {
    /// 基于内容的推荐
    async fn recommend(&self, user_id: &str, context: &RecommendationContext) -> WasmResult<Vec<PluginRecommendation>> {
        let mut recommendations = Vec::new();

        // 简化实现：基于项目类型推荐
        if let Some(project_type) = &context.project_type {
            let recommended_plugins = match project_type {
                ProjectType::DataPipeline => vec!["etl-processor", "data-validator", "format-converter"],
                ProjectType::RealTimeAnalytics => vec!["stream-processor", "aggregator", "alert-manager"],
                ProjectType::BatchProcessing => vec!["batch-processor", "scheduler", "monitor"],
                ProjectType::MachineLearning => vec!["ml-processor", "feature-extractor", "model-deployer"],
                ProjectType::DataVisualization => vec!["chart-generator", "dashboard-builder", "report-generator"],
            };

            for plugin_id in recommended_plugins {
                recommendations.push(PluginRecommendation {
                    plugin_id: plugin_id.to_string(),
                    score: 0.7,
                    reason: RecommendationReason::ContentSimilarity,
                    similar_users_count: 5,
                    expected_benefits: vec!["适合项目类型".to_string()],
                });
            }
        }

        Ok(recommendations)
    }
}

impl UsagePatternAnalyzer {
    /// 基于使用模式的推荐
    async fn recommend(&self, user_id: &str, context: &RecommendationContext) -> WasmResult<Vec<PluginRecommendation>> {
        let mut recommendations = Vec::new();

        // 简化实现：基于数据源类型推荐
        for data_source in &context.data_sources {
            let recommended_plugins = match data_source.as_str() {
                "csv" => vec!["csv-parser", "data-cleaner"],
                "json" => vec!["json-processor", "schema-validator"],
                "database" => vec!["sql-connector", "query-optimizer"],
                "api" => vec!["http-client", "rate-limiter"],
                _ => vec!["universal-processor"],
            };

            for plugin_id in recommended_plugins {
                recommendations.push(PluginRecommendation {
                    plugin_id: plugin_id.to_string(),
                    score: 0.6,
                    reason: RecommendationReason::WorkflowMatch,
                    similar_users_count: 3,
                    expected_benefits: vec![format!("适合{}数据源", data_source)],
                });
            }
        }

        Ok(recommendations)
    }
}

/// 工作流需求
#[derive(Debug, Clone)]
pub struct WorkflowRequirements {
    /// 项目类型
    pub project_type: Option<ProjectType>,
    /// 数据源
    pub data_sources: Vec<String>,
    /// 目标格式
    pub target_formats: Vec<String>,
    /// 性能要求
    pub performance_requirements: PerformanceRequirements,
}

/// 工作流模板
#[derive(Debug, Clone)]
pub struct WorkflowTemplate {
    /// 模板ID
    pub id: String,
    /// 模板名称
    pub name: String,
    /// 模板描述
    pub description: String,
    /// 组件列表
    pub components: Vec<String>,
    /// 预估设置时间（分钟）
    pub estimated_setup_time: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_recommendation_engine() {
        let config = RecommendationConfig::default();
        let engine = PluginRecommendationEngine::new(config).await.unwrap();

        let context = RecommendationContext {
            project_type: Some(ProjectType::DataPipeline),
            data_sources: vec!["csv".to_string()],
            target_formats: vec!["json".to_string()],
            performance_requirements: PerformanceRequirements {
                max_latency_ms: Some(1000),
                min_throughput_rps: Some(100),
                memory_limit_mb: Some(512),
            },
            budget_constraints: None,
            tech_stack_preferences: vec![],
        };

        let recommendations = engine.recommend_plugins("test_user", context).await.unwrap();
        assert!(!recommendations.is_empty());
    }
}
