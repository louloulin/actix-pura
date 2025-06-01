use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use actix::prelude::*;
use log::{info, debug, warn, error};
use rand::Rng;
use serde::{Serialize, Deserialize};

use crate::cluster::{ClusterMessage, ClusterMessageResult, TradingClusterManager, ActorType};
use crate::consensus::state_machine::{StateMachine, StateMachineCommand, TradingStateMachine};

/// Raft节点状态
#[derive(Debug, Clone, PartialEq)]
pub enum RaftNodeState {
    /// 跟随者
    Follower,
    /// 候选人
    Candidate,
    /// 领导者
    Leader,
}

/// Raft配置
#[derive(Debug, Clone)]
pub struct RaftConfig {
    /// 选举超时下限(毫秒)
    pub election_timeout_min: u64,
    /// 选举超时上限(毫秒)
    pub election_timeout_max: u64,
    /// 心跳间隔(毫秒)
    pub heartbeat_interval: u64,
    /// 日志复制并行度
    pub replication_parallelism: usize,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_min: 150,
            election_timeout_max: 300,
            heartbeat_interval: 50,
            replication_parallelism: 5,
        }
    }
}

/// Raft消息类型
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "RaftResponseMessage")]
pub enum RaftRequestMessage {
    /// 请求投票 (term, candidate_id, last_log_index, last_log_term)
    RequestVote {
        term: u64,
        candidate_id: String,
        last_log_index: u64,
        last_log_term: u64,
    },
    
    /// 附加日志 (term, leader_id, prev_log_index, prev_log_term, entries, leader_commit)
    AppendEntries {
        term: u64,
        leader_id: String,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    },
    
    /// 安装快照 (term, leader_id, last_included_index, last_included_term, data)
    InstallSnapshot {
        term: u64,
        leader_id: String,
        last_included_index: u64,
        last_included_term: u64,
        data: Vec<u8>,
    },
}

/// Raft响应消息
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub enum RaftResponseMessage {
    /// 请求投票响应 (term, vote_granted)
    RequestVoteResponse {
        term: u64,
        vote_granted: bool,
    },
    
    /// 附加日志响应 (term, success, match_index)
    AppendEntriesResponse {
        term: u64,
        success: bool,
        match_index: u64,
    },
    
    /// 安装快照响应 (term, success)
    InstallSnapshotResponse {
        term: u64,
        success: bool,
    },
}

/// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 任期号
    pub term: u64,
    /// 索引
    pub index: u64,
    /// 命令
    pub command: StateMachineCommand,
}

/// 客户端命令
#[derive(Debug, Clone, Message)]
#[rtype(result = "ClientCommandResult")]
pub struct ClientCommand {
    /// 命令ID
    pub command_id: String,
    /// 命令内容
    pub command: StateMachineCommand,
}

/// 客户端命令结果
#[derive(Debug, Clone, MessageResponse)]
pub enum ClientCommandResult {
    /// 成功
    Success {
        /// 结果
        result: Option<String>,
        /// 领导者ID
        leader_id: Option<String>,
        /// 应用索引
        applied_index: u64,
    },
    /// 失败
    Failure {
        /// 错误信息
        error: String,
        /// 领导者ID
        leader_id: Option<String>,
    },
    /// 重定向到领导者
    Redirect {
        /// 领导者ID
        leader_id: String,
    },
}

/// Raft节点Actor
pub struct RaftNodeActor {
    /// 节点ID
    pub node_id: String,
    /// 当前任期
    current_term: u64,
    /// 投票给谁
    voted_for: Option<String>,
    /// 日志条目
    log: Vec<LogEntry>,
    /// 提交索引
    commit_index: u64,
    /// 应用索引
    last_applied: u64,
    /// 下一个要发送的日志索引(仅Leader)
    next_index: HashMap<String, u64>,
    /// 已经复制的最高日志索引(仅Leader)
    match_index: HashMap<String, u64>,
    /// 节点状态
    state: RaftNodeState,
    /// 当前领导者ID
    current_leader_id: Option<String>,
    /// 最后收到心跳的时间
    last_heartbeat: Instant,
    /// 选举超时
    election_timeout: Duration,
    /// 配置
    config: RaftConfig,
    /// 集群节点
    cluster_nodes: Vec<String>,
    /// 状态机
    state_machine: Box<dyn StateMachine>,
    /// 集群管理器
    cluster_manager: Option<Arc<TradingClusterManager>>,
    /// 心跳定时器
    heartbeat_timer: Option<SpawnHandle>,
    /// 选举定时器
    election_timer: Option<SpawnHandle>,
}

