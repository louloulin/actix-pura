use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

use actix::prelude::*;
use actix_cluster::{
    node::{NodeId, NodeInfo},
    transport::{P2PTransport, TransportMessage, MessageHandler},
    config::NodeRole,
    serialization::SerializationFormat,
    error::ClusterResult,
    registry::{ActorRef, LocalActorRef, ActorRegistry},
    message::AnyMessage,
    ClusterConfig, ClusterSystem, Architecture, DiscoveryMethod,
};
use parking_lot::Mutex;
use tokio::sync::mpsc;

// 测试用的标记结构体
struct MessageReceived {
    received: Arc<AtomicBool>,
    message_content: Arc<Mutex<Option<String>>>,
}

impl MessageReceived {
    fn new() -> Self {
        Self {
            received: Arc::new(AtomicBool::new(false)),
            message_content: Arc::new(Mutex::new(None)),
        }
    }
    
    fn set_received(&self, content: String) {
        self.received.store(true, Ordering::SeqCst);
        let mut message = self.message_content.lock();
        *message = Some(content);
    }
    
    fn is_received(&self) -> bool {
        self.received.load(Ordering::SeqCst)
    }
    
    fn get_content(&self) -> Option<String> {
        let message = self.message_content.lock();
        message.clone()
    }
}

// 测试用Actor
struct TestActor {
    marker: Arc<MessageReceived>,
}

impl TestActor {
    fn new(marker: Arc<MessageReceived>) -> Self {
        Self { marker }
    }
}

impl Actor for TestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("TestActor started");
    }
}

impl Handler<AnyMessage> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, _ctx: &mut Self::Context) {
        println!("TestActor received message");
        
        // 尝试从AnyMessage中提取字符串
        if let Some(content) = msg.0.downcast_ref::<String>() {
            self.marker.set_received(content.clone());
            println!("TestActor received content: {}", content);
        }
    }
}

#[tokio::test]
async fn test_local_actor_registration() {
    // Use a LocalSet to properly handle spawn_local
    let local = tokio::task::LocalSet::new();
    
    local.run_until(async {
        // 创建测试标记
        let marker = Arc::new(MessageReceived::new());
        
        // 创建节点配置
        let node_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10001);
        
        let config = ClusterConfig::new()
            .architecture(Architecture::Decentralized)
            .node_role(NodeRole::Peer)
            .bind_addr(node_addr)
            .cluster_name("test-cluster".to_string())
            .discovery(DiscoveryMethod::Static {
                seed_nodes: vec![],
            })
            .serialization_format(SerializationFormat::Bincode)
            .build()
            .expect("Failed to create config");
        
        // 创建集群系统
        let mut system = ClusterSystem::new("test-node", config);
        let system_addr = system.start().await.expect("Failed to start system");
        
        // 创建并启动测试Actor
        let test_actor = TestActor::new(marker.clone()).start();
        
        // 注册Actor
        system.register("test_actor", test_actor).await.expect("Failed to register actor");
        
        // 查找Actor
        let actor_ref = system.lookup("test_actor").await.expect("Failed to lookup actor");
        
        // 向Actor发送消息
        let message = Box::new("Hello, local actor!".to_string());
        actor_ref.send_any(message).expect("Failed to send message");
        
        // 等待消息处理
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // 验证消息已接收
        assert!(marker.is_received(), "Message was not received");
        assert_eq!(marker.get_content().unwrap(), "Hello, local actor!");
    }).await;
}

