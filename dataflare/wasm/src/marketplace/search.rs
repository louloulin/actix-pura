//! 插件搜索引擎

use super::types::*;
use super::registry::PluginRegistryEntry;
use crate::{WasmError, WasmResult};
use std::collections::HashMap;
use std::time::Instant;

/// 插件搜索引擎
pub struct PluginSearchEngine {
    /// 搜索配置
    config: SearchConfig,
    /// 搜索索引
    index: SearchIndex,
}

/// 搜索索引
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    /// 名称索引
    name_index: HashMap<String, Vec<String>>,
    /// 描述索引
    description_index: HashMap<String, Vec<String>>,
    /// 关键词索引
    keyword_index: HashMap<String, Vec<String>>,
    /// 分类索引
    category_index: HashMap<String, Vec<String>>,
    /// 作者索引
    author_index: HashMap<String, Vec<String>>,
}

impl PluginSearchEngine {
    /// 创建新的搜索引擎
    pub async fn new(config: SearchConfig) -> WasmResult<Self> {
        Ok(Self {
            config,
            index: SearchIndex::default(),
        })
    }

    /// 初始化搜索引擎
    pub async fn initialize(&self) -> WasmResult<()> {
        log::info!("初始化插件搜索引擎");
        Ok(())
    }

    /// 构建搜索索引
    pub async fn build_index(&mut self, plugins: &[PluginRegistryEntry]) -> WasmResult<()> {
        log::info!("构建搜索索引，插件数量: {}", plugins.len());

        // 清空现有索引
        self.index = SearchIndex::default();

        for plugin in plugins {
            self.index_plugin(plugin)?;
        }

        log::info!("搜索索引构建完成");
        Ok(())
    }

    /// 添加插件到索引
    pub fn index_plugin(&mut self, plugin: &PluginRegistryEntry) -> WasmResult<()> {
        let plugin_id = plugin.metadata.id.clone();

        // 索引名称
        let name_terms = self.tokenize(&plugin.metadata.name);
        for term in name_terms {
            self.index.name_index.entry(term).or_insert_with(Vec::new).push(plugin_id.clone());
        }

        // 索引描述
        let desc_terms = self.tokenize(&plugin.metadata.description);
        for term in desc_terms {
            self.index.description_index.entry(term).or_insert_with(Vec::new).push(plugin_id.clone());
        }

        // 索引关键词
        for keyword in &plugin.metadata.keywords {
            let keyword_terms = self.tokenize(keyword);
            for term in keyword_terms {
                self.index.keyword_index.entry(term).or_insert_with(Vec::new).push(plugin_id.clone());
            }
        }

        // 索引分类
        for category in &plugin.metadata.categories {
            let category_terms = self.tokenize(category);
            for term in category_terms {
                self.index.category_index.entry(term).or_insert_with(Vec::new).push(plugin_id.clone());
            }
        }

        // 索引作者
        let author_terms = self.tokenize(&plugin.metadata.author);
        for term in author_terms {
            self.index.author_index.entry(term).or_insert_with(Vec::new).push(plugin_id.clone());
        }

        Ok(())
    }

    /// 从索引中移除插件
    pub fn remove_plugin_from_index(&mut self, plugin_id: &str) -> WasmResult<()> {
        self.remove_from_all_indices(plugin_id);
        Ok(())
    }

    /// 搜索插件
    pub async fn search(&self, query: &SearchQuery, plugins: &[PluginRegistryEntry]) -> WasmResult<SearchResult> {
        let start_time = Instant::now();

        let mut results = if let Some(q) = &query.query {
            self.search_by_query(q, plugins)?
        } else {
            // 如果没有查询词，返回所有插件
            plugins.iter().map(|p| (p.metadata.id.clone(), 1.0)).collect()
        };

        // 应用过滤器
        results = self.apply_filters(&results, query, plugins)?;

        // 排序
        results = self.sort_results(results, query, plugins)?;

        // 分页
        let total_count = results.len() as u64;
        let total_pages = (total_count + query.pagination.page_size as u64 - 1) / query.pagination.page_size as u64;

        let start = (query.pagination.page * query.pagination.page_size) as usize;
        let end = start + query.pagination.page_size as usize;

        let paged_results = if start < results.len() {
            results[start..end.min(results.len())].to_vec()
        } else {
            Vec::new()
        };

        // 转换为插件摘要
        let plugin_summaries = self.convert_to_summaries(&paged_results, plugins)?;

        let search_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(SearchResult {
            plugins: plugin_summaries,
            total_count,
            current_page: query.pagination.page,
            total_pages: total_pages as u32,
            search_time_ms,
        })
    }

