//! Package command implementation

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;
use log::{info, warn, error};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::io::Read;

use crate::commands::CommandUtils;
use crate::config::PluginConfig;

/// Package metadata structure
#[derive(Debug, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub plugin_type: String,
    pub language: String,
    pub wasm_file: String,
    pub config_file: String,
    pub checksum: String,
    pub size: u64,
    pub created_at: String,
    pub dataflare_version: String,
}

/// Package structure
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginPackage {
    pub metadata: PackageMetadata,
    pub files: Vec<PackageFile>,
    pub signature: Option<String>,
}

/// Package file entry
#[derive(Debug, Serialize, Deserialize)]
pub struct PackageFile {
    pub path: String,
    pub content: Vec<u8>,
    pub checksum: String,
}

/// Execute the package command
pub async fn execute(
    sign: bool,
    output: Option<String>,
) -> Result<()> {
    info!("Packaging plugin...");

    // Ensure we're in a plugin project
    if !CommandUtils::is_plugin_project() {
        anyhow::bail!("Not in a plugin project directory. Run 'dataflare-plugin new' to create a new project.");
    }

    // Load plugin configuration
    let config_path = CommandUtils::find_plugin_config()?;
    let config = PluginConfig::load_from_file(&config_path)?;

    // Validate configuration
    config.validate()?;

    info!("Packaging plugin: {}", config.plugin.name.cyan().bold());

    // Create progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());

    // Step 1: Build the plugin first
    pb.set_message("Building plugin for packaging...");
    build_plugin_for_packaging(&config).await?;

    // Step 2: Collect files
    pb.set_message("Collecting plugin files...");
    let files = collect_plugin_files(&config)?;

    // Step 3: Create package metadata
    pb.set_message("Creating package metadata...");
    let metadata = create_package_metadata(&config, &files)?;

    // Step 4: Create package
    pb.set_message("Creating package...");
    let mut package = PluginPackage {
        metadata,
        files,
        signature: None,
    };

    // Step 5: Sign package if requested
    if sign {
        pb.set_message("Signing package...");
        package.signature = Some(sign_package(&package)?);
    }

    // Step 6: Write package file
    pb.set_message("Writing package file...");
    let package_path = write_package_file(&package, &config, output)?;

    pb.finish_with_message("Package created successfully!");

    // Print package summary
    print_package_summary(&package, &package_path);

    Ok(())
}

/// Build plugin for packaging
async fn build_plugin_for_packaging(config: &PluginConfig) -> Result<()> {
    // Use the build command to ensure we have a fresh build
    crate::commands::build::execute(
        true,  // release mode
        true,  // optimize
        "wasm32-wasip1".to_string(),
        None,
    ).await
}

/// Collect all files needed for the package
fn collect_plugin_files(config: &PluginConfig) -> Result<Vec<PackageFile>> {
    let root_dir = CommandUtils::get_plugin_root()?;
    let mut files = Vec::new();

    // Add WASM file
    let wasm_file = find_wasm_file(&root_dir, &config.plugin.name)?;
    files.push(create_package_file("plugin.wasm", &wasm_file)?);

    // Add plugin configuration
    let config_path = CommandUtils::find_plugin_config()?;
    files.push(create_package_file("plugin.toml", &config_path)?);

    // Add README if exists
    let readme_path = root_dir.join("README.md");
    if readme_path.exists() {
        files.push(create_package_file("README.md", &readme_path)?);
    }

    // Add LICENSE if exists
    let license_path = root_dir.join("LICENSE");
    if license_path.exists() {
        files.push(create_package_file("LICENSE", &license_path)?);
    }

    // Add Cargo.toml for Rust projects
    let cargo_toml = root_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        files.push(create_package_file("Cargo.toml", &cargo_toml)?);
    }

    // Add package.json for JavaScript projects
    let package_json = root_dir.join("package.json");
    if package_json.exists() {
        files.push(create_package_file("package.json", &package_json)?);
    }

    Ok(files)
}

