//! New plugin project creation command

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;
use log::{info, warn};
use colored::*;

use crate::template::PluginTemplate;
use crate::config::PluginConfig;

/// Execute the new command
pub async fn execute(
    name: String,
    lang: String,
    plugin_type: String,
    template: String,
    features: Vec<String>,
    output: Option<String>,
) -> Result<()> {
    info!("Creating new plugin project: {}", name.green().bold());

    // Validate inputs
    validate_inputs(&name, &lang, &plugin_type, &template)?;

    // Determine output directory
    let output_dir = match output {
        Some(dir) => PathBuf::from(dir),
        None => std::env::current_dir()?.join(&name),
    };

    // Check if directory already exists
    if output_dir.exists() {
        anyhow::bail!("Directory '{}' already exists", output_dir.display());
    }

    // Create project directory
    fs::create_dir_all(&output_dir)?;
    info!("Created directory: {}", output_dir.display());

    // Generate project from template
    let template_engine = PluginTemplate::new()?;
    template_engine.generate_project(
        &output_dir,
        &name,
        &lang,
        &plugin_type,
        &template,
        &features,
    ).await?;

    // Create plugin configuration
    create_plugin_config(&output_dir, &name, &plugin_type, &features)?;

    // Initialize git repository if git is available
    if which::which("git").is_ok() {
        init_git_repo(&output_dir)?;
    } else {
        warn!("Git not found, skipping repository initialization");
    }

    // Print success message and next steps
    print_success_message(&name, &output_dir, &lang);

    Ok(())
}

/// Validate command inputs
fn validate_inputs(name: &str, lang: &str, plugin_type: &str, template: &str) -> Result<()> {
    // Validate plugin name
    if name.is_empty() {
        anyhow::bail!("Plugin name cannot be empty");
    }

    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        anyhow::bail!("Plugin name can only contain alphanumeric characters, hyphens, and underscores");
    }

    // Validate language
    let supported_langs = ["rust", "javascript", "python", "cpp", "go"];
    if !supported_langs.contains(&lang) {
        anyhow::bail!("Unsupported language: {}. Supported: {}", lang, supported_langs.join(", "));
    }

    // Validate plugin type
    let supported_types = ["source", "destination", "processor", "transformer", "filter", "aggregator", "ai-processor"];
    if !supported_types.contains(&plugin_type) {
        anyhow::bail!("Unsupported plugin type: {}. Supported: {}", plugin_type, supported_types.join(", "));
    }

    // Validate template
    let supported_templates = ["basic", "advanced", "ai"];
    if !supported_templates.contains(&template) {
        anyhow::bail!("Unsupported template: {}. Supported: {}", template, supported_templates.join(", "));
    }

    Ok(())
}

/// Create plugin configuration file
fn create_plugin_config(
    output_dir: &Path,
    name: &str,
    plugin_type: &str,
    features: &[String],
) -> Result<()> {
    let config = PluginConfig {
        plugin: crate::config::PluginInfo {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("A {} plugin for DataFlare", plugin_type),
            author: "Your Name <your.email@example.com>".to_string(),
            license: "MIT".to_string(),
            repository: None,
            documentation: None,
            homepage: None,
        },
        dataflare: crate::config::DataFlareInfo {
            min_version: "4.0.0".to_string(),
            max_version: None,
        },
        capabilities: crate::config::PluginCapabilities {
            component_types: vec![plugin_type.to_string()],
            operations: get_default_operations(plugin_type),
            batch_processing: true,
            streaming: true,
            stateful: false,
            memory_intensive: false,
        },
        resources: crate::config::ResourceLimits {
            max_memory_mb: 256,
            max_cpu_percent: 50,
            max_execution_time_ms: 5000,
        },
        dependencies: crate::config::PluginDependencies {
            external_apis: vec![],
            required_env_vars: vec![],
            optional_features: features.to_vec(),
        },
        build: crate::config::BuildConfig {
            target: "wasm32-wasi".to_string(),
            optimization: "size".to_string(),
            debug_info: false,
        },
        test: crate::config::TestConfig {
            test_data_dir: "tests/test_data".to_string(),
            benchmark_enabled: true,
            integration_tests: true,
        },
        publish: crate::config::PublishConfig {
            registry: "https://plugins.dataflare.io".to_string(),
            categories: get_default_categories(plugin_type),
            keywords: get_default_keywords(plugin_type),
        },
    };

    let config_path = output_dir.join("plugin.toml");
    let config_content = toml::to_string_pretty(&config)?;
    fs::write(config_path, config_content)?;

    info!("Created plugin configuration: plugin.toml");
    Ok(())
}

/// Get default operations for plugin type
fn get_default_operations(plugin_type: &str) -> Vec<String> {
    match plugin_type {
        "source" => vec!["read".to_string(), "stream".to_string()],
        "destination" => vec!["write".to_string(), "batch_write".to_string()],
        "processor" => vec!["process".to_string(), "batch_process".to_string()],
        "transformer" => vec!["transform".to_string(), "map".to_string()],
        "filter" => vec!["filter".to_string(), "validate".to_string()],
        "aggregator" => vec!["aggregate".to_string(), "reduce".to_string()],
        "ai-processor" => vec!["analyze".to_string(), "predict".to_string(), "embed".to_string()],
        _ => vec!["process".to_string()],
    }
}

/// Get default categories for plugin type
fn get_default_categories(plugin_type: &str) -> Vec<String> {
    match plugin_type {
        "source" => vec!["data-sources".to_string()],
        "destination" => vec!["data-destinations".to_string()],
        "processor" => vec!["data-processing".to_string()],
        "transformer" => vec!["data-transformation".to_string()],
        "filter" => vec!["data-filtering".to_string()],
        "aggregator" => vec!["data-aggregation".to_string()],
        "ai-processor" => vec!["artificial-intelligence".to_string(), "machine-learning".to_string()],
        _ => vec!["data-processing".to_string()],
    }
}

/// Get default keywords for plugin type
fn get_default_keywords(plugin_type: &str) -> Vec<String> {
    let mut keywords = vec!["dataflare".to_string(), "plugin".to_string(), "wasm".to_string()];
    keywords.push(plugin_type.to_string());
    keywords
}

/// Initialize git repository
fn init_git_repo(output_dir: &Path) -> Result<()> {
    use std::process::Command;

    let output = Command::new("git")
        .args(&["init"])
        .current_dir(output_dir)
        .output()?;

    if output.status.success() {
        info!("Initialized git repository");

        // Create .gitignore
        let gitignore_content = include_str!("../../templates/gitignore.template");
        fs::write(output_dir.join(".gitignore"), gitignore_content)?;
        info!("Created .gitignore file");
    } else {
        warn!("Failed to initialize git repository");
    }

    Ok(())
}

/// Print success message and next steps
fn print_success_message(name: &str, output_dir: &Path, lang: &str) {
    println!();
    println!("{}", "✅ Plugin project created successfully!".green().bold());
    println!();
    println!("📁 Project: {}", name.cyan().bold());
    println!("📍 Location: {}", output_dir.display().to_string().cyan());
    println!("🔧 Language: {}", lang.cyan());
    println!();
    println!("{}", "Next steps:".yellow().bold());
    println!("  1. cd {}", name);
    println!("  2. dataflare-plugin build");
    println!("  3. dataflare-plugin test");
    println!();
    println!("{}", "Happy coding! 🚀".green());
}
