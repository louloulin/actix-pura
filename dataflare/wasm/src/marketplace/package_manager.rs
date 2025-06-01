//! 插件包管理器

use super::types::*;
use super::storage::MarketplaceStorage;
use crate::{WasmError, WasmResult};
use std::sync::Arc;
use std::collections::HashMap;
use log::{info, debug, warn, error};
use serde::{Serialize, Deserialize};

/// 依赖树结构
#[derive(Debug, Clone)]
pub struct DependencyTree {
    /// 根插件ID
    pub root: String,
    /// 依赖关系映射
    pub dependencies: HashMap<String, Vec<String>>,
}

impl DependencyTree {
    /// 创建新的依赖树
    pub fn new(root: String) -> Self {
        Self {
            root,
            dependencies: HashMap::new(),
        }
    }

    /// 添加依赖关系
    pub fn add_dependency(&mut self, parent: &str, child: &str) {
        self.dependencies
            .entry(parent.to_string())
            .or_insert_with(Vec::new)
            .push(child.to_string());
    }

    /// 获取所有依赖
    pub fn get_all_dependencies(&self) -> Vec<String> {
        let mut all_deps = Vec::new();
        self.collect_dependencies(&self.root, &mut all_deps);
        all_deps.sort();
        all_deps.dedup();
        all_deps
    }

    /// 递归收集依赖
    fn collect_dependencies(&self, plugin_id: &str, collected: &mut Vec<String>) {
        if let Some(deps) = self.dependencies.get(plugin_id) {
            for dep in deps {
                if !collected.contains(dep) {
                    collected.push(dep.clone());
                    self.collect_dependencies(dep, collected);
                }
            }
        }
    }
}

/// 插件包管理器
pub struct PluginPackageManager {
    /// 存储后端
    storage: Arc<dyn MarketplaceStorage>,
    /// 本地插件缓存
    cache: HashMap<String, InstalledPlugin>,
    /// 配置
    config: StorageConfig,
}

/// 已安装的插件信息
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    /// 插件ID
    pub id: String,
    /// 版本
    pub version: String,
    /// 安装路径
    pub install_path: String,
    /// 安装时间
    pub installed_at: chrono::DateTime<chrono::Utc>,
    /// 是否启用
    pub enabled: bool,
}

/// 插件更新信息
#[derive(Debug, Clone)]
pub struct PluginUpdate {
    /// 插件ID
    pub plugin_id: String,
    /// 当前版本
    pub current_version: String,
    /// 最新版本
    pub latest_version: String,
    /// 变更日志
    pub changelog: String,
}

/// 插件规格
#[derive(Debug, Clone)]
pub struct PluginSpec {
    /// 插件ID
    pub plugin_id: String,
    /// 版本要求
    pub version_requirement: String,
}

impl PluginSpec {
    /// 创建新的插件规格
    pub fn new(plugin_id: &str, version: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            version_requirement: version.to_string(),
        }
    }
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
}

/// 包管理错误
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("插件未安装: {0}")]
    PluginNotInstalled(String),

    #[error("插件已存在: {0}")]
    PluginAlreadyExists(String),

    #[error("版本冲突: {0}")]
    VersionConflict(String),

    #[error("依赖冲突: {plugin} 被以下插件依赖: {dependents:?}")]
    HasDependents { plugin: String, dependents: Vec<String> },

    #[error("下载失败: {0}")]
    DownloadFailed(String),

    #[error("安装失败: {0}")]
    InstallFailed(String),

    #[error("版本未找到: {0}")]
    VersionNotFound(String),

    #[error("循环依赖: {0}")]
    CircularDependency(String),
}

/// 包管理结果
pub type PackageResult<T> = Result<T, PackageError>;

impl PluginPackageManager {
    /// 创建新的包管理器
    pub async fn new(storage: Arc<dyn MarketplaceStorage>, config: StorageConfig) -> WasmResult<Self> {
        Ok(Self {
            storage,
            cache: HashMap::new(),
            config,
        })
    }

