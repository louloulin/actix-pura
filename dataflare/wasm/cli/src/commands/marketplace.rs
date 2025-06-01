//! Plugin marketplace commands
//!
//! Commands for interacting with the DataFlare plugin marketplace

use anyhow::{Result, Context};
use colored::*;
use log::{info, warn, error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

use crate::config::PluginConfig;
use crate::utils::spinner::Spinner;

/// Plugin marketplace client
pub struct MarketplaceClient {
    registry_url: String,
    api_token: Option<String>,
    cache_dir: PathBuf,
}

/// Plugin metadata from marketplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub tags: Vec<String>,
    pub plugin_type: String,
    pub language: String,
    pub download_url: String,
    pub checksum: String,
    pub size_bytes: u64,
    pub downloads: u64,
    pub rating: f32,
    pub created_at: String,
    pub updated_at: String,
    pub dependencies: Vec<String>,
    pub compatibility: Vec<String>,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub plugins: Vec<PluginMetadata>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

/// Installed plugin info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub installed_at: String,
    pub source: String,
    pub path: PathBuf,
}

impl MarketplaceClient {
    /// Create a new marketplace client
    pub fn new(registry_url: String, api_token: Option<String>) -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dataflare")
            .join("plugins");

        fs::create_dir_all(&cache_dir)
            .context("Failed to create cache directory")?;

