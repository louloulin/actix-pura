use std::sync::Arc;
use std::time::Duration;

use actix_cluster::{
    node::NodeId,
    transport::{TransportMessage, MessageHandler},
    error::ClusterResult,
};
use parking_lot::Mutex;

// 自定义消息处理器，用于测试
struct TestMessageHandler {
    received: Arc<Mutex<Vec<TransportMessage>>>,
}

impl TestMessageHandler {
    fn new() -> Self {
        Self { 
            received: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    fn has_received_messages(&self) -> bool {
        !self.received.lock().is_empty()
    }
    
    fn get_received_messages(&self) -> Vec<TransportMessage> {
        self.received.lock().clone()
    }
    
    fn add_message(&self, message: TransportMessage) {
        let mut received = self.received.lock();
        received.push(message);
    }
}

#[async_trait::async_trait]
impl MessageHandler for TestMessageHandler {
    async fn handle_message(&self, sender: NodeId, message: TransportMessage) -> ClusterResult<()> {
        println!("TestMessageHandler received message from {}: {:?}", sender, message);
        
        // Store the message - note we don't hold the lock across await points
        self.add_message(message);
        
        // 模拟异步处理
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        Ok(())
    }
}

#[tokio::test]
async fn test_message_handler() {
    // Create a node ID and test message
    let node_id = NodeId::new();
    let test_message = TransportMessage::StatusUpdate(node_id.clone(), "Testing".to_string());
    
    // Create the handler and test it directly
    let handler = TestMessageHandler::new();
    let handler_arc = Arc::new(Mutex::new(handler));
    
    // Test handler directly without transport
    let node_id_clone = node_id.clone();
    let message_clone = test_message.clone();
    let handler_clone = handler_arc.clone();
    
    tokio::task::spawn_blocking(move || {
        // 在新线程中获取锁
        let handler_locked = handler_clone.lock();
        
        // 构建运行时执行异步代码
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        if let Err(e) = rt.block_on(handler_locked.handle_message(node_id_clone, message_clone)) {
            eprintln!("Error handling message: {:?}", e);
        }
    }).await.unwrap();
    
    // 等待消息处理完成
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // 验证消息已被处理
    let handler_locked = handler_arc.lock();
    assert!(handler_locked.has_received_messages(), "Message was not processed by handler");
}

#[tokio::test]
async fn test_concurrent_message_handlers() {
    // Create a node ID for testing
    let node_id = NodeId::new();
    
    // Create handler
    let handler = TestMessageHandler::new();
    let handler_arc = Arc::new(Mutex::new(handler));
    
    // 创建多个并发消息处理任务
    let mut handles = vec![];
    let num_messages = 10;
    
    for i in 0..num_messages {
        let test_message = TransportMessage::StatusUpdate(
            node_id.clone(), 
            format!("Concurrent message {}", i)
        );
        
        let node_id_clone = node_id.clone();
        let handler_clone = handler_arc.clone();
        
        let handle = tokio::task::spawn_blocking(move || {
            // 在新线程中获取锁
            let handler_locked = handler_clone.lock();
            
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
    let handler_locked = handler_arc.lock();
    assert!(handler_locked.has_received_messages(), "No messages were processed by handler");
    assert_eq!(handler_locked.get_received_messages().len(), num_messages, "Not all messages were processed");
} 