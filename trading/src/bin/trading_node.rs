use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use std::env;
use clap::{App, Arg};
use log::{info, error};
use actix::prelude::*;

use trading::{
    OrderActor, ExecutionEngine, RiskManager, TradingClusterManager
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 配置命令行参数
    let matches = App::new("Trading Node")
        .version("1.0")
        .author("Trading System Team")
        .about("Distributed Trading System Node")
        .arg(Arg::new("node_id")
            .short('n')
            .long("node-id")
            .value_name("ID")
            .help("Node identifier")
            .takes_value(true)
            .required(false))
        .arg(Arg::new("port")
            .short('p')
            .long("port")
            .value_name("PORT")
            .help("Binding port")
            .takes_value(true)
            .required(false))
        .arg(Arg::new("seed")
            .short('s')
            .long("seed")
            .value_name("SEED_ADDR")
            .help("Seed node address (can be specified multiple times)")
            .takes_value(true)
            .multiple_occurrences(true)
            .required(false))
        .get_matches();

    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // 获取节点标识符
    let node_id = matches.value_of("node_id")
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    
    // 获取端口
    let port = matches.value_of("port")
        .map(|s| s.parse::<u16>().unwrap_or(8558))
        .unwrap_or(8558);
    
    // 创建绑定地址
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
    
    // 创建集群管理器
    let mut cluster_manager = TradingClusterManager::new(
        node_id.clone(), 
        bind_addr,
        "trading-cluster".to_string()
    );
    
    // 添加种子节点
    if let Some(seeds) = matches.values_of("seed") {
        for seed in seeds {
            cluster_manager.add_seed_node(seed.to_string());
        }
    }
    
    // 初始化集群
    match cluster_manager.initialize() {
        Ok(_) => {
            info!("Trading节点已启动: {}", node_id);
            
            // 注册各服务路径
            cluster_manager.register_actor("/user/order")?;
            cluster_manager.register_actor("/user/execution")?;
            cluster_manager.register_actor("/user/risk")?;
            
            info!("Actor路径已注册");
            
            // 创建Actix系统
            let system = System::new();
            system.block_on(async {
                // 启动Actix Actor
                let order_actor = OrderActor::new(node_id.clone()).start();
                
                // 处理订单测试
                info!("发送测试订单");
                let order_msg = trading::cluster::OrderMessage {
                    order_id: "test-1".to_string(),
                    symbol: "BTC/USD".to_string(),
                    price: 50000.0,
                    quantity: 1,
                };
                
                match order_actor.send(order_msg).await {
                    Ok(result) => {
                        match result {
                            Ok(msg) => info!("订单结果: {}", msg),
                            Err(e) => error!("订单错误: {}", e),
                        }
                    },
                    Err(e) => error!("发送失败: {}", e),
                }
                
                // 等待5秒后关闭
                tokio::time::sleep(Duration::from_secs(5)).await;
                System::current().stop();
            });
            
            Ok(())
        },
        Err(e) => {
            error!("启动集群失败: {}", e);
            Err(e)
        }
    }
} 