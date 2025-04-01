use std::sync::Arc;
use log::{info, warn, error};
use tokio::sync::mpsc;
use std::error::Error;

use trading::actor::{Actor, ActorRef, ActorSystem};
use trading::consensus::RaftService;
use trading::execution::ExecutionEngine;
use trading::risk::RiskManager;
use trading::actor::OrderActor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 初始化日志系统
    env_logger::init();
    
    info!("Starting Trading System...");
    
    // 创建Actor系统
    let actor_system = ActorSystem::new();
    info!("Actor system created: {}", actor_system.name);
    
    // 创建节点ID
    let node_id = format!("node-{}", uuid::Uuid::new_v4());
    info!("Local node ID: {}", node_id);
    
    // 创建Raft服务
    let (tx, rx) = mpsc::channel(100);
    info!("Creating Raft service...");
    
    // 创建执行引擎
    let execution_engine = ExecutionEngine::new(node_id.clone());
    let execution_addr = actor_system.create_actor(Box::new(execution_engine)).await;
    info!("Execution engine started at: {}", execution_addr.path());
    
    // 创建风控管理器
    let risk_manager = RiskManager::new(node_id.clone());
    let risk_addr = actor_system.create_actor(Box::new(risk_manager)).await;
    info!("Risk manager started at: {}", risk_addr.path());
    
    // 创建订单管理器
    let order_actor = OrderActor::new(node_id.clone());
    let order_addr = actor_system.create_actor(Box::new(order_actor)).await;
    info!("Order manager started at: {}", order_addr.path());
    
    info!("Trading system started on node {}", node_id);
    info!("Press Ctrl+C to stop...");
    
    // 等待中断信号
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");
    
    Ok(())
} 