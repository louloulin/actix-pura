use std::net::SocketAddr;
use std::time::Duration;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex};

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

// Command line arguments
#[derive(StructOpt, Debug)]
#[structopt(name = "chat_system", about = "Distributed chat system example for ActixCluster")]
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

// User message
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
struct ChatMessage {
    from: String,
    room: String,
    content: String,
    timestamp: u64,
}

// Room management messages
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
struct JoinRoom {
    user_id: String,
    room: String,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
struct LeaveRoom {
    user_id: String,
    room: String,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Vec<String>")]
struct GetRoomUsers {
    room: String,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Vec<String>")]
struct GetAllRooms;

// Chat service actor
struct ChatServiceActor {
    node_id: String,
    // room -> set of user_ids
    rooms: HashMap<String, HashSet<String>>,
    // user_id -> actor_ref for direct messaging
    users: HashMap<String, Box<dyn ActorRef>>,
}

impl ChatServiceActor {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            rooms: HashMap::new(),
            users: HashMap::new(),
        }
    }
    
    fn now_micros() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_micros() as u64
    }
}

impl Actor for ChatServiceActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("ChatServiceActor started on node {}", self.node_id);
    }
}

// Handle chat messages
impl Handler<ChatMessage> for ChatServiceActor {
    type Result = ();
    
    fn handle(&mut self, msg: ChatMessage, _: &mut Self::Context) -> Self::Result {
        info!("Message in room '{}' from {}: {}", msg.room, msg.from, msg.content);
        
        // Get users in the room
        if let Some(users) = self.rooms.get(&msg.room) {
            // Forward message to all users in the room
            for user_id in users {
                if let Some(user_ref) = self.users.get(user_id) {
                    // Skip sending to the original sender
                    if *user_id != msg.from {
                        let boxed_msg = Box::new(msg.clone()) as Box<dyn std::any::Any + Send>;
                        if let Err(e) = user_ref.send_any(boxed_msg) {
                            warn!("Failed to send message to {}: {}", user_id, e);
                        }
                    }
                }
            }
        } else {
            warn!("Message sent to non-existent room: {}", msg.room);
        }
    }
}

// Room management
impl Handler<JoinRoom> for ChatServiceActor {
    type Result = ();
    
    fn handle(&mut self, msg: JoinRoom, _: &mut Self::Context) -> Self::Result {
        info!("User {} is joining room {}", msg.user_id, msg.room);
        
        // Create room if it doesn't exist
        let room_users = self.rooms.entry(msg.room.clone()).or_insert_with(HashSet::new);
        room_users.insert(msg.user_id.clone());
        
        // Broadcast join notification to room
        let join_notification = ChatMessage {
            from: "SYSTEM".to_string(),
            room: msg.room.clone(),
            content: format!("{} has joined the room", msg.user_id),
            timestamp: Self::now_micros(),
        };
        
        self.handle(join_notification, &mut Context::new());
    }
}

impl Handler<LeaveRoom> for ChatServiceActor {
    type Result = ();
    
    fn handle(&mut self, msg: LeaveRoom, _: &mut Self::Context) -> Self::Result {
        info!("User {} is leaving room {}", msg.user_id, msg.room);
        
        if let Some(room_users) = self.rooms.get_mut(&msg.room) {
            room_users.remove(&msg.user_id);
            
            // Remove room if empty
            if room_users.is_empty() {
                self.rooms.remove(&msg.room);
                info!("Room {} is now empty and has been removed", msg.room);
            } else {
                // Broadcast leave notification
                let leave_notification = ChatMessage {
                    from: "SYSTEM".to_string(),
                    room: msg.room.clone(),
                    content: format!("{} has left the room", msg.user_id),
                    timestamp: Self::now_micros(),
                };
                
                self.handle(leave_notification, &mut Context::new());
            }
        }
    }
}

impl Handler<GetRoomUsers> for ChatServiceActor {
    type Result = Vec<String>;
    
    fn handle(&mut self, msg: GetRoomUsers, _: &mut Self::Context) -> Self::Result {
        if let Some(users) = self.rooms.get(&msg.room) {
            users.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }
}

impl Handler<GetAllRooms> for ChatServiceActor {
    type Result = Vec<String>;
    
