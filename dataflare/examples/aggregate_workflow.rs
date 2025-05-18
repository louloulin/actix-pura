//! 聚合处理器示例
//!
//! 演示如何使用 DataFlare 的聚合处理器进行数据聚合操作。

use dataflare::{
    workflow::{WorkflowBuilder, WorkflowExecutor},
    message::WorkflowProgress,
    processor::AggregateProcessor,
};
use std::sync::{Arc, Mutex};

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    println!("创建聚合工作流...");
    
    // 创建工作流
    let workflow = WorkflowBuilder::new("aggregate-workflow", "Aggregate Workflow")
        .description("演示聚合处理器功能的工作流")
        .version("1.0.0")
        // 内存源数据（销售数据）
        .source("sales", "memory", serde_json::json!({
            "data": [
                {"id": 1, "product": "Laptop", "category": "Electronics", "price": 1200, "quantity": 1, "date": "2023-01-15"},
                {"id": 2, "product": "Smartphone", "category": "Electronics", "price": 800, "quantity": 2, "date": "2023-01-16"},
                {"id": 3, "product": "Headphones", "category": "Electronics", "price": 150, "quantity": 3, "date": "2023-01-17"},
                {"id": 4, "product": "Book", "category": "Books", "price": 25, "quantity": 5, "date": "2023-01-15"},
                {"id": 5, "product": "Notebook", "category": "Office", "price": 10, "quantity": 10, "date": "2023-01-16"},
                {"id": 6, "product": "Pen", "category": "Office", "price": 2, "quantity": 20, "date": "2023-01-17"},
                {"id": 7, "product": "Tablet", "category": "Electronics", "price": 500, "quantity": 1, "date": "2023-01-18"},
                {"id": 8, "product": "Monitor", "category": "Electronics", "price": 300, "quantity": 2, "date": "2023-01-18"}
            ]
        }))
        // 聚合处理器 - 按类别聚合
        .transformation("category_aggregation", "aggregate", vec!["sales"], serde_json::json!({
            "group_by": ["category"],
            "aggregations": [
                {
                    "source": "price",
                    "destination": "avg_price",
                    "function": "average"
                },
                {
                    "source": "quantity",
                    "destination": "total_quantity",
                    "function": "sum"
                },
                {
                    "source": "price",
                    "destination": "total_revenue",
                    "function": "sum"
                },
                {
                    "source": "id",
                    "destination": "count",
                    "function": "count"
                }
            ],
            "keep_original_fields": false
        }))
        // 内存目标
        .destination("aggregated_results", "memory", vec!["category_aggregation"], serde_json::json!({}))
        .build()
        .unwrap();
    
    println!("聚合工作流已创建: {}", workflow.id);
    
    // 创建进度计数器
    let progress_counter = Arc::new(Mutex::new(0));
    let progress_counter_clone = progress_counter.clone();
    
    // 创建工作流执行器
    let mut executor = WorkflowExecutor::new()
        .with_progress_callback(move |progress: WorkflowProgress| {
            println!(
                "进度: 工作流={}, 阶段={:?}, 进度={:.2}, 消息={}",
                progress.workflow_id, progress.phase, progress.progress, progress.message
            );
            
            // 增加计数器
            let mut counter = progress_counter_clone.lock().unwrap();
            *counter += 1;
        });
    
    println!("初始化执行器...");
    executor.initialize()?;
    
    println!("准备工作流...");
    executor.prepare(&workflow)?;
    
    println!("执行工作流...");
    executor.execute(&workflow).await?;
    
    // 验证收到进度更新
    let counter = progress_counter.lock().unwrap();
    println!("收到 {} 个进度更新", *counter);
    
    println!("完成执行器...");
    executor.finalize()?;
    
    println!("聚合工作流成功完成!");
    
    Ok(())
}
