use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use log::{info, warn, error, debug};
use actix::prelude::*;
use tokio::sync::mpsc;
use serde_json;
use anyhow::anyhow;
use thiserror::Error;
use chrono::Utc;

use crate::models::message::{LogEntry};

/// Raft错误
#[derive(Debug, Error)]
pub enum RaftError {
    #[error("Not the leader")]
    NotLeader,
    
    #[error("Cluster error: {0}")]
    ClusterError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Timeout error")]
    Timeout,
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Raft节点角色
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
}

/// Raft服务
pub struct RaftService {
    node_id: String,
    addr: Option<Addr<Self>>,
    
    // Raft状态
    current_term: Arc<Mutex<u64>>,
    voted_for: Arc<Mutex<Option<String>>>,
    log: Arc<Mutex<Vec<LogEntry>>>,
    commit_index: Arc<Mutex<u64>>,
    last_applied: Arc<Mutex<u64>>,
    
    // 服务状态
    state: Arc<Mutex<RaftState>>,
    
    // 状态应用通道
    apply_ch: mpsc::Sender<LogEntry>,
}

/// Raft内部状态
#[derive(Debug, Clone)]
struct RaftState {
    current_role: NodeState,
    current_leader: Option<String>,
    last_heartbeat: chrono::DateTime<Utc>,
    peers: HashMap<String, String>,
}

impl Actor for RaftService {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("RaftService started on node {}", self.node_id);
        
        // 保存Actor地址
        self.addr = Some(ctx.address());
        
        // 启动选举计时器
        self.start_election_timer();
        
        // 启动心跳定时器
        self.start_heartbeat_timer();
    }
}

impl RaftService {
    /// 创建新的Raft服务
    pub fn new(
        node_id: String,
        apply_tx: mpsc::Sender<LogEntry>,
    ) -> Self {
        let raft_state = RaftState {
            current_role: NodeState::Follower,
            current_leader: None,
            last_heartbeat: Utc::now(),
            peers: HashMap::new(),
        };
        
        Self {
            node_id,
            addr: None,
            current_term: Arc::new(Mutex::new(0)),
            voted_for: Arc::new(Mutex::new(None)),
            log: Arc::new(Mutex::new(Vec::new())),
            commit_index: Arc::new(Mutex::new(0)),
            last_applied: Arc::new(Mutex::new(0)),
            state: Arc::new(Mutex::new(raft_state)),
            apply_ch: apply_tx,
        }
    }
    
    /// 启动选举计时器
    fn start_election_timer(&self) {
        // 获取状态和克隆所需的对象
        let state = Arc::clone(&self.state);
        let addr_option = self.addr.clone();
        let current_term = Arc::clone(&self.current_term);
        let node_id = self.node_id.clone();
        
        // 启动选举计时器
        actix::spawn(async move {
            loop {
                // 检查地址是否存在
                let addr = match addr_option {
                    Some(ref addr) => addr.clone(),
                    None => {
                        warn!("RaftService address not set, election timer exiting");
                        return;
                    }
                };
                
                // 生成随机超时时间 (150-300ms)
                let timeout = tokio::time::Duration::from_millis(
                    150 + (rand::random::<u64>() % 150)
                );
                
                tokio::time::sleep(timeout).await;
                
                // 检查上次心跳时间
                let should_start_election = {
                    let mut s = state.lock().unwrap();
                    let role = s.current_role;
                    let elapsed = Utc::now()
                        .signed_duration_since(s.last_heartbeat)
                        .num_milliseconds();
                    
                    // 如果是Follower并且超时，则启动选举
                    if role == NodeState::Follower && elapsed > 300 {
                        s.current_role = NodeState::Candidate;
                        true
                    } else {
                        false
                    }
                };
                
                // 如果需要启动选举
                if should_start_election {
                    debug!("Starting election on node {}", node_id);
                    
                    // 增加当前任期
                    {
                        let mut term = current_term.lock().unwrap();
                        *term += 1;
                    }
                    
                    // 发送开始选举消息给自己的Actor
                    addr.do_send(StartElection);
                }
            }
        });
    }
    
