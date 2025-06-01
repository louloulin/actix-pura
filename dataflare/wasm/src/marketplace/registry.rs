//! 插件注册表实现

use super::types::*;
use super::storage::MarketplaceStorage;
use crate::{WasmError, WasmResult};
use std::collections::HashMap;
use std::sync::Arc;
use chrono::{Utc, Datelike, Timelike};
use log::{info, debug, warn, error};

/// 插件注册表
pub struct PluginRegistry {
    /// 存储后端
    storage: Arc<dyn MarketplaceStorage>,
    /// 内存缓存
    cache: HashMap<String, PluginRegistryEntry>,
    /// 统计信息
    stats: RegistryStats,
}

/// 注册表统计信息
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
    /// 插件总数
    pub total_plugins: u64,
    /// 总下载量
    pub total_downloads: u64,
    /// 活跃用户数
    pub active_users: u64,
    /// 本月新增插件数
    pub new_plugins_this_month: u64,
    /// 热门分类
    pub top_categories: Vec<(String, u64)>,
    /// 平均评分
    pub average_rating: f64,
}

/// 插件注册表条目
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginRegistryEntry {
    /// 插件基本信息
    pub metadata: PluginMetadata,
    /// 版本信息
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
    /// 发布状态
    pub status: PublishStatus,
}

impl PluginRegistry {
    /// 创建新的插件注册表
    pub async fn new(storage: Arc<dyn MarketplaceStorage>) -> WasmResult<Self> {
        Ok(Self {
            storage,
            cache: HashMap::new(),
            stats: RegistryStats::default(),
        })
    }

    /// 初始化注册表
    pub async fn initialize(&mut self) -> WasmResult<()> {
        info!("初始化插件注册表");

        // 从存储加载所有插件
        let plugins = self.storage.list_all_plugins().await?;

        for plugin in plugins {
            self.cache.insert(plugin.metadata.id.clone(), plugin);
        }

        // 更新统计信息
        self.update_stats().await?;

        info!("插件注册表初始化完成，加载了 {} 个插件", self.cache.len());
        Ok(())
    }

    /// 注册新插件
    pub async fn register_plugin(&mut self, entry: PluginRegistryEntry) -> WasmResult<()> {
        let plugin_id = entry.metadata.id.clone();

        info!("注册插件: {}", plugin_id);

        // 验证插件ID唯一性
        if self.cache.contains_key(&plugin_id) {
            return Err(WasmError::plugin_execution(
                format!("插件ID {} 已存在", plugin_id)
            ));
        }

        // 验证插件元数据
        self.validate_plugin_metadata(&entry.metadata)?;

        // 保存到存储
        self.storage.save_plugin(&entry).await?;

        // 更新缓存
        self.cache.insert(plugin_id.clone(), entry);

        // 更新统计信息
        self.update_stats().await?;

        info!("插件 {} 注册成功", plugin_id);
        Ok(())
    }

    /// 更新插件
    pub async fn update_plugin(&mut self, plugin_id: &str, entry: PluginRegistryEntry) -> WasmResult<()> {
        info!("更新插件: {}", plugin_id);

        // 检查插件是否存在
        if !self.cache.contains_key(plugin_id) {
            return Err(WasmError::plugin_execution(
                format!("插件 {} 不存在", plugin_id)
            ));
        }

        // 验证插件元数据
        self.validate_plugin_metadata(&entry.metadata)?;

        // 保存到存储
        self.storage.save_plugin(&entry).await?;

        // 更新缓存
        self.cache.insert(plugin_id.to_string(), entry);

        // 更新统计信息
        self.update_stats().await?;

        info!("插件 {} 更新成功", plugin_id);
        Ok(())
    }