impl RaftNodeActor {
    /// 创建新的Raft节点
    pub fn new(node_id: String, config: RaftConfig) -> Self {
        let mut rng = rand::thread_rng();
        let election_timeout = Duration::from_millis(
            rng.gen_range(config.election_timeout_min..=config.election_timeout_max)
        );
        
        Self {
            node_id,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            state: RaftNodeState::Follower,
            current_leader_id: None,
            last_heartbeat: Instant::now(),
            election_timeout,
            config,
            cluster_nodes: Vec::new(),
            state_machine: Box::new(TradingStateMachine::new()),
            cluster_manager: None,
            heartbeat_timer: None,
            election_timer: None,
        }
    }
    
    /// 与集群管理器关联
    pub fn with_cluster_manager(mut self, manager: Arc<TradingClusterManager>) -> Self {
        self.cluster_manager = Some(manager);
        self
    }
    
    /// 添加集群节点
    pub fn add_cluster_node(&mut self, node_id: String) -> Result<(), String> {
        let node_id_clone = node_id.clone();
        if !self.cluster_nodes.contains(&node_id_clone) && node_id_clone != self.node_id {
            self.cluster_nodes.push(node_id_clone.clone());
            
            // 初始化节点的log索引
            self.next_index.insert(node_id.clone(), 1);
            self.match_index.insert(node_id.clone(), 0);
            
            info!("节点 {} 添加到集群", node_id_clone);
            Ok(())
        } else {
            Err(format!("节点 {} 已经在集群中或是当前节点", node_id))
        }
    }
    
    /// 设置集群节点列表
    pub fn set_cluster_nodes(&mut self, nodes: Vec<String>) {
        self.cluster_nodes = nodes
            .into_iter()
            .filter(|node_id| node_id != &self.node_id)
            .collect();
        
        // 如果是领导者，为所有节点初始化复制状态
        if self.state == RaftNodeState::Leader {
            let next_log_index = self.log.last().map(|entry| entry.index + 1).unwrap_or(1);
            for node_id in &self.cluster_nodes {
                self.next_index.insert(node_id.clone(), next_log_index);
                self.match_index.insert(node_id.clone(), 0);
            }
        }
    }
    
    /// 重置选举超时
    fn reset_election_timeout(&mut self, ctx: &mut <Self as Actor>::Context) {
        let mut rng = rand::thread_rng();
        self.election_timeout = Duration::from_millis(
            rng.gen_range(self.config.election_timeout_min..=self.config.election_timeout_max)
        );
        
        self.last_heartbeat = Instant::now();
        
        // 重置选举定时器
        if let Some(handle) = self.election_timer.take() {
            ctx.cancel_future(handle);
        }
        
        let election_timeout = self.election_timeout;
        self.election_timer = Some(ctx.run_later(election_timeout, |actor, ctx| {
            if actor.state != RaftNodeState::Leader && 
               Instant::now().duration_since(actor.last_heartbeat) >= actor.election_timeout {
                actor.start_election(ctx);
            }
        }));
    }
    
    /// 开始选举
    fn start_election(&mut self, ctx: &mut <Self as Actor>::Context) {
        if self.state == RaftNodeState::Leader {
            return;
        }
        
        // 变为候选人
        self.state = RaftNodeState::Candidate;
        // 增加任期
        self.current_term += 1;
        // 给自己投票
        self.voted_for = Some(self.node_id.clone());
        
        info!("Raft节点 {} 开始选举，任期 {}", self.node_id, self.current_term);
        
        // 获取最后日志条目的索引和任期
        let last_log_index = self.log.last().map(|entry| entry.index).unwrap_or(0);
        let last_log_term = self.log.last().map(|entry| entry.term).unwrap_or(0);
        
        // 重置选举超时
        self.reset_election_timeout(ctx);
        
        // 获得的票数(包括自己的一票)
        let votes_received = 1;
        let votes_needed = (self.cluster_nodes.len() + 1) / 2 + 1;
        
        // 给集群中其他节点发送请求投票RPC
        for node_id in &self.cluster_nodes {
            let request = RaftRequestMessage::RequestVote {
                term: self.current_term,
                candidate_id: self.node_id.clone(),
                last_log_index,
                last_log_term,
            };
            
            // 在实际实现中应通过网络发送给对应节点
            // 这里我们使用集群管理器模拟消息发送
            if let Some(manager) = &self.cluster_manager {
                let target_path = format!("/system/raft/{}", node_id);
                if let Err(e) = manager.send_message(&target_path, "raft.request_vote") {
                    error!("发送投票请求失败: {}", e);
                } else {
                    debug!("向节点 {} 发送投票请求", node_id);
                }
            }
        }
        
        // 在实际实现中，收到投票响应后应该更新votes_received并检查是否当选
    }
    
