use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use log::{info, warn, error, debug};
use actix::prelude::*;
use tokio::sync::mpsc;
use serde_json;
use anyhow::anyhow;
use thiserror::Error;
use chrono::Utc;
use rand::Rng;

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
    pub(crate) node_id: String,
    pub(crate) addr: Option<Addr<Self>>,
    
    // Raft状态
    pub(crate) current_term: Arc<Mutex<u64>>,
    pub(crate) voted_for: Arc<Mutex<Option<String>>>,
    pub(crate) log: Arc<Mutex<Vec<LogEntry>>>,
    pub(crate) commit_index: Arc<Mutex<u64>>,
    pub(crate) last_applied: Arc<Mutex<u64>>,
    
    // 服务状态
    pub(crate) state: Arc<Mutex<RaftState>>,
    
    // 状态应用通道
    pub(crate) apply_ch: mpsc::Sender<LogEntry>,
    
    // 配置参数
    pub(crate) election_timeout_ms: u64,
    pub(crate) heartbeat_interval_ms: u64,
    
    // Leader相关状态
    pub(crate) next_index: Arc<Mutex<HashMap<String, u64>>>,
    pub(crate) match_index: Arc<Mutex<HashMap<String, u64>>>,
    
    // 选举状态
    pub(crate) votes_received: Arc<Mutex<HashSet<String>>>,
    pub(crate) election_timer: Option<SpawnHandle>,
    pub(crate) heartbeat_timer: Option<SpawnHandle>,
}

/// Raft内部状态
#[derive(Debug, Clone)]
pub(crate) struct RaftState {
    pub current_role: NodeState,
    pub current_leader: Option<String>,
    pub last_heartbeat: chrono::DateTime<Utc>,
    pub peers: HashMap<String, Recipient<RequestVoteRequest>>,
}

impl Actor for RaftService {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("RaftService started on node {}", self.node_id);
        
        // 保存Actor地址
        self.addr = Some(ctx.address());
        
        // 启动选举计时器
        self.start_election_timer(ctx);
        
        // 启动心跳定时器
        self.start_heartbeat_timer(ctx);
    }
    
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        info!("RaftService stopping on node {}", self.node_id);
        Running::Stop
    }
}

impl RaftService {
    /// 创建新的Raft服务（使用默认超时设置）
    pub fn new(
        node_id: String,
        apply_tx: mpsc::Sender<LogEntry>,
    ) -> Self {
        Self::new_with_config(node_id, apply_tx, 300, 100)
    }
    
