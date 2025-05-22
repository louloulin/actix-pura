use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use actix::prelude::*;
use dataflare_runtime::actor::MessageBus;
use dataflare_runtime::actor::message_bus::{DataFlareMessage, ActorId, MessageHandler};
use dataflare_runtime::actor::router::{RouterConfig, RouterActor, RouteMessage, GetRouterStats};
use dataflare_runtime::actor::pool::{ActorPool, PoolBuilder, PoolStrategy, PoolConfig, StopWorker};

// 工作负载消息
#[derive(Debug, Clone, Message)]
#[rtype(result = "u64")]
struct ProcessWork {
    id: usize,
    data_size: usize,
}

// 工作完成通知
#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
struct WorkCompleted {
    id: usize,
    result: usize,
    processing_time_ms: u64,
}

// 获取统计信息
#[derive(Debug, Message)]
#[rtype(result = "WorkerStats")]
struct GetStats;

// 工作者统计
#[derive(Debug, Clone)]
struct WorkerStats {
    processed_count: usize,
    total_processing_time_ms: u64,
    processing_results: Vec<usize>,
}

// 工作者Actor
struct WorkerActor {
    id: String,
    stats: WorkerStats,
}

impl Actor for WorkerActor {
    type Context = Context<Self>;
}

impl Handler<ProcessWork> for WorkerActor {
    type Result = MessageResult<ProcessWork>;

    fn handle(&mut self, msg: ProcessWork, _ctx: &mut Self::Context) -> Self::Result {
        let start = Instant::now();

        // 模拟计算工作
        let mut result = 0u64;
        for i in 0..msg.data_size {
            result = result.wrapping_add(i as u64);
        }

        let elapsed = start.elapsed();

        // 更新统计信息
        self.stats.processed_count += 1;
        self.stats.total_processing_time_ms += elapsed.as_millis() as u64;
        self.stats.processing_results.push(result as usize);

        MessageResult(result)
    }
}

impl Handler<GetStats> for WorkerActor {
    type Result = MessageResult<GetStats>;

    fn handle(&mut self, _: GetStats, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.stats.clone())
    }
}

impl Handler<DataFlareMessage> for WorkerActor {
    type Result = ();

    fn handle(&mut self, msg: DataFlareMessage, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(payload) = msg.downcast::<ProcessWork>() {
            let start = Instant::now();

            // 模拟计算工作
            let mut result = 0u64;
            for i in 0..payload.data_size {
                result = result.wrapping_add(i as u64);
            }

            let elapsed = start.elapsed();

            // 更新统计信息
            self.stats.processed_count += 1;
            self.stats.total_processing_time_ms += elapsed.as_millis() as u64;
            self.stats.processing_results.push(result as usize);
        }
    }
}

impl Handler<StopWorker> for WorkerActor {
    type Result = ();