    /// 成为领导者
    fn become_leader(&mut self, ctx: &mut <Self as Actor>::Context) {
        if self.state != RaftNodeState::Candidate {
            return;
        }
        
        info!("节点 {} 当选为领导者，任期 {}", self.node_id, self.current_term);
        
        // 变为领导者
        self.state = RaftNodeState::Leader;
        self.current_leader_id = Some(self.node_id.clone());
        
        // 初始化领导者状态
        let next_index = self.log.last().map(|entry| entry.index + 1).unwrap_or(1);
        for node_id in &self.cluster_nodes {
            self.next_index.insert(node_id.clone(), next_index);
            self.match_index.insert(node_id.clone(), 0);
        }
        
        // 取消选举定时器
        if let Some(handle) = self.election_timer.take() {
            ctx.cancel_future(handle);
        }
        
        // 立即发送心跳
        self.send_heartbeats(ctx);
        
        // 设置心跳定时器
        let heartbeat_interval = Duration::from_millis(self.config.heartbeat_interval);
        self.heartbeat_timer = Some(ctx.run_interval(heartbeat_interval, |actor, ctx| {
            if actor.state == RaftNodeState::Leader {
                actor.send_heartbeats(ctx);
            }
        }));
    }
    
    /// 发送心跳
    fn send_heartbeats(&mut self, ctx: &mut <Self as Actor>::Context) {
        if self.state != RaftNodeState::Leader {
            return;
        }
        
        debug!("领导者 {} 发送心跳，任期 {}", self.node_id, self.current_term);
        
        for node_id in &self.cluster_nodes {
            // 获取要发送的下一个日志索引
            let next_idx = self.next_index.get(node_id).cloned().unwrap_or(1);
            let prev_log_index = next_idx - 1;
            
            // 获取prev_log_index对应的日志条目的任期
            let prev_log_term = if prev_log_index > 0 {
                self.log.iter()
                    .find(|entry| entry.index == prev_log_index)
                    .map(|entry| entry.term)
                    .unwrap_or(0)
            } else {
                0
            };
            
            // 创建空心跳消息(无日志条目)
            let request = RaftRequestMessage::AppendEntries {
                term: self.current_term,
                leader_id: self.node_id.clone(),
                prev_log_index,
                prev_log_term,
                entries: Vec::new(),
                leader_commit: self.commit_index,
            };
            
            // 在实际实现中应通过网络发送给对应节点
            // 这里我们使用集群管理器模拟消息发送
            if let Some(manager) = &self.cluster_manager {
                let target_path = format!("/system/raft/{}", node_id);
                if let Err(e) = manager.send_message(&target_path, "raft.append_entries") {
                    error!("发送心跳失败: {}", e);
                } else {
                    debug!("向节点 {} 发送心跳", node_id);
                }
            }
        }
    }
    
