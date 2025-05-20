use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;

use actix::prelude::*;
use dataflare_core::error::Result;
use dataflare_runtime::actor::{
    MessageBus, DataFlareMessage, ActorId,
    MessageRouter, RouterConfig, RouterActor,
    ActorPool, PoolBuilder, PoolStrategy, PoolConfig, StopWorker
};

// 简单消息类型用于基准测试
#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
struct BenchMessage(usize);

// 测试用的Actor - 只记录收到的消息数量
struct BenchActor {
    id: String,
    count: usize,
}

impl Actor for BenchActor {
    type Context = Context<Self>;
}

impl Handler<BenchMessage> for BenchActor {
    type Result = ();
    
    fn handle(&mut self, msg: BenchMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.count += msg.0;
    }
}

impl Handler<DataFlareMessage> for BenchActor {
    type Result = ();
    
    fn handle(&mut self, msg: DataFlareMessage, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(payload) = msg.downcast::<BenchMessage>() {
            self.count += payload.0;
        }
    }
}

impl Handler<StopWorker> for BenchActor {
    type Result = ();
    
    fn handle(&mut self, _: StopWorker, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

// 工作者Actor，用于池测试
struct WorkerActor {
    id: usize,
    processed: usize,
}

impl Actor for WorkerActor {
    type Context = Context<Self>;
}

impl Handler<BenchMessage> for WorkerActor {
    type Result = ();
    
    fn handle(&mut self, msg: BenchMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.processed += msg.0;
        
        // 模拟一些处理工作
        let mut sum = 0;
        for i in 0..100 {
            sum += i;
        }
        black_box(sum);
    }
}

impl Handler<StopWorker> for WorkerActor {
    type Result = ();
    
    fn handle(&mut self, _: StopWorker, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

// 消息总线基准测试
fn bench_message_bus(c: &mut Criterion) {
    let mut group = c.benchmark_group("MessageBus");
    
    for num_actors in [5, 10, 20].iter() {
        for num_messages in [100, 1000, 10000].iter() {
            group.bench_with_input(
                BenchmarkId::new(format!("actors_{}_msgs_{}", num_actors, num_messages), 0),
                &(*num_actors, *num_messages),
                |b, &(num_actors, num_messages)| {
                    b.iter(|| {
                        let sys = System::new();
                        sys.block_on(async {
                            let message_bus = Arc::new(MessageBus::new());
                            
                            // 创建和注册Actors
                            let mut actors = vec![];
                            for i in 0..num_actors {
                                let actor = BenchActor {
                                    id: format!("actor_{}", i),
                                    count: 0,
                                };
                                let addr = actor.start();
                                actors.push(addr.clone());
                                message_bus.register(format!("actor_{}", i), addr).unwrap();
                            }
                            
                            // 发送消息
                            for _ in 0..num_messages {
                                for i in 0..num_actors {
                                    message_bus.send(format!("actor_{}", i), BenchMessage(1)).unwrap();
                                }
                            }
                            
                            // 清理
                            for addr in actors {
                                addr.do_send(StopWorker);
                            }
                        });
                    })
                },
            );
        }
    }
    group.finish();
}

// 消息路由器基准测试
fn bench_router(c: &mut Criterion) {
    let mut group = c.benchmark_group("Router");
    
    for num_messages in [100, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new(format!("msgs_{}", num_messages), 0),
            num_messages,
            |b, &num_messages| {
                b.iter(|| {
                    let sys = System::new();
                    sys.block_on(async {
                        let message_bus = Arc::new(MessageBus::new());
                        let router_config = RouterConfig {
                            debug_logging: false,
                            collect_metrics: true,
                            max_retries: 1,
                            retry_delay_ms: 10,
                        };
                        
                        // 创建和启动路由器
                        let router = RouterActor::new(message_bus.clone(), router_config);
                        let router_addr = router.start();
                        
                        // 创建和注册Actors
                        let actor1 = BenchActor {
                            id: "bench_actor".to_string(),
                            count: 0,
                        };
                        let addr1 = actor1.start();
                        message_bus.register("bench_actor", addr1.clone()).unwrap();
                        
                        // 通过路由器发送消息
                        for _ in 0..num_messages {
                            router_addr.do_send(dataflare_runtime::actor::router::RouteMessage {
                                sender: "router".into(),
                                target: "bench_actor".into(),
                                message: BenchMessage(1),
                            });
                        }
                        
                        // 清理
                        addr1.do_send(StopWorker);
                        router_addr.do_send(StopWorker);
                    });
                })
            },
        );
    }
    group.finish();
}

// Actor池基准测试
fn bench_actor_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("ActorPool");
    
    for pool_size in [2, 4, 8].iter() {
        for num_messages in [100, 1000, 5000].iter() {
            group.bench_with_input(
                BenchmarkId::new(format!("size_{}_msgs_{}", pool_size, num_messages), 0),
                &(*pool_size, *num_messages),
                |b, &(pool_size, num_messages)| {
                    b.iter(|| {
                        let sys = System::new();
                        sys.block_on(async {
                            // 创建Actor池
                            let (pool, monitor_addr) = PoolBuilder::<WorkerActor>::new()
                                .with_size(pool_size)
                                .with_strategy(PoolStrategy::RoundRobin)
                                .with_factory(|| WorkerActor { 
                                    id: 0, 
                                    processed: 0,
                                })
                                .build()
                                .unwrap();
                            
                            // 发送消息到池
                            {
                                let pool = pool.lock().unwrap();
                                for _ in 0..num_messages {
                                    pool.send(BenchMessage(1)).unwrap();
                                }
                            }
                            
                            // 等待所有消息被处理
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            
                            // 清理
                            monitor_addr.do_send(StopWorker);
                        });
                    })
                },
            );
        }
    }
    group.finish();
}

// 嵌套vs扁平化通信基准测试
fn bench_nested_vs_flat(c: &mut Criterion) {
    let mut group = c.benchmark_group("NestedVsFlat");
    
    for num_messages in [100, 1000].iter() {
        // 测试嵌套通信
        group.bench_with_input(
            BenchmarkId::new(format!("nested_msgs_{}", num_messages), 0),
            num_messages,
            |b, &num_messages| {
                b.iter(|| {
                    let sys = System::new();
                    sys.block_on(async {
                        // 创建嵌套的Actor链 (Supervisor -> Workflow -> Processor -> Destination)
                        let destination = BenchActor {
                            id: "destination".to_string(),
                            count: 0,
                        };
                        let dest_addr = destination.start();
                        
                        let processor = BenchActor {
                            id: "processor".to_string(),
                            count: 0,
                        };
                        let proc_addr = processor.start();
                        
                        let workflow = BenchActor {
                            id: "workflow".to_string(),
                            count: 0,
                        };
                        let wf_addr = workflow.start();
                        
                        let supervisor = BenchActor {
                            id: "supervisor".to_string(),
                            count: 0,
                        };
                        let sup_addr = supervisor.start();
                        
                        // 通过层层嵌套发送消息
                        for _ in 0..num_messages {
                            // 模拟消息从supervisor经过每一层传递到destination
                            sup_addr.do_send(BenchMessage(0));
                            wf_addr.do_send(BenchMessage(0));
                            proc_addr.do_send(BenchMessage(0));
                            dest_addr.do_send(BenchMessage(1));
                        }
                        
                        // 清理
                        sup_addr.do_send(StopWorker);
                        wf_addr.do_send(StopWorker);
                        proc_addr.do_send(StopWorker);
                        dest_addr.do_send(StopWorker);
                    });
                })
            },
        );
        
        // 测试扁平化通信
        group.bench_with_input(
            BenchmarkId::new(format!("flat_msgs_{}", num_messages), 0),
            num_messages,
            |b, &num_messages| {
                b.iter(|| {
                    let sys = System::new();
                    sys.block_on(async {
                        let message_bus = Arc::new(MessageBus::new());
                        
                        // 创建相同的Actor，但使用扁平化通信
                        let destination = BenchActor {
                            id: "destination".to_string(),
                            count: 0,
                        };
                        let dest_addr = destination.start();
                        message_bus.register("destination", dest_addr.clone()).unwrap();
                        
                        let processor = BenchActor {
                            id: "processor".to_string(),
                            count: 0,
                        };
                        let proc_addr = processor.start();
                        message_bus.register("processor", proc_addr.clone()).unwrap();
                        
                        let workflow = BenchActor {
                            id: "workflow".to_string(),
                            count: 0,
                        };
                        let wf_addr = workflow.start();
                        message_bus.register("workflow", wf_addr.clone()).unwrap();
                        
                        let supervisor = BenchActor {
                            id: "supervisor".to_string(),
                            count: 0,
                        };
                        let sup_addr = supervisor.start();
                        message_bus.register("supervisor", sup_addr.clone()).unwrap();
                        
                        // 直接从supervisor发送消息到destination
                        for _ in 0..num_messages {
                            message_bus.send("destination", BenchMessage(1)).unwrap();
                        }
                        
                        // 清理
                        sup_addr.do_send(StopWorker);
                        wf_addr.do_send(StopWorker);
                        proc_addr.do_send(StopWorker);
                        dest_addr.do_send(StopWorker);
                    });
                })
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_message_bus,
    bench_router,
    bench_actor_pool,
    bench_nested_vs_flat,
);
criterion_main!(benches); 