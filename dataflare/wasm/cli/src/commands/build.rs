//! Build command implementation

use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;
use log::{info, warn, error};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};

use crate::commands::CommandUtils;
use crate::config::PluginConfig;

/// Execute the build command
pub async fn execute(
    release: bool,
    optimize: bool,
    target: String,
    output: Option<String>,
) -> Result<()> {
    info!("Building plugin...");
    
    // Ensure we're in a plugin project
    if !CommandUtils::is_plugin_project() {
        anyhow::bail!("Not in a plugin project directory. Run 'dataflare-plugin new' to create a new project.");
    }
    
    // Load plugin configuration
    let config_path = CommandUtils::find_plugin_config()?;
    let config = PluginConfig::load_from_file(&config_path)?;
    
    // Validate configuration
    config.validate()?;
    
    info!("Building plugin: {}", config.plugin.name.cyan().bold());
    
    // Determine build mode
    let build_mode = if release { "release" } else { "debug" };
    info!("Build mode: {}", build_mode.yellow());
    
    // Create progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());
    
    // Step 1: Compile to WASM
    pb.set_message("Compiling to WebAssembly...");
    let wasm_file = compile_to_wasm(&config, release, &target)?;
    
    // Step 2: Optimize if requested
    let final_wasm_file = if optimize {
        pb.set_message("Optimizing WASM binary...");
        optimize_wasm(&wasm_file, &config)?
    } else {
        wasm_file
    };
    
    // Step 3: Copy to output directory
    pb.set_message("Copying to output directory...");
    let output_file = copy_to_output(&final_wasm_file, &config, output)?;
    
    pb.finish_with_message("Build completed successfully!");
    
    // Print build summary
    print_build_summary(&config, &output_file, release, optimize);
    
    Ok(())
}

/// Compile Rust code to WASM
fn compile_to_wasm(config: &PluginConfig, release: bool, target: &str) -> Result<PathBuf> {
    let root_dir = CommandUtils::get_plugin_root()?;
    
    // Check if Cargo.toml exists
    let cargo_toml = root_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("Cargo.toml not found. This command only supports Rust projects currently.");
    }
    
    // Build cargo command
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root_dir);
    cmd.args(&["build", "--target", target]);
    
    if release {
        cmd.arg("--release");
    }
    
    // Execute build
    info!("Running: cargo build --target {} {}", target, if release { "--release" } else { "" });
    let output = cmd.output()?;
    
    if !output.status.success() {
        error!("Cargo build failed:");
        error!("{}", String::from_utf8_lossy(&output.stderr));
        anyhow::bail!("Build failed");
    }
    
    // Find the generated WASM file
    let target_dir = root_dir.join("target").join(target);
    let build_dir = if release {
        target_dir.join("release")
    } else {
        target_dir.join("debug")
    };
    
    let wasm_file = build_dir.join(format!("{}.wasm", config.plugin.name.replace("-", "_")));
    
    if !wasm_file.exists() {
        anyhow::bail!("WASM file not found at: {}", wasm_file.display());
    }
    
    info!("Generated WASM file: {}", wasm_file.display());
    Ok(wasm_file)
}

/// Optimize WASM binary using wasm-opt
fn optimize_wasm(wasm_file: &PathBuf, config: &PluginConfig) -> Result<PathBuf> {
    // Check if wasm-opt is available
    if which::which("wasm-opt").is_err() {
        warn!("wasm-opt not found, skipping optimization. Install binaryen for optimization support.");
        return Ok(wasm_file.clone());
    }
    
    let optimized_file = wasm_file.with_extension("opt.wasm");
    
    // Build wasm-opt command based on optimization level
    let mut cmd = Command::new("wasm-opt");
    
    match config.build.optimization.as_str() {
        "none" => {
            // No optimization, just copy
            std::fs::copy(wasm_file, &optimized_file)?;
            return Ok(optimized_file);
        }
        "basic" => {
            cmd.args(&["-O1"]);
        }
        "full" => {
            cmd.args(&["-O3"]);
        }
        "size" => {
            cmd.args(&["-Oz"]);
        }
        _ => {
            cmd.args(&["-O2"]); // Default
        }
    }
    
    cmd.args(&[
        wasm_file.to_str().unwrap(),
        "-o",
        optimized_file.to_str().unwrap(),
    ]);
    
    info!("Running wasm-opt with {} optimization", config.build.optimization);
    let output = cmd.output()?;
    
    if !output.status.success() {
        warn!("wasm-opt failed, using unoptimized binary");
        warn!("{}", String::from_utf8_lossy(&output.stderr));
        return Ok(wasm_file.clone());
    }
    
    // Compare file sizes
    let original_size = std::fs::metadata(wasm_file)?.len();
    let optimized_size = std::fs::metadata(&optimized_file)?.len();
    let reduction = ((original_size - optimized_size) as f64 / original_size as f64) * 100.0;
    
    info!("Optimization complete: {:.1}% size reduction", reduction);
    
    Ok(optimized_file)
}

/// Copy WASM file to output directory
fn copy_to_output(
    wasm_file: &PathBuf,
    config: &PluginConfig,
    output: Option<String>,
) -> Result<PathBuf> {
    let output_dir = match output {
        Some(dir) => PathBuf::from(dir),
        None => CommandUtils::get_output_dir()?,
    };
    
    CommandUtils::ensure_dir(&output_dir)?;
    
    let output_file = output_dir.join(format!("{}.wasm", config.plugin.name));
    std::fs::copy(wasm_file, &output_file)?;
    
    info!("Output file: {}", output_file.display());
    Ok(output_file)
}

/// Print build summary
fn print_build_summary(
    config: &PluginConfig,
    output_file: &PathBuf,
    release: bool,
    optimize: bool,
) {
    println!();
    println!("{}", "🎉 Build Summary".green().bold());
    println!("{}", "─".repeat(50));
    
    println!("📦 Plugin: {}", config.plugin.name.cyan().bold());
    println!("🔖 Version: {}", config.plugin.version.cyan());
    println!("🏗️  Mode: {}", if release { "Release".green() } else { "Debug".yellow() });
    println!("⚡ Optimized: {}", if optimize { "Yes".green() } else { "No".yellow() });
    
    if let Ok(metadata) = std::fs::metadata(output_file) {
        let size_kb = metadata.len() as f64 / 1024.0;
        println!("📏 Size: {:.1} KB", size_kb);
    }
    
    println!("📁 Output: {}", output_file.display().to_string().cyan());
    
    println!();
    println!("{}", "Next steps:".yellow().bold());
    println!("  • Test your plugin: {}", "dataflare-plugin test".cyan());
    println!("  • Validate plugin: {}", "dataflare-plugin validate".cyan());
    println!("  • Package for distribution: {}", "dataflare-plugin package".cyan());
}
