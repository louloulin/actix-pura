//! Integration tests for the Actor communication optimizations

use std::sync::{Arc, Mutex};
use std::time::Duration;

use actix::prelude::*;
use dataflare_runtime::actor::MessageBus;
use dataflare_runtime::actor::message_bus::{DataFlareMessage, ActorId, MessageHandler};
use dataflare_runtime::actor::router::{RouterConfig, RouterActor, RouteMessage, GetRouterStats, ResetRouterStats};
use dataflare_runtime::actor::pool::{ActorPool, PoolBuilder, PoolStrategy, PoolConfig, StopWorker};

// Test actor that counts received messages
struct TestActor {
    id: String,
    received_messages: Vec<String>,
}

impl Actor for TestActor {
    type Context = Context<Self>;
}

impl MessageHandler for TestActor {
    fn handle_message(&mut self, msg: DataFlareMessage, _ctx: &mut Self::Context) {
        if let Some(payload) = msg.downcast::<String>() {
            self.received_messages.push(payload.clone());
        }
    }
}

// Test message for direct actor communication
#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
struct TestMessage(String);

impl Handler<TestMessage> for TestActor {
    type Result = ();

    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.received_messages.push(msg.0);
    }
}

// Implement DataFlareMessage handler for TestActor
impl Handler<DataFlareMessage> for TestActor {
    type Result = ();

    fn handle(&mut self, msg: DataFlareMessage, ctx: &mut Self::Context) -> Self::Result {
        self.handle_message(msg, ctx);
    }
}

// Implement Handler<StopWorker> for TestActor
impl Handler<StopWorker> for TestActor {
    type Result = ();

    fn handle(&mut self, _: StopWorker, ctx: &mut Self::Context) {
        ctx.stop();
    }
}

// Worker actor for pool testing
struct WorkerActor {
    id: usize,
    processed_messages: Mutex<Vec<String>>,
}

impl Actor for WorkerActor {
    type Context = Context<Self>;
}

impl Handler<TestMessage> for WorkerActor {
    type Result = ();

    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        if let Ok(mut processed) = self.processed_messages.lock() {
            processed.push(msg.0);
        }
    }
}

// Implement Handler<StopWorker> for WorkerActor
impl Handler<StopWorker> for WorkerActor {
    type Result = ();

    fn handle(&mut self, _: StopWorker, ctx: &mut Self::Context) {
        ctx.stop();
    }
}

// Message to check actor state
#[derive(Message)]
#[rtype(result = "Vec<String>")]
struct GetMessages;

impl Handler<GetMessages> for TestActor {
    type Result = MessageResult<GetMessages>;

    fn handle(&mut self, _: GetMessages, _: &mut Self::Context) -> Self::Result {
        MessageResult(self.received_messages.clone())
    }
}

#[actix::test]
async fn test_message_bus_communication() {
    // Create the message bus
    let message_bus = Arc::new(MessageBus::new());

    // Create and register actors
    let actor1 = TestActor {
        id: "actor1".to_string(),
        received_messages: Vec::new(),
    };

    let actor2 = TestActor {
        id: "actor2".to_string(),
        received_messages: Vec::new(),
    };

    let addr1 = actor1.start();
    let addr2 = actor2.start();

    message_bus.register("actor1", addr1.clone()).unwrap();
    message_bus.register("actor2", addr2.clone()).unwrap();

    // Send messages via the message bus
    message_bus.send("actor1", "Hello from bus to actor1".to_string()).unwrap();
    message_bus.send("actor2", "Hello from bus to actor2".to_string()).unwrap();

    // Wait for message processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify messages were received by checking actor state
    let msgs1 = addr1.send(GetMessages).await.unwrap();
    let msgs2 = addr2.send(GetMessages).await.unwrap();

    assert_eq!(msgs1, vec!["Hello from bus to actor1"]);
    assert_eq!(msgs2, vec!["Hello from bus to actor2"]);

    // Clean up
    addr1.do_send(StopWorker);
    addr2.do_send(StopWorker);
}

