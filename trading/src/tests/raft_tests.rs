use std::sync::Arc;
use actix::prelude::*;
use uuid::Uuid;

use crate::cluster::{TradingClusterManager, ActorType};
use crate::consensus::raft::{
    RaftNodeActor, RaftConfig, RaftRequestMessage, 
    RaftResponseMessage, ClientCommand, LogEntry
};
use crate::consensus::state_machine::{
    StateMachineCommand, StateMachineResponse, TradingStateMachine
};
use crate::models::order::{OrderRequest, OrderStatus, OrderType, OrderSide};

#[test]
fn test_raft_config_default() {
    let config = RaftConfig::default();
    
    assert_eq!(config.election_timeout_min, 150);
    assert_eq!(config.election_timeout_max, 300);
    assert_eq!(config.heartbeat_interval, 50);
    assert_eq!(config.replication_parallelism, 5);
}

#[test]
fn test_trading_state_machine() {
    let mut state_machine = TradingStateMachine::new();
    
    // 测试创建订单
    let order_id = Uuid::new_v4().to_string();
    let order_request = OrderRequest {
        order_id: Some(order_id.clone()),
        client_id: "client1".to_string(),
        symbol: "AAPL".to_string(),
        order_type: OrderType::Limit,
        side: OrderSide::Buy,
        quantity: 100,
        price: Some(150.0),
        account_id: "account1".to_string(),
    };
    
    let cmd = StateMachineCommand::CreateOrder(order_request);
    let response = state_machine.apply(cmd);
    
    match response {
        StateMachineResponse::Success { result } => {
            assert_eq!(result, Some(order_id.clone()));
        },
        _ => panic!("创建订单应该成功"),
    }
    
    // 验证订单已创建
    let order = state_machine.get_order(&order_id);
    assert!(order.is_some());
    let order = order.unwrap();
    assert_eq!(order.order_id, order_id);
    assert_eq!(order.symbol, "AAPL");
    assert_eq!(order.status, OrderStatus::Pending);
    
    // 测试更新订单状态
    let cmd = StateMachineCommand::UpdateOrderStatus { 
        order_id: order_id.clone(), 
        new_status: OrderStatus::Filled 
    };
    let response = state_machine.apply(cmd);
    
    match response {
        StateMachineResponse::Success { .. } => {
            // 验证状态已更新
            let updated_order = state_machine.get_order(&order_id).unwrap();
            assert_eq!(updated_order.status, OrderStatus::Filled);
        },
        _ => panic!("更新订单状态应该成功"),
    }
    
    // 测试更新不存在的订单
    let cmd = StateMachineCommand::UpdateOrderStatus { 
        order_id: "non-existent".to_string(), 
        new_status: OrderStatus::Cancelled 
    };
    let response = state_machine.apply(cmd);
    
    match response {
        StateMachineResponse::Failure { .. } => {
            // 预期失败
        },
        _ => panic!("更新不存在的订单应该失败"),
    }
}

#[actix_rt::test]
async fn test_raft_node_creation() {
    let node_id = "node1".to_string();
    let config = RaftConfig::default();
    
    let raft_node = RaftNodeActor::new(node_id.clone(), config.clone());
    let addr = raft_node.start();
    
    // 验证Actor已启动
    assert!(addr.connected());
    
    // 清理
    addr.stop();
    System::current().stop();
}

#[actix_rt::test]
async fn test_raft_request_vote() {
    let system = System::new();
    
    system.block_on(async {
        // 创建两个Raft节点
        let node1_id = "node1".to_string();
        let node2_id = "node2".to_string();
        let config = RaftConfig::default();
        
        let raft_node1 = RaftNodeActor::new(node1_id.clone(), config.clone());
        let raft_node2 = RaftNodeActor::new(node2_id.clone(), config.clone());
        
        let addr1 = raft_node1.start();
        let addr2 = raft_node2.start();
        
        // 节点2请求投票
        let request_vote = RaftRequestMessage::RequestVote {
            term: 1,
            candidate_id: node2_id.clone(),
            last_log_index: 0,
            last_log_term: 0,
        };
        
        let response = addr1.send(request_vote).await.unwrap();
        
        match response {
            RaftResponseMessage::RequestVoteResponse { term, vote_granted } => {
                assert_eq!(term, 1);
                assert!(vote_granted);
            },
            _ => panic!("预期RequestVoteResponse响应"),
        }
        
        // 清理
        addr1.stop();
        addr2.stop();
        System::current().stop();
    })
}