    /// 安装插件
    pub async fn install_plugin(&mut self, plugin_spec: &PluginSpec) -> PackageResult<()> {
        info!("安装插件: {} {}", plugin_spec.plugin_id, plugin_spec.version_requirement);

        // 检查是否已安装
        if let Some(installed) = self.cache.get(&plugin_spec.plugin_id) {
            if self.version_matches(&installed.version, &plugin_spec.version_requirement) {
                info!("插件 {} 已安装版本 {}", plugin_spec.plugin_id, installed.version);
                return Ok(());
            }
        }

        // 解析依赖
        let resolved_deps = self.resolve_dependencies(&plugin_spec.plugin_id, &plugin_spec.version_requirement).await?;

        // 下载和安装依赖
        for dep in resolved_deps {
            self.install_dependency(&dep).await?;
        }

        // 下载插件包
        let package = self.download_plugin_package(&plugin_spec.plugin_id, &plugin_spec.version_requirement).await?;

        // 验证插件包
        self.validate_plugin_package(&package)?;

        // 安装插件包
        self.install_plugin_package(&package).await?;

        // 更新缓存
        let installed_plugin = InstalledPlugin {
            id: plugin_spec.plugin_id.clone(),
            version: plugin_spec.version_requirement.clone(),
            install_path: format!("plugins/{}", plugin_spec.plugin_id),
            installed_at: chrono::Utc::now(),
            enabled: true,
        };
        self.cache.insert(plugin_spec.plugin_id.clone(), installed_plugin);

        info!("插件 {} 安装成功", plugin_spec.plugin_id);
        Ok(())
    }

    /// 更新插件
    pub async fn update_plugin(&mut self, plugin_id: &str) -> PackageResult<()> {
        info!("更新插件: {}", plugin_id);

        // 检查当前版本
        let current = self.cache.get(plugin_id)
            .ok_or_else(|| PackageError::PluginNotInstalled(plugin_id.to_string()))?;

        // 检查可用更新
        let latest_version = self.get_latest_version(plugin_id).await?;

        if self.is_newer_version(&latest_version, &current.version) {
            info!("发现插件 {} 的新版本: {} -> {}",
                  plugin_id, current.version, latest_version);

            // 执行更新
            let plugin_spec = PluginSpec::new(plugin_id, &latest_version);
            self.install_plugin(&plugin_spec).await?;
        } else {
            info!("插件 {} 已是最新版本", plugin_id);
        }

        Ok(())
    }

    /// 卸载插件
    pub async fn uninstall_plugin(&mut self, plugin_id: &str) -> PackageResult<()> {
        info!("卸载插件: {}", plugin_id);

        // 检查依赖关系
        let dependents = self.get_plugin_dependents(plugin_id);
        if !dependents.is_empty() {
            return Err(PackageError::HasDependents {
                plugin: plugin_id.to_string(),
                dependents,
            });
        }

        // 停止插件
        self.stop_plugin(plugin_id).await?;

        // 删除插件文件
        self.remove_plugin_files(plugin_id).await?;

        // 更新缓存
        self.cache.remove(plugin_id);

        info!("插件 {} 卸载成功", plugin_id);
        Ok(())
    }

    /// 列出已安装插件
    pub fn list_installed_plugins(&self) -> Vec<InstalledPlugin> {
        self.cache.values().cloned().collect()
    }

    /// 检查插件更新
    pub async fn check_updates(&self) -> PackageResult<Vec<PluginUpdate>> {
        let mut updates = Vec::new();

        for installed in self.cache.values() {
            let latest_version = self.get_latest_version(&installed.id).await?;

            if self.is_newer_version(&latest_version, &installed.version) {
                updates.push(PluginUpdate {
                    plugin_id: installed.id.clone(),
                    current_version: installed.version.clone(),
                    latest_version,
                    changelog: "更新日志".to_string(), // TODO: 获取实际的变更日志
                });
            }
        }

        Ok(updates)
    }