    /// 删除插件
    pub async fn delete_plugin(&mut self, plugin_id: &str) -> WasmResult<()> {
        info!("删除插件: {}", plugin_id);

        // 检查插件是否存在
        if !self.cache.contains_key(plugin_id) {
            return Err(WasmError::plugin_execution(
                format!("插件 {} 不存在", plugin_id)
            ));
        }

        // 从存储删除
        self.storage.delete_plugin(plugin_id).await?;

        // 从缓存删除
        self.cache.remove(plugin_id);

        // 更新统计信息
        self.update_stats().await?;

        info!("插件 {} 删除成功", plugin_id);
        Ok(())
    }

    /// 获取插件详情
    pub async fn get_plugin(&self, plugin_id: &str) -> WasmResult<Option<PluginRegistryEntry>> {
        if let Some(entry) = self.cache.get(plugin_id) {
            Ok(Some(entry.clone()))
        } else {
            // 尝试从存储加载
            self.storage.get_plugin(plugin_id).await
        }
    }

    /// 列出所有插件
    pub fn list_plugins(&self) -> Vec<PluginSummary> {
        self.cache.values().map(|entry| {
            PluginSummary {
                id: entry.metadata.id.clone(),
                name: entry.metadata.name.clone(),
                description: entry.metadata.description.clone(),
                author: entry.metadata.author.clone(),
                latest_version: entry.versions.last()
                    .map(|v| v.version.clone())
                    .unwrap_or_else(|| "0.0.0".to_string()),
                download_count: entry.stats.download_count,
                rating: entry.stats.rating,
                updated_at: entry.metadata.updated_at,
                keywords: entry.metadata.keywords.clone(),
                security_rating: entry.security_rating.clone(),
            }
        }).collect()
    }

