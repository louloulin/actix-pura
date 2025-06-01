//! 处理Raft共识消息的处理器
//!
//! 这个模块负责处理所有与Raft共识相关的网络消息。

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use openraft;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::consensus::{
    ConsensusActor, 
    ConsensusCommand, 
    ConsensusResponse,
    ConsensusState,
    RaftTypeConfig
};
use crate::consensus_network::{RaftNetworkImpl, RaftNetworkMessage};
use crate::node::{NodeId, NodeInfo};
use crate::transport::{MessageHandler, P2PTransport, TransportMessage};
use crate::error::{ClusterError, ClusterResult};

/// 共识消息处理器，用于处理传入的Raft共识消息
pub struct ConsensusMessageHandler {
    /// 当前节点ID
    node_id: NodeId,
    
    /// Raft共识Actor的引用
    consensus_actor: Arc<RwLock<ConsensusActor>>,
    
    /// Raft网络实现
    network: Arc<RaftNetworkImpl>,
    
    /// 节点映射表
    node_mapping: Arc<RwLock<HashMap<u64, NodeId>>>,
}

impl ConsensusMessageHandler {
    /// 创建一个新的共识消息处理器
    pub fn new(
        node_id: NodeId,
        consensus_actor: Arc<RwLock<ConsensusActor>>,
        network: Arc<RaftNetworkImpl>,
    ) -> Self {
        Self {
            node_id,
            consensus_actor,
            network,
            node_mapping: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册一个节点，建立NodeId和Raft节点ID之间的映射
    pub async fn register_node(&self, node_id: NodeId, raft_node_id: u64) -> Result<()> {
        let mut mapping = self.node_mapping.write().await;
        mapping.insert(raft_node_id, node_id.clone());
        
        // 同时更新网络实现中的映射
        self.network.register_node(raft_node_id, node_id).await?;
        
        Ok(())
    }
    
    /// 处理Raft网络消息
    async fn handle_raft_message(&self, message: RaftNetworkMessage, sender: NodeId) -> ClusterResult<()> {
        debug!("Handling Raft message: {:?} from {}", message, sender);
        
        // 根据消息类型进行处理
        match message {
            RaftNetworkMessage::AppendEntries(rpc) => {
                debug!("Received AppendEntries from {}", sender);
                
                // 获取共识Actor来处理请求
                let consensus_actor = self.consensus_actor.read().await;
                if let Some(raft) = &consensus_actor.raft {
                    // 找出目标节点ID，通常是当前节点
                    let response = match raft.append_entries(rpc).await {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("Error processing AppendEntries: {}", e);
                            return Err(ClusterError::ConsensusError(format!("Failed to process AppendEntries: {}", e)));
                        }
                    };
                    
                    // 创建响应消息
                    let response_message = RaftNetworkMessage::AppendEntriesResponse(response);
                    let transport_message = TransportMessage::Consensus(
                        bincode::serialize(&response_message).map_err(|e| {
                            ClusterError::SerializationError(format!("Failed to serialize AppendEntriesResponse: {}", e))
                        })?
                    );
                    
                    // 发送响应
                    let mut transport = consensus_actor.transport.lock().await;
                    transport.send_message(&sender, transport_message).await?;
                } else {
                    error!("Raft instance not initialized");
                    return Err(ClusterError::ConsensusError("Raft instance not initialized".to_string()));
                }
            },
            
            RaftNetworkMessage::Vote(rpc) => {
                debug!("Received Vote from {}", sender);
                
                // 获取共识Actor来处理请求
                let consensus_actor = self.consensus_actor.read().await;
                if let Some(raft) = &consensus_actor.raft {
                    // 处理投票请求
                    let response = match raft.vote(rpc).await {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("Error processing Vote: {}", e);
                            return Err(ClusterError::ConsensusError(format!("Failed to process Vote: {}", e)));
                        }
                    };
                    
                    // 创建响应消息
                    let response_message = RaftNetworkMessage::VoteResponse(response);
                    let transport_message = TransportMessage::Consensus(
                        bincode::serialize(&response_message).map_err(|e| {
                            ClusterError::SerializationError(format!("Failed to serialize VoteResponse: {}", e))
                        })?
                    );
                    
                    // 发送响应
                    let mut transport = consensus_actor.transport.lock().await;
                    transport.send_message(&sender, transport_message).await?;
                } else {
                    error!("Raft instance not initialized");
                    return Err(ClusterError::ConsensusError("Raft instance not initialized".to_string()));
                }
            },
            
            RaftNetworkMessage::InstallSnapshot(rpc) => {
                debug!("Received InstallSnapshot from {}", sender);
                
                // 获取共识Actor来处理请求
                let consensus_actor = self.consensus_actor.read().await;
                if let Some(raft) = &consensus_actor.raft {
                    // 处理快照安装请求
                    let response = match raft.install_snapshot(rpc).await {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("Error processing InstallSnapshot: {}", e);
                            return Err(ClusterError::ConsensusError(format!("Failed to process InstallSnapshot: {}", e)));
                        }
                    };
                    
                    // 创建响应消息
                    let response_message = RaftNetworkMessage::InstallSnapshotResponse(response);
                    let transport_message = TransportMessage::Consensus(
                        bincode::serialize(&response_message).map_err(|e| {
                            ClusterError::SerializationError(format!("Failed to serialize InstallSnapshotResponse: {}", e))
                        })?
                    );
                    
                    // 发送响应
                    let mut transport = consensus_actor.transport.lock().await;
                    transport.send_message(&sender, transport_message).await?;
                } else {
                    error!("Raft instance not initialized");
                    return Err(ClusterError::ConsensusError("Raft instance not initialized".to_string()));
                }
            },
            
            // 响应消息通常是由上层调用者处理的，这里不需要单独处理
            RaftNetworkMessage::AppendEntriesResponse(_) |
            RaftNetworkMessage::VoteResponse(_) |
            RaftNetworkMessage::InstallSnapshotResponse(_) => {
                debug!("Received response message, but it should be handled by the caller");
            },
        }
        
        Ok(())
    }
    
    /// 提交命令到Raft集群
    pub async fn submit_command(&self, command: ConsensusCommand) -> Result<ConsensusResponse> {
        let consensus_actor = self.consensus_actor.read().await;
        consensus_actor.submit_command(command).await
    }
}

// 实现MessageHandler trait，使其能处理TransportMessage
#[async_trait]
impl MessageHandler for ConsensusMessageHandler {
    async fn handle_message(&self, sender: NodeId, message: TransportMessage) -> ClusterResult<()> {
        match message {
            TransportMessage::Consensus(data) => {
                // 尝试反序列化为RaftNetworkMessage
                match bincode::deserialize::<RaftNetworkMessage>(&data) {
                    Ok(raft_message) => {
                        // 处理Raft消息
                        self.handle_raft_message(raft_message, sender).await?;
                    },
                    Err(e) => {
                        error!("Failed to deserialize Raft message: {}", e);
                        return Err(ClusterError::SerializationError(format!("Failed to deserialize Raft message: {}", e)));
                    }
                }
            },
            _ => {
                // 忽略非共识消息
                debug!("Ignoring non-consensus message: {:?}", message);
            }
        }
        
        Ok(())
    }
} 