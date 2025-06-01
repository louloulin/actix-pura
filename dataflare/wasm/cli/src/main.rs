//! DataFlare Plugin CLI Tool
//!
//! A command-line tool for developing, building, testing, and publishing DataFlare WASM plugins.
//! Updated for package command implementation.

use clap::Parser;
use anyhow::Result;
use log::error;
use dataflare_wasm_cli::{WasmCli, execute_wasm_command};

#[derive(Parser)]
#[command(name = "dataflare-plugin")]
#[command(about = "DataFlare WASM Plugin CLI Tool")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(flatten)]
    wasm_cli: WasmCli,
}



#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Execute WASM CLI command
    if let Err(e) = execute_wasm_command(cli.wasm_cli).await {
        error!("Command failed: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