    /// 附加日志条目
    fn append_entries(&mut self, request: RaftRequestMessage) -> RaftResponseMessage {
        match request {
            RaftRequestMessage::AppendEntries { 
                term, leader_id, prev_log_index, prev_log_term, entries, leader_commit 
            } => {
                // 如果消息任期小于当前任期，拒绝请求
                if term < self.current_term {
                    return RaftResponseMessage::AppendEntriesResponse {
                        term: self.current_term,
                        success: false,
                        match_index: 0,
                    };
                }
                
                // 如果消息任期大于当前任期，更新任期并转为跟随者
                if term > self.current_term {
                    self.current_term = term;
                    self.state = RaftNodeState::Follower;
                    self.voted_for = None;
                }
                
                // 更新当前领导者
                self.current_leader_id = Some(leader_id);
                
                // 更新最后收到心跳的时间
                self.last_heartbeat = Instant::now();
                
                // 日志一致性检查
                let log_consistent = prev_log_index == 0 || 
                    self.log.iter().any(|entry| 
                        entry.index == prev_log_index && entry.term == prev_log_term
                    );
                
                if !log_consistent {
                    return RaftResponseMessage::AppendEntriesResponse {
                        term: self.current_term,
                        success: false,
                        match_index: 0,
                    };
                }
                
                // 处理日志条目
                let mut match_index = prev_log_index;
                
                if !entries.is_empty() {
                    // 删除冲突的日志条目
                    self.log.retain(|entry| entry.index <= prev_log_index);
                    
                    // 追加新的日志条目
                    for entry in &entries {
                        if !self.log.iter().any(|e| e.index == entry.index) {
                            self.log.push(entry.clone());
                            match_index = entry.index;
                        }
                    }
                    
                    // 排序日志条目
                    self.log.sort_by_key(|entry| entry.index);
                }
                
                // 更新commit_index
                if leader_commit > self.commit_index {
                    self.commit_index = std::cmp::min(
                        leader_commit,
                        self.log.last().map(|entry| entry.index).unwrap_or(0)
                    );
                }
                
                // 返回成功
                RaftResponseMessage::AppendEntriesResponse {
                    term: self.current_term,
                    success: true,
                    match_index,
                }
            },
            _ => {
                // 这里不应该发生
                error!("非AppendEntries请求传入append_entries方法");
                RaftResponseMessage::AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                    match_index: 0,
                }
            }
        }
    }
    
    /// 处理请求投票
    fn handle_request_vote(&mut self, request: RaftRequestMessage) -> RaftResponseMessage {
        match request {
            RaftRequestMessage::RequestVote { 
                term, candidate_id, last_log_index, last_log_term 
            } => {
                // 如果消息任期小于当前任期，拒绝投票
                if term < self.current_term {
                    return RaftResponseMessage::RequestVoteResponse {
                        term: self.current_term,
                        vote_granted: false,
                    };
                }
                
                // 如果消息任期大于当前任期，更新任期并转为跟随者
                if term > self.current_term {
                    self.current_term = term;
                    self.state = RaftNodeState::Follower;
                    self.voted_for = None;
                }
                
                // 如果已经投票给其他候选人，拒绝投票
                if self.voted_for.is_some() && self.voted_for.as_ref() != Some(&candidate_id) {
                    return RaftResponseMessage::RequestVoteResponse {
                        term: self.current_term,
                        vote_granted: false,
                    };
                }
                
                // 检查候选人的日志是否至少与本节点一样新
                let last_self_log_index = self.log.last().map(|entry| entry.index).unwrap_or(0);
                let last_self_log_term = self.log.last().map(|entry| entry.term).unwrap_or(0);
                
                let log_is_up_to_date = last_log_term > last_self_log_term || 
                    (last_log_term == last_self_log_term && last_log_index >= last_self_log_index);
                
                if log_is_up_to_date {
                    // 投票给候选人
                    self.voted_for = Some(candidate_id);
                    self.last_heartbeat = Instant::now();
                    
                    RaftResponseMessage::RequestVoteResponse {
                        term: self.current_term,
                        vote_granted: true,
                    }
                } else {
                    RaftResponseMessage::RequestVoteResponse {
                        term: self.current_term,
                        vote_granted: false,
                    }
                }
            },
            _ => {
                // 这里不应该发生
                error!("非RequestVote请求传入handle_request_vote方法");
                RaftResponseMessage::RequestVoteResponse {
                    term: self.current_term,
                    vote_granted: false,
                }
            }
        }
    }
    
    /// 处理客户端命令
    fn handle_client_command(&mut self, command: ClientCommand, ctx: &mut <Self as Actor>::Context) -> ClientCommandResult {
        // 如果不是领导者，重定向到领导者
        if self.state != RaftNodeState::Leader {
            return ClientCommandResult::Redirect { 
                leader_id: self.current_leader_id.clone().unwrap_or_else(|| "unknown".to_string()) 
            };
        }
        
        info!("领导者 {} 处理客户端命令: {:?}", self.node_id, command.command);
        
        // 获取下一个日志索引
        let next_index = self.log.last().map(|entry| entry.index + 1).unwrap_or(1);
        
        // 创建日志条目
        let log_entry = LogEntry {
            term: self.current_term,
            index: next_index,
            command: command.command.clone(),
        };
        
        // 添加到本地日志
        self.log.push(log_entry.clone());
        
        // 仅有一个节点的特殊情况，直接提交并应用
        if self.cluster_nodes.is_empty() {
            self.commit_index = next_index;
            self.apply_committed_entries(ctx);
            
            // 返回成功
            return ClientCommandResult::Success { 
                result: None, 
                leader_id: Some(self.node_id.clone()),
                applied_index: next_index,
            };
        }
        
        // 向其他节点复制日志
        let nodes = self.cluster_nodes.clone(); // Clone nodes to avoid borrowing conflict
        for node_id in &nodes {
            // Avoid mutable borrow conflict by using a clone
            let node_id_clone = node_id.clone();
            self.replicate_log_to_follower(node_id_clone, ctx);
        }
        
        // 注意: 在实际实现中，我们应该等待大多数节点确认后再返回
        // 这里为了简化，直接返回成功
        ClientCommandResult::Success { 
            result: None, 
            leader_id: Some(self.node_id.clone()),
            applied_index: next_index,
        }
    }
    
    /// 向跟随者复制日志
    fn replicate_log_to_follower(&mut self, follower_id: String, ctx: &mut <Self as Actor>::Context) {
        if self.state != RaftNodeState::Leader {
            return;
        }
        
        let next_idx = self.next_index.get(&follower_id).cloned().unwrap_or(1);
        let prev_log_index = next_idx - 1;
        
        // 获取prev_log_index对应的日志条目的任期
        let prev_log_term = if prev_log_index > 0 {
            self.log.iter()
                .find(|entry| entry.index == prev_log_index)
                .map(|entry| entry.term)
                .unwrap_or(0)
        } else {
            0
        };
        
        // 获取要发送的日志条目
        let entries: Vec<LogEntry> = self.log.iter()
            .filter(|entry| entry.index >= next_idx)
            .cloned()
            .collect();
        
        // 创建AppendEntries请求
        let request = RaftRequestMessage::AppendEntries {
            term: self.current_term,
            leader_id: self.node_id.clone(),
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit: self.commit_index,
        };
        
        // 在实际实现中应通过网络发送给对应节点
        // 这里我们使用集群管理器模拟消息发送
        if let Some(manager) = &self.cluster_manager {
            let target_path = format!("/system/raft/{}", follower_id);
            if let Err(e) = manager.send_message(&target_path, "raft.append_entries") {
                error!("发送日志复制请求失败: {}", e);
            } else {
                debug!("向节点 {} 发送日志复制请求", follower_id);
            }
        }
    }
    
    /// 应用已提交的日志条目
    fn apply_committed_entries(&mut self, ctx: &mut <Self as Actor>::Context) {
        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            
            // 查找对应索引的日志条目
            if let Some(entry) = self.log.iter().find(|e| e.index == self.last_applied) {
                // 应用命令到状态机
                let response = self.state_machine.apply(&entry.command);
                
                info!("应用日志条目 {}, 任期 {}, 结果: {:?}", 
                      entry.index, entry.term, response);
                
                // 更新状态机的应用进度
                if let Some(state_machine) = self.state_machine.as_any_mut().downcast_mut::<TradingStateMachine>() {
                    state_machine.update_applied(entry.index, entry.term);
                }
            } else {
                warn!("找不到索引 {} 的日志条目", self.last_applied);
            }
        }
    }
    
    /// 处理附加日志响应
    fn handle_append_entries_response(&mut self, 
                                     follower_id: String, 
                                     response: RaftResponseMessage,
                                     ctx: &mut <Self as Actor>::Context) {
        match response {
            RaftResponseMessage::AppendEntriesResponse { term, success, match_index } => {
                // 如果响应任期大于当前任期，转为跟随者
                if term > self.current_term {
                    self.current_term = term;
                    self.state = RaftNodeState::Follower;
                    self.voted_for = None;
                    self.current_leader_id = None;
                    
                    // 取消心跳定时器
                    if let Some(handle) = self.heartbeat_timer.take() {
                        ctx.cancel_future(handle);
                    }
                    
                    return;
                }
                
                if success {
                    // 更新match_index和next_index
                    self.match_index.insert(follower_id.clone(), match_index);
                    self.next_index.insert(follower_id.clone(), match_index + 1);
                    
                    // 检查是否可以提交新的日志条目
                    let log_entries = self.log.clone(); // Clone to avoid borrowing conflict
                    for entry in &log_entries {
                        if entry.term == self.current_term && entry.index > self.commit_index {
                            // 计算有多少节点已经复制了这个日志条目
                            let replication_count = self.match_index.values()
                                .filter(|&&idx| idx >= entry.index)
                                .count() + 1; // +1 是因为包括自己
                            
                            // 如果多数节点已经复制，提交这个日志条目
                            if replication_count * 2 > self.cluster_nodes.len() + 1 {
                                info!("提交日志条目 {}, 任期 {}", entry.index, entry.term);
                                self.commit_index = entry.index;
                                self.apply_committed_entries(ctx);
                            }
                        }
                    }
                } else {
                    // 日志不一致，回退next_index
                    let current_next_idx = self.next_index.get(&follower_id).cloned().unwrap_or(1);
                    if current_next_idx > 1 {
                        self.next_index.insert(follower_id.clone(), current_next_idx - 1);
                        
                        // 重试复制
                        self.replicate_log_to_follower(follower_id, ctx);
                    }
                }
            },
            _ => {
                // 这里不应该发生
                error!("非AppendEntriesResponse响应传入handle_append_entries_response方法");
            }
        }
    }
}

