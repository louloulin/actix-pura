//! 工作流Actor测试示例
//!
//! 这个示例演示了如何使用WorkflowActor创建和运行数据工作流

use std::path::PathBuf;
use std::time::Instant;
use actix::prelude::*;
use log::{info, error, LevelFilter};
use serde_json::{json, Value};

// 导入必要的DataFlare组件
use dataflare_core::error::Result;
use dataflare_connector::csv::{CsvSourceConnector, CsvDestinationConnector};
use dataflare_connector::source::SourceConnector;
use dataflare_connector::destination::DestinationConnector;

use dataflare_runtime::actor::{
    Initialize, GetStatus, SourceActor, DestinationActor, 
    WorkflowActor, TaskActor, TaskKind
};
use dataflare_runtime::actor::workflow::{StartWorkflow, StopWorkflow, GetWorkflowStats};
use dataflare_runtime::actor::task::AddDownstream;

// 设置日志
fn setup_logger() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .init();
    Ok(())
}

#[actix_rt::main]
async fn main() -> Result<()> {
    // 初始化日志
    setup_logger()?;

    info!("开始工作流测试");

    // 配置文件路径 - 使用绝对路径确保访问
    let input_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/large_test.csv");
    let output_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/workflow_output.csv");

    info!("使用输入文件: {}", input_path.display());
    info!("输出将写入: {}", output_path.display());

    // 检查文件是否存在
    if !input_path.exists() {
        error!("输入文件不存在: {}", input_path.display());
        return Ok(());
    }
    
    info!("输入文件存在，大小为: {} 字节", std::fs::metadata(&input_path)?.len());

    // 创建源连接器 - 正确配置文件路径
    let source = CsvSourceConnector::new(json!({
        "file_path": input_path.to_str().unwrap(),
        "delimiter": ",",
        "has_header": true
    }));
    
    // 创建目标连接器 - 正确配置文件路径
    let dest = CsvDestinationConnector::new(json!({
        "file_path": output_path.to_str().unwrap(),
        "delimiter": ",", 
        "write_header": true
    }));

    // 创建工作流组件
    info!("创建Actor系统组件");
    
    // 创建Source Actor
    let source_actor = SourceActor::new("csv-source", Box::new(source)).start();
    
    // 创建目标Actor
    let dest_actor = DestinationActor::new("csv-dest", Box::new(dest)).start();

    // 创建源任务
    let source_task = TaskActor::new("csv-source-task", TaskKind::Source).start();
    
    // 创建目标任务
    let dest_task = TaskActor::new("csv-dest-task", TaskKind::Destination).start();
    
    // 设置任务之间的关系
    let source_task_addr = source_task.clone();
    let dest_task_addr = dest_task.clone();
    
    // 添加下游关系 - 将目标任务添加为源任务的下游
    source_task.do_send(AddDownstream {
        actor_addr: dest_task_addr,
    });
    
    source_task.send(Initialize {
        workflow_id: "test-workflow".to_string(),
        config: json!({
            "id": "csv-source-task",
            "name": "CSV Source Task",
            "task_type": "source",
            "config": {
                "source": "csv"
            }
        })
    }).await??;
    dest_task.send(Initialize {
        workflow_id: "test-workflow".to_string(),
        config: json!({
            "id": "csv-dest-task",
            "name": "CSV Destination Task",
            "task_type": "destination",
            "config": {
                "destination": "csv"
            }
        })
    }).await??;
    
    // 创建工作流Actor
    info!("创建工作流Actor");
    let workflow_actor = WorkflowActor::new("test-workflow".to_string()).start();
    
    // 初始化工作流
    let workflow_config = json!({
        "id": "test-workflow",
        "name": "CSV处理工作流",
        "description": "简单的CSV处理测试",
        "version": "1.0.0",
        "created_at": chrono::Utc::now().to_rfc3339(),
        "updated_at": chrono::Utc::now().to_rfc3339(),
        "sources": {
            "csv-source": {
                "type": "csv",
                "config": {
                    "file_path": input_path.to_str().unwrap(),
                    "delimiter": ",",
                    "has_header": true
                }
            }
        },
        "destinations": {
            "csv-destination": {
                "type": "csv",
                "inputs": ["csv-source"],
                "config": {
                    "file_path": output_path.to_str().unwrap(),
                    "delimiter": ",",
                    "write_header": true
                }
            }
        },
        "transformations": {},
        "metadata": {}
    });

    info!("初始化工作流");
    workflow_actor.send(Initialize {
        workflow_id: "test-workflow".to_string(),
        config: workflow_config,
    }).await??;

    // 检查初始状态
    let status = workflow_actor.send(GetStatus).await??;
    info!("初始工作流状态: {:?}", status);
    
    // 开始计时
    let start_time = Instant::now();

    // 启动工作流
    info!("启动工作流");
    workflow_actor.send(StartWorkflow{}).await??;

    // 再次检查状态
    let status = workflow_actor.send(GetStatus).await??;
    info!("启动后工作流状态: {:?}", status);

    // 等待一段时间让工作流完成
    info!("等待工作流处理...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // 获取工作流统计信息
    let stats = workflow_actor.send(GetWorkflowStats{}).await??;
    info!("工作流统计: {:?}", stats);

    // 停止工作流
    info!("停止工作流");
    workflow_actor.send(StopWorkflow{}).await??;

    let elapsed = start_time.elapsed();
    info!("工作流执行完毕，耗时 {:.2} 秒", elapsed.as_secs_f64());

    // 检查输出文件是否存在
    if output_path.exists() {
        info!("成功创建输出文件: {}", output_path.display());
    } else {
        error!("输出文件未创建: {}", output_path.display());
    }

    // 关闭Actor系统
    System::current().stop();
    info!("测试完成");

    Ok(())
}

