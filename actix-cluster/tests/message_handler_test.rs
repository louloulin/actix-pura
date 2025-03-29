use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

use actix_cluster::{
    node::{NodeId, NodeInfo},
    transport::{P2PTransport, TransportMessage, MessageHandler},
    config::NodeRole,
    serialization::SerializationFormat,
    error::ClusterResult,
};
use parking_lot::Mutex;
use tokio::sync::mpsc;

// 自定义消息处理器，用于测试
struct TestMessageHandler {
    messages_received: Arc<AtomicBool>,
}

impl TestMessageHandler {
    fn new(messages_received: Arc<AtomicBool>) -> Self {
        Self { messages_received }
    }
}

#[async_trait::async_trait]
impl MessageHandler for TestMessageHandler {
    async fn handle_message(&mut self, sender: NodeId, message: TransportMessage) -> ClusterResult<()> {
        println!("TestMessageHandler received message from {}: {:?}", sender, message);
        // 标记消息已接收
        self.messages_received.store(true, Ordering::SeqCst);
        
        // 模拟异步处理
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        Ok(())
    }
}

#[tokio::test]
async fn test_message_handler() {
    // 创建消息已接收标志
    let messages_received = Arc::new(AtomicBool::new(false));
    
    // 创建两个节点
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9601);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9602);
    
    let node1_id = NodeId::new();
    let node2_id = NodeId::new();
    
    let node1_info = NodeInfo::new(
        node1_id.clone(),
        "test-handler-node-1".to_string(),
        NodeRole::Peer,
        node1_addr,
    );
    
    let node2_info = NodeInfo::new(
        node2_id.clone(),
        "test-handler-node-2".to_string(),
        NodeRole::Peer,
        node2_addr,
    );
    
    // 创建传输层
    let mut transport1 = P2PTransport::new(node1_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport1");
    
    // 设置消息处理器
    let handler = TestMessageHandler::new(messages_received.clone());
    transport1.set_message_handler_direct(Arc::new(Mutex::new(handler)));
    
    // 手动添加节点2作为对等节点
    transport1.add_peer(node2_id.clone(), node2_info.clone());
    
    // 创建测试消息
    let test_message = TransportMessage::StatusUpdate(node2_id.clone(), "Testing".to_string());
    
    // 模拟接收消息处理
    if let Some(handler) = transport1.get_message_handler() {
        // 我们将使用与transport.rs中相同的方式处理消息
        let node_id_clone = node2_id.clone();
        let message_clone = test_message.clone();
        let handler_clone = handler.clone();
        
        tokio::task::spawn_blocking(move || {
            // 在新线程中获取锁
            let mut handler_locked = handler_clone.lock();
            
            // 构建运行时执行异步代码
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            
            if let Err(e) = rt.block_on(handler_locked.handle_message(node_id_clone, message_clone)) {
                eprintln!("Error handling message: {:?}", e);
            }
        }).await.unwrap();
    }
    
    // 等待消息处理完成
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // 验证消息已被处理
    assert!(messages_received.load(Ordering::SeqCst), "Message was not processed by handler");
}

#[tokio::test]
async fn test_concurrent_message_handlers() {
    // 创建消息已接收标志
    let messages_received = Arc::new(AtomicBool::new(false));
    
    // 创建节点信息
    let node_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9603);
    let node_id = NodeId::new();
    
    let node_info = NodeInfo::new(
        node_id.clone(),
        "test-concurrent-node".to_string(),
        NodeRole::Peer,
        node_addr,
    );
    
    // 创建传输层
    let mut transport = P2PTransport::new(node_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport");
    
    // 设置消息处理器
    let handler = TestMessageHandler::new(messages_received.clone());
    transport.set_message_handler_direct(Arc::new(Mutex::new(handler)));
    
    // 创建多个并发消息处理任务
    let mut handles = vec![];
    let num_messages = 10;
    
    for i in 0..num_messages {
        let test_message = TransportMessage::StatusUpdate(
            node_id.clone(), 
            format!("Concurrent message {}", i)
        );
        
        let node_id_clone = node_id.clone();
        let handler_clone = transport.get_message_handler().unwrap().clone();
        
        let handle = tokio::task::spawn_blocking(move || {
            // 在新线程中获取锁
            let mut handler_locked = handler_clone.lock();
            
            // 构建运行时执行异步代码
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            
            if let Err(e) = rt.block_on(handler_locked.handle_message(node_id_clone, test_message)) {
                eprintln!("Error handling concurrent message: {:?}", e);
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }
    
    // 验证消息已被处理
    assert!(messages_received.load(Ordering::SeqCst), "No messages were processed by handler");
} 