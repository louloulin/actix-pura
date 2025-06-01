//! Plugin configuration structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub plugin: PluginInfo,
    pub dataflare: DataFlareInfo,
    pub capabilities: PluginCapabilities,
    pub resources: ResourceLimits,
    pub dependencies: PluginDependencies,
    pub build: BuildConfig,
    pub test: TestConfig,
    pub publish: PublishConfig,
}

/// Basic plugin information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub homepage: Option<String>,
}

/// DataFlare compatibility information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlareInfo {
    pub min_version: String,
    pub max_version: Option<String>,
}

/// Plugin capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapabilities {
    pub component_types: Vec<String>,
    pub operations: Vec<String>,
    pub batch_processing: bool,
    pub streaming: bool,
    pub stateful: bool,
    pub memory_intensive: bool,
}

/// Resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u32,
    pub max_cpu_percent: u32,
    pub max_execution_time_ms: u32,
}

/// Plugin dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependencies {
    pub external_apis: Vec<String>,
    pub required_env_vars: Vec<String>,
    pub optional_features: Vec<String>,
}

/// Build configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub target: String,
    pub optimization: String,
    pub debug_info: bool,
}

/// Test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub test_data_dir: String,
    pub benchmark_enabled: bool,
    pub integration_tests: bool,
}

/// Publish configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishConfig {
    pub registry: String,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
}

impl PluginConfig {
    /// Load configuration from file
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: PluginConfig = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Save configuration to file
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate plugin name
        if self.plugin.name.is_empty() {
            anyhow::bail!("Plugin name cannot be empty");
        }
        
        // Validate version
        semver::Version::parse(&self.plugin.version)
            .map_err(|_| anyhow::anyhow!("Invalid version format: {}", self.plugin.version))?;
        
        // Validate DataFlare version
        semver::Version::parse(&self.dataflare.min_version)
            .map_err(|_| anyhow::anyhow!("Invalid DataFlare min_version format: {}", self.dataflare.min_version))?;
        
        if let Some(ref max_version) = self.dataflare.max_version {
            semver::Version::parse(max_version)
                .map_err(|_| anyhow::anyhow!("Invalid DataFlare max_version format: {}", max_version))?;
        }
        
        // Validate component types
        let valid_types = ["source", "destination", "processor", "transformer", "filter", "aggregator", "ai-processor"];
        for component_type in &self.capabilities.component_types {
            if !valid_types.contains(&component_type.as_str()) {
                anyhow::bail!("Invalid component type: {}", component_type);
            }
        }
        
        // Validate build target
        let valid_targets = ["wasm32-wasi", "wasm32-unknown-unknown"];
        if !valid_targets.contains(&self.build.target.as_str()) {
            anyhow::bail!("Invalid build target: {}", self.build.target);
        }
        
        // Validate optimization level
        let valid_optimizations = ["none", "basic", "full", "size"];
        if !valid_optimizations.contains(&self.build.optimization.as_str()) {
            anyhow::bail!("Invalid optimization level: {}", self.build.optimization);
        }
        
        Ok(())
    }
    
    /// Get the plugin's full name with version
    pub fn full_name(&self) -> String {
        format!("{}@{}", self.plugin.name, self.plugin.version)
    }
    
    /// Check if plugin supports a specific operation
    pub fn supports_operation(&self, operation: &str) -> bool {
        self.capabilities.operations.contains(&operation.to_string())
    }
    
    /// Check if plugin supports a specific component type
    pub fn supports_component_type(&self, component_type: &str) -> bool {
        self.capabilities.component_types.contains(&component_type.to_string())
    }
}

/// CLI configuration for global settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub default_registry: String,
    pub auth_tokens: HashMap<String, String>,
    pub cache_dir: String,
    pub plugins_dir: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            default_registry: "https://plugins.dataflare.io".to_string(),
            auth_tokens: HashMap::new(),
            cache_dir: "~/.dataflare/cache".to_string(),
            plugins_dir: "~/.dataflare/plugins".to_string(),
        }
    }
}

impl CliConfig {
    /// Get the CLI config file path
    pub fn config_path() -> anyhow::Result<std::path::PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home_dir.join(".dataflare").join("config.toml"))
    }
    
    /// Load CLI configuration
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path()?;
        
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            let config: CliConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            // Create default config
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }
    
    /// Save CLI configuration
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path()?;
        
        // Ensure directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }
    
    /// Get auth token for registry
    pub fn get_auth_token(&self, registry: &str) -> Option<&String> {
        self.auth_tokens.get(registry)
    }
    
    /// Set auth token for registry
    pub fn set_auth_token(&mut self, registry: String, token: String) {
        self.auth_tokens.insert(registry, token);
    }
}
