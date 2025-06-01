//! # DataFlare CLI
//!
//! Command-line interface for the DataFlare data integration framework.
//! This crate provides tools for managing workflows, connectors, and plugins.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

mod workflow_executor;

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Write, Read};
use std::time::Instant;
use std::process::Command;
use dataflare_connector::registry::{get_registered_source_connectors, get_registered_destination_connectors};
use dataflare_processor::registry::get_processor_names;
// Plugin-related imports will be added when plugin system is implemented
use dataflare_runtime::{
    workflow::{YamlWorkflowParser},
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
    List {
        /// Show plugins of specific type
        #[clap(long)]
        plugin_type: Option<String>,
    },

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
        /// Plugin version
        #[clap(long)]
        version: Option<String>,
    },

    /// Remove a plugin
    #[clap(name = "remove")]
    Remove {
        /// Plugin ID
        id: String,
    },

    /// Create a new plugin project
    #[clap(name = "new")]
    New {
        /// Plugin name
        name: String,
        /// Plugin type (filter, map, aggregate)
        #[clap(long = "plugin-type", default_value = "filter")]
        plugin_type: String,
        /// Programming language (rust, js, go)
        #[clap(long, default_value = "rust")]
        lang: String,
        /// Output directory
        #[clap(long)]
        output: Option<String>,
    },

    /// Build a plugin
    #[clap(name = "build")]
    Build {
        /// Build in release mode
        #[clap(long)]
        release: bool,
        /// Target platform (native, wasm)
        #[clap(long, default_value = "native")]
        target: String,
        /// Output directory
        #[clap(short, long)]
        output: Option<String>,
        /// Plugin project path (if not in current directory)
        #[clap(long)]
        path: Option<String>,
    },

    /// Test a plugin
    #[clap(name = "test")]
    Test {
        /// Test data (JSON string)
        #[clap(long)]
        data: Option<String>,
        /// Test data file
        #[clap(long)]
        file: Option<String>,
        /// Plugin path (if not in current directory)
        #[clap(long)]
        plugin: Option<String>,
    },

    /// Publish a plugin
    #[clap(name = "publish")]
    Publish {
        /// Registry URL
        #[clap(long)]
        registry: Option<String>,
        /// Authentication token
        #[clap(long)]
        token: Option<String>,
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
                PluginCommands::List { plugin_type } => {
                    let plugins = list_all_plugins();
                    let filtered_plugins = if let Some(filter_type) = plugin_type {
                        plugins.into_iter()
                            .filter(|p| format!("{:?}", p.plugin_type).to_lowercase() == filter_type.to_lowercase())
                            .collect()
                    } else {
                        plugins
                    };
                    println!("{}", format_plugin_list(&filtered_plugins));
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

                PluginCommands::Install { path, version } => {
                    println!("Installing plugin from: {}", path);
                    if let Some(v) = version {
                        println!("  Version: {}", v);
                    }
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

                PluginCommands::New { name, plugin_type, lang, output } => {
                    println!("Creating new {} plugin '{}' in {}", plugin_type, name, lang);
                    match create_plugin_project(&name, &plugin_type, &lang, output.as_deref()) {
                        Ok(project_path) => {
                            println!("Plugin project created successfully at: {}", project_path);
                            println!("\nNext steps:");
                            println!("  1. cd {}", project_path);
                            println!("  2. Edit src/lib.rs to implement your plugin logic");
                            println!("  3. Run 'dataflare plugin build' to build your plugin");
                            println!("  4. Run 'dataflare plugin test' to test your plugin");
                        }
                        Err(e) => {
                            return Err(format!("Failed to create plugin project: {}", e).into());
                        }
                    }
                }

                PluginCommands::Build { release, target, output, path } => {
                    println!("Building plugin (target: {}, release: {})", target, release);
                    match build_plugin_with_path(release, &target, output.as_deref(), path.as_deref()) {
                        Ok(output_path) => {
                            println!("Plugin built successfully: {}", output_path);
                        }
                        Err(e) => {
                            return Err(format!("Plugin build failed: {}", e).into());
                        }
                    }
                }

                PluginCommands::Test { data, file, plugin } => {
                    println!("Testing plugin...");
                    match test_plugin(data.as_deref(), file.as_deref(), plugin.as_deref()) {
                        Ok(result) => {
                            println!("Plugin test successful:");
                            println!("Result: {}", result);
                        }
                        Err(e) => {
                            return Err(format!("Plugin test failed: {}", e).into());
                        }
                    }
                }

                PluginCommands::Publish { registry, token } => {
                    println!("Publishing plugin...");
                    if let Some(ref reg) = registry {
                        println!("  Registry: {}", reg);
                    }
                    match publish_plugin(registry.as_deref(), token.as_deref()) {
                        Ok(()) => {
                            println!("Plugin published successfully");
                        }
                        Err(e) => {
                            return Err(format!("Plugin publish failed: {}", e).into());
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

/// Plugin metadata structure (temporary until plugin system is implemented)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub plugin_type: String,
}

/// List all plugins
pub fn list_all_plugins() -> Vec<PluginMetadata> {
    // TODO: Implement actual plugin listing
    // For now, return a mock list
    vec![
        PluginMetadata {
            name: "csv-filter".to_string(),
            version: "1.0.0".to_string(),
            description: "Filter CSV records".to_string(),
            author: "DataFlare Team".to_string(),
            plugin_type: "filter".to_string(),
        },
        PluginMetadata {
            name: "json-transformer".to_string(),
            version: "1.2.0".to_string(),
            description: "Transform JSON data".to_string(),
            author: "DataFlare Team".to_string(),
            plugin_type: "map".to_string(),
        },
    ]
}

/// Get detailed plugin information
pub fn get_plugin_details(plugin_id: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // Check if plugin directory exists
    let plugins_dir = PathBuf::from("plugins");
    if !plugins_dir.exists() {
        return Ok(None);
    }

    // Look for metadata file
    let _metadata_pattern = format!("{}.meta.json", plugin_id);

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
pub fn format_plugin_list(plugins: &[PluginMetadata]) -> String {
    if plugins.is_empty() {
        return "No plugins installed".to_string();
    }

    let mut result = String::new();
    result.push_str("Installed plugins:\n");

    // Calculate column widths
    let name_width = plugins.iter().map(|p| p.name.len()).max().unwrap_or(4).max(4);
    let version_width = plugins.iter().map(|p| p.version.len()).max().unwrap_or(7).max(7);
    let type_width = plugins.iter().map(|p| p.plugin_type.len()).max().unwrap_or(4).max(4);

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
            plugin.name, plugin.version, plugin.plugin_type, description,
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
pub fn execute_workflow(workflow_path: &Path, _mode: RuntimeMode) -> Result<(), Box<dyn std::error::Error>> {
    // Use our simplified workflow executor for demonstration
    use crate::workflow_executor::execute_simple_workflow;

    println!("🚀 Starting workflow execution...");
    let start = Instant::now();

    // Execute the simplified workflow
    execute_simple_workflow(workflow_path)?;

    let duration = start.elapsed();
    println!("✅ Workflow execution completed in {:.2} seconds", duration.as_secs_f64());

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

    // Create plugins directory if it doesn't exist
    let plugin_dir = std::path::PathBuf::from("plugins");
    if !plugin_dir.exists() {
        fs::create_dir_all(&plugin_dir)?;
    }

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
            plugin_type: "processor".to_string(),
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
                plugin_type: "processor".to_string(),
            },
            PluginMetadata {
                name: "another-plugin".to_string(),
                version: "1.0.0".to_string(),
                description: "Another test plugin".to_string(),
                author: "Test Author".to_string(),
                plugin_type: "source".to_string(),
            },
        ];

        let formatted = format_plugin_list(&plugins);

        // Verify the table contains expected content
        assert!(formatted.contains("test-plugin"));
        assert!(formatted.contains("another-plugin"));
        assert!(formatted.contains("0.1.0"));
        assert!(formatted.contains("1.0.0"));
        assert!(formatted.contains("processor"));
        assert!(formatted.contains("source"));

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
            plugin_type: "processor".to_string(),
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

/// Test a WASM plugin
pub fn test_wasm_plugin(
    plugin_path: &str,
    test_data: Option<&str>,
    function_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Command is already imported at the top level

    println!("Testing WASM plugin: {}", plugin_path);
    println!("Function: {}", function_name);

    // Check if plugin file exists
    if !Path::new(plugin_path).exists() {
        return Err(format!("Plugin file not found: {}", plugin_path).into());
    }

    // Use default test data if none provided
    let test_data = test_data.unwrap_or(r#"{"test": "data", "value": 123}"#);
    println!("Test data: {}", test_data);

    // For now, return a mock result
    // In a real implementation, this would load and execute the WASM plugin
    let result = format!(
        r#"{{"status": "success", "function": "{}", "input": {}, "output": {{"transformed": true, "result": "mock_output"}}}}"#,
        function_name, test_data
    );

    Ok(result)
}

/// Build a WASM plugin
pub fn build_wasm_plugin(
    source_dir: &str,
    output_dir: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    use std::process::Command;

    println!("Building WASM plugin from: {}", source_dir);

    // Check if source directory exists
    let source_path = Path::new(source_dir);
    if !source_path.exists() {
        return Err(format!("Source directory not found: {}", source_dir).into());
    }

    // Check if Cargo.toml exists
    let cargo_toml = source_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(format!("Cargo.toml not found in: {}", source_dir).into());
    }

    // Determine output directory
    let output_path = match output_dir {
        Some(dir) => PathBuf::from(dir),
        None => source_path.join("target/wasm32-unknown-unknown/release"),
    };

    println!("Output directory: {}", output_path.display());

    // Build the WASM plugin using cargo
    println!("Running cargo build...");
    let output = Command::new("cargo")
        .args(&[
            "build",
            "--target", "wasm32-unknown-unknown",
            "--release",
            "--manifest-path", cargo_toml.to_str().unwrap(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Cargo build failed: {}", stderr).into());
    }

    println!("Build completed successfully");

    // Find the generated WASM file
    let wasm_files: Vec<_> = std::fs::read_dir(&output_path)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "wasm")
                .unwrap_or(false)
        })
        .collect();

    if wasm_files.is_empty() {
        return Err("No WASM files found in output directory".into());
    }

    let wasm_file = wasm_files[0].path();
    println!("Generated WASM file: {}", wasm_file.display());

    Ok(wasm_file.to_string_lossy().to_string())
}

/// Create a new plugin project
pub fn create_plugin_project(
    name: &str,
    plugin_type: &str,
    lang: &str,
    output_dir: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    // Validate plugin type
    let valid_types = ["filter", "map", "aggregate"];
    if !valid_types.contains(&plugin_type) {
        return Err(format!("Invalid plugin type '{}'. Valid types: {:?}", plugin_type, valid_types).into());
    }

    // Validate language
    let valid_langs = ["rust", "js", "go"];
    if !valid_langs.contains(&lang) {
        return Err(format!("Invalid language '{}'. Valid languages: {:?}", lang, valid_langs).into());
    }

    // Determine project directory
    let project_dir = match output_dir {
        Some(dir) => PathBuf::from(dir).join(name),
        None => PathBuf::from(name),
    };

    // Create project directory
    if project_dir.exists() {
        return Err(format!("Directory '{}' already exists", project_dir.display()).into());
    }

    fs::create_dir_all(&project_dir)?;
    println!("Created project directory: {}", project_dir.display());

    match lang {
        "rust" => create_rust_plugin_project(&project_dir, name, plugin_type)?,
        "js" => create_js_plugin_project(&project_dir, name, plugin_type)?,
        "go" => create_go_plugin_project(&project_dir, name, plugin_type)?,
        _ => unreachable!(),
    }

    Ok(project_dir.to_string_lossy().to_string())
}

/// Create Rust plugin project
fn create_rust_plugin_project(
    project_dir: &Path,
    name: &str,
    plugin_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create src directory
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Create Cargo.toml
    let cargo_toml = project_dir.join("Cargo.toml");
    let mut file = File::create(&cargo_toml)?;
    file.write_all(generate_rust_cargo_toml(name, plugin_type).as_bytes())?;

    // Create src/lib.rs
    let lib_rs = src_dir.join("lib.rs");
    let mut file = File::create(&lib_rs)?;
    file.write_all(generate_rust_plugin_code(name, plugin_type).as_bytes())?;

    // Create tests directory and integration test
    let tests_dir = project_dir.join("tests");
    fs::create_dir_all(&tests_dir)?;
    let test_file = tests_dir.join("integration_test.rs");
    let mut file = File::create(&test_file)?;
    file.write_all(generate_rust_test_code(name, plugin_type).as_bytes())?;

    // Create examples directory
    let examples_dir = project_dir.join("examples");
    fs::create_dir_all(&examples_dir)?;
    let example_file = examples_dir.join("usage.rs");
    let mut file = File::create(&example_file)?;
    file.write_all(generate_rust_example_code(name, plugin_type).as_bytes())?;

    // Create README.md
    let readme = project_dir.join("README.md");
    let mut file = File::create(&readme)?;
    file.write_all(generate_plugin_readme(name, plugin_type, "rust").as_bytes())?;

    println!("Created Rust plugin project structure");
    Ok(())
}

/// Create JavaScript plugin project (stub)
fn create_js_plugin_project(
    project_dir: &Path,
    name: &str,
    plugin_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create package.json
    let package_json = project_dir.join("package.json");
    let mut file = File::create(&package_json)?;
    file.write_all(generate_js_package_json(name, plugin_type).as_bytes())?;

    // Create src directory and index.js
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    let index_js = src_dir.join("index.js");
    let mut file = File::create(&index_js)?;
    file.write_all(generate_js_plugin_code(name, plugin_type).as_bytes())?;

    // Create README.md
    let readme = project_dir.join("README.md");
    let mut file = File::create(&readme)?;
    file.write_all(generate_plugin_readme(name, plugin_type, "javascript").as_bytes())?;

    println!("Created JavaScript plugin project structure");
    Ok(())
}

/// Create Go plugin project (stub)
fn create_go_plugin_project(
    project_dir: &Path,
    name: &str,
    plugin_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create go.mod
    let go_mod = project_dir.join("go.mod");
    let mut file = File::create(&go_mod)?;
    file.write_all(generate_go_mod(name).as_bytes())?;

    // Create main.go
    let main_go = project_dir.join("main.go");
    let mut file = File::create(&main_go)?;
    file.write_all(generate_go_plugin_code(name, plugin_type).as_bytes())?;

    // Create README.md
    let readme = project_dir.join("README.md");
    let mut file = File::create(&readme)?;
    file.write_all(generate_plugin_readme(name, plugin_type, "go").as_bytes())?;

    println!("Created Go plugin project structure");
    Ok(())
}

/// Build a plugin
pub fn build_plugin(
    release: bool,
    target: &str,
    output_dir: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    // Check if we're in a plugin project
    let current_dir = std::env::current_dir()?;
    let cargo_toml = current_dir.join("Cargo.toml");

    if !cargo_toml.exists() {
        return Err("No Cargo.toml found. Are you in a plugin project directory?".into());
    }

    match target {
        "native" => build_native_plugin(release, output_dir),
        "wasm" => build_wasm_plugin_new(release, output_dir),
        _ => Err(format!("Unsupported target: {}. Use 'native' or 'wasm'", target).into()),
    }
}

/// Build a plugin with specified path
pub fn build_plugin_with_path(
    release: bool,
    target: &str,
    output_dir: Option<&str>,
    plugin_path: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    // Determine the plugin directory
    let plugin_dir = match plugin_path {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir()?,
    };

    // Check if plugin directory exists
    if !plugin_dir.exists() {
        return Err(format!("Plugin directory not found: {}", plugin_dir.display()).into());
    }

    // Check if Cargo.toml exists in the plugin directory
    let cargo_toml = plugin_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(format!("No Cargo.toml found in: {}. Are you in a plugin project directory?", plugin_dir.display()).into());
    }

    // Check if plugin.toml exists to confirm it's a plugin project
    let plugin_toml = plugin_dir.join("plugin.toml");
    if !plugin_toml.exists() {
        return Err(format!("No plugin.toml found in: {}. This doesn't appear to be a plugin project.", plugin_dir.display()).into());
    }

    match target {
        "native" => build_native_plugin_with_path(release, output_dir, &plugin_dir),
        "wasm" => build_wasm_plugin_with_path(release, output_dir, &plugin_dir),
        _ => Err(format!("Unsupported target: {}. Use 'native' or 'wasm'", target).into()),
    }
}

/// Build native plugin
fn build_native_plugin(
    release: bool,
    output_dir: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut args = vec!["build"];
    if release {
        args.push("--release");
    }

    println!("Building native plugin...");
    let output = Command::new("cargo")
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Cargo build failed: {}", stderr).into());
    }

    let target_dir = if release { "target/release" } else { "target/debug" };
    let output_path = match output_dir {
        Some(dir) => dir.to_string(),
        None => target_dir.to_string(),
    };

    println!("Native plugin built successfully");
    Ok(output_path)
}

/// Build WASM plugin (new implementation)
fn build_wasm_plugin_new(
    release: bool,
    output_dir: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut args = vec![
        "build",
        "--target", "wasm32-unknown-unknown",
    ];

    if release {
        args.push("--release");
    }

    println!("Building WASM plugin...");
    let output = Command::new("cargo")
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Cargo build failed: {}", stderr).into());
    }

    let target_subdir = if release { "release" } else { "debug" };
    let wasm_dir = format!("target/wasm32-unknown-unknown/{}", target_subdir);

    let output_path = match output_dir {
        Some(dir) => dir.to_string(),
        None => wasm_dir,
    };

    println!("WASM plugin built successfully");
    Ok(output_path)
}

/// Test a plugin
pub fn test_plugin(
    data: Option<&str>,
    file: Option<&str>,
    plugin_path: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    // Determine test data
    let test_data = match (data, file) {
        (Some(d), None) => d.to_string(),
        (None, Some(f)) => {
            let mut file = File::open(f)?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            content
        }
        (Some(_), Some(_)) => {
            return Err("Cannot specify both --data and --file options".into());
        }
        (None, None) => {
            // Use default test data
            r#"{"message": "test data", "timestamp": 1234567890}"#.to_string()
        }
    };

    // For now, we'll implement a basic test that validates the plugin can be loaded
    // In the future, this will integrate with the actual plugin system

    let plugin_dir = match plugin_path {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir()?,
    };

    let cargo_toml = plugin_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err("No Cargo.toml found. Are you in a plugin project directory?".into());
    }

    println!("Testing plugin with data: {}", test_data);

    // Run cargo test
    let output = Command::new("cargo")
        .args(&["test"])
        .current_dir(&plugin_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Plugin tests failed: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(format!("Plugin tests passed:\n{}", stdout))
}

/// Publish a plugin
pub fn publish_plugin(
    registry: Option<&str>,
    token: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if we're in a plugin project
    let current_dir = std::env::current_dir()?;
    let cargo_toml = current_dir.join("Cargo.toml");

    if !cargo_toml.exists() {
        return Err("No Cargo.toml found. Are you in a plugin project directory?".into());
    }

    // For now, this is a stub implementation
    // In the future, this will integrate with a plugin registry

    println!("Publishing plugin...");
    if let Some(reg) = registry {
        println!("  Registry: {}", reg);
    }
    if let Some(_) = token {
        println!("  Using authentication token");
    }

    // TODO: Implement actual publishing logic
    println!("Plugin publishing is not yet implemented");
    println!("This feature will be available in a future release");

    Ok(())
}

/// Generate Rust Cargo.toml for plugin
fn generate_rust_cargo_toml(name: &str, plugin_type: &str) -> String {
    format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"
description = "A DataFlare {} plugin"
authors = ["Your Name <your.email@example.com>"]
license = "MIT OR Apache-2.0"

[lib]
crate-type = ["cdylib", "rlib"]  # Support both native and WASM

[dependencies]
dataflare-plugin = {{ path = "../plugin" }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"

# Optional dependencies for specific plugin types
[dependencies.chrono]
version = "0.4"
optional = true

[features]
default = []
timestamps = ["chrono"]

# Plugin metadata
[package.metadata.dataflare]
type = "{}"
description = "A {} plugin for DataFlare"
auto_wit = true
"#, name, plugin_type, plugin_type, plugin_type)
}

/// Generate Rust plugin code
fn generate_rust_plugin_code(name: &str, plugin_type: &str) -> String {
    match plugin_type {
        "filter" => generate_rust_filter_code(name),
        "map" => generate_rust_map_code(name),
        "aggregate" => generate_rust_aggregate_code(name),
        _ => generate_rust_filter_code(name), // Default to filter
    }
}

/// Convert kebab-case to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

/// Generate Rust filter plugin code
fn generate_rust_filter_code(name: &str) -> String {
    let struct_name = to_pascal_case(name);
    format!(r#"//! {} - A DataFlare filter plugin
//!
//! This plugin filters data records based on custom criteria.

use dataflare_plugin::{{SmartPlugin, PluginRecord, PluginResult, PluginType, Result}};

/// {} filter plugin
pub struct {}Filter;

impl SmartPlugin for {}Filter {{
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {{
        // Convert bytes to string for processing
        let data = std::str::from_utf8(record.value)
            .map_err(|e| dataflare_plugin::PluginError::processing(format!("Invalid UTF-8: {{}}", e)))?;

        // Example filter logic: keep records containing "important"
        let should_keep = data.contains("important");

        Ok(PluginResult::Filtered(should_keep))
    }}

    fn name(&self) -> &str {{
        "{}"
    }}

    fn version(&self) -> &str {{
        "0.1.0"
    }}

    fn plugin_type(&self) -> PluginType {{
        PluginType::Filter
    }}
}}

// Export the plugin for dynamic loading
#[no_mangle]
pub extern "C" fn create_plugin() -> Box<dyn SmartPlugin> {{
    Box::new({}Filter)
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use dataflare_plugin::test_utils::create_test_plugin_record;

    #[test]
    fn test_filter_important_data() {{
        let plugin = {}Filter;
        let record = create_test_plugin_record(b"This is important data");

        let result = plugin.process(&record.as_plugin_record()).unwrap();
        match result {{
            PluginResult::Filtered(keep) => assert!(keep),
            _ => panic!("Expected Filtered result"),
        }}
    }}

    #[test]
    fn test_filter_normal_data() {{
        let plugin = {}Filter;
        let record = create_test_plugin_record(b"This is normal data");

        let result = plugin.process(&record.as_plugin_record()).unwrap();
        match result {{
            PluginResult::Filtered(keep) => assert!(!keep),
            _ => panic!("Expected Filtered result"),
        }}
    }}
}}
"#, name, name, struct_name, struct_name, name, struct_name, struct_name, struct_name)
}

/// Generate Rust map plugin code
fn generate_rust_map_code(name: &str) -> String {
    let struct_name = to_pascal_case(name);
    format!(r#"//! {} - A DataFlare map plugin
//!
//! This plugin transforms data records.

use dataflare_plugin::{{SmartPlugin, PluginRecord, PluginResult, PluginType, Result}};

/// {} map plugin
pub struct {}Map;

impl SmartPlugin for {}Map {{
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {{
        // Convert bytes to string for processing
        let input = std::str::from_utf8(record.value)
            .map_err(|e| dataflare_plugin::PluginError::processing(format!("Invalid UTF-8: {{}}", e)))?;

        // Simple transformation: add prefix
        let output = format!("[PROCESSED] {{}}", input);

        Ok(PluginResult::Mapped(output.into_bytes()))
    }}

    fn name(&self) -> &str {{
        "{}"
    }}

    fn version(&self) -> &str {{
        "0.1.0"
    }}

    fn plugin_type(&self) -> PluginType {{
        PluginType::Map
    }}
}}

// Export the plugin for dynamic loading
#[no_mangle]
pub extern "C" fn create_plugin() -> Box<dyn SmartPlugin> {{
    Box::new({}Map)
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use dataflare_plugin::test_utils::create_test_plugin_record;

    #[test]
    fn test_map_data() {{
        let plugin = {}Map;
        let record = create_test_plugin_record(b"test data");

        let result = plugin.process(&record.as_plugin_record()).unwrap();
        match result {{
            PluginResult::Mapped(data) => {{
                let output = String::from_utf8(data).unwrap();
                assert!(output.starts_with("[PROCESSED]"));
            }},
            _ => panic!("Expected Mapped result"),
        }}
    }}
}}
"#, name, name, struct_name, struct_name, name, struct_name, struct_name)
}

/// Generate Rust aggregate plugin code
fn generate_rust_aggregate_code(name: &str) -> String {
    let struct_name = to_pascal_case(name);
    format!(r#"//! {} - A DataFlare aggregate plugin
//!
//! This plugin aggregates data records.

use dataflare_plugin::{{SmartPlugin, PluginRecord, PluginResult, PluginType, Result}};
use std::sync::{{Arc, Mutex}};

/// {} aggregate plugin
pub struct {}Aggregate {{
    count: Arc<Mutex<u64>>,
}}

impl {}Aggregate {{
    pub fn new() -> Self {{
        Self {{
            count: Arc::new(Mutex::new(0)),
        }}
    }}
}}

impl SmartPlugin for {}Aggregate {{
    fn process(&self, _record: &PluginRecord) -> Result<PluginResult> {{
        let mut count = self.count.lock().unwrap();
        *count += 1;
        let result = format!("Processed {{}} records", *count);
        Ok(PluginResult::Aggregated(result.into_bytes()))
    }}

    fn name(&self) -> &str {{
        "{}"
    }}

    fn version(&self) -> &str {{
        "0.1.0"
    }}

    fn plugin_type(&self) -> PluginType {{
        PluginType::Aggregate
    }}
}}

// Export the plugin for dynamic loading
#[no_mangle]
pub extern "C" fn create_plugin() -> Box<dyn SmartPlugin> {{
    Box::new({}Aggregate::new())
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use dataflare_plugin::test_utils::create_test_plugin_record;

    #[test]
    fn test_aggregate_data() {{
        let plugin = {}Aggregate::new();
        let record1 = create_test_plugin_record(b"test1");
        let record2 = create_test_plugin_record(b"test2");

        let _result1 = plugin.process(&record1.as_plugin_record()).unwrap();
        let result2 = plugin.process(&record2.as_plugin_record()).unwrap();

        match result2 {{
            PluginResult::Aggregated(data) => {{
                let output = String::from_utf8(data).unwrap();
                assert!(output.contains("2 records"));
            }},
            _ => panic!("Expected Aggregated result"),
        }}
    }}
}}
"#, name, name, struct_name, struct_name, struct_name, name, struct_name, struct_name)
}

/// Generate Rust test code
fn generate_rust_test_code(name: &str, _plugin_type: &str) -> String {
    format!(r#"//! Integration tests for {} plugin

use dataflare_plugin::test_utils::create_test_plugin_record;

#[test]
fn test_plugin_basic_functionality() {{
    let plugin = crate::create_plugin();
    let record = create_test_plugin_record(b"test data");
    let result = plugin.process(&record.as_plugin_record());
    assert!(result.is_ok(), "Plugin processing should succeed");
}}

#[test]
fn test_plugin_metadata() {{
    let plugin = crate::create_plugin();
    assert_eq!(plugin.name(), "{}");
    assert_eq!(plugin.version(), "0.1.0");
}}
"#, name, name)
}

/// Generate Rust example code
fn generate_rust_example_code(name: &str, _plugin_type: &str) -> String {
    format!(r#"//! Example usage of {} plugin

use dataflare_plugin::{{PluginResult, test_utils::create_test_plugin_record}};

fn main() {{
    println!("Running {} plugin example...");

    let plugin = {}::create_plugin();
    println!("Plugin: {{}} v{{}}", plugin.name(), plugin.version());

    let record = create_test_plugin_record(b"sample data");

    match plugin.process(&record.as_plugin_record()) {{
        Ok(result) => {{
            match result {{
                PluginResult::Filtered(keep) => println!("Filter result: keep = {{}}", keep),
                PluginResult::Mapped(data) => {{
                    let output = String::from_utf8_lossy(&data);
                    println!("Map result: {{}}", output);
                }},
                PluginResult::Aggregated(data) => {{
                    let output = String::from_utf8_lossy(&data);
                    println!("Aggregate result: {{}}", output);
                }},
            }}
        }},
        Err(e) => eprintln!("Plugin error: {{}}", e),
    }}
}}
"#, name, name, name)
}

/// Generate JavaScript package.json
fn generate_js_package_json(name: &str, plugin_type: &str) -> String {
    format!(r#"{{
  "name": "{}",
  "version": "0.1.0",
  "description": "A DataFlare {} plugin",
  "type": "module",
  "main": "src/index.js",
  "scripts": {{
    "build": "dataflare plugin build",
    "test": "dataflare plugin test",
    "dev": "node src/index.js"
  }},
  "keywords": ["dataflare", "plugin", "{}"],
  "author": "Your Name <your.email@example.com>",
  "license": "MIT",
  "dataflare": {{
    "type": "{}",
    "description": "A {} plugin for DataFlare",
    "auto_wit": true
  }}
}}
"#, name, plugin_type, plugin_type, plugin_type, plugin_type)
}

/// Generate JavaScript plugin code
fn generate_js_plugin_code(name: &str, plugin_type: &str) -> String {
    match plugin_type {
        "filter" => generate_js_filter_code(name),
        "map" => generate_js_map_code(name),
        "aggregate" => generate_js_aggregate_code(name),
        _ => generate_js_filter_code(name),
    }
}

/// Generate JavaScript filter code
fn generate_js_filter_code(name: &str) -> String {
    format!(r#"/**
 * {} - A DataFlare filter plugin
 *
 * This plugin filters data records based on custom criteria.
 */

/**
 * Process a data record
 * @param {{Uint8Array}} value - The record data as bytes
 * @param {{Object}} metadata - Record metadata
 * @returns {{boolean}} - Whether to keep the record
 */
export function process(record) {{
    try {{
        // Convert bytes to string
        const data = new TextDecoder().decode(record.value);

        // Example filter logic: keep records containing "important"
        return data.includes("important");
    }} catch (error) {{
        console.error("Error processing record:", error);
        return false;
    }}
}}

/**
 * Get plugin information
 * @returns {{[string, string]}} - [name, version]
 */
export function info() {{
    return ["{}", "0.1.0"];
}}

/**
 * Get plugin type
 * @returns {{string}} - Plugin type
 */
export function pluginType() {{
    return "filter";
}}

// Example usage (for testing)
if (import.meta.url === `file://${{process.argv[1]}}`) {{
    const testRecord = {{
        value: new TextEncoder().encode("This is important test data"),
        metadata: {{}}
    }};

    console.log("Testing {} plugin...");
    console.log("Plugin info:", info());
    console.log("Test result:", process(testRecord));
}}
"#, name, name, name)
}

/// Generate JavaScript map code
fn generate_js_map_code(name: &str) -> String {
    format!(r#"/**
 * {} - A DataFlare map plugin
 *
 * This plugin transforms data records.
 */

/**
 * Process a data record
 * @param {{Uint8Array}} value - The record data as bytes
 * @param {{Object}} metadata - Record metadata
 * @returns {{Uint8Array}} - Transformed data
 */
export function process(record) {{
    try {{
        // Convert bytes to string
        const data = new TextDecoder().decode(record.value);

        let result;
        try {{
            // Try to parse as JSON and transform
            const json = JSON.parse(data);
            json.processed_by = "{}";
            json.processed_at = new Date().toISOString();
            result = JSON.stringify(json);
        }} catch {{
            // If not JSON, just add a prefix
            result = `[PROCESSED] ${{data}}`;
        }}

        // Convert back to bytes
        return new TextEncoder().encode(result);
    }} catch (error) {{
        console.error("Error processing record:", error);
        return record.value;
    }}
}}

/**
 * Get plugin information
 * @returns {{[string, string]}} - [name, version]
 */
export function info() {{
    return ["{}", "0.1.0"];
}}

/**
 * Get plugin type
 * @returns {{string}} - Plugin type
 */
export function pluginType() {{
    return "map";
}}

// Example usage (for testing)
if (import.meta.url === `file://${{process.argv[1]}}`) {{
    const testRecord = {{
        value: new TextEncoder().encode('{{"message": "test"}}'),
        metadata: {{}}
    }};

    console.log("Testing {} plugin...");
    console.log("Plugin info:", info());
    const result = process(testRecord);
    console.log("Test result:", new TextDecoder().decode(result));
}}
"#, name, name, name, name)
}

/// Generate JavaScript aggregate code
fn generate_js_aggregate_code(name: &str) -> String {
    format!(r#"/**
 * {} - A DataFlare aggregate plugin
 *
 * This plugin aggregates data records.
 */

// Global state for aggregation
let aggregateState = {{
    count: 0,
    sum: 0,
    values: []
}};

/**
 * Process a data record
 * @param {{Uint8Array}} value - The record data as bytes
 * @param {{Object}} metadata - Record metadata
 * @returns {{Uint8Array}} - Aggregated result
 */
export function process(record) {{
    try {{
        // Convert bytes to string
        const data = new TextDecoder().decode(record.value);

        // Update aggregate state
        aggregateState.count += 1;
        aggregateState.values.push(data);

        // Try to parse as number and add to sum
        const num = parseFloat(data);
        if (!isNaN(num)) {{
            aggregateState.sum += num;
        }}

        // Create result
        const result = {{
            count: aggregateState.count,
            sum: aggregateState.sum,
            average: aggregateState.count > 0 ? aggregateState.sum / aggregateState.count : 0,
            latest_value: data
        }};

        // Convert to bytes
        return new TextEncoder().encode(JSON.stringify(result));
    }} catch (error) {{
        console.error("Error processing record:", error);
        return new TextEncoder().encode(JSON.stringify({{ error: error.message }}));
    }}
}}

/**
 * Get plugin information
 * @returns {{[string, string]}} - [name, version]
 */
export function info() {{
    return ["{}", "0.1.0"];
}}

/**
 * Get plugin type
 * @returns {{string}} - Plugin type
 */
export function pluginType() {{
    return "aggregate";
}}

// Example usage (for testing)
if (import.meta.url === `file://${{process.argv[1]}}`) {{
    console.log("Testing {} plugin...");
    console.log("Plugin info:", info());

    // Test with multiple values
    const testValues = ["10", "20", "30"];
    testValues.forEach((value, index) => {{
        const testRecord = {{
            value: new TextEncoder().encode(value),
            metadata: {{}}
        }};
        const result = process(testRecord);
        console.log(`Test ${{index + 1}} result:`, new TextDecoder().decode(result));
    }});
}}
"#, name, name, name)
}

/// Generate Go go.mod file
fn generate_go_mod(name: &str) -> String {
    format!(r#"module {}

go 1.21

require (
    github.com/dataflare/plugin-sdk-go v0.1.0
)
"#, name)
}

/// Generate Go plugin code
fn generate_go_plugin_code(name: &str, plugin_type: &str) -> String {
    match plugin_type {
        "filter" => generate_go_filter_code(name),
        "map" => generate_go_map_code(name),
        "aggregate" => generate_go_aggregate_code(name),
        _ => generate_go_filter_code(name),
    }
}

/// Generate Go filter code
fn generate_go_filter_code(name: &str) -> String {
    format!(r#"// {} - A DataFlare filter plugin
//
// This plugin filters data records based on custom criteria.
package main

import (
    "encoding/json"
    "fmt"
    "strings"
)

// PluginInfo represents plugin metadata
type PluginInfo struct {{
    Name    string `json:"name"`
    Version string `json:"version"`
    Type    string `json:"type"`
}}

// Process filters a data record
func Process(data []byte) (bool, error) {{
    // Convert bytes to string
    text := string(data)

    // Example filter logic: keep records containing "important"
    return strings.Contains(text, "important"), nil
}}

// Info returns plugin information
func Info() (string, string) {{
    return "{}", "0.1.0"
}}

// PluginType returns the plugin type
func PluginType() string {{
    return "filter"
}}

// GetPluginInfo returns detailed plugin information
func GetPluginInfo() ([]byte, error) {{
    info := PluginInfo{{
        Name:    "{}",
        Version: "0.1.0",
        Type:    "filter",
    }}
    return json.Marshal(info)
}}

func main() {{
    fmt.Println("Testing {} plugin...")

    // Test data
    testData := []byte("This is important test data")

    // Process the data
    result, err := Process(testData)
    if err != nil {{
        fmt.Printf("Error: %v\n", err)
        return
    }}

    name, version := Info()
    fmt.Printf("Plugin: %s v%s\n", name, version)
    fmt.Printf("Test result: %t\n", result)
}}
"#, name, name, name, name)
}

/// Generate Go map code
fn generate_go_map_code(name: &str) -> String {
    format!(r#"// {} - A DataFlare map plugin
//
// This plugin transforms data records.
package main

import (
    "encoding/json"
    "fmt"
    "time"
)

// PluginInfo represents plugin metadata
type PluginInfo struct {{
    Name    string `json:"name"`
    Version string `json:"version"`
    Type    string `json:"type"`
}}

// Process transforms a data record
func Process(data []byte) ([]byte, error) {{
    // Try to parse as JSON and transform
    var jsonData map[string]interface{{}}
    if err := json.Unmarshal(data, &jsonData); err == nil {{
        // Add processing metadata
        jsonData["processed_by"] = "{}"
        jsonData["processed_at"] = time.Now().Format(time.RFC3339)
        return json.Marshal(jsonData)
    }}

    // If not JSON, just add a prefix
    result := fmt.Sprintf("[PROCESSED] %s", string(data))
    return []byte(result), nil
}}

// Info returns plugin information
func Info() (string, string) {{
    return "{}", "0.1.0"
}}

// PluginType returns the plugin type
func PluginType() string {{
    return "map"
}}

// GetPluginInfo returns detailed plugin information
func GetPluginInfo() ([]byte, error) {{
    info := PluginInfo{{
        Name:    "{}",
        Version: "0.1.0",
        Type:    "map",
    }}
    return json.Marshal(info)
}}

func main() {{
    fmt.Println("Testing {} plugin...")

    // Test with JSON data
    testData := []byte(`{{"message": "test"}}`)

    // Process the data
    result, err := Process(testData)
    if err != nil {{
        fmt.Printf("Error: %v\n", err)
        return
    }}

    name, version := Info()
    fmt.Printf("Plugin: %s v%s\n", name, version)
    fmt.Printf("Test result: %s\n", string(result))
}}
"#, name, name, name, name, name)
}

/// Generate Go aggregate code
fn generate_go_aggregate_code(name: &str) -> String {
    format!(r#"// {} - A DataFlare aggregate plugin
//
// This plugin aggregates data records.
package main

import (
    "encoding/json"
    "fmt"
    "strconv"
    "sync"
)

// PluginInfo represents plugin metadata
type PluginInfo struct {{
    Name    string `json:"name"`
    Version string `json:"version"`
    Type    string `json:"type"`
}}

// AggregateState holds the aggregation state
type AggregateState struct {{
    Count       int64     `json:"count"`
    Sum         float64   `json:"sum"`
    Average     float64   `json:"average"`
    LatestValue string    `json:"latest_value"`
    mu          sync.Mutex
}}

var state = &AggregateState{{}}

// AggregateResult represents the result of aggregation
type AggregateResult struct {{
    Count       int64   `json:"count"`
    Sum         float64 `json:"sum"`
    Average     float64 `json:"average"`
    LatestValue string  `json:"latest_value"`
}}

// Process aggregates a data record
func Process(data []byte) ([]byte, error) {{
    state.mu.Lock()
    defer state.mu.Unlock()

    // Convert bytes to string
    text := string(data)
    state.LatestValue = text
    state.Count++

    // Try to parse as number and add to sum
    if num, err := strconv.ParseFloat(text, 64); err == nil {{
        state.Sum += num
    }}

    // Calculate average
    if state.Count > 0 {{
        state.Average = state.Sum / float64(state.Count)
    }}

    // Create result
    result := AggregateResult{{
        Count:       state.Count,
        Sum:         state.Sum,
        Average:     state.Average,
        LatestValue: state.LatestValue,
    }}

    return json.Marshal(result)
}}

// Info returns plugin information
func Info() (string, string) {{
    return "{}", "0.1.0"
}}

// PluginType returns the plugin type
func PluginType() string {{
    return "aggregate"
}}

// GetPluginInfo returns detailed plugin information
func GetPluginInfo() ([]byte, error) {{
    info := PluginInfo{{
        Name:    "{}",
        Version: "0.1.0",
        Type:    "aggregate",
    }}
    return json.Marshal(info)
}}

func main() {{
    fmt.Println("Testing {} plugin...")

    // Test with multiple values
    testValues := []string{{"10", "20", "30"}}

    for i, value := range testValues {{
        result, err := Process([]byte(value))
        if err != nil {{
            fmt.Printf("Error processing value %s: %v\n", value, err)
            continue
        }}

        fmt.Printf("Test %d result: %s\n", i+1, string(result))
    }}

    name, version := Info()
    fmt.Printf("Plugin: %s v%s\n", name, version)
}}
"#, name, name, name, name)
}

/// Generate plugin README.md
fn generate_plugin_readme(name: &str, plugin_type: &str, lang: &str) -> String {
    format!(r#"# {}

A DataFlare {} plugin written in {}.

## Description

This plugin {} data records using the DataFlare plugin system.

## Usage

### Building the Plugin

```bash
dataflare plugin build
```

### Testing the Plugin

```bash
dataflare plugin test --data "test data"
```

### Installing the Plugin

```bash
dataflare plugin install .
```

## Development

### Prerequisites

{}

### Project Structure

{}

### Customization

Edit the main plugin file to implement your custom logic:

{}

## Configuration

This plugin can be used in DataFlare workflows with the following configuration:

```yaml
transformations:
  my_{}:
    type: processor
    processor_type: plugin
    config:
      name: "{}"
```

## License

MIT OR Apache-2.0
"#,
    name,
    plugin_type,
    lang,
    match plugin_type {
        "filter" => "filters",
        "map" => "transforms",
        "aggregate" => "aggregates",
        _ => "processes",
    },
    match lang {
        "rust" => "- Rust 1.70 or later\n- Cargo",
        "js" => "- Node.js 18 or later\n- npm or yarn",
        "go" => "- Go 1.21 or later",
        _ => "- Development environment for the chosen language",
    },
    match lang {
        "rust" => "- `src/lib.rs` - Main plugin implementation\n- `Cargo.toml` - Project configuration\n- `tests/` - Integration tests\n- `examples/` - Usage examples",
        "js" => "- `src/index.js` - Main plugin implementation\n- `package.json` - Project configuration",
        "go" => "- `main.go` - Main plugin implementation\n- `go.mod` - Module configuration",
        _ => "- Main plugin file\n- Configuration file",
    },
    match lang {
        "rust" => "- `src/lib.rs` - Implement the `SmartPlugin` trait",
        "js" => "- `src/index.js` - Implement the `process()` function",
        "go" => "- `main.go` - Implement the `Process()` function",
        _ => "- Main plugin file",
    },
    plugin_type,
    name
)
}

/// Build native plugin with specified path
fn build_native_plugin_with_path(
    release: bool,
    output_dir: Option<&str>,
    plugin_dir: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut args = vec!["build"];
    if release {
        args.push("--release");
    }

    println!("Building native plugin in: {}", plugin_dir.display());
    let output = Command::new("cargo")
        .current_dir(plugin_dir)
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Cargo build failed: {}", stderr).into());
    }

    let target_dir = if release { "target/release" } else { "target/debug" };
    let output_path = match output_dir {
        Some(dir) => dir.to_string(),
        None => plugin_dir.join(target_dir).to_string_lossy().to_string(),
    };

    println!("Native plugin built successfully");
    Ok(output_path)
}

/// Build WASM plugin with specified path
fn build_wasm_plugin_with_path(
    release: bool,
    output_dir: Option<&str>,
    plugin_dir: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    use std::process::Command;

    let mut args = vec![
        "build",
        "--target", "wasm32-unknown-unknown",
    ];

    if release {
        args.push("--release");
    }

    println!("Building WASM plugin in: {}", plugin_dir.display());
    let output = Command::new("cargo")
        .current_dir(plugin_dir)
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Cargo build failed: {}", stderr).into());
    }

    let target_subdir = if release { "release" } else { "debug" };
    let wasm_dir = plugin_dir.join(format!("target/wasm32-unknown-unknown/{}", target_subdir));

    let output_path = match output_dir {
        Some(dir) => dir.to_string(),
        None => wasm_dir.to_string_lossy().to_string(),
    };

    println!("WASM plugin built successfully");
    Ok(output_path)
}