    /// 根据查询词搜索
    fn search_by_query(&self, query: &str, plugins: &[PluginRegistryEntry]) -> WasmResult<Vec<(String, f64)>> {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scores: HashMap<String, f64> = HashMap::new();

        // 搜索名称
        for term in &query_terms {
            for (indexed_term, plugin_ids) in &self.index.name_index {
                if self.term_matches(indexed_term, term) {
                    for plugin_id in plugin_ids {
                        *scores.entry(plugin_id.clone()).or_insert(0.0) += 3.0; // 名称匹配权重最高
                    }
                }
            }
        }

        // 搜索描述
        for term in &query_terms {
            for (indexed_term, plugin_ids) in &self.index.description_index {
                if self.term_matches(indexed_term, term) {
                    for plugin_id in plugin_ids {
                        *scores.entry(plugin_id.clone()).or_insert(0.0) += 1.0; // 描述匹配权重中等
                    }
                }
            }
        }

        // 搜索关键词
        for term in &query_terms {
            for (indexed_term, plugin_ids) in &self.index.keyword_index {
                if self.term_matches(indexed_term, term) {
                    for plugin_id in plugin_ids {
                        *scores.entry(plugin_id.clone()).or_insert(0.0) += 2.0; // 关键词匹配权重较高
                    }
                }
            }
        }

        // 如果启用模糊搜索，进行额外的模糊匹配
        if self.config.enable_fuzzy_search {
            for plugin in plugins {
                let plugin_id = &plugin.metadata.id;
                if !scores.contains_key(plugin_id) {
                    let fuzzy_score = self.calculate_fuzzy_score(&query_lower, plugin);
                    if fuzzy_score > 0.3 {
                        scores.insert(plugin_id.clone(), fuzzy_score);
                    }
                }
            }
        }

        // 转换为排序的结果
        let mut results: Vec<(String, f64)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(results)
    }