impl Actor for RaftNodeActor {
    type Context = Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Raft节点 {} 启动", self.node_id);
        
        // 注册到集群管理器
        if let Some(manager) = &self.cluster_manager {
            let path = format!("/system/raft/{}", self.node_id);
            if let Err(e) = manager.register_actor_path(&path, ActorType::RaftConsensus) {
                error!("注册Raft节点到集群失败: {}", e);
            }
        }
        
        // 设置选举定时器
        self.reset_election_timeout(ctx);
    }
    
    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Raft节点 {} 停止", self.node_id);
    }
}

impl Handler<RaftRequestMessage> for RaftNodeActor {
    type Result = RaftResponseMessage;
    
    fn handle(&mut self, msg: RaftRequestMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            RaftRequestMessage::RequestVote { .. } => {
                self.handle_request_vote(msg)
            },
            RaftRequestMessage::AppendEntries { .. } => {
                self.append_entries(msg)
            },
            RaftRequestMessage::InstallSnapshot { .. } => {
                // 简化实现，暂不处理快照
                RaftResponseMessage::InstallSnapshotResponse {
                    term: self.current_term,
                    success: false,
                }
            }
        }
    }
}

impl Handler<ClientCommand> for RaftNodeActor {
    type Result = ClientCommandResult;
    