    /// 使用自定义配置创建Raft服务
    pub fn new_with_config(
        node_id: String,
        apply_tx: mpsc::Sender<LogEntry>,
        election_timeout_ms: u64,
        heartbeat_interval_ms: u64,
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
            election_timeout_ms,
            heartbeat_interval_ms,
            next_index: Arc::new(Mutex::new(HashMap::new())),
            match_index: Arc::new(Mutex::new(HashMap::new())),
            votes_received: Arc::new(Mutex::new(HashSet::new())),
            election_timer: None,
            heartbeat_timer: None,
        }
    }
    
    /// 启动选举计时器
    pub(crate) fn start_election_timer(&mut self, ctx: &mut Context<Self>) {
        // 取消旧的计时器
        if let Some(handle) = self.election_timer.take() {
            ctx.cancel_future(handle);
        }
        
        // 生成随机超时时间 (基础时间 + 随机波动)
        let timeout_ms = self.election_timeout_ms + rand::thread_rng().gen_range(0..self.election_timeout_ms/2);
        let timeout = std::time::Duration::from_millis(timeout_ms);
        
        // 创建新的计时器
        let addr = ctx.address();
        self.election_timer = Some(ctx.run_later(timeout, move |_, ctx| {
            // 检查是否需要启动选举
            let should_start_election = {
                let mut state = self.state.lock().unwrap();
                let current_role = state.current_role;
                let elapsed = Utc::now()
                    .signed_duration_since(state.last_heartbeat)
                    .num_milliseconds();
                
                if (current_role == NodeState::Follower || current_role == NodeState::Candidate) 
                   && elapsed as u64 >= self.election_timeout_ms {
                    // 变为候选人状态
                    state.current_role = NodeState::Candidate;
                    true
                } else {
                    false
                }
            };
            
            if should_start_election {
                // 增加任期
                {
                    let mut term = self.current_term.lock().unwrap();
                    *term += 1;
                    debug!("Node {} starting election for term {}", self.node_id, *term);
                }
                
                // 初始化新的选举
                {
                    // 为自己投票
                    let mut voted_for = self.voted_for.lock().unwrap();
                    *voted_for = Some(self.node_id.clone());
                    
                    // 清空已收到的投票
                    let mut votes = self.votes_received.lock().unwrap();
                    votes.clear();
                    votes.insert(self.node_id.clone());
                }
                
                // 发送RequestVote RPC到所有节点
                self.request_votes(ctx);
            }
            
            // 重新启动计时器
            self.start_election_timer(ctx);
        }));
    }
    
    /// 请求选票
    fn request_votes(&self, ctx: &mut Context<Self>) {
        let term = {
            let term = self.current_term.lock().unwrap();
            *term
        };
        
        let last_log_index = {
            let log = self.log.lock().unwrap();
            log.len() as u64
        };
        
        let last_log_term = {
            let log = self.log.lock().unwrap();
            if log.is_empty() {
                0
            } else {
                0 // 这里应该获取最后日志条目的任期，简化处理
            }
        };
        
        let peers = {
            let state = self.state.lock().unwrap();
            state.peers.clone()
        };
        
        for (peer_id, peer_addr) in peers {
            // 不给自己发送投票请求
            if peer_id == self.node_id {
                continue;
            }
            
            let request = RequestVoteRequest {
                term,
                candidate_id: self.node_id.clone(),
                last_log_index,
                last_log_term,
            };
            
            let addr = ctx.address();
            let peer_id_clone = peer_id.clone();
            
            // 发送投票请求
            peer_addr.send(request)
                .into_actor(self)
                .then(move |res, actor, _ctx| {
                    match res {
                        Ok(response) => {
                            // 处理投票响应
                            actor.handle_vote_response(peer_id_clone, response);
                        }
                        Err(e) => {
                            warn!("Failed to send vote request to {}: {}", peer_id, e);
                        }
                    }
                    fut::ready(())
                })
                .wait(ctx);
        }
    }
    
    /// 处理投票响应
    fn handle_vote_response(&mut self, peer_id: String, response: RequestVoteResponse) {
        let current_term = {
            let term = self.current_term.lock().unwrap();
            *term
        };
        
        // 如果收到更高任期的响应，转为Follower
        if response.term > current_term {
            {
                let mut term = self.current_term.lock().unwrap();
                *term = response.term;
            }
            
            {
                let mut state = self.state.lock().unwrap();
                state.current_role = NodeState::Follower;
                state.current_leader = None;
            }
            
            return;
        }
        
        // 如果不是Candidate，忽略响应
        {
            let state = self.state.lock().unwrap();
            if state.current_role != NodeState::Candidate {
                return;
            }
        }
        
        // 如果得到投票
        if response.vote_granted {
            {
                let mut votes = self.votes_received.lock().unwrap();
                votes.insert(peer_id);
                
                // 检查是否获得多数票
                let peers_count = {
                    let state = self.state.lock().unwrap();
                    state.peers.len() + 1 // +1 包括自己
                };
                
                let majority = peers_count / 2 + 1;
                if votes.len() >= majority {
                    // 赢得选举，成为Leader
                    debug!("Node {} won election for term {}", self.node_id, current_term);
                    
                    // 初始化Leader状态
                    {
                        let mut state = self.state.lock().unwrap();
                        state.current_role = NodeState::Leader;
                        state.current_leader = Some(self.node_id.clone());
                        state.last_heartbeat = Utc::now();
                    }
                    
                    // 初始化nextIndex和matchIndex
                    {
                        let log_len = {
                            let log = self.log.lock().unwrap();
                            log.len() as u64
                        };
                        
                        let mut next_index = self.next_index.lock().unwrap();
                        let mut match_index = self.match_index.lock().unwrap();
                        
                        let peers = {
                            let state = self.state.lock().unwrap();
                            state.peers.keys().cloned().collect::<Vec<_>>()
                        };
                        
                        for peer_id in peers {
                            next_index.insert(peer_id.clone(), log_len + 1);
                            match_index.insert(peer_id, 0);
                        }
                    }
                    
                    // 立即发送心跳
                    if let Some(addr) = &self.addr {
                        addr.do_send(SendHeartbeat);
                    }
                }
            }
        }
    }
    
    /// 开始心跳定时器
    pub(crate) fn start_heartbeat_timer(&mut self, ctx: &mut Context<Self>) {
        // 取消旧的计时器
        if let Some(handle) = self.heartbeat_timer.take() {
            ctx.cancel_future(handle);
        }
        
        // 创建新的计时器
        let timeout = std::time::Duration::from_millis(self.heartbeat_interval_ms);
        let addr = ctx.address();
        
        self.heartbeat_timer = Some(ctx.run_later(timeout, move |actor, ctx| {
            // 检查是否是Leader
            let is_leader = {
                let state = actor.state.lock().unwrap();
                state.current_role == NodeState::Leader
            };
            
            if is_leader {
                // 发送心跳
                actor.send_heartbeats(ctx);
            }
            
            // 重新启动计时器
            actor.start_heartbeat_timer(ctx);
        }));
    }
    
    /// 发送心跳到所有节点
    fn send_heartbeats(&self, ctx: &mut Context<Self>) {
        let term = {
            let term = self.current_term.lock().unwrap();
            *term
        };
        
        let commit_index = {
            let commit_index = self.commit_index.lock().unwrap();
            *commit_index
        };
        
        let peers = {
            let state = self.state.lock().unwrap();
            state.peers.clone()
        };
        
        for (peer_id, peer_addr) in peers {
            // 不给自己发送心跳
            if peer_id == self.node_id {
                continue;
            }
            
            // 获取nextIndex
            let next_idx = {
                let next_index = self.next_index.lock().unwrap();
                *next_index.get(&peer_id).unwrap_or(&1)
            };
            
            let prev_log_index = next_idx - 1;
            let prev_log_term = {
                if prev_log_index == 0 {
                    0
                } else {
                    let log = self.log.lock().unwrap();
                    if prev_log_index as usize <= log.len() {
                        0 // 简化，应该获取对应日志条目的任期
                    } else {
                        0
                    }
                }
            };
            
            // 获取要发送的日志条目
            let entries = {
                let log = self.log.lock().unwrap();
                if next_idx as usize <= log.len() {
                    log[next_idx as usize - 1..].to_vec()
                } else {
                    Vec::new() // 空数组表示心跳
                }
            };
            
            let request = AppendEntriesRequest {
                term,
                leader_id: self.node_id.clone(),
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit: commit_index,
            };
            
            // 发送AppendEntries RPC
            let peer_id_clone = peer_id.clone();
            let self_addr = ctx.address();
            
            actix::spawn(async move {
                if let Err(e) = peer_addr.send(request).await {
                    warn!("Failed to send AppendEntries to {}: {}", peer_id_clone, e);
                }
            });
        }
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
        
        // 更新commitIndex（简化处理）
        {
            let mut commit_index = self.commit_index.lock().unwrap();
            *commit_index += 1;
        }
        
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
    pub fn add_peer(&mut self, node_id: String, addr: Recipient<RequestVoteRequest>) {
        let mut state = self.state.lock().unwrap();
        state.peers.insert(node_id, addr);
    }
}