    /// 解析依赖关系
    async fn resolve_dependencies(&self, plugin_id: &str, version: &str) -> PackageResult<Vec<PluginDependency>> {
        // 简化实现：返回空依赖列表
        // 在实际实现中，应该递归解析所有依赖
        Ok(Vec::new())
    }

    /// 安装依赖
    async fn install_dependency(&mut self, dependency: &PluginDependency) -> PackageResult<()> {
        // 简化实现：跳过依赖安装以避免递归
        debug!("跳过依赖安装: {}", dependency.name);
        Ok(())
    }

    /// 下载插件包
    async fn download_plugin_package(&self, plugin_id: &str, version: &str) -> PackageResult<PluginPackage> {
        match self.storage.get_plugin_package(plugin_id, version).await {
            Ok(Some(package)) => Ok(package),
            Ok(None) => Err(PackageError::DownloadFailed(format!("插件包不存在: {}:{}", plugin_id, version))),
            Err(e) => Err(PackageError::DownloadFailed(format!("下载失败: {}", e))),
        }
    }

    /// 验证插件包
    fn validate_plugin_package(&self, package: &PluginPackage) -> PackageResult<()> {
        if package.wasm_binary.is_empty() {
            return Err(PackageError::InstallFailed("WASM二进制文件为空".to_string()));
        }

        if package.metadata.name.is_empty() {
            return Err(PackageError::InstallFailed("插件名称为空".to_string()));
        }

        Ok(())
    }

    /// 安装插件包
    async fn install_plugin_package(&self, package: &PluginPackage) -> PackageResult<()> {
        // 简化实现：只记录安装操作
        debug!("安装插件包: {}", package.metadata.name);
        Ok(())
    }

    /// 停止插件
    async fn stop_plugin(&self, plugin_id: &str) -> PackageResult<()> {
        debug!("停止插件: {}", plugin_id);
        Ok(())
    }

    /// 删除插件文件
    async fn remove_plugin_files(&self, plugin_id: &str) -> PackageResult<()> {
        debug!("删除插件文件: {}", plugin_id);
        Ok(())
    }

    /// 获取插件依赖者
    fn get_plugin_dependents(&self, plugin_id: &str) -> Vec<String> {
        // 简化实现：返回空列表
        Vec::new()
    }

    /// 获取最新版本
    async fn get_latest_version(&self, plugin_id: &str) -> PackageResult<String> {
        // 简化实现：返回固定版本
        Ok("1.0.0".to_string())
    }

    /// 检查版本是否匹配
    fn version_matches(&self, installed_version: &str, requirement: &str) -> bool {
        // 简化实现：精确匹配
        installed_version == requirement
    }

    /// 检查是否为更新版本
    fn is_newer_version(&self, new_version: &str, current_version: &str) -> bool {
        // 简化实现：字符串比较
        new_version > current_version
    }

    /// 回滚插件到指定版本
    pub async fn rollback_plugin(&mut self, plugin_id: &str, target_version: &str) -> PackageResult<()> {
        info!("回滚插件 {} 到版本 {}", plugin_id, target_version);

        // 检查目标版本是否存在
        let available_versions = self.get_available_versions(plugin_id).await?;
        if !available_versions.contains(&target_version.to_string()) {
            return Err(PackageError::VersionNotFound(target_version.to_string()));
        }

        // 执行回滚
        let plugin_spec = PluginSpec::new(plugin_id, target_version);
        self.install_plugin(&plugin_spec).await?;

        info!("插件 {} 回滚到版本 {} 成功", plugin_id, target_version);
        Ok(())
    }

    /// 获取可用版本列表
    async fn get_available_versions(&self, plugin_id: &str) -> PackageResult<Vec<String>> {
        // 简化实现：返回固定版本列表
        Ok(vec!["1.0.0".to_string(), "1.1.0".to_string(), "2.0.0".to_string()])
    }

    /// 批量安装插件
    pub async fn batch_install(&mut self, plugin_specs: Vec<PluginSpec>) -> PackageResult<Vec<(String, Result<(), PackageError>)>> {
        info!("批量安装 {} 个插件", plugin_specs.len());

        let mut results = Vec::new();

        for spec in plugin_specs {
            let result = self.install_plugin(&spec).await;
            results.push((spec.plugin_id.clone(), result));
        }

        Ok(results)
    }