    fn handle(&mut self, _: StopWorker, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

// 协调者Actor
struct CoordinatorActor {
    workers: Vec<Addr<WorkerActor>>,
    message_bus: Arc<MessageBus>,
    completed_work: usize,
    expected_work: usize,
    start_time: Option<Instant>,
    results: Vec<usize>,
}

impl CoordinatorActor {
    fn new(workers: Vec<Addr<WorkerActor>>, message_bus: Arc<MessageBus>) -> Self {
        Self {
            workers,
            message_bus,
            completed_work: 0,
            expected_work: 0,
            start_time: None,
            results: Vec::new(),
        }
    }
}

impl Actor for CoordinatorActor {
    type Context = Context<Self>;
}

// 开始处理工作
#[derive(Message)]
#[rtype(result = "()")]
struct StartProcessing {
    work_items: usize,
    data_size: usize,
    use_message_bus: bool,
}

impl Handler<StartProcessing> for CoordinatorActor {
    type Result = ();

    fn handle(&mut self, msg: StartProcessing, ctx: &mut Self::Context) -> Self::Result {
        self.expected_work = msg.work_items;
        self.completed_work = 0;
        self.results.clear();
        self.start_time = Some(Instant::now());

        let worker_count = self.workers.len();

        // 分发工作
        for i in 0..msg.work_items {
            let worker_idx = i % worker_count;
            let work = ProcessWork {
                id: i,
                data_size: msg.data_size,
            };

            if msg.use_message_bus {
                // 通过消息总线发送
                let worker_id = format!("worker_{}", worker_idx);
                self.message_bus.send(worker_id, work).unwrap();
            } else {
                // 直接发送
                let worker = &self.workers[worker_idx];
                worker.do_send(work);
            }
        }
    }
}

impl Handler<WorkCompleted> for CoordinatorActor {
    type Result = ();

    fn handle(&mut self, msg: WorkCompleted, _ctx: &mut Self::Context) -> Self::Result {
        self.completed_work += 1;
        self.results.push(msg.result);

        // 检查是否所有工作都已完成
        if self.completed_work >= self.expected_work {
            if let Some(start_time) = self.start_time {
                let elapsed = start_time.elapsed();
                println!(
                    "All {} work items completed in {:?} (avg: {:?} per item)",
                    self.expected_work,
                    elapsed,
                    elapsed / self.expected_work as u32
                );
            }
        }
    }
}

impl Handler<StopWorker> for CoordinatorActor {
    type Result = ();

    fn handle(&mut self, _: StopWorker, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

// 高负载测试 - 直接通信 vs 消息总线通信
#[actix::test]
async fn test_high_load_comparison() {
    const NUM_WORKERS: usize = 8;
    const WORK_ITEMS: usize = 1000;
    const DATA_SIZE: usize = 10000;

    // 创建消息总线
    let message_bus = Arc::new(MessageBus::new());

    // 创建工作者
    let mut worker_addrs = Vec::with_capacity(NUM_WORKERS);
    for i in 0..NUM_WORKERS {
        let worker = WorkerActor {
            id: format!("worker_{}", i),
            stats: WorkerStats {
                processed_count: 0,
                total_processing_time_ms: 0,
                processing_results: Vec::new(),
            },
        };
        let addr = worker.start();

        // 注册到消息总线
        message_bus.register(format!("worker_{}", i), addr.clone()).unwrap();

        worker_addrs.push(addr);
    }

    // 创建协调者
    let coordinator = CoordinatorActor::new(worker_addrs.clone(), message_bus.clone());
    let coord_addr = coordinator.start();

    // 测试1: 直接通信
    println!("\n--- Testing direct communication ---");
    let start = Instant::now();

    coord_addr.send(StartProcessing {
        work_items: WORK_ITEMS,
        data_size: DATA_SIZE,
        use_message_bus: false,
    }).await.unwrap();

    // 等待所有工作完成
    tokio::time::sleep(Duration::from_millis(500)).await;

    let direct_elapsed = start.elapsed();
    println!("Direct communication completed in {:?}", direct_elapsed);

    // 收集统计信息
    let mut direct_total_time = 0;
    for (i, worker) in worker_addrs.iter().enumerate() {
        let stats = worker.send(GetStats).await.unwrap();
        println!(
            "Worker {} processed {} items in {}ms",
            i, stats.processed_count, stats.total_processing_time_ms
        );
        direct_total_time += stats.total_processing_time_ms;
    }

    // 重置工作者统计信息
    for worker in &worker_addrs {
        worker.do_send(GetStats);
    }

    // 测试2: 消息总线通信
    println!("\n--- Testing message bus communication ---");
    let start = Instant::now();

    coord_addr.send(StartProcessing {
        work_items: WORK_ITEMS,
        data_size: DATA_SIZE,
        use_message_bus: true,
    }).await.unwrap();

    // 等待所有工作完成
    tokio::time::sleep(Duration::from_millis(500)).await;

    let bus_elapsed = start.elapsed();
    println!("Message bus communication completed in {:?}", bus_elapsed);

    // 收集统计信息
    let mut bus_total_time = 0;
    for (i, worker) in worker_addrs.iter().enumerate() {
        let stats = worker.send(GetStats).await.unwrap();
        println!(
            "Worker {} processed {} items in {}ms",
            i, stats.processed_count, stats.total_processing_time_ms
        );
        bus_total_time += stats.total_processing_time_ms;
    }

    // 比较结果
    println!("\n--- Performance Comparison ---");
    println!("Direct communication total time: {:?}", direct_elapsed);
    println!("Message bus communication total time: {:?}", bus_elapsed);
    println!(
        "Ratio (bus/direct): {:.2}",
        bus_elapsed.as_millis() as f64 / direct_elapsed.as_millis() as f64
    );

    // 清理
    coord_addr.do_send(StopWorker);
    for worker in worker_addrs {
        worker.do_send(StopWorker);
    }
}

// Actor池负载测试
#[actix::test]
async fn test_actor_pool_load() {
    const POOL_SIZES: [usize; 3] = [2, 4, 8];
    const WORK_ITEMS: usize = 1000;
    const DATA_SIZE: usize = 10000;

    println!("\n--- Testing ActorPool Performance ---");

    for &pool_size in &POOL_SIZES {
        // 创建Actor池
        let (pool, monitor_addr) = PoolBuilder::<WorkerActor>::new()
            .with_size(pool_size)
            .with_strategy(PoolStrategy::RoundRobin)
            .with_factory(|| WorkerActor {
                id: format!("pooled_worker"),
                stats: WorkerStats {
                    processed_count: 0,
                    total_processing_time_ms: 0,
                    processing_results: Vec::new(),
                },
            })
            .build()
            .unwrap();

        println!("\nTesting with pool size: {}", pool_size);
        let start = Instant::now();

        // 发送工作到池
        {
            let pool = pool.lock().unwrap();
            for i in 0..WORK_ITEMS {
                pool.send(ProcessWork {
                    id: i,
                    data_size: DATA_SIZE,
                }).unwrap();
            }
        }

        // 等待所有工作完成 - 池中的Actor共享工作
        tokio::time::sleep(Duration::from_millis(1000)).await;

        let elapsed = start.elapsed();
        println!(
            "Pool size {} processed {} items in {:?} (avg: {:?} per item)",
            pool_size, WORK_ITEMS, elapsed, elapsed / WORK_ITEMS as u32
        );

        // 清理
        monitor_addr.do_send(StopWorker);
    }
}

// 消息路由器压力测试
#[actix::test]
async fn test_router_under_pressure() {
    const NUM_WORKERS: usize = 4;
    const WORK_ITEMS: usize = 1000;
    const DATA_SIZE: usize = 5000;

    // 创建消息总线
    let message_bus = Arc::new(MessageBus::new());

    // 创建路由器
    let router_config = RouterConfig {
        debug_logging: false,
        collect_metrics: true,
        max_retries: 2,
        retry_delay_ms: 50,
    };
    let router = RouterActor::new(message_bus.clone(), router_config);
    let router_addr = router.start();

    println!("\n--- Testing Router Under Pressure ---");

    // 创建工作者
    let mut worker_addrs = Vec::with_capacity(NUM_WORKERS);
    for i in 0..NUM_WORKERS {
        let worker = WorkerActor {
            id: format!("worker_{}", i),
            stats: WorkerStats {
                processed_count: 0,
                total_processing_time_ms: 0,
                processing_results: Vec::new(),
            },
        };
        let addr = worker.start();

        // 注册到消息总线
        message_bus.register(format!("worker_{}", i), addr.clone()).unwrap();

        worker_addrs.push(addr);
    }

    // 使用路由器发送消息
    let start = Instant::now();

    for i in 0..WORK_ITEMS {
        let worker_idx = i % NUM_WORKERS;
        let target = format!("worker_{}", worker_idx);

        router_addr.do_send(RouteMessage {
            sender: "test".into(),
            target: target.into(),
            message: ProcessWork {
                id: i,
                data_size: DATA_SIZE,
            },
        });
    }

    // 等待处理完成
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let elapsed = start.elapsed();

    // 获取路由器统计信息
    let stats = router_addr.send(GetRouterStats).await.unwrap();

    println!("Router processed {} items in {:?}", WORK_ITEMS, elapsed);
    println!("Router stats: total={}, successful={}, failed={}, retried={}",
             stats.total_messages, stats.successful_messages, stats.failed_messages, stats.retried_messages);

    // 收集工作者统计
    let mut total_processed = 0;
    for (i, worker) in worker_addrs.iter().enumerate() {
        let stats = worker.send(GetStats).await.unwrap();
        println!(
            "Worker {} processed {} items in {}ms",
            i, stats.processed_count, stats.total_processing_time_ms
        );
        total_processed += stats.processed_count;
    }

    println!("Total processed: {}/{}", total_processed, WORK_ITEMS);

    // 清理
    router_addr.do_send(StopWorker);
    for worker in worker_addrs {
        worker.do_send(StopWorker);
    }
}