#[actix::test]
async fn test_message_router() {
    // Create the message bus and router
    let message_bus = Arc::new(MessageBus::new());
    let router_config = RouterConfig {
        debug_logging: true,
        ..RouterConfig::default()
    };

    // Start router actor
    let router = RouterActor::new(message_bus.clone(), router_config);
    let router_addr = router.start();

    // Create and register actors
    let actor1 = TestActor {
        id: "actor1".to_string(),
        received_messages: Vec::new(),
    };

    let actor2 = TestActor {
        id: "actor2".to_string(),
        received_messages: Vec::new(),
    };

    let addr1 = actor1.start();
    let addr2 = actor2.start();

    message_bus.register("actor1", addr1.clone()).unwrap();
    message_bus.register("actor2", addr2.clone()).unwrap();

    // Send messages via the router
    router_addr.do_send(RouteMessage {
        sender: "router".into(),
        target: "actor1".into(),
        message: "Hello from router to actor1".to_string(),
    });

    router_addr.do_send(RouteMessage {
        sender: "router".into(),
        target: "actor2".into(),
        message: "Hello from router to actor2".to_string(),
    });

    // Wait longer for message processing to ensure they are delivered
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get router stats
    let stats = router_addr.send(GetRouterStats).await.unwrap();
    assert_eq!(stats.total_messages, 2);
    assert_eq!(stats.successful_messages, 2);
    assert_eq!(stats.failed_messages, 0);

    // Verify messages were received by checking actor state
    let msgs1 = addr1.send(GetMessages).await.unwrap();
    let msgs2 = addr2.send(GetMessages).await.unwrap();

    assert_eq!(msgs1, vec!["Hello from router to actor1"]);
    assert_eq!(msgs2, vec!["Hello from router to actor2"]);

    // Clean up
    router_addr.do_send(ResetRouterStats);
    addr1.do_send(StopWorker);
    addr2.do_send(StopWorker);
    router_addr.do_send(StopWorker);
}

#[actix::test]
async fn test_actor_pool() {
    // Create actor pool
    let config = PoolConfig {
        size: 3,
        strategy: PoolStrategy::RoundRobin,
        ..PoolConfig::default()
    };

    let (pool, monitor_addr) = PoolBuilder::<WorkerActor>::new()
        .with_size(3)
        .with_strategy(PoolStrategy::RoundRobin)
        .with_factory(|| WorkerActor {
            id: 0,
            processed_messages: Mutex::new(Vec::new())
        })
        .build()
        .unwrap();

    // Send messages to the pool
    let num_messages = 9;
    {
        let pool = pool.lock().unwrap();
        for i in 0..num_messages {
            pool.send(TestMessage(format!("Message {}", i))).unwrap();
        }
    }

    // Wait for message processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify pool size
    let pool = pool.lock().unwrap();
    assert_eq!(pool.size(), 3);

    // Clean up
    monitor_addr.do_send(StopWorker);
}

#[actix::test]
async fn test_reduced_nesting() {
    // This test demonstrates how we can simplify actor communication
    // by reducing nesting and using the message bus

    // Create the message bus
    let message_bus = Arc::new(MessageBus::new());

    // In the old nested approach, we might have had:
    // Supervisor -> Workflow -> Source -> Processor -> Destination
    // Now we can flatten this to direct communication

    // Create the actors
    let supervisor = TestActor {
        id: "supervisor".to_string(),
        received_messages: Vec::new(),
    };

    let workflow = TestActor {
        id: "workflow".to_string(),
        received_messages: Vec::new(),
    };

    let source = TestActor {
        id: "source".to_string(),
        received_messages: Vec::new(),
    };

    let processor = TestActor {
        id: "processor".to_string(),
        received_messages: Vec::new(),
    };

    let destination = TestActor {
        id: "destination".to_string(),
        received_messages: Vec::new(),
    };

    // Start and register actors
    let supervisor_addr = supervisor.start();
    let workflow_addr = workflow.start();
    let source_addr = source.start();
    let processor_addr = processor.start();
    let destination_addr = destination.start();

    message_bus.register("supervisor", supervisor_addr.clone()).unwrap();
    message_bus.register("workflow", workflow_addr.clone()).unwrap();
    message_bus.register("source", source_addr.clone()).unwrap();
    message_bus.register("processor", processor_addr.clone()).unwrap();
    message_bus.register("destination", destination_addr.clone()).unwrap();

    // Now instead of going through the hierarchy, any actor can communicate directly
    message_bus.send("destination", "Direct message from supervisor".to_string()).unwrap();

    // Wait for message processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify message was received directly
    let msgs = destination_addr.send(GetMessages).await.unwrap();
    assert_eq!(msgs, vec!["Direct message from supervisor"]);

    // Clean up
    supervisor_addr.do_send(StopWorker);
    workflow_addr.do_send(StopWorker);
    source_addr.do_send(StopWorker);
    processor_addr.do_send(StopWorker);
    destination_addr.do_send(StopWorker);
}