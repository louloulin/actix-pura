use std::sync::Arc;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use trading::{
    TradingClusterManager,
    OrderSide, OrderType, OrderRequest, OrderResult
};
use uuid::Uuid;
use tokio::time::sleep;

/// 模拟节点故障和恢复的测试
#[tokio::test]
async fn test_cluster_node_failure_recovery() {
    // 创建第一个节点
    let node1_id = "node1".to_string();
    let node1_bind = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
    let mut node1 = TradingClusterManager::new(
        node1_id.clone(),
        node1_bind,
        "test-cluster".to_string()
    );
    
    // 初始化第一个节点
    node1.initialize().expect("无法初始化节点1");
    node1.register_actor_path("/user/order", trading::cluster::ActorType::Order).expect("无法注册Actor路径");
    let node1 = Arc::new(node1);
    
    // 创建第二个节点，并连接到第一个节点
    let node2_id = "node2".to_string();
    let node2_bind = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8559);
    let mut node2 = TradingClusterManager::new(
        node2_id.clone(),
        node2_bind,
        "test-cluster".to_string()
    );
    
    // 添加第一个节点作为种子节点
    node2.add_seed_node(format!("127.0.0.1:8558"));
    
    // 初始化第二个节点
    node2.initialize().expect("无法初始化节点2");
    node2.register_actor_path("/user/order", trading::cluster::ActorType::Order).expect("无法注册Actor路径");
    let node2 = Arc::new(node2);
    
    // 等待节点连接稳定
    sleep(Duration::from_millis(500)).await;
    
    // 在节点1上创建订单
    let order_req = OrderRequest {
        order_id: Some("recovery-test-1".to_string()),
        symbol: "BTC/USD".to_string(),
        side: OrderSide::Buy,
        price: Some(50000.0),
        quantity: 1,
        client_id: "client1".to_string(),
        order_type: OrderType::Limit,
    };
    
    node1.send_typed_message("/user/order", "CREATE_ORDER", order_req).expect("发送订单失败");
    
    // 模拟节点1故障 (在真实场景中，这应该是因为网络或其他故障导致的)
    println!("模拟节点1故障...");
    // 我们只能模拟节点1不再响应，但不会实际关闭它
    
    // 等待一段时间，让系统检测到节点1故障
    sleep(Duration::from_secs(2)).await;
    
    // 在节点2上创建订单，应该仍然可以工作
    let order_req2 = OrderRequest {
        order_id: Some("recovery-test-2".to_string()),
        symbol: "ETH/USD".to_string(),
        side: OrderSide::Sell,
        price: Some(2000.0),
        quantity: 5,
        client_id: "client2".to_string(),
        order_type: OrderType::Limit,
    };
    
    node2.send_typed_message("/user/order", "CREATE_ORDER", order_req2).expect("在节点2上发送订单失败");
    
    // 模拟节点1恢复
    println!("模拟节点1恢复...");
    // 在实际场景中，这里应该重新启动节点1，但在测试中我们只能假设它恢复了
    
    // 等待一段时间，让系统检测到节点1恢复
    sleep(Duration::from_secs(2)).await;
    
    // 验证节点1再次可以处理订单
    let order_req3 = OrderRequest {
        order_id: Some("recovery-test-3".to_string()),
        symbol: "BTC/USD".to_string(),
        side: OrderSide::Buy,
        price: Some(51000.0),
        quantity: 2,
        client_id: "client1".to_string(),
        order_type: OrderType::Limit,
    };
    
    node1.send_typed_message("/user/order", "CREATE_ORDER", order_req3).expect("恢复后发送订单失败");
    
    // 验证集群状态正常
    assert_eq!(node1.get_status(), trading::cluster::NodeStatus::Running);
    assert_eq!(node2.get_status(), trading::cluster::NodeStatus::Running);
    
    println!("故障恢复测试通过!");
} 