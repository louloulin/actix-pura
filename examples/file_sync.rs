use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::fs;
use std::io;

use actix::prelude::*;
use actix_cluster::{
    ClusterSystem, ClusterConfig, Architecture, NodeRole, 
    SerializationFormat, NodeId, AnyMessage
};
use log::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use structopt::StructOpt;
use actix_cluster::registry::ActorRef;
use actix_cluster::cluster::SimpleActorRef;
use tokio::time::interval;

// Command line arguments
#[derive(StructOpt, Debug)]
#[structopt(name = "file_sync", about = "Distributed file synchronization service")]
struct Args {
    /// Node ID
    #[structopt(short, long, default_value = "node1")]
    id: String,

    /// Bind address
    #[structopt(short, long, default_value = "127.0.0.1:8080")]
    address: SocketAddr,
    
    /// Seed node address
    #[structopt(short, long)]
    seed: Option<String>,

    /// Directory to synchronize
    #[structopt(short, long)]
    dir: PathBuf,
    
    /// Synchronization interval in seconds
    #[structopt(short, long, default_value = "30")]
    interval: u64,
    
    /// Start multiple nodes (for local testing)
    #[structopt(long)]
    multi: bool,
    
    /// Number of nodes to start
    #[structopt(long, default_value = "3")]
    nodes: usize,
    
    /// Base port for multi-node mode
    #[structopt(long, default_value = "9000")]
    base_port: u16,
}

// File system entity
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileInfo {
    path: String,           // Relative path from sync dir
    is_dir: bool,
    size: u64,              // File size in bytes (0 for directories)
    modified: u64,          // Last modified time in milliseconds
    hash: Option<String>,   // Content hash (None for directories)
}

// Synchronization messages
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Vec<FileInfo>")]
struct GetFileList {
    prefix: String, // If empty, get all files
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Result<FileContent, String>")]
struct GetFile {
    path: String,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Result<bool, String>")]
struct UpdateFile {
    file_info: FileInfo,
    content: Vec<u8>,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Result<bool, String>")]
struct DeleteFile {
    path: String,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
struct SyncFiles {
    source_node: String,
}

#[derive(MessageResponse, Serialize, Deserialize, Clone)]
struct FileContent {
    info: FileInfo,
    content: Vec<u8>,
}

// Helper messages
#[derive(Message)]
#[rtype(result = "()")]
struct StartSync;

// File synchronization actor
struct FileSyncActor {
    node_id: String,
    base_dir: PathBuf,
    files: HashMap<String, FileInfo>,
    last_sync: u64,
    remote_actors: HashMap<String, Box<dyn ActorRef>>,
}

impl FileSyncActor {
    fn new(node_id: String, base_dir: PathBuf) -> Self {
        Self {
            node_id,
            base_dir,
            files: HashMap::new(),
            last_sync: 0,
            remote_actors: HashMap::new(),
        }
    }
    
    fn now_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64
    }
    
