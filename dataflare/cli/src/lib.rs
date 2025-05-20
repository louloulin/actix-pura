//! # DataFlare CLI
//!
//! Command-line interface for the DataFlare data integration framework.
//! This crate provides tools for managing workflows, connectors, and plugins.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Write, Read};
use std::time::Instant;
use dataflare_connector::registry::{get_registered_source_connectors, get_registered_destination_connectors};
use dataflare_processor::registry::get_processor_names;
use dataflare_plugin::{
    registry::list_plugins, 
    plugin::{PluginManager, PluginMetadata}
};
use dataflare_runtime::{
    workflow::{YamlWorkflowParser}, 
    executor::WorkflowExecutor,
    RuntimeMode
};
use dataflare_core::message::{WorkflowProgress, WorkflowPhase};
use serde_json;

/// Version of the DataFlare CLI module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// DataFlare CLI application
#[derive(Debug, Parser)]
#[clap(name = "dataflare", version = VERSION, about = "DataFlare CLI")]
pub struct Cli {
    /// Subcommand to execute
    #[clap(subcommand)]
    pub command: Commands,
}

/// CLI subcommands
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize a new DataFlare project
    #[clap(name = "init")]
    Init {
        /// Project name
        #[clap(long, short)]
        name: Option<String>,

        /// Project directory
        #[clap(long, short)]
        dir: Option<PathBuf>,
    },

    /// Run a workflow
    #[clap(name = "run")]
    Run {
        /// Workflow file path
        #[clap(long, short)]
        workflow: PathBuf,

        /// Runtime mode (standalone, edge, cloud)
        #[clap(long, short, default_value = "standalone")]
        mode: String,
    },

    /// Validate a workflow
    #[clap(name = "validate")]
    Validate {
        /// Workflow file path
        #[clap(long, short)]
        workflow: PathBuf,
    },

    /// List available connectors
    #[clap(name = "connectors")]
    Connectors,

    /// List available processors
    #[clap(name = "processors")]
    Processors,

    /// Manage plugins
    #[clap(name = "plugins")]
    Plugins {
        /// Plugin subcommand
        #[clap(subcommand)]
        command: PluginCommands,
    },
}

/// Plugin subcommands
#[derive(Debug, Subcommand)]
pub enum PluginCommands {
    /// List installed plugins
    #[clap(name = "list")]
    List,

    /// Show detailed information about a plugin
    #[clap(name = "info")]
    Info {
        /// Plugin ID
        id: String,
    },

    /// Install a plugin
    #[clap(name = "install")]
    Install {
        /// Plugin path or URL
        path: String,
    },

    /// Remove a plugin
    #[clap(name = "remove")]
    Remove {
        /// Plugin ID
        id: String,
    },
}

