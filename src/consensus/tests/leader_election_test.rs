use std::sync::{Arc, Mutex};
use std::time::Duration;
use actix::prelude::*;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::consensus::raft::{RaftService, NodeState, RequestVoteRequest, RequestVoteResponse};
use crate::models::message::LogEntry;

#[actix_rt::test]
async fn test_raft_leader_election_basic() {
    // 初始化日志
    std::env::set_var("RUST_LOG", "debug");
    let _ = env_logger::try_init();
    
    // 创建消息通道和Raft服务
    let (tx, _) = mpsc::channel(100);
    let raft = RaftService::new("node-1".to_string(), tx);
    
    // 启动Actor
    let raft_addr = raft.start();
    
    // 验证初始状态为Follower
    let state = raft_addr.send(GetState).await.unwrap();
    assert_eq!(state, NodeState::Follower);
    
    // 手动触发选举
    raft_addr.send(ForceElection).await.unwrap();
    
    // 等待一段时间让选举完成
    sleep(Duration::from_millis(500)).await;
    
    // 验证节点变为Candidate或Leader (单节点系统应该成为Leader)
    let state = raft_addr.send(GetState).await.unwrap();
    assert!(matches!(state, NodeState::Candidate | NodeState::Leader));
    
    // 如果是多节点系统，额外验证是否成为Leader
    if state == NodeState::Leader {
        let term = raft_addr.send(GetTerm).await.unwrap();
        assert!(term > 0, "Term should be incremented during election");
    }
    
    // 停止Actor
    raft_addr.send(actix::msgs::TerminateActor).await.unwrap();
}

#[actix_rt::test]
async fn test_vote_request_handling() {
    // 初始化日志
    std::env::set_var("RUST_LOG", "debug");
    let _ = env_logger::try_init();
    
    // 创建消息通道和Raft服务
    let (tx, _) = mpsc::channel(100);
    let raft = RaftService::new("node-1".to_string(), tx);
    
    // 启动Actor
    let raft_addr = raft.start();
    
    // 当前任期为0，发送任期为1的RequestVote请求
    let request = RequestVoteRequest {
        term: 1,
        candidate_id: "node-2".to_string(),
        last_log_index: 0,
        last_log_term: 0,
    };
    
    let response = raft_addr.send(request).await.unwrap();
    
    // 验证得到投票
    assert_eq!(response.term, 1);
    assert!(response.vote_granted);
    
    // 验证节点状态变为Follower并更新任期
    let state = raft_addr.send(GetState).await.unwrap();
    assert_eq!(state, NodeState::Follower);
    
    let term = raft_addr.send(GetTerm).await.unwrap();
    assert_eq!(term, 1);
    
    // 再次发送相同任期的RequestVote请求，但来自不同候选人
    let request = RequestVoteRequest {
        term: 1,
        candidate_id: "node-3".to_string(),
        last_log_index: 0,
        last_log_term: 0,
    };
    
    let response = raft_addr.send(request).await.unwrap();
    
    // 验证拒绝投票（因为已经投给了node-2）
    assert_eq!(response.term, 1);
    assert!(!response.vote_granted);
    
    // 停止Actor
    raft_addr.send(actix::msgs::TerminateActor).await.unwrap();
}

#[actix_rt::test]
async fn test_election_timeout() {
    // 初始化日志
    std::env::set_var("RUST_LOG", "debug");
    let _ = env_logger::try_init();
    
    // 创建消息通道和Raft测试服务，并缩短选举超时时间
    let (tx, _) = mpsc::channel(100);
    let raft = RaftService::new_with_config(
        "node-1".to_string(), 
        tx,
        50,  // 50ms选举超时
        25   // 25ms心跳间隔
    );
    
    // 启动Actor
    let raft_addr = raft.start();
    
    // 初始状态应该是Follower
    let state = raft_addr.send(GetState).await.unwrap();
    assert_eq!(state, NodeState::Follower);
    
    // 等待选举超时触发
    sleep(Duration::from_millis(200)).await;
    
    // 现在应该已经发起选举并成为Candidate或Leader
    let state = raft_addr.send(GetState).await.unwrap();
    assert!(matches!(state, NodeState::Candidate | NodeState::Leader));
    
    // 停止Actor
    raft_addr.send(actix::msgs::TerminateActor).await.unwrap();
}

// 测试辅助消息定义
#[derive(Message)]
#[rtype(result = "NodeState")]
struct GetState;

impl Handler<GetState> for RaftService {
    type Result = NodeState;
    
    fn handle(&mut self, _: GetState, _: &mut Self::Context) -> Self::Result {
        self.get_state()
    }
}

#[derive(Message)]
#[rtype(result = "u64")]
struct GetTerm;

impl Handler<GetTerm> for RaftService {
    type Result = u64;
    
    fn handle(&mut self, _: GetTerm, _: &mut Self::Context) -> Self::Result {
        self.get_term()
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct ForceElection;

impl Handler<ForceElection> for RaftService {
    type Result = ();
    
    fn handle(&mut self, _: ForceElection, ctx: &mut Self::Context) -> Self::Result {
        // 手动触发选举
        let term = {
            let mut term = self.current_term.lock().unwrap();
            *term += 1;
            *term
        };
        
        {
            let mut state = self.state.lock().unwrap();
            state.current_role = NodeState::Candidate;
        }
        
        // 通知自己开始选举
        ctx.notify(super::raft::StartElection);
    }
} 