    // Scan local directory and update file list
    fn scan_directory(&mut self) -> io::Result<()> {
        info!("Scanning directory: {:?}", self.base_dir);
        
        let mut new_files = HashMap::new();
        
        // Helper function to scan directory recursively
        fn scan_dir(
            base: &Path,
            current: &Path,
            files: &mut HashMap<String, FileInfo>,
        ) -> io::Result<()> {
            if !current.exists() {
                return Ok(());
            }
            
            for entry in fs::read_dir(current)? {
                let entry = entry?;
                let path = entry.path();
                let metadata = entry.metadata()?;
                
                // Calculate relative path
                let rel_path = path.strip_prefix(base).unwrap_or(&path);
                let rel_path_str = rel_path.to_string_lossy().to_string();
                
                if metadata.is_dir() {
                    // Add directory entry
                    files.insert(
                        rel_path_str.clone(),
                        FileInfo {
                            path: rel_path_str.clone(),
                            is_dir: true,
                            size: 0,
                            modified: metadata
                                .modified()
                                .unwrap_or_else(|_| std::time::SystemTime::now())
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_else(|_| Duration::from_secs(0))
                                .as_millis() as u64,
                            hash: None,
                        },
                    );
                    
                    // Scan subdirectory
                    scan_dir(base, &path, files)?;
                } else {
                    // Create simple hash based on size + mod time for quick comparison
                    // In a real implementation, use a proper hashing algorithm
                    let hash = format!("{}-{}", metadata.len(), metadata
                        .modified()
                        .unwrap_or_else(|_| std::time::SystemTime::now())
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_else(|_| Duration::from_secs(0))
                        .as_secs()
                    );
                    
                    // Add file entry
                    files.insert(
                        rel_path_str.clone(),
                        FileInfo {
                            path: rel_path_str,
                            is_dir: false,
                            size: metadata.len(),
                            modified: metadata
                                .modified()
                                .unwrap_or_else(|_| std::time::SystemTime::now())
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_else(|_| Duration::from_secs(0))
                                .as_millis() as u64,
                            hash: Some(hash),
                        },
                    );
                }
            }
            
            Ok(())
        }
        
        // Start scan from base directory
        scan_dir(&self.base_dir, &self.base_dir, &mut new_files)?;
        
        // Update file list
        self.files = new_files;
        self.last_sync = Self::now_millis();
        
        info!("Directory scan complete. Found {} files/directories", self.files.len());
        
        Ok(())
    }
    
    // Get file content from disk
    fn read_file(&self, rel_path: &str) -> io::Result<Vec<u8>> {
        let full_path = self.base_dir.join(rel_path);
        fs::read(full_path)
    }
    
    // Write file content to disk
    fn write_file(&self, rel_path: &str, content: &[u8]) -> io::Result<()> {
        let full_path = self.base_dir.join(rel_path);
        
        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(full_path, content)
    }
    
    // Create directory on disk
    fn create_directory(&self, rel_path: &str) -> io::Result<()> {
        let full_path = self.base_dir.join(rel_path);
        fs::create_dir_all(full_path)
    }
    
    // Delete file or directory from disk
    fn delete_file(&self, rel_path: &str) -> io::Result<()> {
        let full_path = self.base_dir.join(rel_path);
        
        if full_path.is_dir() {
            fs::remove_dir_all(full_path)
        } else {
            fs::remove_file(full_path)
        }
    }
    
    // Add a remote actor
    fn add_remote_actor(&mut self, id: String, actor_ref: Box<dyn ActorRef>) {
        info!("Adding remote file sync actor: {}", id);
        self.remote_actors.insert(id, actor_ref);
    }
}

impl Actor for FileSyncActor {
    type Context = Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("FileSyncActor started on node {} with base directory {:?}", 
             self.node_id, self.base_dir);
        
        // Initial scan of the directory
        if let Err(e) = self.scan_directory() {
            error!("Failed to scan directory: {}", e);
        }
        
        // Schedule periodic sync
        ctx.run_later(Duration::from_secs(30), |_, ctx| {
            ctx.address().do_send(StartSync);
        });
    }
}

// Handle file list requests
impl Handler<GetFileList> for FileSyncActor {
    type Result = Vec<FileInfo>;
    
    fn handle(&mut self, msg: GetFileList, _: &mut Self::Context) -> Self::Result {
        let prefix = msg.prefix.trim_start_matches('/');
        
        if prefix.is_empty() {
            // Return all files
            self.files.values().cloned().collect()
        } else {
            // Return files with matching prefix
            self.files
                .values()
                .filter(|info| info.path.starts_with(prefix))
                .cloned()
                .collect()
        }
    }
}

// Handle file content requests
impl Handler<GetFile> for FileSyncActor {
    type Result = Result<FileContent, String>;
    
