use std::sync::Arc;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use actix_web::{App, HttpServer, web};
use actix::prelude::*;
use log::{info, error};
use clap::{App as ClapApp, Arg};
use env_logger::Env;

use trading::TradingClusterManager;
use trading::cluster::OrderActor;
use trading::cluster::ActorType;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    
    // 解析命令行参数
    let matches = ClapApp::new("Trading System")
        .version("1.0")
        .author("Actix Trading Team")
        .about("Distributed Trading System based on Actix")
        .arg(Arg::with_name("node_id")
            .short('n')
            .long("node-id")
            .value_name("ID")
            .help("Node identifier")
            .takes_value(true)
            .required(false))
        .arg(Arg::with_name("http_port")
            .short('p')
            .long("port")
            .value_name("PORT")
            .help("HTTP port for API")
            .takes_value(true)
            .required(false))
        .arg(Arg::with_name("seed_nodes")
            .short('s')
            .long("seed-nodes")
            .value_name("NODES")
            .help("Comma separated list of seed nodes")
            .takes_value(true)
            .required(false))
        .get_matches();
    
    // 获取节点信息
    let node_id = matches.value_of("node_id")
        .unwrap_or("node1")
        .to_string();
    
    let http_port = matches.value_of("http_port")
        .unwrap_or("8080")
        .parse::<u16>()
        .expect("Invalid HTTP port");
    
    let seed_nodes = matches.value_of("seed_nodes")
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect::<Vec<String>>())
        .unwrap_or_else(Vec::new);
    
    info!("Starting Trading System on node: {}", node_id);
    info!("HTTP port: {}", http_port);
    
    if !seed_nodes.is_empty() {
        info!("Seed nodes: {:?}", seed_nodes);
    }
    
    // 创建绑定地址
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0); // 随机端口
    
    // 创建集群管理器
    let mut cluster_manager = TradingClusterManager::new(
        node_id.clone(), 
        bind_addr,
        "trading-cluster".to_string()
    );
    
    // 添加种子节点
    for node in seed_nodes {
        cluster_manager.add_seed_node(node);
    }
    
    // 初始化集群
    match cluster_manager.initialize() {
        Ok(_) => {
            info!("Trading系统已启动: {}", node_id);
            
            // 注册各服务路径
            cluster_manager.register_actor_path("/user/order", ActorType::Order).unwrap();
            
            info!("Actor路径已注册");
            
            // 创建集群管理器的Arc引用
            let cluster_manager = Arc::new(cluster_manager);
            
            // 创建Actix系统
            System::new().block_on(async {
                // 创建订单Actor
                let order_actor = OrderActor::new(node_id.clone());
                let order_addr = order_actor.start();
                
                info!("系统Actors已启动");
                
                // 启动HTTP服务器
                info!("Starting HTTP server on port {}", http_port);
                
                let server = HttpServer::new(move || {
                    App::new()
                        .app_data(web::Data::new(cluster_manager.clone()))
                        .app_data(web::Data::new(order_addr.clone()))
                        .route("/api/health", web::get().to(|| async { "Trading System is running!" }))
                })
                .bind(format!("0.0.0.0:{}", http_port))?;
                
                server.run().await
            })
        },
        Err(e) => {
            error!("启动集群失败: {}", e);
            Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        }
    }
} 