/// Find the WASM file for the plugin
fn find_wasm_file(root_dir: &Path, plugin_name: &str) -> Result<PathBuf> {
    let target_dir = root_dir.join("target").join("wasm32-wasip1").join("release");
    let wasm_name = format!("{}.wasm", plugin_name.replace("-", "_"));
    let wasm_file = target_dir.join(&wasm_name);

    if !wasm_file.exists() {
        anyhow::bail!("WASM file not found: {}. Please build the plugin first.", wasm_file.display());
    }

    Ok(wasm_file)
}

/// Create a package file entry
fn create_package_file(relative_path: &str, file_path: &Path) -> Result<PackageFile> {
    let mut file = fs::File::open(file_path)?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)?;

    // Calculate checksum
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let checksum = format!("{:x}", hasher.finalize());

    Ok(PackageFile {
        path: relative_path.to_string(),
        content,
        checksum,
    })
}

/// Create package metadata
fn create_package_metadata(config: &PluginConfig, files: &[PackageFile]) -> Result<PackageMetadata> {
    // Find WASM file to calculate total size
    let wasm_file = files.iter()
        .find(|f| f.path == "plugin.wasm")
        .ok_or_else(|| anyhow::anyhow!("WASM file not found in package"))?;

    // Calculate total package checksum
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(&file.content);
    }
    let package_checksum = format!("{:x}", hasher.finalize());

    // Calculate total size
    let total_size: u64 = files.iter().map(|f| f.content.len() as u64).sum();

    Ok(PackageMetadata {
        name: config.plugin.name.clone(),
        version: config.plugin.version.clone(),
        description: config.plugin.description.clone(),
        author: config.plugin.author.clone(),
        plugin_type: "unknown".to_string(), // TODO: Add plugin_type to PluginInfo
        language: "rust".to_string(), // TODO: Add language to PluginInfo
        wasm_file: "plugin.wasm".to_string(),
        config_file: "plugin.toml".to_string(),
        checksum: package_checksum,
        size: total_size,
        created_at: chrono::Utc::now().to_rfc3339(),
        dataflare_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Sign the package (placeholder implementation)
fn sign_package(_package: &PluginPackage) -> Result<String> {
    // TODO: Implement actual package signing
    // For now, return a placeholder signature
    warn!("Package signing is not yet implemented. Using placeholder signature.");
    Ok("placeholder_signature".to_string())
}

/// Write package to file
fn write_package_file(
    package: &PluginPackage,
    config: &PluginConfig,
    output: Option<String>,
) -> Result<PathBuf> {
    let package_name = format!("{}-{}.dfpkg", config.plugin.name, config.plugin.version);
    let package_path = match output {
        Some(path) => PathBuf::from(path),
        None => CommandUtils::get_plugin_root()?.join(&package_name),
    };

    // Serialize package to JSON
    let package_json = serde_json::to_string_pretty(package)?;
    fs::write(&package_path, package_json)?;

    Ok(package_path)
}

/// Print package summary
fn print_package_summary(package: &PluginPackage, package_path: &Path) {
    println!("\n{}", "📦 Package Summary".green().bold());
    println!("  Name: {}", package.metadata.name.cyan());
    println!("  Version: {}", package.metadata.version.yellow());
    println!("  Type: {}", package.metadata.plugin_type.blue());
    println!("  Language: {}", package.metadata.language.magenta());
    println!("  Size: {}", format_bytes(package.metadata.size).green());
    println!("  Files: {}", package.files.len().to_string().cyan());
    println!("  Checksum: {}...", package.metadata.checksum[..16].to_string().yellow());
    println!("  Signed: {}", if package.signature.is_some() { "Yes".green() } else { "No".red() });
    println!("  Output: {}", package_path.display().to_string().cyan());

    println!("\n{}", "📋 Package Contents:".green().bold());
    for file in &package.files {
        println!("  • {} ({})", file.path.cyan(), format_bytes(file.content.len() as u64).yellow());
    }

    println!("\n{}", "✅ Package ready for distribution!".green().bold());
    println!("  Install with: {}", format!("dataflare-plugin install {}", package_path.display()).cyan());
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