    fn handle(&mut self, msg: GetFile, _: &mut Self::Context) -> Self::Result {
        let path = msg.path.trim_start_matches('/');
        
        // Look up file info
        if let Some(file_info) = self.files.get(path) {
            if file_info.is_dir {
                return Err(format!("Cannot get content of directory: {}", path));
            }
            
            // Read file content
            match self.read_file(path) {
                Ok(content) => Ok(FileContent {
                    info: file_info.clone(),
                    content,
                }),
                Err(e) => Err(format!("Failed to read file {}: {}", path, e)),
            }
        } else {
            Err(format!("File not found: {}", path))
        }
    }
}

// Handle file update requests
impl Handler<UpdateFile> for FileSyncActor {
    type Result = Result<bool, String>;
    
    fn handle(&mut self, msg: UpdateFile, _: &mut Self::Context) -> Self::Result {
        let path = msg.file_info.path.trim_start_matches('/');
        
        if msg.file_info.is_dir {
            // Create directory
            match self.create_directory(path) {
                Ok(_) => {
                    // Update file info
                    self.files.insert(path.to_string(), msg.file_info);
                    Ok(true)
                },
                Err(e) => Err(format!("Failed to create directory {}: {}", path, e)),
            }
        } else {
            // Write file
            match self.write_file(path, &msg.content) {
                Ok(_) => {
                    // Update file info
                    self.files.insert(path.to_string(), msg.file_info);
                    Ok(true)
                },
                Err(e) => Err(format!("Failed to write file {}: {}", path, e)),
            }
        }
    }
}

// Handle file deletion requests
impl Handler<DeleteFile> for FileSyncActor {
    type Result = Result<bool, String>;
    
    fn handle(&mut self, msg: DeleteFile, _: &mut Self::Context) -> Self::Result {
        let path = msg.path.trim_start_matches('/');
        
        // Remove from file list
        self.files.remove(path);
        
        // Delete from disk
        match self.delete_file(path) {
            Ok(_) => Ok(true),
            Err(e) => Err(format!("Failed to delete {}: {}", path, e)),
        }
    }
}

// Handle sync requests from remote nodes
impl Handler<SyncFiles> for FileSyncActor {
    type Result = ();
    
    fn handle(&mut self, msg: SyncFiles, ctx: &mut Self::Context) -> Self::Result {
        info!("Received sync request from {}", msg.source_node);
        
        // Find the remote actor
        if let Some(remote_actor) = self.remote_actors.get(&msg.source_node) {
            let my_id = self.node_id.clone();
            
            // Scan local directory first
            if let Err(e) = self.scan_directory() {
                error!("Failed to scan directory: {}", e);
                return;
            }
            
            // Get list of files from remote node
            let actor_addr = ctx.address();
            let remote_actor_clone = remote_actor.clone();
            
            // Use message to get file list instead of direct method call
            let get_files_msg = Box::new(GetFileList { prefix: String::new() }) as Box<dyn std::any::Any + Send>;
            
            async move {
                match remote_actor_clone.send_any(get_files_msg) {
                    Ok(_) => {
                        info!("Requested file list from {}", msg.source_node);
                        // Processing will be handled in a real implementation
                        // Here we'd compare files and sync those that differ
                    },
                    Err(e) => error!("Failed to request file list: {}", e),
                }
            };
        } else {
            warn!("Unknown remote node: {}", msg.source_node);
        }
    }
}

// Handle periodic sync trigger
impl Handler<StartSync> for FileSyncActor {
    type Result = ();
    
