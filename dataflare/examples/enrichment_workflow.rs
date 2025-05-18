//! 丰富处理器示例
//!
//! 演示如何使用 DataFlare 的丰富处理器进行数据丰富操作。

use dataflare::{
    workflow::{WorkflowBuilder, WorkflowExecutor},
    message::WorkflowProgress,
    processor::EnrichmentProcessor,
};
use std::sync::{Arc, Mutex};

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    println!("创建丰富工作流...");
    
    // 创建工作流
    let workflow = WorkflowBuilder::new("enrichment-workflow", "Enrichment Workflow")
        .description("演示丰富处理器功能的工作流")
        .version("1.0.0")
        // 内存源数据（订单数据）
        .source("orders", "memory", serde_json::json!({
            "data": [
                {"id": 1, "product_id": 101, "customer_id": 1001, "quantity": 2, "price": 25.99},
                {"id": 2, "product_id": 102, "customer_id": 1002, "quantity": 1, "price": 199.99},
                {"id": 3, "product_id": 103, "customer_id": 1001, "quantity": 3, "price": 15.50},
                {"id": 4, "product_id": 101, "customer_id": 1003, "quantity": 1, "price": 25.99},
                {"id": 5, "product_id": 104, "customer_id": 1002, "quantity": 2, "price": 49.99}
            ]
        }))
        // 丰富处理器 - 添加产品信息
        .transformation("product_enrichment", "enrichment", vec!["orders"], serde_json::json!({
            "source_field": "product_id",
            "target_field": "product",
            "lookup_key": "id",
            "keep_original_fields": true,
            "lookup_data": [
                {"id": 101, "name": "Basic T-shirt", "category": "Clothing", "in_stock": true},
                {"id": 102, "name": "Premium Headphones", "category": "Electronics", "in_stock": true},
                {"id": 103, "name": "Notebook", "category": "Office", "in_stock": true},
                {"id": 104, "name": "USB Cable", "category": "Electronics", "in_stock": false},
                {"id": 105, "name": "Coffee Mug", "category": "Kitchen", "in_stock": true}
            ],
            "default_value": {"name": "Unknown Product", "category": "Unknown", "in_stock": false}
        }))
        // 丰富处理器 - 添加客户信息
        .transformation("customer_enrichment", "enrichment", vec!["product_enrichment"], serde_json::json!({
            "source_field": "customer_id",
            "target_field": "customer",
            "lookup_key": "id",
            "keep_original_fields": true,
            "lookup_data": [
                {"id": 1001, "name": "John Doe", "email": "john.doe@example.com", "tier": "Gold"},
                {"id": 1002, "name": "Jane Smith", "email": "jane.smith@example.com", "tier": "Silver"},
                {"id": 1003, "name": "Bob Johnson", "email": "bob.johnson@example.com", "tier": "Bronze"},
                {"id": 1004, "name": "Alice Brown", "email": "alice.brown@example.com", "tier": "Gold"}
            ],
            "default_value": {"name": "Unknown Customer", "email": "unknown@example.com", "tier": "Standard"}
        }))
        // 内存目标
        .destination("enriched_orders", "memory", vec!["customer_enrichment"], serde_json::json!({}))
        .build()
        .unwrap();
    
    println!("丰富工作流已创建: {}", workflow.id);
    
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
    
    println!("丰富工作流成功完成!");
    
    Ok(())
}
