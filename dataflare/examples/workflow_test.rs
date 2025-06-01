//! 工作流Actor测试示例
//!
//! 这个示例演示了如何使用WorkflowActor创建和运行数据工作流

use std::path::PathBuf;
use std::time::Instant;
use actix::prelude::*;
use log::{info, error, warn, LevelFilter};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use std::sync::Arc;
use std::sync::Mutex;

// 导入必要的DataFlare组件
use dataflare_core::error::Result;
use dataflare_core::message::WorkflowPhase;
use dataflare_connector::csv::{CsvSourceConnector, CsvDestinationConnector};
use dataflare_connector::source::SourceConnector;
use dataflare_connector::destination::DestinationConnector;

use dataflare_runtime::actor::{
    Initialize, GetStatus, SourceActor, DestinationActor, 
    WorkflowActor, TaskActor, TaskKind, ConnectToTask,
    RegisterTask, RegisterSourceActor, RegisterDestinationActor, ActorStatus,
    SubscribeProgress
};
use dataflare_runtime::actor::workflow::{StartWorkflow, StopWorkflow, GetWorkflowStats};
use dataflare_runtime::actor::task::AddDownstream;

/// 工作流状态枚举
enum WorkflowStatus {
    Completed,
    Failed,
    CompletedWithErrors,
}

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

    // 配置文件路径 - 使用绝对路径以确保正确
    let current_dir = std::env::current_dir()?;
    let input_path = current_dir.join("dataflare/examples/data/large_test.csv");
    let output_path = current_dir.join("dataflare/examples/data/workflow_output.csv");

    info!("使用输入文件: {}", input_path.display());
    info!("输出将写入: {}", output_path.display());

    // 检查文件是否存在
    if !input_path.exists() {
        error!("输入文件不存在: {}", input_path.display());
        return Ok(());
    }
    
    info!("输入文件存在，大小为: {} 字节", std::fs::metadata(&input_path)?.len());

    // 创建工作流组件
    info!("创建Actor系统组件");
    
    // 创建WorkflowActor
    let workflow_actor = WorkflowActor::new("test-workflow").start();

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

    // 初始化工作流配置
    info!("初始化工作流");
    workflow_actor.send(Initialize {
        workflow_id: "test-workflow".to_string(),
        config: workflow_config.clone()
    }).await??;

    // 创建Source Actor，直接使用配置
    let source_config = workflow_config["sources"]["csv-source"]["config"].clone();
    let source = CsvSourceConnector::new(source_config);
    let source_actor = SourceActor::new("csv-source", Box::new(source)).start();
    
    // 创建Destination Actor，直接使用配置
    let dest_config = workflow_config["destinations"]["csv-destination"]["config"].clone();
    let dest = CsvDestinationConnector::new(dest_config);
    let dest_actor = DestinationActor::new("csv-destination", Box::new(dest)).start();

    // 创建源任务
    let source_task = TaskActor::new("csv-source-task", TaskKind::Source).start();

    // 创建目标任务
    let dest_task = TaskActor::new("csv-dest-task", TaskKind::Destination).start();

    // 将任务注册到工作流
    workflow_actor.do_send(RegisterTask {
        task_id: "csv-source-task".to_string(),
        task_addr: source_task.clone(),
        task_kind: TaskKind::Source,
    });

    workflow_actor.do_send(RegisterTask {
        task_id: "csv-dest-task".to_string(),
        task_addr: dest_task.clone(),
        task_kind: TaskKind::Destination,
    });

    // 注册SourceActor到工作流
    workflow_actor.do_send(RegisterSourceActor {
        source_id: "csv-source".to_string(),
        source_addr: source_actor.clone(),
    });
    
    // 注册DestinationActor到工作流
    workflow_actor.do_send(RegisterDestinationActor {
        destination_id: "csv-destination".to_string(),
        destination_addr: dest_actor.clone(),
    });

    // 关联SourceActor和源TaskActor
    source_actor.do_send(ConnectToTask {
        task_addr: source_task.clone(),
        task_id: "csv-source-task".to_string(),
    });
    
    // 关联DestinationActor和目标TaskActor
    dest_actor.do_send(ConnectToTask {
        task_addr: dest_task.clone(),
        task_id: "csv-dest-task".to_string(),
    });

    // 构建数据流图：source -> destination
    source_task.do_send(AddDownstream {
        actor_addr: dest_task.clone(),
    });

    // 创建完成通知通道
    let (tx, rx) = oneshot::channel::<WorkflowStatus>();
    
    // 创建一个共享的sender，可以在多处克隆使用
    let status_sender = Arc::new(Mutex::new(Some(tx)));

    // 创建一个专门接收WorkflowProgress消息的Actor
    struct ProgressMonitor {
        status_sender: Arc<Mutex<Option<oneshot::Sender<WorkflowStatus>>>>,
    }

    impl Actor for ProgressMonitor {
        type Context = Context<Self>;
    }

    impl Handler<dataflare_core::message::WorkflowProgress> for ProgressMonitor {
        type Result = ();

        fn handle(&mut self, progress: dataflare_core::message::WorkflowProgress, _: &mut Self::Context) -> Self::Result {
            // 检查工作流是否完成
            match progress.phase {
                WorkflowPhase::Completed => {
                    info!("工作流完成事件: {}", progress.message);
                    if let Some(sender) = self.status_sender.lock().unwrap().take() {
                        let _ = sender.send(WorkflowStatus::Completed);
                    }
                },
                WorkflowPhase::Error => {
                    if progress.progress >= 0.99 {
                        info!("工作流部分完成: {}", progress.message);
                        if let Some(sender) = self.status_sender.lock().unwrap().take() {
                            let _ = sender.send(WorkflowStatus::CompletedWithErrors);
                        }
                    }
                },
                _ => {
                    // 其他进度更新
                    info!("工作流进度: {:.1}%, {}", progress.progress * 100.0, progress.message);
                }
            }
        }
    }

    // 创建进度监视器Actor
    let progress_monitor = ProgressMonitor {
        status_sender: status_sender.clone(),
    }.start();

    // 订阅工作流进度
    workflow_actor.do_send(SubscribeProgress {
        workflow_id: "test-workflow".to_string(),
        recipient: progress_monitor.recipient(),
    });

    // 检查初始状态
    let status = workflow_actor.send(GetStatus).await??;
    info!("初始工作流状态: {:?}", status);
    
    // 开始计时
    let start_time = Instant::now();

    // 启动工作流
    info!("启动工作流");
    let start_result = workflow_actor.send(StartWorkflow{}).await??;

    // 再次检查状态
    let status = workflow_actor.send(GetStatus).await??;
    info!("启动后工作流状态: {:?}", status);

    // 等待工作流完成或超时
    info!("等待工作流处理...");
    match tokio::time::timeout(tokio::time::Duration::from_secs(60), rx).await {
        Ok(status) => {
            match status {
                Ok(WorkflowStatus::Completed) => info!("工作流成功完成"),
                Ok(WorkflowStatus::CompletedWithErrors) => info!("工作流完成但有错误"),
                Ok(WorkflowStatus::Failed) => error!("工作流执行失败"),
                Err(_) => info!("工作流状态通知失败"),
            }
        },
        Err(_) => warn!("等待工作流完成超时，可能仍在处理中"),
    };

    // 获取工作流统计信息
    let stats = workflow_actor.send(GetWorkflowStats{}).await??;
    info!("工作流统计: 处理记录数 {}, 执行时间 {} ms", 
         stats.records_processed, stats.execution_time_ms.unwrap_or(0));

    let elapsed = start_time.elapsed();
    info!("工作流执行完毕，耗时 {:.2} 秒", elapsed.as_secs_f64());

    // 检查输出文件是否存在
    if output_path.exists() {
        info!("成功创建输出文件: {}", output_path.display());
        info!("输出文件大小: {} 字节", std::fs::metadata(&output_path)?.len());
    } else {
        error!("输出文件未创建: {}", output_path.display());
    }

    // 关闭Actor系统
    System::current().stop();
    info!("测试完成");

    Ok(())
}

 