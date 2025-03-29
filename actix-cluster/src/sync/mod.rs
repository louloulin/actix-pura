//! 状态同步和冲突解决模块

use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use std::cmp::Ordering;
use std::fmt::Debug;

use crate::node::NodeId;

/// 具有时间戳的特征
pub trait HasTimestamp {
    /// 获取时间戳
    fn timestamp(&self) -> SystemTime;
}

/// 向量时钟关系
#[derive(Debug, PartialEq, Eq)]
pub enum VectorClockRelation {
    /// 大于
    Greater,
    
    /// 小于
    Less,
    
    /// 并发（无法比较）
    Concurrent,
    
    /// 相等
    Equal,
}

/// 向量时钟
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorClock {
    /// 各节点的逻辑时钟
    counters: HashMap<NodeId, u64>,
}

impl VectorClock {
    /// 创建新的向量时钟
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }
    
    /// 递增节点的计数器
    pub fn increment(&mut self, node_id: &NodeId) {
        let counter = self.counters.entry(node_id.clone()).or_insert(0);
        *counter += 1;
    }
    
    /// 设置节点的计数器值
    pub fn set(&mut self, node_id: &NodeId, value: u64) {
        self.counters.insert(node_id.clone(), value);
    }
    
    /// 获取节点的计数器值
    pub fn get(&self, node_id: &NodeId) -> u64 {
        *self.counters.get(node_id).unwrap_or(&0)
    }
    
    /// 合并两个向量时钟，取每个节点的最大值
    pub fn merge(&self, other: &Self) -> Self {
        let mut result = self.clone();
        
        for (node_id, counter) in &other.counters {
            let entry = result.counters.entry(node_id.clone()).or_insert(0);
            *entry = std::cmp::max(*entry, *counter);
        }
        
        result
    }
    
    /// 比较两个向量时钟
    pub fn compare(&self, other: &Self) -> VectorClockRelation {
        let mut self_gt = false;
        let mut other_gt = false;
        
        // 检查self中的所有节点
        for (node_id, self_counter) in &self.counters {
            let other_counter = other.get(node_id);
            
            if *self_counter > other_counter {
                self_gt = true;
            } else if *self_counter < other_counter {
                other_gt = true;
            }
        }
        
        // 检查other中有但self中没有的节点
        for (node_id, other_counter) in &other.counters {
            if !self.counters.contains_key(node_id) && *other_counter > 0 {
                other_gt = true;
            }
        }
        
        match (self_gt, other_gt) {
            (false, false) => VectorClockRelation::Equal,
            (true, false) => VectorClockRelation::Greater,
            (false, true) => VectorClockRelation::Less,
            (true, true) => VectorClockRelation::Concurrent,
        }
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

/// 具有向量时钟的特征
pub trait HasVectorClock {
    /// 获取向量时钟
    fn vector_clock(&self) -> &VectorClock;
    
    /// 获取可变向量时钟
    fn vector_clock_mut(&mut self) -> &mut VectorClock;
}

/// 冲突解决器特征
pub trait ConflictResolver<T> {
    /// 解决冲突
    fn resolve(&self, local: &T, remote: &T) -> T;
}

/// 最后写入胜出解析器
pub struct LastWriteWinsResolver {
    /// 时钟漂移容忍度
    pub clock_drift_tolerance: Duration,
}

impl LastWriteWinsResolver {
    /// 创建新的LWW解析器
    pub fn new(clock_drift_tolerance: Duration) -> Self {
        Self {
            clock_drift_tolerance,
        }
    }
}

impl<T: Clone + HasTimestamp> ConflictResolver<T> for LastWriteWinsResolver {
    fn resolve(&self, local: &T, remote: &T) -> T {
        let local_time = local.timestamp();
        let remote_time = remote.timestamp();
        
        // 计算本地和远程时间的差值，考虑漂移容忍度
        match remote_time.duration_since(local_time) {
            Ok(duration) if duration > self.clock_drift_tolerance => remote.clone(),
            _ => local.clone(), // 如果远程不是明显更新，选择本地版本
        }
    }
}

/// 基于向量时钟的解析器
pub struct VectorClockResolver;

impl VectorClockResolver {
    /// 创建新的向量时钟解析器
    pub fn new() -> Self {
        Self
    }
}

impl<T: Clone + HasVectorClock> ConflictResolver<T> for VectorClockResolver {
    fn resolve(&self, local: &T, remote: &T) -> T {
        match local.vector_clock().compare(remote.vector_clock()) {
            VectorClockRelation::Greater => local.clone(),
            VectorClockRelation::Less => remote.clone(),
            VectorClockRelation::Concurrent => {
                // 并发修改，需要特殊处理
                // 这里简化处理，默认选择本地版本
                // 实际应用中可能需要更复杂的合并逻辑或人工介入
                local.clone()
            }
            VectorClockRelation::Equal => local.clone(),
        }
    }
}

/// 自定义解析器
pub struct CustomResolver<T, F>
where
    F: Fn(&T, &T) -> T,
{
    resolver_fn: F,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F> CustomResolver<T, F>
where
    F: Fn(&T, &T) -> T,
{
    /// 创建新的自定义解析器
    pub fn new(resolver_fn: F) -> Self {
        Self {
            resolver_fn,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F> ConflictResolver<T> for CustomResolver<T, F>
where
    F: Fn(&T, &T) -> T,
{
    fn resolve(&self, local: &T, remote: &T) -> T {
        (self.resolver_fn)(local, remote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};
    
    // 测试数据结构
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestState {
        id: String,
        value: String,
        timestamp: SystemTime,
        vector_clock: VectorClock,
    }
    
    impl TestState {
        fn new(id: &str, value: &str) -> Self {
            Self {
                id: id.to_string(),
                value: value.to_string(),
                timestamp: SystemTime::now(),
                vector_clock: VectorClock::new(),
            }
        }
        
        fn with_timestamp(mut self, timestamp: SystemTime) -> Self {
            self.timestamp = timestamp;
            self
        }
        
        fn with_vector_clock(mut self, vector_clock: VectorClock) -> Self {
            self.vector_clock = vector_clock;
            self
        }
    }
    
    impl HasTimestamp for TestState {
        fn timestamp(&self) -> SystemTime {
            self.timestamp
        }
    }
    
    impl HasVectorClock for TestState {
        fn vector_clock(&self) -> &VectorClock {
            &self.vector_clock
        }
        
        fn vector_clock_mut(&mut self) -> &mut VectorClock {
            &mut self.vector_clock
        }
    }
    
    #[test]
    fn test_vector_clock_basics() {
        let mut vc = VectorClock::new();
        let node1 = NodeId::new();
        let node2 = NodeId::new();
        
        // 初始状态
        assert_eq!(vc.get(&node1), 0);
        
        // 递增计数器
        vc.increment(&node1);
        assert_eq!(vc.get(&node1), 1);
        
        // 设置计数器
        vc.set(&node2, 5);
        assert_eq!(vc.get(&node2), 5);
        
        // 再次递增
        vc.increment(&node1);
        assert_eq!(vc.get(&node1), 2);
    }
    
    #[test]
    fn test_vector_clock_compare() {
        let node1 = NodeId::new();
        let node2 = NodeId::new();
        
        // 创建两个初始时钟
        let mut vc1 = VectorClock::new();
        let mut vc2 = VectorClock::new();
        
        // 相等的情况
        assert_eq!(vc1.compare(&vc2), VectorClockRelation::Equal);
        
        // vc1 > vc2
        vc1.increment(&node1);
        assert_eq!(vc1.compare(&vc2), VectorClockRelation::Greater);
        assert_eq!(vc2.compare(&vc1), VectorClockRelation::Less);
        
        // 并发修改
        vc2.increment(&node2);
        assert_eq!(vc1.compare(&vc2), VectorClockRelation::Concurrent);
        
        // 让vc1全面大于vc2
        vc1.increment(&node2);
        assert_eq!(vc1.compare(&vc2), VectorClockRelation::Greater);
    }
    
    #[test]
    fn test_vector_clock_merge() {
        let node1 = NodeId::new();
        let node2 = NodeId::new();
        
        let mut vc1 = VectorClock::new();
        let mut vc2 = VectorClock::new();
        
        vc1.set(&node1, 3);
        vc1.set(&node2, 1);
        
        vc2.set(&node1, 2);
        vc2.set(&node2, 5);
        
        let merged = vc1.merge(&vc2);
        
        // 合并应该取每个节点的最大值
        assert_eq!(merged.get(&node1), 3);
        assert_eq!(merged.get(&node2), 5);
    }
    
    #[test]
    fn test_last_write_wins_resolver() {
        let now = SystemTime::now();
        let earlier = now - Duration::from_secs(100);
        let later = now + Duration::from_secs(100);
        
        let local = TestState::new("item1", "local-value").with_timestamp(now);
        let older_remote = TestState::new("item1", "old-remote-value").with_timestamp(earlier);
        let newer_remote = TestState::new("item1", "new-remote-value").with_timestamp(later);
        
        // 使用较小的漂移容忍度
        let resolver = LastWriteWinsResolver::new(Duration::from_secs(1));
        
        // 本地比远程新，应该选择本地版本
        let result = resolver.resolve(&local, &older_remote);
        assert_eq!(result.value, "local-value");
        
        // 远程比本地新，应该选择远程版本
        let result = resolver.resolve(&local, &newer_remote);
        assert_eq!(result.value, "new-remote-value");
        
        // 使用较大的漂移容忍度
        let resolver = LastWriteWinsResolver::new(Duration::from_secs(200));
        
        // 即使远程较新，但在容忍范围内，应该选择本地版本
        let result = resolver.resolve(&local, &newer_remote);
        assert_eq!(result.value, "local-value");
    }
    
    #[test]
    fn test_vector_clock_resolver() {
        let node1 = NodeId::new();
        let node2 = NodeId::new();
        
        // 创建向量时钟
        let mut vc1 = VectorClock::new();
        vc1.set(&node1, 3);
        vc1.set(&node2, 1);
        
        let mut vc2 = VectorClock::new();
        vc2.set(&node1, 2);
        vc2.set(&node2, 5);
        
        let mut vc3 = VectorClock::new();
        vc3.set(&node1, 4);
        vc3.set(&node2, 6);
        
        // 创建测试数据
        let local = TestState::new("item1", "local-value").with_vector_clock(vc1);
        let concurrent_remote = TestState::new("item1", "concurrent-value").with_vector_clock(vc2);
        let newer_remote = TestState::new("item1", "newer-value").with_vector_clock(vc3);
        
        let resolver = VectorClockResolver::new();
        
        // 并发修改，默认选择本地版本
        let result = resolver.resolve(&local, &concurrent_remote);
        assert_eq!(result.value, "local-value");
        
        // 远程更新，选择远程版本
        let result = resolver.resolve(&local, &newer_remote);
        assert_eq!(result.value, "newer-value");
    }
    
    #[test]
    fn test_custom_resolver() {
        // 创建一个始终选择值字典序较大的解析器
        let resolver = CustomResolver::new(|local: &TestState, remote: &TestState| {
            if local.value > remote.value {
                local.clone()
            } else {
                remote.clone()
            }
        });
        
        let local = TestState::new("item1", "banana");
        let remote = TestState::new("item1", "apple");
        
        // banana > apple，应该选择local
        let result = resolver.resolve(&local, &remote);
        assert_eq!(result.value, "banana");
        
        let local = TestState::new("item1", "apple");
        let remote = TestState::new("item1", "banana");
        
        // banana > apple，应该选择remote
        let result = resolver.resolve(&local, &remote);
        assert_eq!(result.value, "banana");
    }
} 