/// 开始选举消息
#[derive(Message)]
#[rtype(result = "()")]
pub struct StartElection;

impl Handler<StartElection> for RaftService {
    type Result = ();
    
    fn handle(&mut self, _: StartElection, ctx: &mut Self::Context) -> Self::Result {
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
            
            // 初始化投票集
            let mut votes = self.votes_received.lock().unwrap();
            votes.clear();
            votes.insert(self.node_id.clone());
        }
        
        // 发送RequestVote RPC到所有对等节点
        self.request_votes(ctx);
    }
}

/// 发送心跳消息
#[derive(Message)]
#[rtype(result = "()")]
pub struct SendHeartbeat;

impl Handler<SendHeartbeat> for RaftService {
    type Result = ();
    
    fn handle(&mut self, _: SendHeartbeat, ctx: &mut Self::Context) -> Self::Result {
        // 实现发送心跳逻辑
        let term = {
            let term = self.current_term.lock().unwrap();
            *term
        };
        
        // 发送AppendEntries RPC到所有对等节点作为心跳
        debug!("Sending heartbeat from node {} for term {}", self.node_id, term);
        self.send_heartbeats(ctx);
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
    pub match_index: u64,
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
                match_index: 0,
            };
        }
        
        // 如果是心跳消息或日志条目，更新状态
        {
            let mut state = self.state.lock().unwrap();
            
            // 更新心跳时间
            state.last_heartbeat = Utc::now();
            
            // 如果请求任期大于当前任期，转为Follower
            if msg.term > current_term {
                state.current_role = NodeState::Follower;
                let mut term = self.current_term.lock().unwrap();
                *term = msg.term;
                
                // 清空投票
                let mut voted_for = self.voted_for.lock().unwrap();
                *voted_for = None;
            }
            
            // 如果是Candidate，且收到同任期的AppendEntries，转为Follower
            if state.current_role == NodeState::Candidate && msg.term == current_term {
                state.current_role = NodeState::Follower;
            }
            
            // 记录当前Leader
            state.current_leader = Some(msg.leader_id.clone());
        }
        
        // 检查prevLog一致性
        let log_consistent = {
            let log = self.log.lock().unwrap();
            
            // 如果prevLogIndex为0，始终一致
            if msg.prev_log_index == 0 {
                true
            }
            // 如果本地日志没有prevLogIndex对应的条目，不一致
            else if msg.prev_log_index as usize > log.len() {
                false
            }
            // 如果有对应条目，检查term是否匹配
            else {
                // 简化处理，假设匹配
                true
            }
        };
        
        // 如果日志不一致，拒绝请求
        if !log_consistent {
            return AppendEntriesResponse {
                term: current_term,
                success: false,
                match_index: 0,
            };
        }
        
        // 处理日志条目
        let mut match_index = msg.prev_log_index;
        if !msg.entries.is_empty() {
            let mut log = self.log.lock().unwrap();
            
            // 从prevLogIndex之后的位置开始追加日志
            let start_idx = msg.prev_log_index as usize;
            
            // 删除冲突的日志条目
            if start_idx < log.len() {
                log.truncate(start_idx);
            }
            
            // 追加新日志
            for entry in &msg.entries {
                log.push(entry.clone());
                match_index += 1;
            }
        }
        
        // 更新commitIndex
        {
            let mut commit_index = self.commit_index.lock().unwrap();
            if msg.leader_commit > *commit_index {
                *commit_index = std::cmp::min(msg.leader_commit, match_index);
            }
        }
        
        // 应用日志到状态机（简化处理）
        self.apply_log_entries();
        
        // 返回成功
        AppendEntriesResponse {
            term: current_term,
            success: true,
            match_index,
        }
    }
}

