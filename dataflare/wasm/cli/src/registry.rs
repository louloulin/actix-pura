//! Plugin registry client

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin registry client
pub struct RegistryClient {
    base_url: String,
    client: reqwest::Client,
    auth_token: Option<String>,
}

impl RegistryClient {
    /// Create a new registry client
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token: None,
        }
    }
    
    /// Set authentication token
    pub fn with_auth_token(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }
    
    /// Search for plugins
    pub async fn search_plugins(&self, query: &str) -> Result<Vec<PluginSearchResult>> {
        let url = format!("{}/api/v1/plugins/search", self.base_url);
        let mut params = HashMap::new();
        params.insert("q", query);
        
        let response = self.client
            .get(&url)
            .query(&params)
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Search request failed: {}", response.status());
        }
        
        let results: SearchResponse = response.json().await?;
        Ok(results.plugins)
    }
    
    /// Get plugin details
    pub async fn get_plugin(&self, name: &str) -> Result<PluginDetails> {
        let url = format!("{}/api/v1/plugins/{}", self.base_url, name);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Plugin not found: {}", name);
        }
        
        let plugin: PluginDetails = response.json().await?;
        Ok(plugin)
    }
    
    /// Download plugin
    pub async fn download_plugin(&self, name: &str, version: &str) -> Result<Vec<u8>> {
        let url = format!("{}/api/v1/plugins/{}/versions/{}/download", self.base_url, name, version);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Download failed: {}", response.status());
        }
        
        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }
    
    /// Publish plugin
    pub async fn publish_plugin(&self, package: &PluginPackage) -> Result<PublishResponse> {
        if self.auth_token.is_none() {
            anyhow::bail!("Authentication token required for publishing");
        }
        
        let url = format!("{}/api/v1/plugins", self.base_url);
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token.as_ref().unwrap()))
            .json(package)
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Publish failed: {}", response.status());
        }
        
        let result: PublishResponse = response.json().await?;
        Ok(result)
    }
}

/// Plugin search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSearchResult {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub rating: f64,
    pub updated_at: String,
}

/// Search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub plugins: Vec<PluginSearchResult>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

/// Plugin details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDetails {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub homepage: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub downloads: u64,
    pub rating: f64,
    pub versions: Vec<PluginVersionInfo>,
    pub created_at: String,
    pub updated_at: String,
}

/// Plugin version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginVersionInfo {
    pub version: String,
    pub published_at: String,
    pub changelog: Option<String>,
    pub download_url: String,
    pub checksum: String,
}

/// Plugin package for publishing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPackage {
    pub metadata: PluginMetadata,
    pub wasm_binary: Vec<u8>,
    pub source_code: Option<String>,
    pub documentation: Option<String>,
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub homepage: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub component_types: Vec<String>,
    pub operations: Vec<String>,
    pub dataflare_version: String,
}

/// Publish response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResponse {
    pub success: bool,
    pub message: String,
    pub plugin_id: Option<String>,
    pub version: Option<String>,
}

/// Default registry URLs
pub const DEFAULT_REGISTRY: &str = "https://plugins.dataflare.io";
pub const STAGING_REGISTRY: &str = "https://staging-plugins.dataflare.io";

/// Get registry client for URL
pub fn get_registry_client(registry_url: Option<String>, auth_token: Option<String>) -> RegistryClient {
    let url = registry_url.unwrap_or_else(|| DEFAULT_REGISTRY.to_string());
    let mut client = RegistryClient::new(url);
    
    if let Some(token) = auth_token {
        client = client.with_auth_token(token);
    }
    
    client
}