#[actix_rt::test]
async fn test_raft_append_entries() {
    let system = System::new();
    
    system.block_on(async {
        // 创建两个Raft节点
        let node1_id = "node1".to_string();
        let node2_id = "node2".to_string();
        let config = RaftConfig::default();
        
        let raft_node1 = RaftNodeActor::new(node1_id.clone(), config.clone());
        let raft_node2 = RaftNodeActor::new(node2_id.clone(), config.clone());
        
        let addr1 = raft_node1.start();
        let addr2 = raft_node2.start();
        
        // 节点1模拟成为领导者并发送心跳
        let append_entries = RaftRequestMessage::AppendEntries {
            term: 1,
            leader_id: node1_id.clone(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: Vec::new(),
            leader_commit: 0,
        };
        
        let response = addr2.send(append_entries).await.unwrap();
        
        match response {
            RaftResponseMessage::AppendEntriesResponse { term, success, .. } => {
                assert_eq!(term, 1);
                assert!(success);
            },
            _ => panic!("预期AppendEntriesResponse响应"),
        }
        
        // 清理
        addr1.stop();
        addr2.stop();
        System::current().stop();
    })
}

#[actix_rt::test]
async fn test_raft_with_cluster_manager() {
    let system = System::new();
    
    system.block_on(async {
        // 创建集群管理器
        let manager = Arc::new(TradingClusterManager::new(
            "cluster1".to_string(),
            "127.0.0.1:8080".parse().unwrap(),
            "trading-cluster".to_string()
        ));
        
        manager.initialize().expect("集群初始化失败");
        
        // 创建三个Raft节点，模拟分布式环境
        let node1_id = "node1".to_string();
        let node2_id = "node2".to_string();
        let node3_id = "node3".to_string();
        let config = RaftConfig::default();
        
        let raft_node1 = RaftNodeActor::new(node1_id.clone(), config.clone())
            .with_cluster_manager(manager.clone());
        
        let raft_node2 = RaftNodeActor::new(node2_id.clone(), config.clone())
            .with_cluster_manager(manager.clone());
        
        let raft_node3 = RaftNodeActor::new(node3_id.clone(), config.clone())
            .with_cluster_manager(manager.clone());
        
        let addr1 = raft_node1.start();
        let addr2 = raft_node2.start();
        let addr3 = raft_node3.start();
        
        // 设置集群节点
        let mut nodes = vec![node1_id.clone(), node2_id.clone(), node3_id.clone()];
        
        addr1.do_send(SetClusterNodesMessage { nodes: nodes.clone() });
        addr2.do_send(SetClusterNodesMessage { nodes: nodes.clone() });
        addr3.do_send(SetClusterNodesMessage { nodes });
        
        // 让节点1成为领导者
        let become_leader = BecomeLeaderMessage {};
        addr1.do_send(become_leader);
        
        // 等待一段时间让领导者发送心跳
        actix_rt::time::sleep(std::time::Duration::from_millis(100)).await;
        
        // 清理
        addr1.stop();
        addr2.stop();
        addr3.stop();
        System::current().stop();
    })
}

// 辅助消息定义，用于测试

#[derive(Message)]
#[rtype(result = "()")]
struct SetClusterNodesMessage {
    nodes: Vec<String>,
}

impl Handler<SetClusterNodesMessage> for RaftNodeActor {
    type Result = ();
    
    fn handle(&mut self, msg: SetClusterNodesMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.set_cluster_nodes(msg.nodes);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct BecomeLeaderMessage {}

impl Handler<BecomeLeaderMessage> for RaftNodeActor {
    type Result = ();
    
    fn handle(&mut self, _msg: BecomeLeaderMessage, ctx: &mut Self::Context) -> Self::Result {
        // 强制设置为候选人，然后调用become_leader
        self.current_term = 1;
        self.state = crate::consensus::raft::RaftNodeState::Candidate;
        self.become_leader(ctx);
    }
} 