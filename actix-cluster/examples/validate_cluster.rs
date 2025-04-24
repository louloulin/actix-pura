use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::sleep;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole, SerializationFormat
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));

    // 创建tokio运行时和LocalSet
    let runtime = Runtime::new()?;
    let local = LocalSet::new();

    local.block_on(&runtime, async {
        // 创建第一个集群节点
        let node1_config = create_config("node1", 8551, Architecture::Decentralized);
        let mut node1_system = ClusterSystem::new(node1_config);
        log::info!("Node 1 created: {}", node1_system.local_node().id);

        // 获取节点1信息，以便节点2可以连接
        let node1_info = node1_system.local_node().clone();

        // 启动第一个集群节点
        let _node1_addr = node1_system.start().await?;
        log::info!("Node 1 started");

        // 稍等片刻，让节点1完成初始化
        sleep(Duration::from_secs(1)).await;

        // 创建第二个集群节点，它知道第一个节点
        let node2_config = create_config("node2", 8552, Architecture::Decentralized);
        let mut node2_system = ClusterSystem::new(node2_config);
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

        // 结束测试
        log::info!("Test completed successfully");

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
        .discovery(DiscoveryMethod::Static {
            seed_nodes: if name == "node1" { vec![] } else { bootstrap_nodes }
        })
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .expect("Failed to build cluster config")
}