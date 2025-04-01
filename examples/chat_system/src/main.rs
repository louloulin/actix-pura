use std::net::SocketAddr;
use std::time::Duration;
use std::sync::{Arc, Mutex as StdMutex};
use rand::Rng;

use actix::prelude::*;
use actix_cluster::{
    ClusterSystem, ClusterConfig, Architecture, NodeRole, 
    SerializationFormat
};
use log::debug;
use structopt::StructOpt;
use tokio::time::interval;

mod actors;
mod messages;
mod utils;
#[cfg(test)]
mod test;

use actors::{ChatServiceActor, UserActor};
use messages::{ChatMessage, JoinRoom, LeaveRoom};
use utils::Args;

// Simulated user client for testing
struct SimulatedClient {
    user_id: String,
    user_actor: Addr<UserActor>,
}

impl SimulatedClient {
    fn new(user_id: String, user_actor: Addr<UserActor>) -> Self {
        Self {
            user_id,
            user_actor,
        }
    }
    
    fn join_room(&self, room: String) {
        let join_msg = JoinRoom {
            user_id: self.user_id.clone(),
            room,
        };
        self.user_actor.do_send(join_msg);
    }
    
    fn leave_room(&self, room: String) {
        let leave_msg = LeaveRoom {
            user_id: self.user_id.clone(),
            room,
        };
        self.user_actor.do_send(leave_msg);
    }
    
    fn send_message(&self, room: String, content: String) {
        let timestamp = utils::now_micros();
        let msg = ChatMessage {
            from: self.user_id.clone(),
            room,
            content,
            timestamp,
        };
        self.user_actor.do_send(msg);
    }
}

fn main() -> std::io::Result<()> {
    // Set up logging
    std::env::set_var("RUST_LOG", "info,actix_cluster=debug");
    env_logger::init();
    
    // Parse command line arguments
    let args = Args::from_args();
    
    if args.multi {
        // Start multiple nodes in the same process for testing
        start_multi_node_test(args.nodes, args.base_port)
    } else {
        // Start a single node
        let system = System::new();
        system.block_on(async {
            start_node(args.id, args.address, args.seed).await.unwrap();
        });
        Ok(())
    }
}

async fn start_node(node_id: String, bind_addr: SocketAddr, seed_node: Option<String>) -> std::io::Result<()> {
    // Set up cluster configuration
    let mut config = ClusterConfig::new();
    config = config.bind_addr(bind_addr)
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .serialization_format(SerializationFormat::Bincode);
    
    let config = config.build().expect("Failed to build cluster configuration");
    
    // Create a new cluster system
    let mut system = ClusterSystem::new(&node_id, config);
    system.start().await.expect("Failed to start cluster");
    
    // Start the chat service actor
    let chat_service = ChatServiceActor::new(node_id.clone()).start();
    
    // Shared collection for keeping track of simulated clients
    let clients = Arc::new(StdMutex::new(Vec::<SimulatedClient>::new()));
    let clients_clone = clients.clone();
    
    // Start a timer to simulate client activity
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let clients = clients_clone.lock().unwrap();
            
            if clients.is_empty() {
                continue;
            }
            
            // Simulate client activity by randomly choosing a client
            // to send a message
            let mut rng = rand::thread_rng();
            if let Some(client) = clients.get(rng.gen_range(0..clients.len())) {
                client.send_message(
                    "general".to_string(),
                    format!("Hello from {}!", client.user_id),
                );
            }
        }
    });
    
    // Simulate creating users and joining rooms
    let mut clients_guard = clients.lock().unwrap();
    for i in 1..=5 {
        let user_id = format!("user{}", i);
        let user_actor = UserActor::new(
            user_id.clone(),
            node_id.clone(),
            chat_service.clone(),
        ).start();
        
        let client = SimulatedClient::new(user_id, user_actor);
        
        // Join the general room
        client.join_room("general".to_string());
        
        // Every other user also joins the random room
        if i % 2 == 0 {
            client.join_room("random".to_string());
        }
        
        clients_guard.push(client);
    }
    drop(clients_guard);
    
    // Keep the system running
    tokio::signal::ctrl_c().await?;
    Ok(())
}

fn start_multi_node_test(node_count: usize, base_port: u16) -> std::io::Result<()> {
    // Create the first node (seed)
    let seed_addr = format!("127.0.0.1:{}", base_port);
    
    // Start the seed node in a separate thread
    std::thread::spawn(move || {
        let id = "node1".to_string();
        let addr: SocketAddr = seed_addr.parse().unwrap();
        let system = System::new();
        system.block_on(async {
            start_node(id, addr, None).await.unwrap();
        });
    });
    
    // Wait for the seed node to start
    std::thread::sleep(Duration::from_secs(2));
    
    // Start additional nodes
    for i in 2..=node_count {
        let port = base_port + (i as u16);
        let seed = format!("127.0.0.1:{}", base_port);
        
        std::thread::spawn(move || {
            let id = format!("node{}", i);
            let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            let system = System::new();
            system.block_on(async {
                start_node(id, addr, Some(seed)).await.unwrap();
            });
        });
        
        // Add a small delay between starting nodes
        std::thread::sleep(Duration::from_millis(500));
    }
    
    // Wait for Ctrl+C to terminate
    std::thread::sleep(Duration::from_secs(3600));
    
    Ok(())
} 