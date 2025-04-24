use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use actix_rt::System;
use actix::prelude::*;
use env_logger;
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
        let config = ClusterConfig::default()
            .discovery(DiscoveryMethod::Static { seed_nodes: vec![] });

        let mut cluster_system = ClusterSystem::new(config);
        log::info!("节点创建成功: {}", cluster_system.local_node().id);

        match cluster_system.start().await {
            Ok(addr) => {
                log::info!("节点启动成功: {}", cluster_system.local_node().id);

                // 等待几秒钟看看节点运行状态
                tokio::time::sleep(Duration::from_secs(5)).await;

                // 记录结束消息
                log::info!("测试完成，主动退出");

                // 无需手动停止，让进程自然结束
            },
            Err(e) => {
                log::error!("节点启动失败: {}", e);
            }
        }
    });

    Ok(())
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