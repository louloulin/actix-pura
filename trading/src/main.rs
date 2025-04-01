use std::sync::Arc;
use actix_web::{App, HttpServer, web};
use actix::Actor;
use log::{info, warn, error};
use clap::{App as ClapApp, Arg};
use env_logger::Env;

mod actor;
mod api;
mod consensus;
mod execution;
mod models;
mod risk;

use crate::actor::order::OrderActor;
use crate::actor::account::AccountActor;
use crate::risk::manager::RiskActor;
use crate::execution::engine::ExecutionActor;
use crate::consensus::raft::RaftService;
use crate::api::gateway::ApiGatewayActor;
use crate::api::gateway::configure_routes;

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
            .short("n")
            .long("node-id")
            .value_name("ID")
            .help("Node identifier")
            .takes_value(true)
            .required(false))
        .arg(Arg::with_name("http_port")
            .short("p")
            .long("port")
            .value_name("PORT")
            .help("HTTP port for API")
            .takes_value(true)
            .required(false))
        .arg(Arg::with_name("raft_port")
            .short("r")
            .long("raft-port")
            .value_name("PORT")
            .help("Port for Raft consensus")
            .takes_value(true)
            .required(false))
        .arg(Arg::with_name("seed_nodes")
            .short("s")
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
    
    let raft_port = matches.value_of("raft_port")
        .unwrap_or("9000")
        .parse::<u16>()
        .expect("Invalid Raft port");
    
    let seed_nodes = matches.value_of("seed_nodes")
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect::<Vec<String>>())
        .unwrap_or_else(Vec::new);
    
    info!("Starting Trading System on node: {}", node_id);
    info!("HTTP port: {}, Raft port: {}", http_port, raft_port);
    
    if !seed_nodes.is_empty() {
        info!("Seed nodes: {:?}", seed_nodes);
    }
    
    // 启动Actors
    info!("Starting system actors...");
    
    // 风控Actor
    let risk_actor = RiskActor::new(node_id.clone()).start();
    
    // 账户Actor
    let account_actor = AccountActor::new(node_id.clone()).start();
    
    // 执行引擎Actor
    let execution_actor = ExecutionActor::new(node_id.clone()).start();
    
    // 订单管理Actor
    let order_actor = OrderActor::new(node_id.clone()).start();
    
    // Raft共识服务 (简化处理，实际应该是Actor)
    let raft_service = Arc::new(RaftService::new(node_id.clone(), raft_port));
    
    // API网关Actor
    let api_gateway = ApiGatewayActor::new(
        node_id.clone(),
        order_actor.clone(),
        account_actor.clone(),
        risk_actor.clone(),
    ).start();
    
    // 启动HTTP服务器
    info!("Starting HTTP server on port {}", http_port);
    
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(api_gateway.clone()))
            .configure(|cfg| configure_routes(cfg, web::Data::new(api_gateway.clone())))
    })
    .bind(format!("0.0.0.0:{}", http_port))?
    .run()
    .await
} 