use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use actix::prelude::*;
use crate::cluster::{
    TradingClusterManager, ClusterMessage, ClusterMessageResult,
    OrderActor, OrderMessage, ActorType, RaftConsensusActor, RaftMessage
};

#[test]
fn test_cluster_manager_creation() {
    let node_id = "node1".to_string();
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let cluster_name = "trading-cluster".to_string();
    
    let manager = TradingClusterManager::new(node_id.clone(), bind_addr, cluster_name.clone());
    
    assert_eq!(manager.node_id, node_id);
    assert_eq!(manager.bind_addr, bind_addr);
    assert_eq!(manager.cluster_name, cluster_name);
    assert!(manager.seed_nodes.is_empty());
}

#[test]
fn test_add_seed_node() {
    let mut manager = TradingClusterManager::new(
        "node1".to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        "trading-cluster".to_string()
    );
    
    let seed1 = "127.0.0.1:8081".to_string();
    let seed2 = "127.0.0.1:8082".to_string();
    
    manager.add_seed_node(seed1.clone());
    manager.add_seed_node(seed2.clone());
    
    assert_eq!(manager.seed_nodes.len(), 2);
    assert_eq!(manager.seed_nodes[0], seed1);
    assert_eq!(manager.seed_nodes[1], seed2);
}

#[test]
fn test_initialize() {
    let mut manager = TradingClusterManager::new(
        "node1".to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        "trading-cluster".to_string()
    );
    
    manager.add_seed_node("127.0.0.1:8081".to_string());
    
    let result = manager.initialize();
    assert!(result.is_ok());
    
    // 此时状态应该是Running
    let status = manager.get_status();
    assert_eq!(format!("{:?}", status), "Running");
}

#[test]
fn test_register_actor_path() {
    let manager = TradingClusterManager::new(
        "node1".to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        "trading-cluster".to_string()
    );
    
    let path = "/user/order/1";
    let result = manager.register_actor_path(path, ActorType::Order);
    assert!(result.is_ok());
    
    let actor_path = manager.find_actor_path(path);
    assert!(actor_path.is_some());
    let actor_path = actor_path.unwrap();
    assert_eq!(actor_path.path, path);
    assert_eq!(actor_path.node_id, "node1");
    assert_eq!(actor_path.actor_type, ActorType::Order);
}

#[test]
fn test_send_message() {
    let manager = TradingClusterManager::new(
        "node1".to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        "trading-cluster".to_string()
    );
    
    let path = "/user/order/1";
    let msg_type = "order.create";
    let result = manager.send_message(path, msg_type);
    assert!(result.is_ok());
}

#[actix_rt::test]
async fn test_order_actor_with_cluster() {
    // 创建集群管理器
    let manager = Arc::new(TradingClusterManager::new(
        "node1".to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        "trading-cluster".to_string()
    ));
    
    // 初始化集群
    manager.initialize().expect("集群初始化失败");
    
    // 创建OrderActor并绑定集群管理器
    let order_actor = OrderActor::with_cluster_manager("test-order".to_string(), manager.clone());
    let addr = order_actor.start();
    
    // 发送消息给Actor
    let order_msg = OrderMessage {
        order_id: "order123".to_string(),
        symbol: "AAPL".to_string(),
        price: 150.0,
        quantity: 100,
    };
    
    let result = addr.send(order_msg).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.is_ok());
    assert_eq!(response.unwrap(), "订单 order123 已接受");
    
    // 发送集群消息
    let cluster_msg = ClusterMessage {
        id: "msg1".to_string(),
        sender_node: "node2".to_string(),
        target_node: "node1".to_string(),
        sender_path: "/user/order/sender".to_string(),
        target_path: "/user/order/test-order".to_string(),
        message_type: "order.update".to_string(),
        payload: vec![],
        timestamp: 123456789,
    };
    
    let result = addr.send(cluster_msg).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(matches!(response, ClusterMessageResult::Success));
}

#[actix_rt::test]
async fn test_raft_consensus_actor() {
    // 创建集群管理器
    let manager = Arc::new(TradingClusterManager::new(
        "node1".to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        "trading-cluster".to_string()
    ));
    
    // 初始化集群
    manager.initialize().expect("集群初始化失败");
    
    // 创建RaftConsensusActor并绑定集群管理器
    let raft_actor = RaftConsensusActor::with_cluster_manager("test-raft".to_string(), manager.clone());
    let addr = raft_actor.start();
    
    // 发送Raft消息
    let raft_msg = RaftMessage {
        term: 1,
        leader_id: Some("leader1".to_string()),
        prev_log_index: 0,
        prev_log_term: 0,
        entries: vec![],
        leader_commit: 0,
    };
    
    let result = addr.send(raft_msg).await;
    assert!(result.is_ok());
    
    // 验证响应
    if let Ok(response) = result {
        match response {
            crate::cluster::RaftResult::Success { term, success } => {
                assert_eq!(term, 1);
                assert!(success);
            },
            _ => panic!("预期响应类型为Success"),
        }
    }
    
    // 发送集群消息
    let cluster_msg = ClusterMessage {
        id: "msg1".to_string(),
        sender_node: "node2".to_string(),
        target_node: "node1".to_string(),
        sender_path: "/system/raft/sender".to_string(),
        target_path: "/system/raft/test-raft".to_string(),
        message_type: "raft.append_entries".to_string(),
        payload: vec![],
        timestamp: 123456789,
    };
    
    let result = addr.send(cluster_msg).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(matches!(response, ClusterMessageResult::Success));
}

// 测试多节点通信（模拟）
#[actix_rt::test]
async fn test_multi_node_communication() {
    // 创建两个集群管理器，模拟两个节点
    let manager1 = Arc::new(TradingClusterManager::new(
        "node1".to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        "trading-cluster".to_string()
    ));
    
    let manager2 = Arc::new(TradingClusterManager::new(
        "node2".to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        "trading-cluster".to_string()
    ));
    
    // 初始化集群
    manager1.initialize().expect("集群初始化失败");
    manager2.initialize().expect("集群初始化失败");
    
    // 创建两个Actor
    let order_actor1 = OrderActor::with_cluster_manager("order1".to_string(), manager1.clone());
    let order_actor2 = OrderActor::with_cluster_manager("order2".to_string(), manager2.clone());
    
    let addr1 = order_actor1.start();
    let addr2 = order_actor2.start();
    
    // 注册Actor路径
    let path1 = "/user/order/order1";
    let path2 = "/user/order/order2";
    
    manager1.register_actor_path(path1, ActorType::Order).expect("注册Actor路径失败");
    manager2.register_actor_path(path2, ActorType::Order).expect("注册Actor路径失败");
    
    // 模拟节点1向节点2发送消息
    let result = manager1.send_message(path2, "order.create");
    assert!(result.is_ok());
    
    // 模拟节点2向节点1发送消息
    let result = manager2.send_message(path1, "order.update");
    assert!(result.is_ok());
    
    // 关闭Actors
    System::current().stop();
} 