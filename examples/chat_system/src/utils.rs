use std::time::{Duration, SystemTime, UNIX_EPOCH};
use structopt::StructOpt;
use std::net::SocketAddr;

// Command line arguments
#[derive(StructOpt, Debug)]
#[structopt(name = "chat_system", about = "Distributed chat system example for ActixCluster")]
pub struct Args {
    /// Node ID
    #[structopt(short, long, default_value = "node1")]
    pub id: String,

    /// Bind address
    #[structopt(short, long, default_value = "127.0.0.1:8080")]
    pub address: SocketAddr,

    /// Seed node address
    #[structopt(short, long)]
    pub seed: Option<String>,
    
    /// Start multiple nodes (for local testing)
    #[structopt(long)]
    pub multi: bool,
    
    /// Number of nodes to start
    #[structopt(long, default_value = "3")]
    pub nodes: usize,
    
    /// Base port for multi-node mode
    #[structopt(long, default_value = "9000")]
    pub base_port: u16,
}

/// Get current time in microseconds
pub fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_micros() as u64
} 