    /// 开始心跳定时器
    fn start_heartbeat_timer(&self) {
        // 获取状态和克隆所需的对象
        let state = Arc::clone(&self.state);
        let addr_option = self.addr.clone();
        
        // 启动心跳定时器
        actix::spawn(async move {
            loop {
                // 检查地址是否存在
                let addr = match addr_option {
                    Some(ref addr) => addr.clone(),
                    None => {
                        warn!("RaftService address not set, heartbeat timer exiting");
                        return;
                    }
                };
                
                // 每100ms发送一次心跳
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                
                // 检查是否是Leader
                let is_leader = {
                    let s = state.lock().unwrap();
                    s.current_role == NodeState::Leader
                };
                
                // 如果是Leader，发送心跳
                if is_leader {
                    addr.do_send(SendHeartbeat);
                }
            }
        });
    }
    
    /// 添加日志条目
    pub async fn append_log(&self, entry: LogEntry) -> Result<(), RaftError> {
        // 检查是否是Leader
        let is_leader = {
            let state = self.state.lock().unwrap();
            state.current_role == NodeState::Leader
        };
        
        if !is_leader {
            // 如果不是Leader，提供错误
            return Err(RaftError::NotLeader);
        }
        
        // 添加到本地日志
        {
            let mut log = self.log.lock().unwrap();
            log.push(entry.clone());
        }
        
        // TODO: 向其他节点复制日志，这里简化处理
        // 将日志同步到状态机
        if let Err(e) = self.apply_ch.send(entry).await {
            error!("Failed to send log entry to state machine: {}", e);
            return Err(RaftError::Other(format!("Failed to apply: {}", e)));
        }
        
        Ok(())
    }
    
    /// 获取当前状态
    pub fn get_state(&self) -> NodeState {
        let state = self.state.lock().unwrap();
        state.current_role
    }
    
    /// 获取当前任期
    pub fn get_term(&self) -> u64 {
        let term = self.current_term.lock().unwrap();
        *term
    }
    
    /// 获取当前Leader
    pub fn get_leader(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        state.current_leader.clone()
    }
    
    /// 添加对等节点
    pub fn add_peer(&mut self, node_id: String, addr: String) {
        let mut state = self.state.lock().unwrap();
        state.peers.insert(node_id, addr);
    }
}

/// 开始选举消息
#[derive(Message)]
#[rtype(result = "()")]
struct StartElection;

impl Handler<StartElection> for RaftService {
    type Result = ();
    
    fn handle(&mut self, _: StartElection, _: &mut Self::Context) -> Self::Result {
        // 实现开始选举逻辑
        let term = {
            let term = self.current_term.lock().unwrap();
            *term
        };
        
        debug!("Node {} starting election for term {}", self.node_id, term);
        
        // 为自己投票
        {
            let mut voted_for = self.voted_for.lock().unwrap();
            *voted_for = Some(self.node_id.clone());
        }
        
        // TODO: 发送RequestVote RPC到所有对等节点
    }
}

/// 发送心跳消息
#[derive(Message)]
#[rtype(result = "()")]
struct SendHeartbeat;

impl Handler<SendHeartbeat> for RaftService {
    type Result = ();
    
    fn handle(&mut self, _: SendHeartbeat, _: &mut Self::Context) -> Self::Result {
        // 实现发送心跳逻辑
        let term = {
            let term = self.current_term.lock().unwrap();
            *term
        };
        
        // TODO: 发送AppendEntries RPC到所有对等节点作为心跳
        debug!("Sending heartbeat from node {} for term {}", self.node_id, term);
    }
}

/// Raft AppendEntries 请求
#[derive(Debug, Message)]
#[rtype(result = "AppendEntriesResponse")]
pub struct AppendEntriesRequest {
    pub term: u64,
    pub leader_id: String,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

/// Raft AppendEntries 响应
#[derive(Debug)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
}

impl Handler<AppendEntriesRequest> for RaftService {
    type Result = AppendEntriesResponse;
    