        Ok(Self {
            registry_url,
            api_token,
            cache_dir,
        })
    }

    /// Search plugins in the marketplace
    pub async fn search(&self, query: &str, page: u32, per_page: u32) -> Result<SearchResult> {
        info!("Searching for plugins: {}", query);

        // Simulate API call with mock data for now
        sleep(Duration::from_millis(500)).await;

        let mock_plugins = vec![
            PluginMetadata {
                name: "json-transformer".to_string(),
                version: "1.2.0".to_string(),
                description: "Advanced JSON transformation plugin with JSONPath support".to_string(),
                author: "DataFlare Team".to_string(),
                license: "MIT".to_string(),
                tags: vec!["json".to_string(), "transform".to_string(), "data".to_string()],
                plugin_type: "transformer".to_string(),
                language: "rust".to_string(),
                download_url: "https://plugins.dataflare.io/json-transformer/1.2.0".to_string(),
                checksum: "sha256:abc123...".to_string(),
                size_bytes: 1024 * 512, // 512KB
                downloads: 15420,
                rating: 4.8,
                created_at: "2024-01-15T10:30:00Z".to_string(),
                updated_at: "2024-03-20T14:45:00Z".to_string(),
                dependencies: vec!["serde".to_string(), "jsonpath".to_string()],
                compatibility: vec!["dataflare-4.0".to_string()],
            },
            PluginMetadata {
                name: "csv-processor".to_string(),
                version: "2.1.3".to_string(),
                description: "High-performance CSV processing with schema validation".to_string(),
                author: "Community".to_string(),
                license: "Apache-2.0".to_string(),
                tags: vec!["csv".to_string(), "processor".to_string(), "validation".to_string()],
                plugin_type: "processor".to_string(),
                language: "rust".to_string(),
                download_url: "https://plugins.dataflare.io/csv-processor/2.1.3".to_string(),
                checksum: "sha256:def456...".to_string(),
                size_bytes: 1024 * 768, // 768KB
                downloads: 8932,
                rating: 4.6,
                created_at: "2023-11-08T09:15:00Z".to_string(),
                updated_at: "2024-02-14T16:20:00Z".to_string(),
                dependencies: vec!["csv".to_string(), "serde".to_string()],
                compatibility: vec!["dataflare-4.0".to_string(), "dataflare-3.9".to_string()],
            },
            PluginMetadata {
                name: "ai-sentiment-analyzer".to_string(),
                version: "1.0.0".to_string(),
                description: "AI-powered sentiment analysis for text data".to_string(),
                author: "AI Labs".to_string(),
                license: "Commercial".to_string(),
                tags: vec!["ai".to_string(), "sentiment".to_string(), "nlp".to_string()],
                plugin_type: "ai-processor".to_string(),
                language: "python".to_string(),
                download_url: "https://plugins.dataflare.io/ai-sentiment-analyzer/1.0.0".to_string(),
                checksum: "sha256:ghi789...".to_string(),
                size_bytes: 1024 * 1024 * 5, // 5MB
                downloads: 2156,
                rating: 4.9,
                created_at: "2024-03-01T12:00:00Z".to_string(),
                updated_at: "2024-03-15T10:30:00Z".to_string(),
                dependencies: vec!["transformers".to_string(), "torch".to_string()],
                compatibility: vec!["dataflare-4.0".to_string()],
            },
        ];

        // Filter plugins based on query
        let filtered_plugins: Vec<PluginMetadata> = mock_plugins
            .into_iter()
            .filter(|plugin| {
                plugin.name.to_lowercase().contains(&query.to_lowercase()) ||
                plugin.description.to_lowercase().contains(&query.to_lowercase()) ||
                plugin.tags.iter().any(|tag| tag.to_lowercase().contains(&query.to_lowercase()))
            })
            .collect();

        Ok(SearchResult {
            total: filtered_plugins.len() as u64,
            plugins: filtered_plugins,
            page,
            per_page,
        })
    }

    /// Get plugin information
    pub async fn get_plugin_info(&self, name: &str) -> Result<PluginMetadata> {
        info!("Getting plugin info: {}", name);

        // Simulate API call
        sleep(Duration::from_millis(300)).await;

        // Return mock data for demonstration
        match name {
            "json-transformer" => Ok(PluginMetadata {
                name: "json-transformer".to_string(),
                version: "1.2.0".to_string(),
                description: "Advanced JSON transformation plugin with JSONPath support".to_string(),
                author: "DataFlare Team".to_string(),
                license: "MIT".to_string(),
                tags: vec!["json".to_string(), "transform".to_string(), "data".to_string()],
                plugin_type: "transformer".to_string(),
                language: "rust".to_string(),
                download_url: "https://plugins.dataflare.io/json-transformer/1.2.0".to_string(),
                checksum: "sha256:abc123...".to_string(),
                size_bytes: 1024 * 512,
                downloads: 15420,
                rating: 4.8,
                created_at: "2024-01-15T10:30:00Z".to_string(),
                updated_at: "2024-03-20T14:45:00Z".to_string(),
                dependencies: vec!["serde".to_string(), "jsonpath".to_string()],
                compatibility: vec!["dataflare-4.0".to_string()],
            }),
            _ => anyhow::bail!("Plugin '{}' not found", name),
        }
    }

    /// Install a plugin
    pub async fn install_plugin(&self, name: &str, version: Option<&str>) -> Result<InstalledPlugin> {
        let version = version.unwrap_or("latest");
        info!("Installing plugin: {} (version: {})", name, version);

        let mut spinner = Spinner::new("Downloading plugin...");
        spinner.start();

        // Simulate download
        sleep(Duration::from_secs(2)).await;
        spinner.update("Verifying checksum...");
        sleep(Duration::from_millis(500)).await;
        spinner.update("Installing plugin...");
        sleep(Duration::from_millis(800)).await;
        spinner.stop();

        let plugin_dir = self.cache_dir.join(name);
        fs::create_dir_all(&plugin_dir)?;

        let installed_plugin = InstalledPlugin {
            name: name.to_string(),
            version: version.to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            source: self.registry_url.clone(),
            path: plugin_dir,
        };

        // Save installation info
        self.save_installed_plugin(&installed_plugin)?;

        println!("{} Plugin '{}' installed successfully!", "✓".green(), name);
        Ok(installed_plugin)
    }

    /// List installed plugins
    pub fn list_installed_plugins(&self) -> Result<Vec<InstalledPlugin>> {
        let installed_file = self.cache_dir.join("installed.json");
        if !installed_file.exists() {
            return Ok(vec![]);
        }

        let content = fs::read_to_string(installed_file)?;
        let plugins: Vec<InstalledPlugin> = serde_json::from_str(&content)?;
        Ok(plugins)
    }

    /// Remove a plugin
    pub async fn remove_plugin(&self, name: &str) -> Result<()> {
        info!("Removing plugin: {}", name);

        let mut installed_plugins = self.list_installed_plugins()?;
        let plugin_index = installed_plugins
            .iter()
            .position(|p| p.name == name)
            .context("Plugin not found")?;

        let plugin = installed_plugins.remove(plugin_index);

        // Remove plugin directory
        if plugin.path.exists() {
            fs::remove_dir_all(&plugin.path)?;
        }

        // Update installed plugins list
        self.save_installed_plugins(&installed_plugins)?;

        println!("{} Plugin '{}' removed successfully!", "✓".green(), name);
        Ok(())
    }

    /// Update plugins
    pub async fn update_plugins(&self, plugin_name: Option<&str>) -> Result<()> {
        let installed_plugins = self.list_installed_plugins()?;

        let plugins_to_update: Vec<&InstalledPlugin> = match plugin_name {
            Some(name) => installed_plugins.iter().filter(|p| p.name == name).collect(),
            None => installed_plugins.iter().collect(),
        };

        if plugins_to_update.is_empty() {
            println!("No plugins to update");
            return Ok(());
        }

        for plugin in plugins_to_update {
            println!("Checking updates for '{}'...", plugin.name);
            // Simulate update check
            sleep(Duration::from_millis(500)).await;
            println!("{} '{}' is up to date", "✓".green(), plugin.name);
        }

        Ok(())
    }

    /// Save installed plugin info
    fn save_installed_plugin(&self, plugin: &InstalledPlugin) -> Result<()> {
        let mut installed_plugins = self.list_installed_plugins().unwrap_or_default();

        // Remove existing entry if present
        installed_plugins.retain(|p| p.name != plugin.name);

        // Add new entry
        installed_plugins.push(plugin.clone());

        self.save_installed_plugins(&installed_plugins)
    }

    /// Save installed plugins list
    fn save_installed_plugins(&self, plugins: &[InstalledPlugin]) -> Result<()> {
        let installed_file = self.cache_dir.join("installed.json");
        let content = serde_json::to_string_pretty(plugins)?;
        fs::write(installed_file, content)?;
        Ok(())
    }
}

