use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use actix_rt::System;
use actix::prelude::*;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole, SerializationFormat
};

fn main() {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));
    
    // 使用actix运行时
    let system = System::new();
    system.block_on(async {
        // 创建集群节点配置
        let config = create_config();
        
        // 创建集群系统
        let cluster_system = ClusterSystem::new("test-node", config);
        log::info!("节点创建成功: {}", cluster_system.local_node().id);
        
        // 启动集群系统
        match cluster_system.start().await {
            Ok(_addr) => {
                log::info!("集群系统成功启动");
                
                // 保持系统运行一段时间
                tokio::time::sleep(Duration::from_secs(5)).await;
                
                // 演示完成，程序自然结束
                log::info!("演示完成，程序结束");
            },
            Err(e) => {
                log::error!("集群系统启动失败: {}", e);
            }
        }
    });
}

fn create_config() -> ClusterConfig {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
    
    ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(addr)
        .cluster_name("test-cluster".to_string())
        .heartbeat_interval(Duration::from_secs(1))
        .node_timeout(Duration::from_secs(5))
        .discovery(DiscoveryMethod::Static { seed_nodes: Vec::new() })
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .expect("创建集群配置失败")
} 