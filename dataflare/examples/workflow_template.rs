//! 工作流模板示例
//!
//! 演示如何使用 DataFlare 的工作流模板功能。

use std::path::Path;
use dataflare::{
    workflow::{WorkflowTemplateManager, TemplateParameterValues, WorkflowExecutor},
    message::WorkflowProgress,
};
use std::sync::{Arc, Mutex};
use serde_json::json;

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    // 模板目录
    let template_dir = Path::new("examples/templates");
    
    println!("加载工作流模板...");
    
    // 创建模板管理器
    let mut template_manager = WorkflowTemplateManager::new(template_dir)?;
    
    // 加载模板
    template_manager.load_templates()?;
    
    // 获取所有模板
    let templates = template_manager.get_templates();
    println!("已加载 {} 个模板:", templates.len());
    
    for (id, template) in templates {
        println!("  - {}: {}", id, template.name);
        println!("    描述: {}", template.description.as_deref().unwrap_or("无"));
        println!("    版本: {}", template.version);
        println!("    参数数量: {}", template.parameters.len());
    }
    
    // 如果没有找到模板，退出
    if templates.is_empty() {
        println!("未找到模板，请确保模板目录存在并包含有效的模板文件。");
        return Ok(());
    }
    
    // 选择 PostgreSQL 到 Elasticsearch 模板
    let template_id = "postgres-to-elasticsearch";
    let template = template_manager.get_template(template_id)
        .ok_or_else(|| format!("模板 '{}' 不存在", template_id))?;
    
    println!("\n使用模板: {}", template.name);
    println!("参数:");
    
    for (name, param) in &template.parameters {
        println!("  - {}: {}", name, param.description.as_deref().unwrap_or("无"));
        if let Some(default) = &param.default {
            println!("    默认值: {:?}", default);
        }
        if param.required {
            println!("    必需: 是");
        }
    }
    
    // 创建参数值
    let param_values = TemplateParameterValues {
        values: serde_json::from_value(json!({
            "workflow_id": "my-postgres-to-es",
            "workflow_name": "My PostgreSQL to Elasticsearch Workflow",
            "postgres_database": "testdb",
            "postgres_username": "postgres",
            "postgres_password": "postgres",
            "postgres_table": "users",
            "extraction_mode": "incremental",
            "elasticsearch_index": "users"
        }))?,
    };
    
    println!("\n应用模板...");
    
    // 应用模板
    let workflow = template_manager.apply_template(template_id, &param_values)?;
    
    println!("工作流已创建: {}", workflow.id);
    println!("名称: {}", workflow.name);
    println!("描述: {}", workflow.description.as_deref().unwrap_or("无"));
    println!("版本: {}", workflow.version);
    println!("源数量: {}", workflow.sources.len());
    println!("转换数量: {}", workflow.transformations.len());
    println!("目标数量: {}", workflow.destinations.len());
    
    // 将工作流保存为 YAML
    let yaml = workflow.to_yaml()?;
    println!("\n生成的工作流 YAML:");
    println!("{}", yaml);
    
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
    
    println!("\n初始化执行器...");
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
    
    println!("工作流模板示例成功完成!");
    
    Ok(())
}
