use std::collections::HashMap;
use std::sync::Arc;

use actix::prelude::*;
use actix_cluster::{
    cluster::{ClusterSystem, Architecture},
    config::{ClusterConfig, DiscoveryMethod, NodeRole},
    consensus::{ConsensusActor, ConsensusCommand, ConsensusState, GetConsensusState},
    node::{NodeId, NodeInfo},
    serialization::SerializationFormat,
    transport::P2PTransport,
};
use tokio::sync::Mutex;

#[actix::test]
async fn test_consensus_basic() {
    // 创建节点配置
    let node1_id = NodeId::new();
    
    println!("Created node ID: {}", node1_id);
    
    // 创建测试节点
    let node1_info = NodeInfo::new(
        node1_id.clone(),
        "test-node-1".to_string(),
        NodeRole::Peer,
        "127.0.0.1:10001".parse().unwrap(),
    );
    
    // 创建P2PTransport
    let transport = P2PTransport::new(
        node1_info.clone(),
        SerializationFormat::Bincode
    ).expect("Failed to create P2PTransport");
    
    // 创建共识Actor
    let consensus = ConsensusActor::new(node1_id.clone(), 
        Arc::new(Mutex::new(transport))).start();
    
    // 初始化共识状态
    let init_msg = ConsensusCommand::AddNode(node1_info.clone());
    
    // 通过消息处理来添加节点到共识状态
    let result = consensus.send(init_msg).await.expect("Failed to send message");
    assert!(result.is_ok(), "Failed to initialize consensus state");
    
    // 验证共识状态是否正确
    let state = consensus.send(GetConsensusState).await.expect("Failed to get state");
    
    match state {
        Ok(state) => {
            assert!(state.nodes.contains_key(&node1_id), "Node 1 should be in consensus state");
            assert_eq!(state.nodes.len(), 1, "Should have exactly one node");
            println!("Consensus state contains the expected node");
        },
        Err(e) => {
            panic!("Failed to get consensus state: {:?}", e);
        }
    }
    
    // 检查是否可以正确添加第二个节点
    let node2_id = NodeId::new();
    let node2_info = NodeInfo::new(
        node2_id.clone(),
        "test-node-2".to_string(),
        NodeRole::Peer,
        "127.0.0.1:10002".parse().unwrap(),
    );
    
    let add_node2 = ConsensusCommand::AddNode(node2_info.clone());
    let result = consensus.send(add_node2).await.expect("Failed to send message");
    assert!(result.is_ok(), "Failed to add node 2");
    
    // 验证共识状态包含两个节点
    let state = consensus.send(GetConsensusState).await.expect("Failed to get state");
    match state {
        Ok(state) => {
            assert!(state.nodes.contains_key(&node1_id), "Node 1 should be in consensus state");
            assert!(state.nodes.contains_key(&node2_id), "Node 2 should be in consensus state");
            assert_eq!(state.nodes.len(), 2, "Should have exactly two nodes");
            println!("Consensus state contains both expected nodes");
        },
        Err(e) => {
            panic!("Failed to get consensus state: {:?}", e);
        }
    }
    
    // 注册一个Actor
    let register_actor = ConsensusCommand::RegisterActor {
        actor_path: "test-actor".to_string(),
        node_id: node1_id.clone(),
    };
    let result = consensus.send(register_actor).await.expect("Failed to send message");
    assert!(result.is_ok(), "Failed to register actor");
    
    // 验证Actor注册
    let state = consensus.send(GetConsensusState).await.expect("Failed to get state");
    match state {
        Ok(state) => {
            assert!(state.actor_registrations.contains_key("test-actor"), "Actor should be registered");
            assert_eq!(state.actor_registrations["test-actor"], node1_id, "Actor should be registered on node 1");
            println!("Actor registration is correct");
        },
        Err(e) => {
            panic!("Failed to get consensus state: {:?}", e);
        }
    }
    
    // 关闭Actor系统
    System::current().stop();
    
    println!("Consensus test completed successfully");
} 