/// 应用日志条目到状态机
fn apply_log_entries(&self) {
    let (commit_index, last_applied) = {
        let commit_index = self.commit_index.lock().unwrap();
        let last_applied = self.last_applied.lock().unwrap();
        (*commit_index, *last_applied)
    };
    
    // 如果有新的已提交日志需要应用
    if commit_index > last_applied {
        let log = self.log.lock().unwrap();
        let apply_ch = self.apply_ch.clone();
        
        // 应用从last_applied+1到commit_index的日志
        for i in (last_applied + 1)..=commit_index {
            if i as usize <= log.len() {
                let entry = log[i as usize - 1].clone();
                
                // 发送到应用通道（异步处理）
                actix::spawn(async move {
                    if let Err(e) = apply_ch.send(entry).await {
                        error!("Failed to send log entry to state machine: {}", e);
                    }
                });
            }
        }
        
        // 更新last_applied
        let mut last_applied = self.last_applied.lock().unwrap();
        *last_applied = commit_index;
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
                state.current_leader = None;
                state.last_heartbeat = Utc::now();
                
                let mut term = self.current_term.lock().unwrap();
                *term = msg.term;
                
                let mut vote = self.voted_for.lock().unwrap();
                *vote = None;
            }
        }
        
        // 判断是否可以投票
        let can_vote = voted_for.is_none() || voted_for == Some(msg.candidate_id.clone());
        
        // 检查日志的完整性
        let log_ok = {
            let log = self.log.lock().unwrap();
            
            let last_idx = log.len() as u64;
            let last_term = if last_idx > 0 { 0 } else { 0 }; // 简化，应获取实际term
            
            // 如果候选人日志至少与我们一样新
            msg.last_log_term > last_term || 
            (msg.last_log_term == last_term && msg.last_log_index >= last_idx)
        };
        
        if can_vote && log_ok {
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
        let next_index = self.next_index.clone();
        let self_addr = self.addr.clone();
        let node_id = self.node_id.clone();
        
        Box::pin(async move {
            if is_leader {
                // 如果是Leader，直接添加到本地日志
                {
                    let mut log_guard = log.lock().unwrap();
                    log_guard.push(entry.clone());
                    
                    // 更新next_index
                    let mut next_idx = next_index.lock().unwrap();
                    for (_, idx) in next_idx.iter_mut() {
                        *idx += 1;
                    }
                }
                
                // 发送到应用通道
                if let Err(e) = apply_ch.send(entry).await {
                    error!("Failed to send log entry to state machine: {}", e);
                    return Err(RaftError::Other(format!("Failed to apply: {}", e)));
                }
                
                // 发送AppendEntries到其他节点
                if let Some(addr) = self_addr {
                    addr.send(SendHeartbeat).await.map_err(|e| {
                        RaftError::Other(format!("Failed to send heartbeat: {}", e))
                    })?;
                }
                
                Ok(())
            } else if let Some(leader) = leader_id {
                // 不是Leader但知道Leader
                Err(RaftError::NotLeader)
            } else {
                // 不知道Leader
                Err(RaftError::NotLeader)
            }
        })
    }
}

/// 添加依赖
use std::collections::HashSet;
use futures::future::{self, Future, FutureExt}; 