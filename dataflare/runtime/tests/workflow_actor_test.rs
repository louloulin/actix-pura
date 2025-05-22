//! Workflow Actor Tests
//!
//! 这个测试文件用于验证WorkflowActor在三层扁平架构中的实现和通信

use actix::prelude::*;
use std::time::Duration;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use chrono;

use dataflare_runtime::actor::{
    ClusterActor, ClusterConfig, GetStatus, ActorStatus, Initialize
};
use dataflare_runtime::actor::workflow::{
    WorkflowActor, GetWorkflowStats, StartWorkflow, StopWorkflow
};
use dataflare_runtime::actor::cluster::{
    GetWorkflowStatus, WorkflowStatus, StopWorkflow as ClusterStopWorkflow
};

#[actix::test]
async fn test_workflow_actor_initialization() {
    // 创建工作流执行器
    let workflow_actor = WorkflowActor::new("test-workflow".to_string());
    let addr = workflow_actor.start();

    // 配置基本工作流
    let config = serde_json::json!({
        "id": "test-workflow",
        "name": "测试工作流",
        "description": "测试工作流初始化",
        "version": "1.0.0",
        "sources": {
            "source1": {
                "type": "test-source",
                "config": { "test": "data" }
            }
        },
        "transformations": {
            "transform1": {
                "inputs": ["source1"],
                "type": "test-transformation",
                "config": { "test": "data" }
            }
        },
        "destinations": {
            "dest1": {
                "inputs": ["transform1"],
                "type": "test-destination",
                "config": { "test": "data" }
            }
        },
        "metadata": {},
        "created_at": chrono::Utc::now().to_rfc3339(),
        "updated_at": chrono::Utc::now().to_rfc3339()
    });

    // 初始化工作流
    let result = addr.send(Initialize {
        workflow_id: "test-workflow".to_string(),
        config,
    }).await.unwrap();

    assert!(result.is_ok());

    // 检查工作流状态
    let status = addr.send(GetStatus).await.unwrap().unwrap();
    assert!(matches!(status, ActorStatus::Initialized));
}

#[actix::test]
async fn test_workflow_lifecycle() {
    // 创建工作流执行器
    let workflow_actor = WorkflowActor::new("test-lifecycle".to_string());
    let addr = workflow_actor.start();

    // 配置基本工作流
    let config = serde_json::json!({
        "id": "test-lifecycle",
        "name": "生命周期测试",
        "description": "测试工作流生命周期",
        "version": "1.0.0",
        "sources": {
            "source1": {
                "type": "test-source",
                "config": { "test": "data" }
            }
        },
        "transformations": {
            "transform1": {
                "inputs": ["source1"],
                "type": "test-transformation",
                "config": { "test": "data" }
            }
        },
        "destinations": {
            "dest1": {
                "inputs": ["transform1"],
                "type": "test-destination",
                "config": { "test": "data" }
            }
        },
        "metadata": {},
        "created_at": chrono::Utc::now().to_rfc3339(),
        "updated_at": chrono::Utc::now().to_rfc3339()
    });

    // 初始化工作流
    let result = addr.send(Initialize {
        workflow_id: "test-lifecycle".to_string(),
        config,
    }).await.unwrap();

    assert!(result.is_ok());

    // 检查工作流状态
    let status = addr.send(GetStatus).await.unwrap().unwrap();
    assert!(matches!(status, ActorStatus::Initialized));

    // 启动工作流
    let result = addr.send(StartWorkflow).await.unwrap();
    assert!(result.is_ok());

    // 检查工作流状态
    let status = addr.send(GetStatus).await.unwrap().unwrap();
    assert!(matches!(status, ActorStatus::Running));

    // 让工作流运行一段时间
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 获取工作流统计信息
    let stats = addr.send(GetWorkflowStats).await.unwrap().unwrap();
    assert!(stats.start_time.is_some());

    // 停止工作流
    let result = addr.send(StopWorkflow).await.unwrap();
    assert!(result.is_ok());

    // 检查工作流状态
    let status = addr.send(GetStatus).await.unwrap().unwrap();
    assert!(matches!(status, ActorStatus::Stopped));
}

#[actix::test]
async fn test_three_layer_architecture() {
    // 创建集群Actor (顶层)
    let config = ClusterConfig::default();
    let cluster_actor = ClusterActor::new(config.clone());
    let cluster_addr = cluster_actor.start();

    // 部署工作流 (中层)
    let workflow_id = "test-three-layer".to_string();
    let workflow_config = serde_json::json!({
        "name": "三层架构测试",
        "sources": {
            "source1": {
                "type": "test-source",
                "config": { "test": "data" }
            }
        },
        "transformations": {
            "transform1": {
                "inputs": ["source1"],
                "type": "test-transformation",
                "config": { "test": "data" }
            }
        },
        "destinations": {
            "dest1": {
                "inputs": ["transform1"],
                "type": "test-destination",
                "config": { "test": "data" }
            }
        }
    });

    // 通过集群Actor部署工作流
    let result = cluster_addr.send(dataflare_runtime::actor::DeployWorkflow {
        id: workflow_id.clone(),
        config: workflow_config,
        node_id: None,
    }).await.unwrap();

    assert!(result.is_ok());

    // 检查工作流状态
    let status = cluster_addr.send(GetWorkflowStatus {
        id: workflow_id.clone(),
    }).await.unwrap().unwrap();

    // 应该是初始化状态
    assert!(matches!(status, WorkflowStatus::Initialized));

    // 停止工作流
    let result = cluster_addr.send(ClusterStopWorkflow {
        id: workflow_id.clone(),
    }).await.unwrap();

    assert!(result.is_ok());

    // 检查工作流状态
    let status = cluster_addr.send(GetWorkflowStatus {
        id: workflow_id,
    }).await.unwrap().unwrap();

    // 应该是停止状态
    assert!(matches!(status, WorkflowStatus::Stopped));
}