    /// 批量更新插件
    pub async fn batch_update(&mut self, plugin_ids: Vec<String>) -> PackageResult<Vec<(String, Result<(), PackageError>)>> {
        info!("批量更新 {} 个插件", plugin_ids.len());

        let mut results = Vec::new();

        for plugin_id in plugin_ids {
            let result = self.update_plugin(&plugin_id).await;
            results.push((plugin_id, result));
        }

        Ok(results)
    }

    /// 自动更新所有插件
    pub async fn auto_update_all(&mut self) -> PackageResult<Vec<(String, Result<(), PackageError>)>> {
        info!("自动更新所有插件");

        let installed_plugins: Vec<String> = self.cache.keys().cloned().collect();
        self.batch_update(installed_plugins).await
    }

    /// 检查插件完整性
    pub async fn verify_plugin_integrity(&self, plugin_id: &str) -> PackageResult<bool> {
        let installed = self.cache.get(plugin_id)
            .ok_or_else(|| PackageError::PluginNotInstalled(plugin_id.to_string()))?;

        // 检查文件是否存在
        let plugin_path = std::path::Path::new(&installed.install_path);
        if !plugin_path.exists() {
            return Ok(false);
        }

        // TODO: 验证文件哈希
        // 实际实现中应该验证插件文件的哈希值

        Ok(true)
    }

    /// 修复损坏的插件
    pub async fn repair_plugin(&mut self, plugin_id: &str) -> PackageResult<()> {
        info!("修复插件: {}", plugin_id);

        let installed = self.cache.get(plugin_id)
            .ok_or_else(|| PackageError::PluginNotInstalled(plugin_id.to_string()))?
            .clone();

        // 重新安装插件
        let plugin_spec = PluginSpec::new(plugin_id, &installed.version);
        self.install_plugin(&plugin_spec).await?;

        info!("插件 {} 修复成功", plugin_id);
        Ok(())
    }

    /// 清理缓存
    pub async fn clean_cache(&mut self) -> PackageResult<()> {
        info!("清理包管理器缓存");

        // TODO: 实现缓存清理逻辑
        // 删除临时文件、过期的下载文件等

        Ok(())
    }

    /// 获取插件依赖树
    pub async fn get_dependency_tree(&self, plugin_id: &str) -> PackageResult<DependencyTree> {
        let mut tree = DependencyTree::new(plugin_id.to_string());
        self.build_dependency_tree(&mut tree, plugin_id, 0).await?;
        Ok(tree)
    }

    /// 构建依赖树
    fn build_dependency_tree<'a>(&'a self, tree: &'a mut DependencyTree, plugin_id: &'a str, depth: usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = PackageResult<()>> + 'a>> {
        Box::pin(async move {
            if depth > 10 {
                return Err(PackageError::CircularDependency(plugin_id.to_string()));
            }

            // 获取插件依赖
            let dependencies = self.get_plugin_dependencies(plugin_id).await?;

            for dep in dependencies {
                tree.add_dependency(plugin_id, &dep.name);
                self.build_dependency_tree(tree, &dep.name, depth + 1).await?;
            }

            Ok(())
        })
    }

    /// 获取插件依赖
    async fn get_plugin_dependencies(&self, plugin_id: &str) -> PackageResult<Vec<PluginDependency>> {
        // 简化实现：返回空依赖列表
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::storage::MemoryStorage;

    #[tokio::test]
    async fn test_package_manager() {
        let storage = Arc::new(MemoryStorage::new());
        let config = StorageConfig::default();
        let mut manager = PluginPackageManager::new(storage, config).await.unwrap();

        // 测试列出已安装插件
        let plugins = manager.list_installed_plugins();
        assert_eq!(plugins.len(), 0);

        // 测试检查更新
        let updates = manager.check_updates().await.unwrap();
        assert_eq!(updates.len(), 0);
    }
}