#[tokio::test]
async fn test_distributed_actor_registry() {
    // Use a LocalSet to properly handle spawn_local
    let local = tokio::task::LocalSet::new();
    
    local.run_until(async {
        // 创建消息接收标记
        let marker1 = Arc::new(MessageReceived::new());
        let marker2 = Arc::new(MessageReceived::new());
        
        // 创建两个节点的配置，使用不同的端口
        let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10002);
        let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10003);
        
        let config1 = ClusterConfig::new()
            .architecture(Architecture::Decentralized)
            .node_role(NodeRole::Peer)
            .bind_addr(node1_addr)
            .cluster_name("test-cluster".to_string())
            .discovery(DiscoveryMethod::Static {
                seed_nodes: vec![node2_addr.to_string()],
            })
            .serialization_format(SerializationFormat::Bincode)
            .build()
            .expect("Failed to create node1 config");
        
        let config2 = ClusterConfig::new()
            .architecture(Architecture::Decentralized)
            .node_role(NodeRole::Peer)
            .bind_addr(node2_addr)
            .cluster_name("test-cluster".to_string())
            .discovery(DiscoveryMethod::Static {
                seed_nodes: vec![node1_addr.to_string()],
            })
            .serialization_format(SerializationFormat::Bincode)
            .build()
            .expect("Failed to create node2 config");
        
        // 创建两个集群节点
        let mut node1 = ClusterSystem::new("node1", config1);
        let mut node2 = ClusterSystem::new("node2", config2);
        
        // 启动节点
        let _node1_addr = node1.start().await.expect("Failed to start node1");
        let _node2_addr = node2.start().await.expect("Failed to start node2");
        
        // 获取节点ID
        let node1_id = node1.local_node().id.clone();
        let node2_id = node2.local_node().id.clone();
        
        println!("Node 1 ID: {}", node1_id);
        println!("Node 2 ID: {}", node2_id);
        
        // 等待节点发现彼此，增加时间
        println!("Waiting for nodes to discover each other (10 seconds)...");
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // 创建并注册测试Actor到节点2
        let test_actor2 = TestActor::new(marker2.clone()).start();
        node2.register("remote_actor", test_actor2).await.expect("Failed to register actor on node2");
        
        // 等待注册完成，增加时间
        println!("Waiting for actor registration to propagate (5 seconds)...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        println!("Actor registered on node2, now trying to discover from node1");
        
        // 尝试多次发现远程Actor，因为节点同步需要时间
        println!("Attempting to discover remote actor with retries...");
        let mut remote_actor = None;
        for attempt in 1..=5 {
            println!("Discovery attempt {} of 5", attempt);
            
            match node1.discover_actor("remote_actor").await {
                Some(actor_ref) => {
                    println!("Successfully discovered remote actor on attempt {}", attempt);
                    remote_actor = Some(actor_ref);
                    break;
                },
                None => {
                    println!("Failed to discover remote actor on attempt {}", attempt);
                    // Wait a bit before retrying
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
        
        // 验证发现成功
        assert!(remote_actor.is_some(), "Failed to discover remote actor after multiple attempts");
        
        let remote_actor = remote_actor.unwrap();
        
        // 发送消息到远程Actor
        let message = Box::new("Hello, remote actor!".to_string());
        remote_actor.send_any(message).expect("Failed to send message to remote actor");
        
        // 等待消息处理，增加时间
        println!("Waiting for message to be processed (5 seconds)...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // 验证消息已接收
        assert!(marker2.is_received(), "Message was not received by remote actor");
        assert_eq!(marker2.get_content().unwrap(), "Hello, remote actor!");
    }).await;
}

// 测试在节点宕机或不可用时的行为
#[tokio::test]
async fn test_actor_discovery_timeout() {
    // Use a LocalSet to properly handle spawn_local
    let local = tokio::task::LocalSet::new();
    
    local.run_until(async {
        println!("Setting up node for timeout test");
        
        // 创建节点配置
        let node_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10004);
        let nonexistent_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10005);
        
        println!("Node address: {}", node_addr);
        println!("Nonexistent node address: {}", nonexistent_addr);
        
        let config = ClusterConfig::new()
            .architecture(Architecture::Decentralized)
            .node_role(NodeRole::Peer)
            .bind_addr(node_addr)
            .cluster_name("test-cluster".to_string())
            .discovery(DiscoveryMethod::Static {
                seed_nodes: vec![nonexistent_addr.to_string()],
            })
            .serialization_format(SerializationFormat::Bincode)
            .build()
            .expect("Failed to create config");
        
        // 创建集群系统
        println!("Creating and starting cluster system");
        let mut system = ClusterSystem::new("test-node", config);
        let _system_addr = system.start().await.expect("Failed to start system");
        
        // Get node ID
        let node_id = system.local_node().id.clone();
        println!("Node ID: {}", node_id);
        
        // 尝试发现不存在的Actor
        println!("Attempting to discover nonexistent actor");
        let start_time = std::time::Instant::now();
        let actor_ref = system.discover_actor("nonexistent_actor").await;
        let elapsed = start_time.elapsed();
        
        println!("Discovery attempt completed in {:?}", elapsed);
        
        // 验证发现失败且超时时间在预期范围内
        assert!(actor_ref.is_none(), "Should not discover nonexistent actor");
        
        // 确保超时时间至少是2秒
        assert!(elapsed.as_secs() >= 2, "Discovery should timeout after at least 2 seconds");
        // 添加上限检查，确保超时不会超过5秒（给系统一些容错空间）
        assert!(elapsed.as_secs() <= 5, "Discovery should not take more than 5 seconds");
    }).await;
} 