    fn handle(&mut self, msg: ClientCommand, ctx: &mut Self::Context) -> Self::Result {
        self.handle_client_command(msg, ctx)
    }
}

impl Handler<ClusterMessage> for RaftNodeActor {
    type Result = ClusterMessageResult;
    
    fn handle(&mut self, msg: ClusterMessage, ctx: &mut Self::Context) -> Self::Result {
        debug!("Raft节点 {} 收到集群消息: type={}", self.node_id, msg.message_type);
        
        // TODO: 在实际实现中应该将msg.payload反序列化为RaftRequestMessage
        // 这里为了示例只做简单处理
        
        match msg.message_type.as_str() {
            "raft.request_vote" => {
                debug!("收到请求投票消息");
                // 模拟RequestVote处理
            },
            "raft.append_entries" => {
                debug!("收到附加日志消息");
                // 模拟AppendEntries处理
                
                // 重置选举超时
                self.last_heartbeat = Instant::now();
            },
            _ => {
                warn!("未知的Raft消息类型: {}", msg.message_type);
            }
        }
        
        ClusterMessageResult::Success
    }
}

/// 状态机转换相关扩展 - 为StateMachine提供Any转换方法
pub trait StateMachineExt: StateMachine {
    /// 将StateMachine转换为Any引用
    fn as_any(&self) -> &dyn std::any::Any;
    
    /// 将StateMachine转换为Any可变引用
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: StateMachine + 'static> StateMachineExt for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
} 