    /// 搜索插件
    pub async fn search_plugins(&self, query: &SearchQuery) -> WasmResult<Vec<PluginSummary>> {
        let mut results = self.list_plugins();

        // 关键词过滤
        if let Some(q) = &query.query {
            let q_lower = q.to_lowercase();
            results.retain(|plugin| {
                plugin.name.to_lowercase().contains(&q_lower) ||
                plugin.description.to_lowercase().contains(&q_lower) ||
                plugin.keywords.iter().any(|k| k.to_lowercase().contains(&q_lower))
            });
        }

        // 分类过滤
        if !query.categories.is_empty() {
            results.retain(|plugin| {
                if let Some(entry) = self.cache.get(&plugin.id) {
                    query.categories.iter().any(|cat|
                        entry.metadata.categories.contains(cat)
                    )
                } else {
                    false
                }
            });
        }

        // 作者过滤
        if let Some(author) = &query.author {
            results.retain(|plugin| plugin.author == *author);
        }

        // 评分过滤
        if let Some(min_rating) = query.min_rating {
            results.retain(|plugin| plugin.rating >= min_rating);
        }

        // 排序
        match query.sort_by {
            SortBy::Relevance => {
                // 简单的相关性排序（基于名称匹配）
                if let Some(q) = &query.query {
                    let q_lower = q.to_lowercase();
                    results.sort_by(|a, b| {
                        let a_score = if a.name.to_lowercase().contains(&q_lower) { 2 } else { 1 };
                        let b_score = if b.name.to_lowercase().contains(&q_lower) { 2 } else { 1 };
                        b_score.cmp(&a_score)
                    });
                }
            },
            SortBy::Downloads => {
                results.sort_by(|a, b| b.download_count.cmp(&a.download_count));
            },
            SortBy::Rating => {
                results.sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap_or(std::cmp::Ordering::Equal));
            },
            SortBy::Updated => {
                results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            },
            SortBy::Created => {
                results.sort_by(|a, b| {
                    if let (Some(entry_a), Some(entry_b)) = (self.cache.get(&a.id), self.cache.get(&b.id)) {
                        entry_b.metadata.created_at.cmp(&entry_a.metadata.created_at)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                });
            },
            SortBy::Name => {
                results.sort_by(|a, b| a.name.cmp(&b.name));
            },
        }

        // 分页
        let start = (query.pagination.page * query.pagination.page_size) as usize;
        let end = start + query.pagination.page_size as usize;

        if start < results.len() {
            results = results[start..end.min(results.len())].to_vec();
        } else {
            results.clear();
        }

        Ok(results)
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> WasmResult<&RegistryStats> {
        Ok(&self.stats)
    }

    /// 更新统计信息
    async fn update_stats(&mut self) -> WasmResult<()> {
        debug!("更新注册表统计信息");

        self.stats.total_plugins = self.cache.len() as u64;
        self.stats.total_downloads = self.cache.values()
            .map(|entry| entry.stats.download_count)
            .sum();

        // 计算平均评分
        let total_rating: f64 = self.cache.values()
            .map(|entry| entry.stats.rating)
            .sum();
        self.stats.average_rating = if self.stats.total_plugins > 0 {
            total_rating / self.stats.total_plugins as f64
        } else {
            0.0
        };

        // 统计分类
        let mut category_counts: HashMap<String, u64> = HashMap::new();
        for entry in self.cache.values() {
            for category in &entry.metadata.categories {
                *category_counts.entry(category.clone()).or_insert(0) += 1;
            }
        }

        // 获取前5个热门分类
        let mut categories: Vec<(String, u64)> = category_counts.into_iter().collect();
        categories.sort_by(|a, b| b.1.cmp(&a.1));
        self.stats.top_categories = categories.into_iter().take(5).collect();

        // 计算本月新增插件数
        let now = Utc::now();
        let month_start = now.with_day(1).unwrap().with_hour(0).unwrap()
            .with_minute(0).unwrap().with_second(0).unwrap();

        self.stats.new_plugins_this_month = self.cache.values()
            .filter(|entry| entry.metadata.created_at >= month_start)
            .count() as u64;

        debug!("统计信息更新完成");
        Ok(())
    }

    /// 验证插件元数据
    fn validate_plugin_metadata(&self, metadata: &PluginMetadata) -> WasmResult<()> {
        if metadata.name.is_empty() {
            return Err(WasmError::plugin_execution("插件名称不能为空"));
        }

        if metadata.description.is_empty() {
            return Err(WasmError::plugin_execution("插件描述不能为空"));
        }

        if metadata.author.is_empty() {
            return Err(WasmError::plugin_execution("插件作者不能为空"));
        }

        // 验证ID格式
        if !metadata.id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(WasmError::plugin_execution("插件ID只能包含字母、数字、连字符和下划线"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::storage::MemoryStorage;

    #[tokio::test]
    async fn test_plugin_registry() {
        let storage = Arc::new(MemoryStorage::new());
        let mut registry = PluginRegistry::new(storage).await.unwrap();

        // 创建测试插件
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            description: "A test plugin".to_string(),
            author: "Test Author".to_string(),
            author_email: Some("test@example.com".to_string()),
            homepage: None,
            repository: None,
            documentation: None,
            keywords: vec!["test".to_string()],
            categories: vec!["testing".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let entry = PluginRegistryEntry {
            metadata,
            versions: vec![],
            stats: PluginStats {
                download_count: 0,
                active_installs: 0,
                rating: 0.0,
                review_count: 0,
                last_updated: Utc::now(),
                downloads_last_30_days: 0,
            },
            quality_score: 0.0,
            security_rating: SecurityRating::Safe,
            license: LicenseInfo {
                license_type: "MIT".to_string(),
                license_text: None,
                is_open_source: true,
                allows_commercial_use: true,
            },
            dependencies: vec![],
            status: PublishStatus::Published,
        };

        // 测试注册插件
        registry.register_plugin(entry).await.unwrap();

        // 测试获取插件
        let retrieved = registry.get_plugin("test-plugin").await.unwrap();
        assert!(retrieved.is_some());

        // 测试列出插件
        let plugins = registry.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "Test Plugin");
    }
}