    fn handle(&mut self, _: GetAllRooms, _: &mut Self::Context) -> Self::Result {
        self.rooms.keys().cloned().collect()
    }
}

// Implement AnyMessage handler to make it discoverable in the cluster
impl Handler<AnyMessage> for ChatServiceActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(chat_msg) = msg.downcast_ref::<ChatMessage>() {
            // Handle chat message
            self.handle(chat_msg.clone(), ctx);
        } else if let Some(join_msg) = msg.downcast_ref::<JoinRoom>() {
            // Handle join room
            self.handle(join_msg.clone(), ctx);
        } else if let Some(leave_msg) = msg.downcast_ref::<LeaveRoom>() {
            // Handle leave room
            self.handle(leave_msg.clone(), ctx);
        } else {
            warn!("Received unknown message type");
        }
    }
}

// User actor representing a connected client
struct UserActor {
    user_id: String,
    node_id: String,
    chat_service: Option<Box<dyn ActorRef>>,
}

impl UserActor {
    fn new(user_id: String, node_id: String) -> Self {
        Self {
            user_id,
            node_id,
            chat_service: None,
        }
    }
    
    fn set_chat_service(&mut self, service_ref: Box<dyn ActorRef>) {
        self.chat_service = Some(service_ref);
    }
}

impl Actor for UserActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("UserActor for {} started on node {}", self.user_id, self.node_id);
    }
    
    fn stopped(&mut self, ctx: &mut Self::Context) {
        info!("UserActor for {} stopped", self.user_id);
        
        // Leave all rooms when the user disconnects
        if let Some(service_ref) = &self.chat_service {
            // In a real implementation, we would track which rooms the user is in
            // and leave them all here
        }
    }
}

// Handle incoming chat messages (from other users)
impl Handler<ChatMessage> for UserActor {
    type Result = ();
    
    fn handle(&mut self, msg: ChatMessage, _: &mut Self::Context) -> Self::Result {
        // In a real implementation, this would forward the message to the connected client
        info!("User {} received: [{}] {}: {}", 
              self.user_id, msg.room, msg.from, msg.content);
    }
}

// Handle AnyMessage for cluster communication
impl Handler<AnyMessage> for UserActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(chat_msg) = msg.downcast_ref::<ChatMessage>() {
            self.handle(chat_msg.clone(), ctx);
        } else {
            warn!("User received unknown message type");
        }
    }
}

// Start a node in the chat system
async fn start_node(node_id: String, addr: SocketAddr, seed_nodes: Vec<String>, is_server: bool) -> std::io::Result<()> {
    // Create cluster configuration
    let mut config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(if is_server { NodeRole::Server } else { NodeRole::Client })
        .bind_addr(addr)
        .cluster_name("chat-cluster".to_string())
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
    
    // Create chat service actor (only on server nodes)
    if is_server {
        let chat_service = ChatServiceActor::new(node_id.clone()).start();
        
        // Register chat service to the cluster
        let service_path = "/user/chat/service";
        info!("Registering chat service: {}", service_path);
        
        // Ensure cluster system is initialized
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        match sys.register(service_path, chat_service.clone()).await {
            Ok(_) => info!("Successfully registered chat service"),
            Err(e) => error!("Failed to register chat service: {}", e),
        }
        
        // Wait for the service to be fully registered
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Create a few sample rooms
        if node_id == "node1" || node_id == "master" {
            let default_rooms = vec!["general", "random", "tech"];
            for room in default_rooms {
                info!("Creating default room: {}", room);
                // In a real implementation, we would store room information persistently
            }
        }
    }
    
    // Create a simulated user actor for testing
    let user_id = format!("user-{}-{}", node_id, 1);
    let user_actor = UserActor::new(user_id.clone(), node_id.clone()).start();
    
    // Register user actor to the cluster
    let user_path = format!("/user/chat/users/{}", user_id);
    info!("Registering user: {}", user_path);
    
    match sys.register(&user_path, user_actor.clone()).await {
        Ok(_) => info!("Successfully registered user {}", user_id),
        Err(e) => error!("Failed to register user {}: {}", user_id, e),
    }
    
    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Find chat service if this is a client node
    if !is_server {
        info!("Looking for chat service...");
        if let Some(service_ref) = sys.lookup("/user/chat/service").await {
            info!("Found chat service");
            
            // Join a room
            let join_msg = JoinRoom {
                user_id: user_id.clone(),
                room: "general".to_string(),
            };
            
            let boxed_msg = Box::new(join_msg) as Box<dyn std::any::Any + Send>;
            match service_ref.send_any(boxed_msg) {
                Ok(_) => info!("User {} joined room general", user_id),
                Err(e) => error!("Failed to join room: {}", e),
            }
            
            // Send a test message
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            let chat_msg = ChatMessage {
                from: user_id.clone(),
                room: "general".to_string(),
                content: format!("Hello from {}!", node_id),
                timestamp: ChatServiceActor::now_micros(),
            };
            
            let boxed_msg = Box::new(chat_msg) as Box<dyn std::any::Any + Send>;
            match service_ref.send_any(boxed_msg) {
                Ok(_) => info!("Message sent to room general"),
                Err(e) => error!("Failed to send message: {}", e),
            }
        } else {
            error!("Could not find chat service");
        }
    }
    
    // Keep the node running
    let wait_time = if is_server { 60 } else { 30 };
    info!("Node {} running for {} seconds", node_id, wait_time);
    tokio::time::sleep(Duration::from_secs(wait_time)).await;
    
    info!("Node {} shutting down", node_id);
    Ok(())
}