    fn handle(&mut self, msg: AppendEntriesRequest, _: &mut Self::Context) -> Self::Result {
        // 获取当前任期
        let current_term = {
            let term = self.current_term.lock().unwrap();
            *term
        };
        
        // 如果请求任期小于当前任期，拒绝
        if msg.term < current_term {
            return AppendEntriesResponse {
                term: current_term,
                success: false,
            };
        }
        
        // 如果是心跳消息，更新状态
        {
            let mut state = self.state.lock().unwrap();
            
            // 更新心跳时间
            state.last_heartbeat = Utc::now();
            
            // 如果请求任期大于当前任期，转为Follower
            if msg.term > current_term {
                state.current_role = NodeState::Follower;
                let mut term = self.current_term.lock().unwrap();
                *term = msg.term;
            }
            
            // 记录当前Leader
            state.current_leader = Some(msg.leader_id.clone());
        }
        
        // TODO: 完成AppendEntries的日志复制逻辑
        
        // 返回成功
        AppendEntriesResponse {
            term: current_term,
            success: true,
        }
    }
}

/// Raft RequestVote请求
#[derive(Debug, Message)]
#[rtype(result = "RequestVoteResponse")]
pub struct RequestVoteRequest {
    pub term: u64,
    pub candidate_id: String,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

/// Raft RequestVote响应
#[derive(Debug)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

impl Handler<RequestVoteRequest> for RaftService {
    type Result = RequestVoteResponse;
    
    fn handle(&mut self, msg: RequestVoteRequest, _: &mut Self::Context) -> Self::Result {
        // 获取当前任期和投票信息
        let (current_term, voted_for) = {
            let term = self.current_term.lock().unwrap();
            let voted_for = self.voted_for.lock().unwrap();
            (*term, voted_for.clone())
        };
        
        // 如果请求任期小于当前任期，拒绝投票
        if msg.term < current_term {
            return RequestVoteResponse {
                term: current_term,
                vote_granted: false,
            };
        }
        
        // 如果请求任期大于当前任期，转为Follower并重置投票
        if msg.term > current_term {
            {
                let mut state = self.state.lock().unwrap();
                state.current_role = NodeState::Follower;
                
                let mut term = self.current_term.lock().unwrap();
                *term = msg.term;
                
                let mut vote = self.voted_for.lock().unwrap();
                *vote = None;
            }
        }
        
        // 判断是否可以投票
        let can_vote = voted_for.is_none() || voted_for == Some(msg.candidate_id.clone());
        
        // TODO: 检查日志的完整性
        
        if can_vote {
            // 授予投票
            {
                let mut vote = self.voted_for.lock().unwrap();
                *vote = Some(msg.candidate_id.clone());
                
                let mut state = self.state.lock().unwrap();
                state.last_heartbeat = Utc::now();
            }
            
            RequestVoteResponse {
                term: msg.term,
                vote_granted: true,
            }
        } else {
            RequestVoteResponse {
                term: msg.term,
                vote_granted: false,
            }
        }
    }
}

/// 处理AppendLogRequest消息
#[derive(Debug, Message)]
#[rtype(result = "Result<(), RaftError>")]
pub struct AppendLogRequest {
    pub entry: LogEntry,
}

impl Handler<AppendLogRequest> for RaftService {
    type Result = ResponseFuture<Result<(), RaftError>>;
    
    fn handle(&mut self, msg: AppendLogRequest, _: &mut Self::Context) -> Self::Result {
        // 克隆需要的成员
        let entry = msg.entry;
        let is_leader = {
            let state = self.state.lock().unwrap();
            state.current_role == NodeState::Leader
        };
        let leader_id = {
            let state = self.state.lock().unwrap();
            state.current_leader.clone()
        };
        let log = self.log.clone();
        let apply_ch = self.apply_ch.clone();
        
        Box::pin(async move {
            if is_leader {
                // 如果是Leader，直接添加到本地日志
                {
                    let mut log_guard = log.lock().unwrap();
                    log_guard.push(entry.clone());
                }
                
                // 发送到应用通道
                if let Err(e) = apply_ch.send(entry).await {
                    error!("Failed to send log entry to state machine: {}", e);
                    return Err(RaftError::Other(format!("Failed to apply: {}", e)));
                }
                
                Ok(())
            } else if leader_id.is_some() {
                // 不是Leader但知道Leader
                Err(RaftError::NotLeader)
            } else {
                // 不知道Leader
                Err(RaftError::NotLeader)
            }
        })
    }
} 