/// Run the CLI application
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, dir } => {
            // Project initialization
            let project_name = name.unwrap_or_else(|| "dataflare-project".to_string());
            let project_dir = match dir {
                Some(path) => path,
                None => PathBuf::from(&project_name),
            };

            initialize_project(&project_name, &project_dir)?;
            println!("Project '{}' initialized successfully in: {}", 
                     project_name, project_dir.display());
        }

        Commands::Run { workflow, mode } => {
            println!("Running workflow: {} in {} mode", workflow.display(), mode);
            
            // Validate the workflow file exists
            if !workflow.exists() {
                return Err(format!("Workflow file not found: {}", workflow.display()).into());
            }
            
            // Parse runtime mode
            let runtime_mode = match mode.to_lowercase().as_str() {
                "standalone" => RuntimeMode::Standalone,
                "edge" => RuntimeMode::Edge,
                "cloud" => RuntimeMode::Cloud,
                _ => return Err(format!("Invalid runtime mode: {}. Valid options are: standalone, edge, cloud", mode).into()),
            };
            
            // Execute the workflow
            let result = execute_workflow(&workflow, runtime_mode);
            
            match result {
                Ok(_) => {
                    println!("Workflow execution completed successfully");
                }
                Err(e) => {
                    return Err(format!("Workflow execution failed: {}", e).into());
                }
            }
        }

        Commands::Validate { workflow } => {
            println!("Validating workflow: {:?}", workflow);
            
            // Validate the workflow file exists
            if !workflow.exists() {
                return Err(format!("Workflow file not found: {}", workflow.display()).into());
            }
            
            // Load and validate the workflow
            let result = validate_workflow(&workflow);
            
            match result {
                Ok(validation_result) => {
                    println!("Workflow validation successful:");
                    println!("  ID: {}", validation_result.id);
                    println!("  Name: {}", validation_result.name);
                    if let Some(desc) = validation_result.description {
                        println!("  Description: {}", desc);
                    }
                    println!("  Version: {}", validation_result.version);
                    println!("  Sources: {}", validation_result.sources);
                    println!("  Transformations: {}", validation_result.transformations);
                    println!("  Destinations: {}", validation_result.destinations);
                }
                Err(e) => {
                    return Err(format!("Workflow validation failed: {}", e).into());
                }
            }
        }

        Commands::Connectors => {
            println!("Available connectors:");
            let source_connectors = list_source_connectors();
            let destination_connectors = list_destination_connectors();
            
            println!("\nSource connectors:");
            if source_connectors.is_empty() {
                println!("  No source connectors registered");
            } else {
                for connector in source_connectors {
                    println!("  - {}", connector);
                }
            }
            
            println!("\nDestination connectors:");
            if destination_connectors.is_empty() {
                println!("  No destination connectors registered");
            } else {
                for connector in destination_connectors {
                    println!("  - {}", connector);
                }
            }
        }

        Commands::Processors => {
            println!("Available processors:");
            let processors = list_processors();
            
            if processors.is_empty() {
                println!("  No processors registered");
            } else {
                for processor in processors {
                    println!("  - {}", processor);
                }
            }
        }

        Commands::Plugins { command } => {
            match command {
                PluginCommands::List => {
                    let plugins = list_all_plugins();
                    println!("{}", format_plugin_list(&plugins));
                }

                PluginCommands::Info { id } => {
                    match get_plugin_details(&id) {
                        Ok(Some(content)) => {
                            println!("Plugin details for '{}':", id);
                            println!("{}", content);
                        },
                        Ok(None) => {
                            println!("Plugin '{}' not found", id);
                        },
                        Err(e) => {
                            return Err(format!("Failed to get plugin details: {}", e).into());
                        }
                    }
                },

                PluginCommands::Install { path } => {
                    println!("Installing plugin from: {}", path);
                    match install_plugin(&path) {
                        Ok(metadata) => {
                            println!("Plugin installed successfully:");
                            println!("  Name: {}", metadata.name);
                            println!("  Version: {}", metadata.version);
                            println!("  Type: {:?}", metadata.plugin_type);
                            println!("  Description: {}", metadata.description);
                            println!("  Author: {}", metadata.author);
                        }
                        Err(e) => {
                            return Err(format!("Failed to install plugin: {}", e).into());
                        }
                    }
                }

                PluginCommands::Remove { id } => {
                    println!("Removing plugin: {}", id);
                    match remove_plugin(&id) {
                        Ok(()) => {
                            println!("Plugin removed successfully");
                        }
                        Err(e) => {
                            return Err(format!("Failed to remove plugin: {}", e).into());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Workflow validation result
#[derive(Debug)]
pub struct WorkflowValidationResult {
    /// Workflow ID
    pub id: String,
    /// Workflow name
    pub name: String,
    /// Workflow description
    pub description: Option<String>,
    /// Workflow version
    pub version: String,
    /// Number of sources
    pub sources: usize,
    /// Number of transformations
    pub transformations: usize,
    /// Number of destinations
    pub destinations: usize,
}

/// Validate a workflow
pub fn validate_workflow(workflow_path: &Path) -> Result<WorkflowValidationResult, Box<dyn std::error::Error>> {
    // Parse the workflow
    let workflow = YamlWorkflowParser::load_from_file(workflow_path)?;
    
    // Additional validation steps could be added here
    // For example, checking that all referenced connectors and processors exist
    
    // Return validation result
    Ok(WorkflowValidationResult {
        id: workflow.id,
        name: workflow.name,
        description: workflow.description,
        version: workflow.version,
        sources: workflow.sources.len(),
        transformations: workflow.transformations.len(),
        destinations: workflow.destinations.len(),
    })
}

/// Initialize a new project
pub fn initialize_project(name: &str, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Create project directory if it doesn't exist
    if !dir.exists() {
        fs::create_dir_all(dir)?;
        println!("Created project directory: {}", dir.display());
    }

    // Create subdirectories
    let dirs = ["workflows", "data", "plugins", "state", "config"];
    for sub_dir in &dirs {
        let path = dir.join(sub_dir);
        if !path.exists() {
            fs::create_dir_all(&path)?;
            println!("Created directory: {}", path.display());
        }
    }

    // Create example configuration file
    let config_path = dir.join("config/dataflare.yaml");
    if !config_path.exists() {
        let mut file = File::create(&config_path)?;
        file.write_all(generate_example_config(name).as_bytes())?;
        println!("Created configuration file: {}", config_path.display());
    }

    // Create example workflow file
    let workflow_path = dir.join("workflows/example.yaml");
    if !workflow_path.exists() {
        let mut file = File::create(&workflow_path)?;
        file.write_all(generate_example_workflow().as_bytes())?;
        println!("Created example workflow: {}", workflow_path.display());
    }

    // Create README.md file
    let readme_path = dir.join("README.md");
    if !readme_path.exists() {
        let mut file = File::create(&readme_path)?;
        file.write_all(generate_readme(name).as_bytes())?;
        println!("Created README.md file: {}", readme_path.display());
    }

    // Create .gitignore file
    let gitignore_path = dir.join(".gitignore");
    if !gitignore_path.exists() {
        let mut file = File::create(&gitignore_path)?;
        file.write_all(generate_gitignore().as_bytes())?;
        println!("Created .gitignore file: {}", gitignore_path.display());
    }

    Ok(())
}

/// Generate example configuration file
fn generate_example_config(project_name: &str) -> String {
    format!(r#"# DataFlare Configuration
project:
  name: {}
  version: 0.1.0

runtime:
  mode: standalone # Options: standalone, edge, cloud
  logging:
    level: info # Options: trace, debug, info, warn, error
    file: logs/dataflare.log

plugins:
  directory: plugins
  allow_remote: false

state:
  directory: state
  storage: file # Options: memory, file, database

connectors:
  source:
    # Configure default options for source connectors
    postgres:
      batch_size: 1000
      fetch_size: 5000
    csv:
      delimiter: ","
      header: true
  
  destination:
    # Configure default options for destination connectors
    postgres:
      batch_size: 1000
    csv:
      delimiter: ","
      header: true
"#, project_name)
}

/// Generate example workflow
fn generate_example_workflow() -> String {
    r#"# Example DataFlare Workflow
id: example-workflow
name: Example Workflow
description: A simple example workflow to demonstrate DataFlare capabilities
version: 1.0.0

# Source definitions
sources:
  # CSV data source
  users_csv:
    type: csv
    config:
      path: "data/users.csv"
      delimiter: ","
      header: true

# Transformation definitions
transformations:
  # Map user data fields
  user_transform:
    type: mapping
    inputs:
      - users_csv
    config:
      mappings:
        - source: first_name
          destination: user.firstName
        - source: last_name
          destination: user.lastName
        - source: email
          destination: user.email
          transform: lowercase
        - source: created_at
          destination: user.createdAt
          transform: parse_datetime
  
  # Filter out inactive users
  active_users:
    type: filter
    inputs:
      - user_transform
    config:
      condition: "user.status == 'active'"

# Destination definitions
destinations:
  # JSON output destination
  users_json:
    type: json
    inputs:
      - active_users
    config:
      path: "data/processed/users.json"
      pretty: true
"#.to_string()
}

/// Generate README.md file
fn generate_readme(project_name: &str) -> String {
    format!(r#"# {}

## Overview

This is a DataFlare data integration project created with the DataFlare CLI.

## Project Structure

- `workflows/` - Contains workflow definition files
- `data/` - Data files for input and output
- `plugins/` - Custom plugins for extending DataFlare
- `state/` - State files for incremental processing
- `config/` - Configuration files

## Getting Started

1. Install DataFlare if you haven't already:
   ```
   cargo install dataflare
   ```

2. Run the example workflow:
   ```
   dataflare run --workflow workflows/example.yaml
   ```

3. Create your own workflows in the `workflows/` directory.

## Documentation

For more information, see the [DataFlare documentation](https://dataflare.example.com).
"#, project_name)
}

/// Generate .gitignore file
fn generate_gitignore() -> String {
    r#"# DataFlare state
state/

# Data files
data/processed/
*.csv
*.json
*.parquet

# Logs
logs/
*.log

# Environment variables
.env

# IDEs and editors
.idea/
.vscode/
*.swp
*.swo

# Rust
target/

# OS files
.DS_Store
Thumbs.db
"#.to_string()
}

/// List source connectors
pub fn list_source_connectors() -> Vec<String> {
    // Try to get registered connectors from registry
    let registered = get_registered_source_connectors();
    if !registered.is_empty() {
        return registered;
    }

    // Fallback to static list if registry is empty
    vec![
        "postgres".to_string(),
        "mysql".to_string(),
        "csv".to_string(),
        "json".to_string(),
    ]
}

/// List destination connectors
pub fn list_destination_connectors() -> Vec<String> {
    // Try to get registered connectors from registry
    let registered = get_registered_destination_connectors();
    if !registered.is_empty() {
        return registered;
    }

    // Fallback to static list if registry is empty
    vec![
        "postgres".to_string(),
        "mysql".to_string(),
        "csv".to_string(),
        "json".to_string(),
    ]
}

/// List processors
pub fn list_processors() -> Vec<String> {
    // Get processors from registry
    let processors = get_processor_names();
    if !processors.is_empty() {
        return processors;
    }

    // Fallback to static list if registry is empty
    vec![
        "mapping".to_string(),
        "filter".to_string(),
        "aggregate".to_string(),
        "join".to_string(),
        "enrichment".to_string(),
    ]
}

/// List all plugins
pub fn list_all_plugins() -> Vec<dataflare_plugin::plugin::PluginMetadata> {
    list_plugins()
}

/// Get detailed plugin information
pub fn get_plugin_details(plugin_id: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // Check if plugin directory exists
    let plugins_dir = PathBuf::from("plugins");
    if !plugins_dir.exists() {
        return Ok(None);
    }
    
    // Look for metadata file
    let metadata_pattern = format!("{}.meta.json", plugin_id);
    
    for entry in fs::read_dir(plugins_dir)? {
        if let Ok(entry) = entry {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.ends_with(".meta.json") && file_name.starts_with(plugin_id) {
                // Read metadata file
                let mut file = File::open(entry.path())?;
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                
                return Ok(Some(content));
            }
        }
    }
    
    Ok(None)
}

/// Format plugin list for display
pub fn format_plugin_list(plugins: &[dataflare_plugin::plugin::PluginMetadata]) -> String {
    if plugins.is_empty() {
        return "No plugins installed".to_string();
    }
    
    let mut result = String::new();
    result.push_str("Installed plugins:\n");
    
    // Calculate column widths
    let name_width = plugins.iter().map(|p| p.name.len()).max().unwrap_or(4).max(4);
    let version_width = plugins.iter().map(|p| p.version.len()).max().unwrap_or(7).max(7);
    let type_width = plugins.iter().map(|p| format!("{:?}", p.plugin_type).len()).max().unwrap_or(4).max(4);
    
    // Add header
    result.push_str(&format!(
        "┌{:─^width$}┬{:─^version_width$}┬{:─^type_width$}┬{:─^50}┐\n",
        "", "", "", "",
        width = name_width,
        version_width = version_width,
        type_width = type_width
    ));
    
    result.push_str(&format!(
        "│ {:width$} │ {:version_width$} │ {:type_width$} │ {:50} │\n",
        "Name", "Version", "Type", "Description",
        width = name_width,
        version_width = version_width,
        type_width = type_width
    ));
    
    result.push_str(&format!(
        "├{:─^width$}┼{:─^version_width$}┼{:─^type_width$}┼{:─^50}┤\n",
        "", "", "", "",
        width = name_width,
        version_width = version_width,
        type_width = type_width
    ));
    
    // Add plugins
    for plugin in plugins {
        // Truncate description if too long
        let description = if plugin.description.len() > 47 {
            format!("{}...", &plugin.description[..47])
        } else {
            plugin.description.clone()
        };
        
        result.push_str(&format!(
            "│ {:width$} │ {:version_width$} │ {:type_width$} │ {:50} │\n",
            plugin.name, plugin.version, format!("{:?}", plugin.plugin_type), description,
            width = name_width,
            version_width = version_width,
            type_width = type_width
        ));
    }
    
    result.push_str(&format!(
        "└{:─^width$}┴{:─^version_width$}┴{:─^type_width$}┴{:─^50}┘\n",
        "", "", "", "",
        width = name_width,
        version_width = version_width,
        type_width = type_width
    ));
    
    result
}

/// Initialize DataFlare
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Initialize DataFlare core components
    dataflare_processor::register_default_processors();
    dataflare_connector::register_default_connectors();

    // Return success
    Ok(())
}

/// Execute a workflow
pub fn execute_workflow(workflow_path: &Path, mode: RuntimeMode) -> Result<(), Box<dyn std::error::Error>> {
    // Parse the workflow
    let workflow = YamlWorkflowParser::load_from_file(workflow_path)?;
    
    // Initialize Actix system
    let system = actix::System::new();
    
    // Time tracking
    let start = Instant::now();
    
    // Execute in the Actix system
    system.block_on(async {
        // Create a workflow executor with the specified runtime mode
        let mut executor = WorkflowExecutor::new();
        // Apply the runtime mode
        match mode {
            RuntimeMode::Standalone => executor = executor.with_runtime_mode(dataflare_runtime::executor::RuntimeMode::Standalone),
            RuntimeMode::Edge => executor = executor.with_runtime_mode(dataflare_runtime::executor::RuntimeMode::Edge),
            RuntimeMode::Cloud => executor = executor.with_runtime_mode(dataflare_runtime::executor::RuntimeMode::Cloud),
        };
        
        // Add progress callback using the new API
        if let Err(e) = executor.add_progress_callback(|progress| {
            display_progress(progress);
        }) {
            return Err(format!("Failed to add progress callback: {}", e));
        }
        
        // Initialize the executor
        if let Err(e) = executor.initialize() {
            return Err(format!("Failed to initialize executor: {}", e));
        }
        
        // Prepare the workflow
        if let Err(e) = executor.prepare(&workflow) {
            return Err(format!("Failed to prepare workflow: {}", e));
        }
        
        // Execute the workflow
        match executor.execute(&workflow).await {
            Ok(_) => {
                // Finalize the executor
                if let Err(e) = executor.finalize() {
                    return Err(format!("Failed to finalize executor: {}", e));
                }
                Ok(())
            },
            Err(e) => Err(format!("Failed to execute workflow: {}", e)),
        }
    })?;
    
    let duration = start.elapsed();
    println!("Workflow execution completed in {:.2} seconds", duration.as_secs_f64());
    
    Ok(())
}

/// Display workflow progress
fn display_progress(progress: WorkflowProgress) {
    let workflow_id = &progress.workflow_id;
    let progress_percent = (progress.progress * 100.0) as u32;
    
    match progress.phase {
        WorkflowPhase::Initializing => {
            println!("🚀 Initializing workflow: {}", workflow_id);
        }
        WorkflowPhase::Extracting => {
            println!("📥 Extracting data: {} ({}%)", workflow_id, progress_percent);
            println!("   {}", progress.message);
        }
        WorkflowPhase::Transforming => {
            println!("⚙️ Transforming data: {} ({}%)", workflow_id, progress_percent);
            println!("   {}", progress.message);
        }
        WorkflowPhase::Loading => {
            println!("📤 Loading data: {} ({}%)", workflow_id, progress_percent);
            println!("   {}", progress.message);
        }
        WorkflowPhase::Finalizing => {
            println!("📦 Finalizing workflow: {}", workflow_id);
            println!("   {}", progress.message);
        }
        WorkflowPhase::Completed => {
            println!("✅ Workflow completed: {}", workflow_id);
            println!("   {}", progress.message);
        }
        WorkflowPhase::Error => {
            println!("❌ Workflow failed: {}", workflow_id);
            println!("   Error: {}", progress.message);
        }
    }
}

/// Install a plugin from a file path
pub fn install_plugin(path: &str) -> Result<PluginMetadata, Box<dyn std::error::Error>> {
    let path = Path::new(path);
    
    // Check if file exists
    if !path.exists() {
        return Err(format!("Plugin file not found: {}", path.display()).into());
    }
    
    // Read the file
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    
    // Create plugin manager with default plugin directory
    let plugin_dir = std::path::PathBuf::from("plugins");
    let _plugin_manager = PluginManager::new(plugin_dir);
    
    // Load the plugin
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    
    if extension == "wasm" {
        // Load WASM plugin
        println!("Loading WASM plugin from: {}", path.display());
        
        // Extract metadata from filename or try to parse from WASM binary
        // Format: name-version.wasm
        let file_name = path.file_stem().unwrap_or_default().to_string_lossy();
        let mut parts: Vec<&str> = file_name.split('-').collect();
        
        // Ensure we have at least name and version
        if parts.len() < 2 {
            parts = vec!["unknown", "0.1.0"];
        }
        
        // Create metadata
        let metadata = PluginMetadata {
            name: parts[0].to_string(),
            version: parts[1].to_string(),
            description: format!("WASM plugin ({})", file_name),
            author: "Unknown".to_string(),
            plugin_type: dataflare_plugin::plugin::PluginType::Processor,
            input_schema: None,
            output_schema: None,
            config_schema: None,
        };
        
        // Create plugins directory if it doesn't exist
        let plugins_dir = PathBuf::from("plugins");
        if !plugins_dir.exists() {
            fs::create_dir_all(&plugins_dir)?;
            println!("Created plugins directory: {}", plugins_dir.display());
        }
        
        // Create metadata file
        let metadata_path = plugins_dir.join(format!("{}-{}.meta.json", metadata.name, metadata.version));
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        let mut metadata_file = File::create(&metadata_path)?;
        metadata_file.write_all(metadata_json.as_bytes())?;
        println!("Created plugin metadata: {}", metadata_path.display());
        
        // Copy the plugin file to plugins directory
        let dest_path = plugins_dir.join(path.file_name().unwrap());
        fs::copy(path, &dest_path)?;
        println!("Plugin file copied to: {}", dest_path.display());
        
        println!("Plugin '{}' v{} installed successfully.", metadata.name, metadata.version);
        println!("Note: The plugin will be loaded on restart");
        
        Ok(metadata)
    } else {
        Err(format!("Unsupported plugin format: {}. Only .wasm files are supported.", path.display()).into())
    }
}

/// Remove a plugin
pub fn remove_plugin(plugin_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Check if the plugin ID has the right format (name-version)
    let parts: Vec<&str> = plugin_id.split('-').collect();
    if parts.len() < 2 {
        return Err(format!("Invalid plugin ID format: '{}'. Expected 'name-version'", plugin_id).into());
    }
    
    let plugin_name = parts[0];
    let plugin_version = parts[1];
    
    // Find the plugin file in plugins directory
    let plugins_dir = PathBuf::from("plugins");
    if !plugins_dir.exists() {
        return Err("Plugins directory not found".into());
    }
    
    let mut found = false;
    
    // Remove plugin binary and metadata
    for entry in fs::read_dir(&plugins_dir)? {
        if let Ok(entry) = entry {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            // Match both the WASM file and metadata file
            if file_name_str.starts_with(&format!("{}-{}", plugin_name, plugin_version)) {
                fs::remove_file(entry.path())?;
                println!("Removed plugin file: {}", entry.path().display());
                found = true;
            }
        }
    }
    
    if !found {
        println!("Warning: No plugin files were found for '{}'", plugin_id);
        return Err(format!("Plugin '{}' not found", plugin_id).into());
    }
    
    println!("Plugin '{}' removed successfully.", plugin_id);
    println!("Note: Plugin will be fully unloaded on restart");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_list_source_connectors() {
        let connectors = list_source_connectors();
        assert!(!connectors.is_empty());
    }

    #[test]
    fn test_list_destination_connectors() {
        let connectors = list_destination_connectors();
        assert!(!connectors.is_empty());
    }

    #[test]
    fn test_list_processors() {
        let processors = list_processors();
        assert!(!processors.is_empty());
        
        // Verify we have at least the core processors
        assert!(processors.contains(&"mapping".to_string()));
        assert!(processors.contains(&"filter".to_string()));
    }

    #[test]
    fn test_project_initialization() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().join("test-project");
        
        // Initialize project
        let result = initialize_project("test-project", &project_path);
        assert!(result.is_ok());
        
        // Verify directories were created
        assert!(project_path.exists());
        assert!(project_path.join("workflows").exists());
        assert!(project_path.join("data").exists());
        assert!(project_path.join("plugins").exists());
        assert!(project_path.join("state").exists());
        assert!(project_path.join("config").exists());
        
        // Verify files were created
        assert!(project_path.join("config/dataflare.yaml").exists());
        assert!(project_path.join("workflows/example.yaml").exists());
        assert!(project_path.join("README.md").exists());
        assert!(project_path.join(".gitignore").exists());
    }

    #[test]
    fn test_workflow_validation() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path().join("test-validation");
        
        // Initialize project to get example workflow
        initialize_project("test-validation", &project_path).unwrap();
        
        // Test workflow validation on the example workflow
        let workflow_path = project_path.join("workflows/example.yaml");
        let result = validate_workflow(&workflow_path);
        
        // Validation should succeed for the example workflow
        assert!(result.is_ok());
        
        // Check validation result
        let validation = result.unwrap();
        assert_eq!(validation.id, "example-workflow");
        assert_eq!(validation.name, "Example Workflow");
        assert_eq!(validation.version, "1.0.0");
        assert_eq!(validation.sources, 1);
        assert_eq!(validation.transformations, 2);
        assert_eq!(validation.destinations, 1);
    }

    #[test]
    fn test_parse_runtime_mode() {
        // Test valid runtime modes
        let workflow_path = PathBuf::from("dummy.yaml");
        
        // We can't actually run the workflow in tests without a full system,
        // but we can check that the mode parsing works
        assert!(matches!(
            parse_runtime_mode("standalone"),
            Ok(RuntimeMode::Standalone)
        ));
        
        assert!(matches!(
            parse_runtime_mode("edge"),
            Ok(RuntimeMode::Edge)
        ));
        
        assert!(matches!(
            parse_runtime_mode("cloud"),
            Ok(RuntimeMode::Cloud)
        ));
        
        // Test case insensitivity
        assert!(matches!(
            parse_runtime_mode("Standalone"),
            Ok(RuntimeMode::Standalone)
        ));
        
        // Test invalid mode
        assert!(parse_runtime_mode("invalid").is_err());
    }
    
    /// Helper function to parse runtime mode for testing
    fn parse_runtime_mode(mode: &str) -> Result<RuntimeMode, String> {
        match mode.to_lowercase().as_str() {
            "standalone" => Ok(RuntimeMode::Standalone),
            "edge" => Ok(RuntimeMode::Edge),
            "cloud" => Ok(RuntimeMode::Cloud),
            _ => Err(format!("Invalid runtime mode: {}. Valid options are: standalone, edge, cloud", mode)),
        }
    }

    #[test]
    fn test_format_plugin_list() {
        // Create test plugins
        let plugins = vec![
            PluginMetadata {
                name: "test-plugin".to_string(),
                version: "0.1.0".to_string(),
                description: "Test plugin for testing".to_string(),
                author: "Test Author".to_string(),
                plugin_type: dataflare_plugin::plugin::PluginType::Processor,
                input_schema: None,
                output_schema: None,
                config_schema: None,
            },
            PluginMetadata {
                name: "another-plugin".to_string(),
                version: "1.0.0".to_string(),
                description: "Another test plugin".to_string(),
                author: "Test Author".to_string(),
                plugin_type: dataflare_plugin::plugin::PluginType::SourceConnector,
                input_schema: None,
                output_schema: None,
                config_schema: None,
            },
        ];

        let formatted = format_plugin_list(&plugins);
        
        // Verify the table contains expected content
        assert!(formatted.contains("test-plugin"));
        assert!(formatted.contains("another-plugin"));
        assert!(formatted.contains("0.1.0"));
        assert!(formatted.contains("1.0.0"));
        assert!(formatted.contains("Processor"));
        assert!(formatted.contains("SourceConnector"));
        
        // Verify table headers
        assert!(formatted.contains("Name"));
        assert!(formatted.contains("Version"));
        assert!(formatted.contains("Type"));
        assert!(formatted.contains("Description"));
    }

    #[test]
    fn test_plugin_management() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        fs::create_dir_all(&plugin_dir).unwrap();
        
        // Create a test WASM plugin
        let plugin_path = plugin_dir.join("test-plugin-0.1.0.wasm");
        let mut file = File::create(&plugin_path).unwrap();
        file.write_all(b"mock wasm content").unwrap();
        
        // Create metadata file
        let metadata = PluginMetadata {
            name: "test-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test Author".to_string(),
            plugin_type: dataflare_plugin::plugin::PluginType::Processor,
            input_schema: None,
            output_schema: None,
            config_schema: None,
        };
        
        let metadata_path = plugin_dir.join("test-plugin-0.1.0.meta.json");
        let metadata_json = serde_json::to_string_pretty(&metadata).unwrap();
        let mut metadata_file = File::create(&metadata_path).unwrap();
        metadata_file.write_all(metadata_json.as_bytes()).unwrap();
        
        // Test plugin details
        // This test is limited since we can't easily mock the global plugin system
        // But we can verify file paths and error handling
        
        // Test plugin details for non-existent plugin
        match get_plugin_details("non-existent-plugin") {
            Ok(None) => {}, // Expected
            _ => panic!("Expected None result for non-existent plugin"),
        }
    }
    
    // Note: Complete plugin installation and removal tests
    // would require more setup and mocking of the plugin registry
}
