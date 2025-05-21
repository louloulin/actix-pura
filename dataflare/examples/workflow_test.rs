//! 工作流执行测试
//! 
//! 用于验证工作流执行过程中的问题

use std::path::PathBuf;
use std::time::Instant;
use serde_json::json;
use tokio::runtime::Runtime;
use log::{info, error, warn, debug, LevelFilter};

use dataflare::{
    DataFlareConfig,
    runtime::workflow::{YamlWorkflowParser, WorkflowExecutor},
    error::Result,
};

fn main() -> Result<()> {
    // 初始化 DataFlare
    let mut config = DataFlareConfig::default();
    config.log_level = LevelFilter::Debug; // 设置更详细的日志级别
    dataflare::init(config)?;

    info!("开始工作流测试");

    // 创建 Tokio 运行时
    let rt = Runtime::new().expect("无法创建Tokio运行时");

    // 工作流文件路径
    let workflow_file = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/workflows/performance_test.yaml");
    
    info!("加载工作流: {:?}", workflow_file);

    // 解析工作流
    let parser = YamlWorkflowParser::new();
    let workflow = parser.parse_file(&workflow_file)?;
    
    info!("工作流解析成功: {}", workflow.id);
    debug!("工作流配置: {:#?}", workflow);

    // 执行工作流
    info!("开始执行工作流");
    let start = Instant::now();
    
    // 创建执行器并运行工作流
    let executor = WorkflowExecutor::new();
    rt.block_on(async {
        match executor.execute(&workflow).await {
            Ok(_) => {
                info!("工作流执行成功, 耗时: {:?}", start.elapsed());
                Ok(())
            },
            Err(e) => {
                error!("工作流执行失败: {}", e);
                Err(e)
            }
        }
    })?;

    info!("工作流测试完成");
    Ok(())
} 