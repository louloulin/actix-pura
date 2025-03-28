use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::sleep;
use actix_rt::System;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole, SerializationFormat
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));
    
    // 使用actix运行时
    let system = System::new();
    system.block_on(async {
        // 创建第一个集群节点
        let node1_config = create_config("node1", 8551, Architecture::Decentralized);
        let node1_system = ClusterSystem::new("node1", node1_config);
        log::info!("Node 1 created: {}", node1_system.local_node().id);
        
        // 启动第一个集群节点
        let _node1_addr = node1_system.start().await?;
        log::info!("Node 1 started");
        
        // 稍等片刻，让节点1完成初始化
        sleep(Duration::from_secs(1)).await;
        
        // 创建第二个集群节点
        let node2_config = create_config("node2", 8552, Architecture::Decentralized);
        let node2_system = ClusterSystem::new("node2", node2_config);
        log::info!("Node 2 created: {}", node2_system.local_node().id);
        
        // 启动第二个集群节点
        let _node2_addr = node2_system.start().await?;
        log::info!("Node 2 started");
        
        // 等待节点发现和心跳通信
        log::info!("Waiting for nodes to discover each other...");
        sleep(Duration::from_secs(5)).await;
        
        // 打印节点状态以验证
        log::info!("Nodes should have discovered each other");
        log::info!("Verification complete!");
        
        // 保持程序运行以观察日志
        log::info!("Press Ctrl+C to exit");
        // 使用信号处理
        let mut signals = signal_hook::iterator::Signals::new([signal_hook::consts::SIGINT])?;
        
        // 检查是否收到信号
        if signals.forever().next().is_some() {
            log::info!("Received interrupt signal, shutting down");
        }
        
        Ok(())
    })
}

fn create_config(name: &str, port: u16, architecture: Architecture) -> ClusterConfig {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
    
    // 在实际环境中，应该将第一个节点的地址作为bootstrap节点提供给第二个节点
    let bootstrap_nodes = vec![
        "127.0.0.1:8551".to_string(),
        "127.0.0.1:8552".to_string(),
    ];
    
    ClusterConfig::new()
        .architecture(architecture)
        .node_role(NodeRole::Peer)
        .bind_addr(addr)
        .cluster_name(format!("{}-cluster", name))
        .heartbeat_interval(Duration::from_secs(1))
        .node_timeout(Duration::from_secs(5))
        .discovery(DiscoveryMethod::LibP2P { 
            bootstrap_nodes,
            enable_mdns: true,
        })
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .expect("Failed to build cluster config")
} 