    fn handle(&mut self, _: StartSync, ctx: &mut Self::Context) -> Self::Result {
        info!("Starting periodic sync");
        
        // Scan local directory
        if let Err(e) = self.scan_directory() {
            error!("Failed to scan directory: {}", e);
        }
        
        // Sync with all remote nodes
        for (node_id, actor_ref) in &self.remote_actors {
            info!("Syncing with node {}", node_id);
            
            let sync_msg = SyncFiles {
                source_node: self.node_id.clone(),
            };
            
            let boxed_msg = Box::new(sync_msg) as Box<dyn std::any::Any + Send>;
            if let Err(e) = actor_ref.send_any(boxed_msg) {
                error!("Failed to send sync request to {}: {}", node_id, e);
            }
        }
        
        // Schedule next sync
        ctx.run_later(Duration::from_secs(30), |_, ctx| {
            ctx.address().do_send(StartSync);
        });
    }
}

// Handle AnyMessage for cluster communication
impl Handler<AnyMessage> for FileSyncActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(files_msg) = msg.downcast_ref::<GetFileList>() {
            let response = self.handle(files_msg.clone(), ctx);
            // In a real implementation, we'd send the response back
        } else if let Some(file_msg) = msg.downcast_ref::<GetFile>() {
            let _ = self.handle(file_msg.clone(), ctx);
        } else if let Some(update_msg) = msg.downcast_ref::<UpdateFile>() {
            let _ = self.handle(update_msg.clone(), ctx);
        } else if let Some(delete_msg) = msg.downcast_ref::<DeleteFile>() {
            let _ = self.handle(delete_msg.clone(), ctx);
        } else if let Some(sync_msg) = msg.downcast_ref::<SyncFiles>() {
            self.handle(sync_msg.clone(), ctx);
        } else {
            warn!("FileSyncActor received unknown message type");
        }
    }
}

// Start a node in the file sync system
async fn start_node(node_id: String, addr: SocketAddr, sync_dir: PathBuf, 
                   seed_nodes: Vec<String>) -> std::io::Result<()> {
    // Create cluster configuration
    let mut config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(addr)
        .cluster_name("file-sync-cluster".to_string())
        .serialization_format(SerializationFormat::Bincode);
    
    // Add seed nodes
    if !seed_nodes.is_empty() {
        let mut seed_addrs = Vec::new();
        for seed in seed_nodes {
            info!("Adding seed node: {}", seed);
            seed_addrs.push(seed);
        }
        config = config.seed_nodes(seed_addrs);
    }
    
    let config = config.build().expect("Failed to create cluster configuration");
    
    // Create and start cluster system
    let mut sys = ClusterSystem::new(&node_id, config);
    sys.start().await.expect("Failed to start cluster");
    
    info!("Node {} started at address {}", node_id, addr);
    
    // Ensure the sync directory exists
    if !sync_dir.exists() {
        fs::create_dir_all(&sync_dir)?;
    }
    
    // Create file sync actor
    let file_sync = FileSyncActor::new(node_id.clone(), sync_dir).start();
    
    // Register to the cluster
    let actor_path = format!("/user/file-sync/{}", node_id);
    info!("Registering file sync actor: {}", actor_path);
    
    // Ensure cluster system is initialized
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    match sys.register(&actor_path, file_sync.clone()).await {
        Ok(_) => info!("Successfully registered file sync actor"),
        Err(e) => error!("Failed to register file sync actor: {}", e),
    }
    
    // Wait for the service to be fully registered
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Discover other nodes
    info!("Looking for other file sync nodes...");
    
    // In a real implementation, we'd use a discovery mechanism to find other nodes
    // For this example, we'll search for nodes with known IDs
    for i in 1..10 {
        let target_id = format!("node{}", i);
        
        // Skip our own node
        if target_id == node_id {
            continue;
        }
        
        let target_path = format!("/user/file-sync/{}", target_id);
        
        // Look up the actor
        if let Some(remote_actor) = sys.lookup(&target_path).await {
            info!("Found remote file sync node: {}", target_id);
            
            // Add to our actor
            file_sync.do_send(SystemMessage::AddRemoteActor(target_id, Box::new(remote_actor)));
        }
    }
    
    // Keep the node running
    info!("File sync node running. Press Ctrl+C to exit.");
    
    // In a real application, we'd wait for a signal to shutdown
    // For this example, just sleep for a while
    tokio::time::sleep(Duration::from_secs(1800)).await; // 30 minutes
    
    info!("Node {} shutting down", node_id);
    Ok(())
}

// System messages for actor configuration
#[derive(Message)]
#[rtype(result = "()")]
enum SystemMessage {
    AddRemoteActor(String, Box<dyn ActorRef>),
}

impl Handler<SystemMessage> for FileSyncActor {
    type Result = ();
    