    /// 应用过滤器
    fn apply_filters(
        &self,
        results: &[(String, f64)],
        query: &SearchQuery,
        plugins: &[PluginRegistryEntry],
    ) -> WasmResult<Vec<(String, f64)>> {
        let plugin_map: HashMap<String, &PluginRegistryEntry> = plugins
            .iter()
            .map(|p| (p.metadata.id.clone(), p))
            .collect();

        let filtered_results: Vec<(String, f64)> = results
            .iter()
            .filter(|(plugin_id, _score)| {
                if let Some(plugin) = plugin_map.get(plugin_id) {
                    // 分类过滤
                    if !query.categories.is_empty() {
                        if !query.categories.iter().any(|cat| plugin.metadata.categories.contains(cat)) {
                            return false;
                        }
                    }

                    // 标签过滤（使用关键词作为标签）
                    if !query.tags.is_empty() {
                        if !query.tags.iter().any(|tag| plugin.metadata.keywords.contains(tag)) {
                            return false;
                        }
                    }

                    // 作者过滤
                    if let Some(author) = &query.author {
                        if plugin.metadata.author != *author {
                            return false;
                        }
                    }

                    // 评分过滤
                    if let Some(min_rating) = query.min_rating {
                        if plugin.stats.rating < min_rating {
                            return false;
                        }
                    }

                    // 许可证过滤
                    if let Some(license) = &query.license {
                        if plugin.license.license_type != *license {
                            return false;
                        }
                    }

                    true
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        Ok(filtered_results)
    }

    /// 排序结果
    fn sort_results(
        &self,
        mut results: Vec<(String, f64)>,
        query: &SearchQuery,
        plugins: &[PluginRegistryEntry],
    ) -> WasmResult<Vec<(String, f64)>> {
        let plugin_map: HashMap<String, &PluginRegistryEntry> = plugins
            .iter()
            .map(|p| (p.metadata.id.clone(), p))
            .collect();

        match query.sort_by {
            SortBy::Relevance => {
                // 已经按相关性排序
            },
            SortBy::Downloads => {
                results.sort_by(|a, b| {
                    let a_downloads = plugin_map.get(&a.0).map(|p| p.stats.download_count).unwrap_or(0);
                    let b_downloads = plugin_map.get(&b.0).map(|p| p.stats.download_count).unwrap_or(0);
                    b_downloads.cmp(&a_downloads)
                });
            },
            SortBy::Rating => {
                results.sort_by(|a, b| {
                    let a_rating = plugin_map.get(&a.0).map(|p| p.stats.rating).unwrap_or(0.0);
                    let b_rating = plugin_map.get(&b.0).map(|p| p.stats.rating).unwrap_or(0.0);
                    b_rating.partial_cmp(&a_rating).unwrap_or(std::cmp::Ordering::Equal)
                });
            },
            SortBy::Updated => {
                results.sort_by(|a, b| {
                    let a_updated = plugin_map.get(&a.0).map(|p| p.metadata.updated_at).unwrap_or_default();
                    let b_updated = plugin_map.get(&b.0).map(|p| p.metadata.updated_at).unwrap_or_default();
                    b_updated.cmp(&a_updated)
                });
            },
            SortBy::Created => {
                results.sort_by(|a, b| {
                    let a_created = plugin_map.get(&a.0).map(|p| p.metadata.created_at).unwrap_or_default();
                    let b_created = plugin_map.get(&b.0).map(|p| p.metadata.created_at).unwrap_or_default();
                    b_created.cmp(&a_created)
                });
            },
            SortBy::Name => {
                results.sort_by(|a, b| {
                    let a_name = plugin_map.get(&a.0).map(|p| &p.metadata.name).unwrap_or(&a.0);
                    let b_name = plugin_map.get(&b.0).map(|p| &p.metadata.name).unwrap_or(&b.0);
                    a_name.cmp(b_name)
                });
            },
        }

        Ok(results)
    }

    /// 转换为插件摘要
    fn convert_to_summaries(
        &self,
        results: &[(String, f64)],
        plugins: &[PluginRegistryEntry],
    ) -> WasmResult<Vec<PluginSummary>> {
        let plugin_map: HashMap<String, &PluginRegistryEntry> = plugins
            .iter()
            .map(|p| (p.metadata.id.clone(), p))
            .collect();

        let summaries = results
            .iter()
            .filter_map(|(plugin_id, _score)| {
                plugin_map.get(plugin_id).map(|plugin| {
                    PluginSummary {
                        id: plugin.metadata.id.clone(),
                        name: plugin.metadata.name.clone(),
                        description: plugin.metadata.description.clone(),
                        author: plugin.metadata.author.clone(),
                        latest_version: plugin.versions.last()
                            .map(|v| v.version.clone())
                            .unwrap_or_else(|| "0.0.0".to_string()),
                        download_count: plugin.stats.download_count,
                        rating: plugin.stats.rating,
                        updated_at: plugin.metadata.updated_at,
                        keywords: plugin.metadata.keywords.clone(),
                        security_rating: plugin.security_rating.clone(),
                    }
                })
            })
            .collect();

        Ok(summaries)
    }

    /// 从所有索引中移除插件
    fn remove_from_all_indices(&mut self, plugin_id: &str) {
        // 从名称索引中移除
        for plugin_ids in self.index.name_index.values_mut() {
            plugin_ids.retain(|id| id != plugin_id);
        }
        self.index.name_index.retain(|_, plugin_ids| !plugin_ids.is_empty());

        // 从描述索引中移除
        for plugin_ids in self.index.description_index.values_mut() {
            plugin_ids.retain(|id| id != plugin_id);
        }
        self.index.description_index.retain(|_, plugin_ids| !plugin_ids.is_empty());

        // 从关键词索引中移除
        for plugin_ids in self.index.keyword_index.values_mut() {
            plugin_ids.retain(|id| id != plugin_id);
        }
        self.index.keyword_index.retain(|_, plugin_ids| !plugin_ids.is_empty());

        // 从分类索引中移除
        for plugin_ids in self.index.category_index.values_mut() {
            plugin_ids.retain(|id| id != plugin_id);
        }
        self.index.category_index.retain(|_, plugin_ids| !plugin_ids.is_empty());

        // 从作者索引中移除
        for plugin_ids in self.index.author_index.values_mut() {
            plugin_ids.retain(|id| id != plugin_id);
        }
        self.index.author_index.retain(|_, plugin_ids| !plugin_ids.is_empty());
    }

    /// 分词
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    /// 检查词项是否匹配
    fn term_matches(&self, indexed_term: &str, query_term: &str) -> bool {
        if self.config.enable_fuzzy_search {
            // 简单的模糊匹配：包含关系或编辑距离
            indexed_term.contains(query_term) ||
            query_term.contains(indexed_term) ||
            self.levenshtein_distance(indexed_term, query_term) <= 2
        } else {
            indexed_term.contains(query_term)
        }
    }

    /// 计算模糊评分
    fn calculate_fuzzy_score(&self, query: &str, plugin: &PluginRegistryEntry) -> f64 {
        let mut score = 0.0;

        // 名称模糊匹配
        let name_score = self.string_similarity(query, &plugin.metadata.name.to_lowercase());
        score += name_score * 3.0;

        // 描述模糊匹配
        let desc_score = self.string_similarity(query, &plugin.metadata.description.to_lowercase());
        score += desc_score * 1.0;

        // 关键词模糊匹配
        for keyword in &plugin.metadata.keywords {
            let keyword_score = self.string_similarity(query, &keyword.to_lowercase());
            score += keyword_score * 2.0;
        }

        score / 6.0 // 归一化
    }

    /// 计算字符串相似度
    fn string_similarity(&self, s1: &str, s2: &str) -> f64 {
        let distance = self.levenshtein_distance(s1, s2);
        let max_len = s1.len().max(s2.len());
        if max_len == 0 {
            1.0
        } else {
            1.0 - (distance as f64 / max_len as f64)
        }
    }

    /// 计算编辑距离
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let len1 = s1.len();
        let len2 = s2.len();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len1][len2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_search_engine() {
        let config = SearchConfig::default();
        let mut engine = PluginSearchEngine::new(config).await.unwrap();

        // 创建测试插件
        let plugin = create_test_plugin();
        let plugins = vec![plugin];

        // 构建索引
        engine.build_index(&plugins).await.unwrap();

        // 测试搜索
        let query = SearchQuery {
            query: Some("test".to_string()),
            categories: vec![],
            tags: vec![],
            author: None,
            license: None,
            min_rating: None,
            sort_by: SortBy::Relevance,
            pagination: Pagination { page: 0, page_size: 10 },
        };

        let result = engine.search(&query, &plugins).await.unwrap();
        assert_eq!(result.plugins.len(), 1);
        assert_eq!(result.plugins[0].name, "Test Plugin");
    }

    fn create_test_plugin() -> PluginRegistryEntry {
        use super::super::registry::PluginRegistryEntry;

        PluginRegistryEntry {
            metadata: PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test Plugin".to_string(),
                description: "A test plugin for testing".to_string(),
                author: "Test Author".to_string(),
                author_email: None,
                homepage: None,
                repository: None,
                documentation: None,
                keywords: vec!["test".to_string(), "plugin".to_string()],
                categories: vec!["testing".to_string()],
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            versions: vec![],
            stats: PluginStats {
                download_count: 100,
                active_installs: 50,
                rating: 4.5,
                review_count: 10,
                last_updated: Utc::now(),
                downloads_last_30_days: 20,
            },
            quality_score: 85.0,
            security_rating: SecurityRating::Safe,
            license: LicenseInfo {
                license_type: "MIT".to_string(),
                license_text: None,
                is_open_source: true,
                allows_commercial_use: true,
            },
            dependencies: vec![],
            status: PublishStatus::Published,
        }
    }
}
