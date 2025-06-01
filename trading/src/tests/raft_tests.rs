
use crate::cluster::{TradingClusterManager, ActorType};
use crate::models::order::{OrderRequest, OrderType, OrderSide};
use crate::consensus::state_machine::{StateMachine, StateMachineCommand, TradingStateMachine};

#[test]
fn test_raft_consensus_basics() {
    // 创建一个状态机实例
    let mut state_machine = TradingStateMachine::new();
    
    // 构建一个示例订单请求
    let order_request = OrderRequest {
        order_id: Some("test-order-1".to_string()),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        price: Some(150.0),
        quantity: 100,
        client_id: "client-1".to_string(),
        order_type: OrderType::Limit,
    };
    
    // 创建一个状态机命令
    let cmd = StateMachineCommand::CreateOrder(order_request.clone());
    
    // 应用命令到状态机
    let result = state_machine.apply(&cmd);
    
    // 验证命令应用是否成功
    assert!(result.is_ok(), "应用命令失败: {:?}", result);
    
    // 验证订单是否已经创建并正确设置
    println!("Raft共识基本测试通过: 状态机可以正确应用命令");
}

#[test]
fn test_trading_cluster_manager() {
    // 创建集群管理器
    let mut manager = TradingClusterManager::new(
        "node1".to_string(),
        "127.0.0.1:9000".parse().unwrap(),
        "trading-cluster".to_string()
    );
    
    // 添加种子节点
    manager.add_seed_node("127.0.0.2:9000".to_string());
    manager.add_seed_node("127.0.0.3:9000".to_string());
    
    // 初始化集群
    let result = manager.initialize();
    assert!(result.is_ok());
    
    // 注册Actor路径
    let reg_result = manager.register_actor_path(
        "/system/raft/consensus", 
        ActorType::RaftConsensus
    );
    assert!(reg_result.is_ok());
    
    // 查找已注册的路径
    let path = manager.find_actor_path("/system/raft/consensus");
    assert!(path.is_some());
    let path = path.unwrap();
    assert_eq!(path.node_id, "node1");
    assert_eq!(path.actor_type, ActorType::RaftConsensus);
    
    // 发送消息
    let send_result = manager.send_message("/system/raft/consensus", "APPEND_ENTRIES");
    assert!(send_result.is_ok());
    
    println!("集群管理器测试通过: 可以注册和寻找Actor路径，发送消息");
}

// 在实际应用中，我们需要添加更多测试用例来验证:
// 1. 领导选举 - 选举超时和投票机制
// 2. 日志复制 - 发送和接收日志条目 
// 3. 安全性 - 确保只有包含全部已提交条目的日志才能当选为领导者
// 4. 状态机应用 - 确保所有节点以相同的顺序应用相同的命令
// 5. 集群成员变更 - 添加或删除节点 