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
use std::io::Write;
use dataflare_connector::registry::{get_registered_source_connectors, get_registered_destination_connectors};
use dataflare_processor::registry::get_processor_names;
use dataflare_plugin::registry::list_plugins;
use dataflare_runtime::workflow::{YamlWorkflowParser, WorkflowParser};

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
            println!("Running workflow: {:?} in {} mode", workflow, mode);
            // TODO: Implement workflow execution
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
                    println!("Installed plugins:");
                    let plugins = list_all_plugins();
                    
                    if plugins.is_empty() {
                        println!("  No plugins installed");
                    } else {
                        for plugin in plugins {
                            println!("  - {} (v{}): {}", 
                                     plugin.name, 
                                     plugin.version,
                                     plugin.description);
                        }
                    }
                }

                PluginCommands::Install { path } => {
                    println!("Installing plugin from: {}", path);
                    // TODO: Implement plugin installation
                }

                PluginCommands::Remove { id } => {
                    println!("Removing plugin: {}", id);
                    // TODO: Implement plugin removal
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
}