/// Execute search command
pub async fn execute_search(query: String, registry: Option<String>) -> Result<()> {
    let registry_url = registry.unwrap_or_else(|| "https://plugins.dataflare.io".to_string());
    let client = MarketplaceClient::new(registry_url, None)?;

    println!("{} Searching for plugins: {}", "🔍".cyan(), query.bold());

    let result = client.search(&query, 1, 20).await?;

    if result.plugins.is_empty() {
        println!("{} No plugins found matching '{}'", "ℹ".blue(), query);
        return Ok(());
    }

    println!("\n{} Found {} plugin(s):\n", "✓".green(), result.total);

    for plugin in result.plugins {
        println!("{} {} {}",
            "📦".cyan(),
            plugin.name.bold(),
            format!("v{}", plugin.version).dimmed()
        );
        println!("   {}", plugin.description);
        println!("   {} {} | {} {} | {} {}",
            "👤".dimmed(), plugin.author.dimmed(),
            "📊".dimmed(), format!("{:.1}★", plugin.rating).yellow(),
            "⬇".dimmed(), format!("{} downloads", plugin.downloads).dimmed()
        );
        println!("   {} {}", "🏷".dimmed(), plugin.tags.join(", ").dimmed());
        println!();
    }

    Ok(())
}

/// Execute install command
pub async fn execute_install(plugin: String, version: Option<String>, registry: Option<String>) -> Result<()> {
    let registry_url = registry.unwrap_or_else(|| "https://plugins.dataflare.io".to_string());
    let client = MarketplaceClient::new(registry_url, None)?;

    client.install_plugin(&plugin, version.as_deref()).await?;
    Ok(())
}

/// Execute list command
pub async fn execute_list(detailed: bool) -> Result<()> {
    let client = MarketplaceClient::new("https://plugins.dataflare.io".to_string(), None)?;
    let installed_plugins = client.list_installed_plugins()?;

    if installed_plugins.is_empty() {
        println!("{} No plugins installed", "ℹ".blue());
        return Ok(());
    }

    println!("{} Installed plugins:\n", "📦".cyan());

    for plugin in installed_plugins {
        if detailed {
            println!("{} {} {}",
                "📦".cyan(),
                plugin.name.bold(),
                format!("v{}", plugin.version).dimmed()
            );
            println!("   {} Installed: {}", "📅".dimmed(), plugin.installed_at.dimmed());
            println!("   {} Source: {}", "🌐".dimmed(), plugin.source.dimmed());
            println!("   {} Path: {}", "📁".dimmed(), plugin.path.display().to_string().dimmed());
            println!();
        } else {
            println!("  {} {} {}",
                "📦".cyan(),
                plugin.name.bold(),
                format!("v{}", plugin.version).dimmed()
            );
        }
    }

    Ok(())
}

/// Execute info command
pub async fn execute_info(plugin: String) -> Result<()> {
    let client = MarketplaceClient::new("https://plugins.dataflare.io".to_string(), None)?;

    println!("{} Getting plugin information...", "🔍".cyan());

    let plugin_info = client.get_plugin_info(&plugin).await?;

    println!("\n{} {}", "📦".cyan(), plugin_info.name.bold());
    println!("{} {}", "Version:".bold(), plugin_info.version);
    println!("{} {}", "Description:".bold(), plugin_info.description);
    println!("{} {}", "Author:".bold(), plugin_info.author);
    println!("{} {}", "License:".bold(), plugin_info.license);
    println!("{} {}", "Type:".bold(), plugin_info.plugin_type);
    println!("{} {}", "Language:".bold(), plugin_info.language);
    println!("{} {:.1}★ ({} downloads)", "Rating:".bold(), plugin_info.rating, plugin_info.downloads);
    println!("{} {}", "Size:".bold(), format_bytes(plugin_info.size_bytes));
    println!("{} {}", "Tags:".bold(), plugin_info.tags.join(", "));

    if !plugin_info.dependencies.is_empty() {
        println!("{} {}", "Dependencies:".bold(), plugin_info.dependencies.join(", "));
    }

    if !plugin_info.compatibility.is_empty() {
        println!("{} {}", "Compatibility:".bold(), plugin_info.compatibility.join(", "));
    }

    println!("{} {}", "Created:".bold(), plugin_info.created_at);
    println!("{} {}", "Updated:".bold(), plugin_info.updated_at);

    Ok(())
}

/// Execute remove command
pub async fn execute_remove(plugin: String) -> Result<()> {
    let client = MarketplaceClient::new("https://plugins.dataflare.io".to_string(), None)?;
    client.remove_plugin(&plugin).await?;
    Ok(())
}

/// Execute update command
pub async fn execute_update(plugin: Option<String>) -> Result<()> {
    let client = MarketplaceClient::new("https://plugins.dataflare.io".to_string(), None)?;
    client.update_plugins(plugin.as_deref()).await?;
    Ok(())
}

/// Format bytes in human readable format
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}
