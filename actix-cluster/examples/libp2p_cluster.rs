//! Example of a decentralized Actix cluster using libp2p

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use actix_cluster::{
    ClusterSystem, ClusterConfig, Architecture, NodeRole, DiscoveryMethod,
    SerializationFormat, Node, NodeId, NodeInfo,
};
use actix::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Message)]
#[rtype(result = "String")]
struct Ping(String);

struct PingActor {
    node_id: NodeId,
}

impl Actor for PingActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("PingActor started on node: {}", self.node_id);
    }
}

impl Handler<Ping> for PingActor {
    type Result = String;
    
    fn handle(&mut self, msg: Ping, _ctx: &mut Self::Context) -> Self::Result {
        let response = format!("Pong from node {}: {}", self.node_id, msg.0);
        println!("Responding: {}", response);
        response
    }
}

#[actix_rt::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let is_bootstrap = args.len() > 1 && args[1] == "bootstrap";
    let port = if is_bootstrap { 8558 } else { 8559 };
    
    // Create cluster configuration
    let bootstrap_nodes = vec!["/ip4/127.0.0.1/tcp/8558".to_string()];
    
    let config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port))
        .discovery(DiscoveryMethod::LibP2P { 
            bootstrap_nodes: bootstrap_nodes.clone(), 
            enable_mdns: true,
        })
        .serialization_format(SerializationFormat::Bincode)
        .heartbeat_interval(Duration::from_secs(5))
        .build()?;
    
    // Create and start the cluster
    println!("Starting {} node on port {}", 
        if is_bootstrap { "bootstrap" } else { "client" }, 
        port);
    
    let cluster = ClusterSystem::new("libp2p-example", config);
    // 保存本地节点信息
    let local_node_id = cluster.local_node().id.clone();
    
    // 获取可变引用后再启动
    let mut cluster = cluster;
    let _system_actor = cluster.start().await?;
    
    // Start a ping actor on the current node
    let ping_actor = PingActor {
        node_id: local_node_id,
    }.start();
    
    if !is_bootstrap {
        // If we're not the bootstrap node, wait a bit and then ping other nodes
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // In a real application, we would use the distributed actor reference system
        // to get a reference to the remote actor and send a message to it
        println!("In a complete implementation, we would send messages to remote actors.");
        println!("For now, just printing the local actor response:");
        
        let response = ping_actor.send(Ping("Hello from local actor".to_string())).await?;
        println!("Got response: {}", response);
    }
    
    // Keep the program running
    println!("Press Ctrl+C to exit");
    
    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    
    println!("Shutting down...");
    System::current().stop();
    
    Ok(())
} 