// Run multi-node test locally
async fn run_multi_node_test(node_count: usize, base_port: u16) -> std::io::Result<()> {
    info!("Starting chat system with {} nodes", node_count);
    
    if node_count < 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Node count must be at least 2 (1 server and 1 client)"
        ));
    }
    
    // Create a LocalSet for running local tasks
    let local = tokio::task::LocalSet::new();
    
    // Set up the master (server) node
    let master_addr = format!("127.0.0.1:{}", base_port);
    
    // Run all node tasks in the LocalSet
    local.run_until(async move {
        // Start the server node first
        let server_handle = tokio::task::spawn_local(async move {
            match start_node(
                "master".to_string(),
                master_addr.parse().unwrap(),
                Vec::new(),
                true
            ).await {
                Ok(_) => info!("Server node completed"),
                Err(e) => error!("Server node failed: {:?}", e),
            }
        });
        
        // Let the server start up first
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Start client nodes
        let mut client_handles = Vec::new();
        for i in 1..node_count {
            let port = base_port + i as u16;
            let node_id = format!("client{}", i);
            let addr = format!("127.0.0.1:{}", port);
            let seed = master_addr.clone();
            
            info!("Starting client node {}, address: {}, seed: {}", node_id, addr, seed);
            
            let client_handle = tokio::task::spawn_local(async move {
                match start_node(
                    node_id.clone(),
                    addr.parse().unwrap(),
                    vec![seed],
                    false
                ).await {
                    Ok(_) => info!("Client node {} completed", node_id),
                    Err(e) => error!("Client node {} failed: {:?}", node_id, e),
                }
            });
            
            client_handles.push(client_handle);
            
            // Space out client starts to avoid overloading
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        // Wait for all clients to complete
        for (i, handle) in client_handles.into_iter().enumerate() {
            if let Err(e) = handle.await {
                error!("Client task {} error: {:?}", i+1, e);
            }
        }
        
        // Wait for server to complete
        if let Err(e) = server_handle.await {
            error!("Server task error: {:?}", e);
        }
        
        info!("Chat system test completed");
    }).await;
    
    Ok(())
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
        run_multi_node_test(args.nodes, args.base_port).await?;
    } else {
        // Single node mode - manual connection
        let mut seed_nodes = Vec::new();
        if let Some(seed) = args.seed {
            seed_nodes.push(seed);
        }
        
        // If no seed nodes, this is a server, otherwise a client
        let is_server = seed_nodes.is_empty();
        
        info!("Starting {} node ID: {}, address: {}", 
            if is_server { "server" } else { "client" }, 
            args.id, 
            args.address
        );
        
        if !seed_nodes.is_empty() {
            info!("Seed nodes: {:?}", seed_nodes);
        }
        
        // Start node
        start_node(args.id, args.address, seed_nodes, is_server).await?;
    }
    
    Ok(())
} 