    fn handle(&mut self, msg: SystemMessage, _: &mut Self::Context) -> Self::Result {
        match msg {
            SystemMessage::AddRemoteActor(id, actor_ref) => {
                self.add_remote_actor(id, actor_ref);
            }
        }
    }
}

// Run multi-node test locally
async fn run_multi_node_test(node_count: usize, base_port: u16, base_dir: PathBuf) -> std::io::Result<()> {
    info!("Starting file sync system with {} nodes", node_count);
    
    // Create a LocalSet for running local tasks
    let local = tokio::task::LocalSet::new();
    
    // Run all node tasks in the LocalSet
    local.run_until(async move {
        // Create node directories
        let mut node_dirs = Vec::new();
        for i in 0..node_count {
            let node_dir = base_dir.join(format!("node{}", i+1));
            fs::create_dir_all(&node_dir)?;
            node_dirs.push(node_dir);
        }
        
        // Start the first node as the seed
        let first_node_addr = format!("127.0.0.1:{}", base_port);
        
        // Start first node
        let first_node_handle = tokio::task::spawn_local({
            let node_dir = node_dirs[0].clone();
            async move {
                match start_node(
                    "node1".to_string(),
                    first_node_addr.parse().unwrap(),
                    node_dir,
                    Vec::new()
                ).await {
                    Ok(_) => info!("Node1 completed"),
                    Err(e) => error!("Node1 failed: {:?}", e),
                }
            }
        });
        
        // Let the first node start
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Start other nodes
        let mut node_handles = Vec::new();
        
        for i in 1..node_count {
            let port = base_port + i as u16;
            let node_id = format!("node{}", i+1);
            let addr = format!("127.0.0.1:{}", port);
            let seed = first_node_addr.clone();
            let node_dir = node_dirs[i].clone();
            
            info!("Starting node {}, address: {}, seed: {}", node_id, addr, seed);
            
            let node_handle = tokio::task::spawn_local(async move {
                match start_node(
                    node_id.clone(),
                    addr.parse().unwrap(),
                    node_dir,
                    vec![seed]
                ).await {
                    Ok(_) => info!("Node {} completed", node_id),
                    Err(e) => error!("Node {} failed: {:?}", node_id, e),
                }
            });
            
            node_handles.push(node_handle);
            
            // Space out node starts
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        // Create some test files in the first node's directory
        let test_files = [
            ("test1.txt", "Hello, World!"),
            ("test2.txt", "File synchronization test"),
            ("docs/readme.md", "# File Sync Example\n\nThis is a test file."),
        ];
        
        for (path, content) in &test_files {
            let full_path = node_dirs[0].join(path);
            
            // Ensure parent directory exists
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            fs::write(full_path, content)?;
            info!("Created test file: {}", path);
        }
        
        // Wait for all nodes
        for (i, handle) in node_handles.into_iter().enumerate() {
            if let Err(e) = handle.await {
                error!("Node{} task error: {:?}", i+2, e);
            }
        }
        
        if let Err(e) = first_node_handle.await {
            error!("Node1 task error: {:?}", e);
        }
        
        info!("File sync test completed");
        
        Ok(())
    }).await
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // Parse command line arguments
    let args = Args::from_args();
    
    if args.multi {
        // Multi-node mode - automatically start multiple nodes
        info!("Starting in multi-node mode with {} nodes, base port: {}", args.nodes, args.base_port);
        run_multi_node_test(args.nodes, args.base_port, args.dir).await?;
    } else {
        // Single node mode - manual connection
        let mut seed_nodes = Vec::new();
        if let Some(seed) = args.seed {
            seed_nodes.push(seed);
        }
        
        info!("Starting node ID: {}, address: {}", args.id, args.address);
        info!("Sync directory: {:?}", args.dir);
        
        if !seed_nodes.is_empty() {
            info!("Seed nodes: {:?}", seed_nodes);
        }
        
        // Start node
        start_node(args.id, args.address, args.dir, seed_nodes).await?;
    }
    
    Ok(())
} 