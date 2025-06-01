//! Validate command implementation

use anyhow::Result;
use std::path::PathBuf;
use log::{info, warn};
use colored::*;

use crate::commands::CommandUtils;
use crate::config::PluginConfig;

/// Execute the validate command
pub async fn execute(strict: bool, plugin: Option<String>) -> Result<()> {
    info!("Validating plugin...");

    let (config, plugin_file) = if let Some(plugin_path) = plugin {
        // Validate specific plugin file
        let plugin_file = PathBuf::from(plugin_path);
        if !plugin_file.exists() {
            anyhow::bail!("Plugin file not found: {}", plugin_file.display());
        }

        // Try to find config in the same directory or parent directories
        let config = find_config_for_plugin(&plugin_file)?;
        (config, Some(plugin_file))
    } else {
        // Validate current project
        if !CommandUtils::is_plugin_project() {
            anyhow::bail!("Not in a plugin project directory. Specify a plugin file with --plugin option.");
        }

        let config_path = CommandUtils::find_plugin_config()?;
        let config = PluginConfig::load_from_file(&config_path)?;
        (config, None)
    };

    info!("Validating plugin: {}", config.plugin.name.cyan().bold());

    let mut validation_results = Vec::new();

    // Validate configuration
    validation_results.push(validate_configuration(&config, strict).await);

    // Validate plugin file if provided
    if let Some(plugin_file) = plugin_file {
        validation_results.push(validate_plugin_file(&plugin_file, &config, strict).await);
    } else {
        // Build and validate the plugin
        validation_results.push(validate_built_plugin(&config, strict).await);
    }

    // Validate metadata
    validation_results.push(validate_metadata(&config, strict).await);

    // Validate dependencies
    validation_results.push(validate_dependencies(&config, strict).await);

    // Validate security requirements
    validation_results.push(validate_security(&config, strict).await);

    // Print validation summary
    print_validation_summary(&config, &validation_results, strict);

    // Check if validation passed
    let all_passed = validation_results.iter().all(|result| result.passed);
    if !all_passed {
        anyhow::bail!("Plugin validation failed");
    }

    Ok(())
}

/// Validation result
#[derive(Debug)]
struct ValidationResult {
    category: String,
    passed: bool,
    warnings: Vec<String>,
    errors: Vec<String>,
}

/// Find configuration file for a plugin
fn find_config_for_plugin(plugin_file: &PathBuf) -> Result<PluginConfig> {
    let mut dir = plugin_file.parent().unwrap_or_else(|| std::path::Path::new("."));

    loop {
        let config_path = dir.join("plugin.toml");
        if config_path.exists() {
            return PluginConfig::load_from_file(&config_path);
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    anyhow::bail!("Could not find plugin.toml for plugin file: {}", plugin_file.display());
}

/// Validate plugin configuration
async fn validate_configuration(config: &PluginConfig, strict: bool) -> ValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Validate basic configuration
    match config.validate() {
        Ok(_) => {},
        Err(e) => errors.push(format!("Configuration validation failed: {}", e)),
    }

    // Check for recommended fields
    if config.plugin.repository.is_none() {
        if strict {
            errors.push("Repository URL is required in strict mode".to_string());
        } else {
            warnings.push("Repository URL is recommended".to_string());
        }
    }

    if config.plugin.documentation.is_none() {
        if strict {
            errors.push("Documentation URL is required in strict mode".to_string());
        } else {
            warnings.push("Documentation URL is recommended".to_string());
        }
    }

    if config.plugin.description.len() < 20 {
        warnings.push("Plugin description should be more descriptive (at least 20 characters)".to_string());
    }

    // Validate version format
    if let Err(e) = semver::Version::parse(&config.plugin.version) {
        errors.push(format!("Invalid version format: {}", e));
    }

    // Check resource limits
    if config.resources.max_memory_mb > 1024 {
        warnings.push("High memory limit (>1GB) may impact performance".to_string());
    }

    if config.resources.max_execution_time_ms > 30000 {
        warnings.push("Long execution time (>30s) may cause timeouts".to_string());
    }

    ValidationResult {
        category: "Configuration".to_string(),
        passed: errors.is_empty(),
        warnings,
        errors,
    }
}

/// Validate plugin file
async fn validate_plugin_file(plugin_file: &PathBuf, _config: &PluginConfig, _strict: bool) -> ValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Check file exists and is readable
    if !plugin_file.exists() {
        errors.push(format!("Plugin file not found: {}", plugin_file.display()));
        return ValidationResult {
            category: "Plugin File".to_string(),
            passed: false,
            warnings,
            errors,
        };
    }

    // Check file size
    if let Ok(metadata) = std::fs::metadata(plugin_file) {
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
        if size_mb > 10.0 {
            warnings.push(format!("Large plugin file ({:.1} MB)", size_mb));
        }
    }

    // Validate WASM binary
    match std::fs::read(plugin_file) {
        Ok(bytes) => {
            if let Err(e) = validate_wasm_binary(&bytes) {
                errors.push(format!("Invalid WASM binary: {}", e));
            }
        }
        Err(e) => {
            errors.push(format!("Failed to read plugin file: {}", e));
        }
    }

    ValidationResult {
        category: "Plugin File".to_string(),
        passed: errors.is_empty(),
        warnings,
        errors,
    }
}

