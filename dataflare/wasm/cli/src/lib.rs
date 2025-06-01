//! DataFlare WASM Plugin CLI Library
//!
//! This library provides the core functionality for the DataFlare WASM plugin CLI tool.
//! It can be used as a standalone binary or integrated into other CLI applications.

use clap::{Parser, Subcommand};
use anyhow::Result;
use log::{info, error};
use colored;

pub mod commands;
pub mod config;
pub mod template;
pub mod registry;
pub mod utils;

use commands::*;

/// WASM Plugin CLI configuration
#[derive(Parser, Clone)]
#[command(name = "plugin")]
#[command(about = "DataFlare WASM Plugin management")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct WasmCli {
    #[command(subcommand)]
    pub command: WasmCommands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,
}

/// WASM Plugin subcommands
#[derive(Subcommand, Clone, Debug)]
pub enum WasmCommands {
    /// Create a new plugin project
    New {
        /// Plugin name
        name: String,

        /// Programming language (rust, javascript, python, cpp, go)
        #[arg(short, long, default_value = "rust")]
        lang: String,

        /// Plugin type (source, destination, processor, transformer, filter, aggregator, ai-processor)
        #[arg(short = 't', long, default_value = "processor")]
        plugin_type: String,

        /// Template to use (basic, advanced, ai)
        #[arg(long, default_value = "basic")]
        template: String,

        /// Additional features to enable
        #[arg(short, long)]
        features: Vec<String>,

        /// Target directory
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Build the plugin
    Build {
        /// Enable release mode
        #[arg(short, long)]
        release: bool,

        /// Enable optimization
        #[arg(long)]
        optimize: bool,

        /// Target architecture
        #[arg(long, default_value = "wasm32-wasip1")]
        target: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Run tests
    Test {
        /// Enable coverage reporting
        #[arg(short, long)]
        coverage: bool,

        /// Run integration tests
        #[arg(short, long)]
        integration: bool,

        /// Test filter pattern
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Run benchmarks
    Benchmark {
        /// Number of iterations
        #[arg(short, long, default_value = "100")]
        iterations: u32,

        /// Benchmark filter pattern
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Validate plugin
    Validate {
        /// Enable strict validation
        #[arg(short, long)]
        strict: bool,

        /// Plugin file path
        #[arg(short, long)]
        plugin: Option<String>,
    },

    /// Package plugin for distribution
    Package {
        /// Sign the package
        #[arg(short, long)]
        sign: bool,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Publish plugin to registry
    Publish {
        /// Registry URL
        #[arg(short, long, default_value = "https://plugins.dataflare.io")]
        registry: String,

        /// API token
        #[arg(short, long)]
        token: Option<String>,

        /// Dry run (don't actually publish)
        #[arg(long)]
        dry_run: bool,
    },

    /// Install a plugin
    Install {
        /// Plugin name or path
        plugin: String,

        /// Plugin version
        #[arg(long)]
        version: Option<String>,

        /// Registry URL
        #[arg(short, long)]
        registry: Option<String>,
    },

    /// Update plugins
    Update {
        /// Plugin name (update all if not specified)
        plugin: Option<String>,
    },

    /// Remove a plugin
    Remove {
        /// Plugin name
        plugin: String,
    },

    /// List installed plugins
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Show plugin information
    Info {
        /// Plugin name
        plugin: String,
    },

    /// Search plugins in registry
    Search {
        /// Search query
        query: String,

        /// Registry URL
        #[arg(short, long)]
        registry: Option<String>,
    },
}

/// Execute WASM CLI command
pub async fn execute_wasm_command(cli: WasmCli) -> Result<()> {
    // Initialize logging if not already done
    if std::env::var("RUST_LOG").is_err() {
        let log_level = if cli.verbose { "debug" } else { "info" };
        std::env::set_var("RUST_LOG", log_level);
        env_logger::try_init().ok(); // Ignore error if already initialized
    }

    // Disable colors if requested
    if cli.no_color {
        colored::control::set_override(false);
    }

    info!("DataFlare WASM Plugin CLI v{}", env!("CARGO_PKG_VERSION"));

    // Execute command
    match cli.command {
        WasmCommands::New { name, lang, plugin_type, template, features, output } => {
            new::execute(name, lang, plugin_type, template, features, output).await
        }
        WasmCommands::Build { release, optimize, target, output } => {
            build::execute(release, optimize, target, output).await
        }
        WasmCommands::Test { coverage, integration, filter } => {
            test::execute(coverage, integration, filter).await
        }
        WasmCommands::Benchmark { iterations, filter } => {
            benchmark::execute(iterations, filter).await
        }
        WasmCommands::Validate { strict, plugin } => {
            validate::execute(strict, plugin).await
        }
        WasmCommands::Package { sign, output } => {
            package::execute(sign, output).await
        }
        WasmCommands::Publish { registry: _, token: _, dry_run: _ } => {
            println!("🚧 Publish command not yet implemented");
            Ok(())
        }
        WasmCommands::Install { plugin, version, registry } => {
            marketplace::execute_install(plugin, version, registry).await
        }
        WasmCommands::Update { plugin } => {
            marketplace::execute_update(plugin).await
        }
        WasmCommands::Remove { plugin } => {
            marketplace::execute_remove(plugin).await
        }
        WasmCommands::List { detailed } => {
            marketplace::execute_list(detailed).await
        }
        WasmCommands::Info { plugin } => {
            marketplace::execute_info(plugin).await
        }
        WasmCommands::Search { query, registry } => {
            marketplace::execute_search(query, registry).await
        }
    }
}

/// Initialize WASM CLI logging
pub fn init_logging(verbose: bool) {
    let log_level = if verbose { "debug" } else { "info" };
    std::env::set_var("RUST_LOG", log_level);
    env_logger::try_init().ok(); // Ignore error if already initialized
}
