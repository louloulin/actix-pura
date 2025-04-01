use std::collections::HashMap;
use tokio::test;

use crate::persistence::snapshot::{SnapshotProvider, InMemorySnapshot, SystemSnapshot};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct TestState {
    name: String,
    values: HashMap<String, i32>,
    status: String,
}

#[test]
async fn test_system_snapshot_creation_restoration() {
    // 创建测试状态
    let mut values = HashMap::new();
    values.insert("key1".to_string(), 100);
    values.insert("key2".to_string(), 200);
    
    let test_state = TestState {
        name: "Test State".to_string(),
        values,
        status: "Active".to_string(),
    };
    
    // 创建快照
    let snapshot = SystemSnapshot::new(
        "node1",
        Some("Test snapshot".to_string()),
        "1.0",
        &test_state
    ).unwrap();
    
    // 检查快照元数据
    assert_eq!(snapshot.metadata.node_id, "node1");
    assert_eq!(snapshot.metadata.description.as_ref().unwrap(), "Test snapshot");
    assert_eq!(snapshot.metadata.version, "1.0");
    
    // 从快照恢复状态
    let restored_state: TestState = snapshot.restore().unwrap();
    
    // 检查恢复的状态与原状态一致
    assert_eq!(restored_state.name, test_state.name);
    assert_eq!(restored_state.values, test_state.values);
    assert_eq!(restored_state.status, test_state.status);
}

#[test]
async fn test_in_memory_snapshot_provider() {
    let provider = InMemorySnapshot::new();
    
    // 创建测试状态
    let test_state = TestState {
        name: "State 1".to_string(),
        values: {
            let mut map = HashMap::new();
            map.insert("key1".to_string(), 100);
            map
        },
        status: "Active".to_string(),
    };
    
    // 创建并保存快照
    let snapshot = SystemSnapshot::new(
        "node1",
        Some("First snapshot".to_string()),
        "1.0",
        &test_state
    ).unwrap();
    
    provider.save_snapshot(snapshot.clone()).await.unwrap();
    
    // 加载快照
    let loaded_snapshot = provider.load_snapshot(&snapshot.metadata.id).await.unwrap();
    
    // 检查加载的快照元数据
    assert_eq!(loaded_snapshot.metadata.id, snapshot.metadata.id);
    assert_eq!(loaded_snapshot.metadata.node_id, "node1");
    
    // 从加载的快照恢复状态
    let loaded_state: TestState = loaded_snapshot.restore().unwrap();
    assert_eq!(loaded_state.name, test_state.name);
    
    // 测试获取最新快照
    let latest_snapshot = provider.get_latest_snapshot().await.unwrap();
    assert!(latest_snapshot.is_some());
    assert_eq!(latest_snapshot.unwrap().metadata.id, snapshot.metadata.id);
    
    // 创建第二个具有更新时间戳的快照
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    let test_state2 = TestState {
        name: "State 2".to_string(),
        values: {
            let mut map = HashMap::new();
            map.insert("key1".to_string(), 200);
            map
        },
        status: "Inactive".to_string(),
    };
    
    let snapshot2 = SystemSnapshot::new(
        "node1",
        Some("Second snapshot".to_string()),
        "1.0",
        &test_state2
    ).unwrap();
    
    provider.save_snapshot(snapshot2.clone()).await.unwrap();
    
    // 获取最新快照应该返回第二个快照
    let latest_snapshot = provider.get_latest_snapshot().await.unwrap().unwrap();
    let latest_state: TestState = latest_snapshot.restore().unwrap();
    assert_eq!(latest_state.name, "State 2");
    
    // 列出所有快照
    let all_snapshots = provider.list_snapshots().await.unwrap();
    assert_eq!(all_snapshots.len(), 2);
    
    // 删除一个快照
    provider.delete_snapshot(&snapshot.metadata.id).await.unwrap();
    
    // 再次列出快照，应该只剩一个
    let remaining_snapshots = provider.list_snapshots().await.unwrap();
    assert_eq!(remaining_snapshots.len(), 1);
    assert_eq!(remaining_snapshots[0].id, snapshot2.metadata.id);
} 