/// Validate built plugin
async fn validate_built_plugin(config: &PluginConfig, strict: bool) -> ValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Try to build the plugin first
    let build_result = crate::commands::build::execute(true, true, "wasm32-wasip1".to_string(), None).await;

    if let Err(e) = build_result {
        errors.push(format!("Failed to build plugin: {}", e));
        return ValidationResult {
            category: "Build Validation".to_string(),
            passed: false,
            warnings,
            errors,
        };
    }

    // Find the built WASM file
    let output_dir = CommandUtils::get_output_dir().unwrap_or_else(|_| PathBuf::from("dist"));
    let wasm_file = output_dir.join(format!("{}.wasm", config.plugin.name));

    if wasm_file.exists() {
        // Validate the built WASM file
        let file_result = validate_plugin_file(&wasm_file, config, strict).await;
        warnings.extend(file_result.warnings);
        errors.extend(file_result.errors);
    } else {
        errors.push("Built WASM file not found".to_string());
    }

    ValidationResult {
        category: "Build Validation".to_string(),
        passed: errors.is_empty(),
        warnings,
        errors,
    }
}

/// Validate plugin metadata
async fn validate_metadata(config: &PluginConfig, strict: bool) -> ValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Check required metadata fields
    if config.plugin.name.is_empty() {
        errors.push("Plugin name is required".to_string());
    }

    if config.plugin.author.is_empty() {
        if strict {
            errors.push("Author is required in strict mode".to_string());
        } else {
            warnings.push("Author information is recommended".to_string());
        }
    }

    // Validate component types
    if config.capabilities.component_types.is_empty() {
        errors.push("At least one component type must be specified".to_string());
    }

    // Validate operations
    if config.capabilities.operations.is_empty() {
        warnings.push("No operations specified".to_string());
    }

    // Check keywords and categories
    if config.publish.keywords.is_empty() {
        warnings.push("Keywords help with plugin discovery".to_string());
    }

    if config.publish.categories.is_empty() {
        warnings.push("Categories help with plugin organization".to_string());
    }

    ValidationResult {
        category: "Metadata".to_string(),
        passed: errors.is_empty(),
        warnings,
        errors,
    }
}

/// Validate dependencies
async fn validate_dependencies(config: &PluginConfig, _strict: bool) -> ValidationResult {
    let mut warnings = Vec::new();
    let errors = Vec::new();

    // Check for external API dependencies
    for api in &config.dependencies.external_apis {
        if !api.starts_with("https://") {
            warnings.push(format!("External API should use HTTPS: {}", api));
        }
    }

    // Check environment variables
    for env_var in &config.dependencies.required_env_vars {
        if env_var.is_empty() {
            warnings.push("Empty environment variable name".to_string());
        }
    }

    ValidationResult {
        category: "Dependencies".to_string(),
        passed: errors.is_empty(),
        warnings,
        errors,
    }
}

/// Validate security requirements
async fn validate_security(config: &PluginConfig, _strict: bool) -> ValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Check resource limits
    if config.resources.max_memory_mb == 0 {
        errors.push("Memory limit must be greater than 0".to_string());
    }

    if config.resources.max_execution_time_ms == 0 {
        errors.push("Execution time limit must be greater than 0".to_string());
    }

    // Security recommendations
    if !config.dependencies.external_apis.is_empty() {
        warnings.push("Plugin requires network access - ensure this is necessary".to_string());
    }

    if !config.dependencies.required_env_vars.is_empty() {
        warnings.push("Plugin requires environment variables - ensure sensitive data is protected".to_string());
    }

    ValidationResult {
        category: "Security".to_string(),
        passed: errors.is_empty(),
        warnings,
        errors,
    }
}

/// Validate WASM binary
fn validate_wasm_binary(bytes: &[u8]) -> Result<()> {
    // Check WASM magic number
    if bytes.len() < 4 || &bytes[0..4] != b"\0asm" {
        anyhow::bail!("Invalid WASM magic number");
    }

    // Check WASM version
    if bytes.len() < 8 || &bytes[4..8] != &[1, 0, 0, 0] {
        anyhow::bail!("Unsupported WASM version");
    }

    // Basic validation using wasmparser
    let parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(bytes) {
        match payload {
            Ok(_) => continue,
            Err(e) => anyhow::bail!("WASM validation error: {}", e),
        }
    }

    Ok(())
}

/// Print validation summary
fn print_validation_summary(config: &PluginConfig, results: &[ValidationResult], strict: bool) {
    println!();
    println!("{}", "🔍 Validation Summary".green().bold());
    println!("{}", "─".repeat(60));

    println!("📦 Plugin: {}", config.plugin.name.cyan().bold());
    println!("🔧 Mode: {}", if strict { "Strict".red() } else { "Standard".yellow() });

    let mut total_warnings = 0;
    let mut total_errors = 0;

    for result in results {
        let status = if result.passed {
            "PASS".green().bold()
        } else {
            "FAIL".red().bold()
        };

        println!("📋 {}: {} ({} warnings, {} errors)",
                 result.category,
                 status,
                 result.warnings.len(),
                 result.errors.len());

        // Print errors
        for error in &result.errors {
            println!("   {} {}", "❌".red(), error.red());
        }

        // Print warnings
        for warning in &result.warnings {
            println!("   {} {}", "⚠️".yellow(), warning.yellow());
        }

        total_warnings += result.warnings.len();
        total_errors += result.errors.len();
    }

    println!("{}", "─".repeat(60));

    if total_errors == 0 {
        println!("{} Plugin validation passed! ({} warnings)",
                 "✅".green(),
                 total_warnings);
    } else {
        println!("{} Plugin validation failed! ({} errors, {} warnings)",
                 "❌".red(),
                 total_errors,
                 total_warnings);
    }
}
