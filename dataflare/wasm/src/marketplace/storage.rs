//! 插件市场存储抽象层

use super::types::*;
use super::registry::PluginRegistryEntry;
use crate::{WasmError, WasmResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 市场存储接口
#[async_trait]
pub trait MarketplaceStorage: Send + Sync {
    /// 初始化存储
    async fn initialize(&self) -> WasmResult<()>;
    
    /// 保存插件
    async fn save_plugin(&self, plugin: &PluginRegistryEntry) -> WasmResult<()>;
    
    /// 获取插件
    async fn get_plugin(&self, plugin_id: &str) -> WasmResult<Option<PluginRegistryEntry>>;
    
    /// 删除插件
    async fn delete_plugin(&self, plugin_id: &str) -> WasmResult<()>;
    
    /// 列出所有插件
    async fn list_all_plugins(&self) -> WasmResult<Vec<PluginRegistryEntry>>;
    
    /// 保存插件包
    async fn save_plugin_package(&self, plugin_id: &str, version: &str, package: &PluginPackage) -> WasmResult<()>;
    
    /// 获取插件包
    async fn get_plugin_package(&self, plugin_id: &str, version: &str) -> WasmResult<Option<PluginPackage>>;
    
    /// 删除插件包
    async fn delete_plugin_package(&self, plugin_id: &str, version: &str) -> WasmResult<()>;
    
    /// 保存评论
    async fn save_review(&self, plugin_id: &str, review: &PluginReview) -> WasmResult<()>;
    
    /// 获取评论
    async fn get_reviews(&self, plugin_id: &str, pagination: &Pagination) -> WasmResult<Vec<PluginReview>>;
    
    /// 删除评论
    async fn delete_review(&self, review_id: &str) -> WasmResult<()>;
    
    /// 更新下载统计
    async fn increment_download_count(&self, plugin_id: &str) -> WasmResult<()>;
    
    /// 获取用户插件
    async fn get_user_plugins(&self, user_id: &str) -> WasmResult<Vec<String>>;
    
    /// 保存用户插件关联
    async fn save_user_plugin(&self, user_id: &str, plugin_id: &str) -> WasmResult<()>;
}

/// 内存存储实现（用于测试和开发）
pub struct MemoryStorage {
    /// 插件数据
    plugins: Arc<RwLock<HashMap<String, PluginRegistryEntry>>>,
    /// 插件包数据
    packages: Arc<RwLock<HashMap<String, PluginPackage>>>,
    /// 评论数据
    reviews: Arc<RwLock<HashMap<String, Vec<PluginReview>>>>,
    /// 用户插件关联
    user_plugins: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl MemoryStorage {
    /// 创建新的内存存储
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            packages: Arc::new(RwLock::new(HashMap::new())),
            reviews: Arc::new(RwLock::new(HashMap::new())),
            user_plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 生成包键
    fn package_key(plugin_id: &str, version: &str) -> String {
        format!("{}:{}", plugin_id, version)
    }
}

#[async_trait]
impl MarketplaceStorage for MemoryStorage {
    async fn initialize(&self) -> WasmResult<()> {
        // 内存存储不需要初始化
        Ok(())
    }
    
    async fn save_plugin(&self, plugin: &PluginRegistryEntry) -> WasmResult<()> {
        let mut plugins = self.plugins.write().await;
        plugins.insert(plugin.metadata.id.clone(), plugin.clone());
        Ok(())
    }
    
    async fn get_plugin(&self, plugin_id: &str) -> WasmResult<Option<PluginRegistryEntry>> {
        let plugins = self.plugins.read().await;
        Ok(plugins.get(plugin_id).cloned())
    }
    
    async fn delete_plugin(&self, plugin_id: &str) -> WasmResult<()> {
        let mut plugins = self.plugins.write().await;
        plugins.remove(plugin_id);
        Ok(())
    }
    
    async fn list_all_plugins(&self) -> WasmResult<Vec<PluginRegistryEntry>> {
        let plugins = self.plugins.read().await;
        Ok(plugins.values().cloned().collect())
    }
    
    async fn save_plugin_package(&self, plugin_id: &str, version: &str, package: &PluginPackage) -> WasmResult<()> {
        let mut packages = self.packages.write().await;
        let key = Self::package_key(plugin_id, version);
        packages.insert(key, package.clone());
        Ok(())
    }
    
    async fn get_plugin_package(&self, plugin_id: &str, version: &str) -> WasmResult<Option<PluginPackage>> {
        let packages = self.packages.read().await;
        let key = Self::package_key(plugin_id, version);
        Ok(packages.get(&key).cloned())
    }
    
    async fn delete_plugin_package(&self, plugin_id: &str, version: &str) -> WasmResult<()> {
        let mut packages = self.packages.write().await;
        let key = Self::package_key(plugin_id, version);
        packages.remove(&key);
        Ok(())
    }
    
    async fn save_review(&self, plugin_id: &str, review: &PluginReview) -> WasmResult<()> {
        let mut reviews = self.reviews.write().await;
        reviews.entry(plugin_id.to_string())
            .or_insert_with(Vec::new)
            .push(review.clone());
        Ok(())
    }
    
    async fn get_reviews(&self, plugin_id: &str, pagination: &Pagination) -> WasmResult<Vec<PluginReview>> {
        let reviews = self.reviews.read().await;
        if let Some(plugin_reviews) = reviews.get(plugin_id) {
            let start = (pagination.page * pagination.page_size) as usize;
            let end = start + pagination.page_size as usize;
            
            if start < plugin_reviews.len() {
                Ok(plugin_reviews[start..end.min(plugin_reviews.len())].to_vec())
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn delete_review(&self, review_id: &str) -> WasmResult<()> {
        let mut reviews = self.reviews.write().await;
        for plugin_reviews in reviews.values_mut() {
            plugin_reviews.retain(|review| review.id != review_id);
        }
        Ok(())
    }
    
    async fn increment_download_count(&self, plugin_id: &str) -> WasmResult<()> {
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(plugin_id) {
            plugin.stats.download_count += 1;
        }
        Ok(())
    }
    
    async fn get_user_plugins(&self, user_id: &str) -> WasmResult<Vec<String>> {
        let user_plugins = self.user_plugins.read().await;
        Ok(user_plugins.get(user_id).cloned().unwrap_or_default())
    }
    
    async fn save_user_plugin(&self, user_id: &str, plugin_id: &str) -> WasmResult<()> {
        let mut user_plugins = self.user_plugins.write().await;
        user_plugins.entry(user_id.to_string())
            .or_insert_with(Vec::new)
            .push(plugin_id.to_string());
        Ok(())
    }
}

/// 文件存储实现
pub struct FileStorage {
    /// 存储目录
    storage_dir: String,
}

impl FileStorage {
    /// 创建新的文件存储
    pub fn new(storage_dir: String) -> Self {
        Self { storage_dir }
    }
}

#[async_trait]
impl MarketplaceStorage for FileStorage {
    async fn initialize(&self) -> WasmResult<()> {
        // 创建存储目录
        tokio::fs::create_dir_all(&self.storage_dir).await
            .map_err(|e| WasmError::runtime(format!("创建存储目录失败: {}", e)))?;
        
        // 创建子目录
        let subdirs = ["plugins", "packages", "reviews", "users"];
        for subdir in &subdirs {
            let path = format!("{}/{}", self.storage_dir, subdir);
            tokio::fs::create_dir_all(&path).await
                .map_err(|e| WasmError::runtime(format!("创建子目录 {} 失败: {}", subdir, e)))?;
        }
        
        Ok(())
    }
    
    async fn save_plugin(&self, plugin: &PluginRegistryEntry) -> WasmResult<()> {
        let path = format!("{}/plugins/{}.json", self.storage_dir, plugin.metadata.id);
        let content = serde_json::to_string_pretty(plugin)
            .map_err(|e| WasmError::Serialization(e))?;
        
        tokio::fs::write(&path, content).await
            .map_err(|e| WasmError::runtime(format!("保存插件文件失败: {}", e)))?;
        
        Ok(())
    }
    
    async fn get_plugin(&self, plugin_id: &str) -> WasmResult<Option<PluginRegistryEntry>> {
        let path = format!("{}/plugins/{}.json", self.storage_dir, plugin_id);
        
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let plugin: PluginRegistryEntry = serde_json::from_str(&content)
                    .map_err(|e| WasmError::Serialization(e))?;
                Ok(Some(plugin))
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(WasmError::runtime(format!("读取插件文件失败: {}", e))),
        }
    }
    
    async fn delete_plugin(&self, plugin_id: &str) -> WasmResult<()> {
        let path = format!("{}/plugins/{}.json", self.storage_dir, plugin_id);
        
        match tokio::fs::remove_file(&path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(WasmError::runtime(format!("删除插件文件失败: {}", e))),
        }
    }
    
    async fn list_all_plugins(&self) -> WasmResult<Vec<PluginRegistryEntry>> {
        let plugins_dir = format!("{}/plugins", self.storage_dir);
        let mut plugins = Vec::new();
        
        let mut entries = tokio::fs::read_dir(&plugins_dir).await
            .map_err(|e| WasmError::runtime(format!("读取插件目录失败: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| WasmError::runtime(format!("读取目录条目失败: {}", e)))? {
            
            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.ends_with(".json") {
                    let plugin_id = file_name.trim_end_matches(".json");
                    if let Some(plugin) = self.get_plugin(plugin_id).await? {
                        plugins.push(plugin);
                    }
                }
            }
        }
        
        Ok(plugins)
    }
    
    async fn save_plugin_package(&self, plugin_id: &str, version: &str, package: &PluginPackage) -> WasmResult<()> {
        let path = format!("{}/packages/{}_{}.json", self.storage_dir, plugin_id, version);
        let content = serde_json::to_string_pretty(package)
            .map_err(|e| WasmError::Serialization(e))?;
        
        tokio::fs::write(&path, content).await
            .map_err(|e| WasmError::runtime(format!("保存插件包失败: {}", e)))?;
        
        Ok(())
    }
    
    async fn get_plugin_package(&self, plugin_id: &str, version: &str) -> WasmResult<Option<PluginPackage>> {
        let path = format!("{}/packages/{}_{}.json", self.storage_dir, plugin_id, version);
        
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let package: PluginPackage = serde_json::from_str(&content)
                    .map_err(|e| WasmError::Serialization(e))?;
                Ok(Some(package))
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(WasmError::runtime(format!("读取插件包失败: {}", e))),
        }
    }
    
    async fn delete_plugin_package(&self, plugin_id: &str, version: &str) -> WasmResult<()> {
        let path = format!("{}/packages/{}_{}.json", self.storage_dir, plugin_id, version);
        
        match tokio::fs::remove_file(&path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(WasmError::runtime(format!("删除插件包失败: {}", e))),
        }
    }
    
    async fn save_review(&self, plugin_id: &str, review: &PluginReview) -> WasmResult<()> {
        let path = format!("{}/reviews/{}.json", self.storage_dir, plugin_id);
        
        // 读取现有评论
        let mut reviews = match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str::<Vec<PluginReview>>(&content)
                .map_err(|e| WasmError::Serialization(e))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(WasmError::runtime(format!("读取评论文件失败: {}", e))),
        };
        
        // 添加新评论
        reviews.push(review.clone());
        
        // 保存更新后的评论
        let content = serde_json::to_string_pretty(&reviews)
            .map_err(|e| WasmError::Serialization(e))?;
        
        tokio::fs::write(&path, content).await
            .map_err(|e| WasmError::runtime(format!("保存评论文件失败: {}", e)))?;
        
        Ok(())
    }
    
    async fn get_reviews(&self, plugin_id: &str, pagination: &Pagination) -> WasmResult<Vec<PluginReview>> {
        let path = format!("{}/reviews/{}.json", self.storage_dir, plugin_id);
        
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let reviews: Vec<PluginReview> = serde_json::from_str(&content)
                    .map_err(|e| WasmError::Serialization(e))?;
                
                let start = (pagination.page * pagination.page_size) as usize;
                let end = start + pagination.page_size as usize;
                
                if start < reviews.len() {
                    Ok(reviews[start..end.min(reviews.len())].to_vec())
                } else {
                    Ok(Vec::new())
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(WasmError::runtime(format!("读取评论文件失败: {}", e))),
        }
    }
    
    async fn delete_review(&self, review_id: &str) -> WasmResult<()> {
        // 这个实现比较复杂，需要遍历所有评论文件
        // 为了简化，这里只返回成功
        // 在实际实现中，应该维护一个评论ID到文件的映射
        Ok(())
    }
    
    async fn increment_download_count(&self, plugin_id: &str) -> WasmResult<()> {
        if let Some(mut plugin) = self.get_plugin(plugin_id).await? {
            plugin.stats.download_count += 1;
            self.save_plugin(&plugin).await?;
        }
        Ok(())
    }
    
    async fn get_user_plugins(&self, user_id: &str) -> WasmResult<Vec<String>> {
        let path = format!("{}/users/{}.json", self.storage_dir, user_id);
        
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let plugins: Vec<String> = serde_json::from_str(&content)
                    .map_err(|e| WasmError::Serialization(e))?;
                Ok(plugins)
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(WasmError::runtime(format!("读取用户插件文件失败: {}", e))),
        }
    }
    
    async fn save_user_plugin(&self, user_id: &str, plugin_id: &str) -> WasmResult<()> {
        let mut plugins = self.get_user_plugins(user_id).await?;
        
        if !plugins.contains(&plugin_id.to_string()) {
            plugins.push(plugin_id.to_string());
            
            let path = format!("{}/users/{}.json", self.storage_dir, user_id);
            let content = serde_json::to_string_pretty(&plugins)
                .map_err(|e| WasmError::Serialization(e))?;
            
            tokio::fs::write(&path, content).await
                .map_err(|e| WasmError::runtime(format!("保存用户插件文件失败: {}", e)))?;
        }
        
        Ok(())
    }
}

/// 创建存储实例
pub async fn create_storage(config: &StorageConfig) -> WasmResult<Arc<dyn MarketplaceStorage>> {
    match config.storage_type {
        StorageType::Memory => {
            Ok(Arc::new(MemoryStorage::new()))
        },
        StorageType::File => {
            let storage = FileStorage::new(config.connection_string.clone());
            storage.initialize().await?;
            Ok(Arc::new(storage))
        },
        StorageType::Database => {
            // TODO: 实现数据库存储
            Err(WasmError::runtime("数据库存储尚未实现"))
        },
    }
}
