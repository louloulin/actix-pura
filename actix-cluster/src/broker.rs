//! Message broker module for pub/sub messaging in the cluster.

use std::collections::HashMap;
use std::sync::Arc;
use actix::prelude::*;
use tokio::sync::{mpsc, RwLock};
use serde::{Deserialize, Serialize};

use crate::error::{ClusterError, ClusterResult};
use crate::node::NodeId;

/// Message wrapper for serialized messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerMessage {
    /// Topic the message belongs to
    pub topic: String,
    
    /// Serialized message payload
    pub payload: Vec<u8>,
    
    /// Message ID
    pub id: uuid::Uuid,
    
    /// Source node ID
    pub source_node: NodeId,
    
    /// Timestamp when the message was sent
    pub timestamp: u64,
}

/// Message delivery type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryType {
    /// At most once delivery (fire and forget)
    AtMostOnce,
    
    /// At least once delivery (requires ACK)
    AtLeastOnce,
    
    /// Exactly once delivery (requires deduplication)
    ExactlyOnce,
}

/// Topic subscription options
#[derive(Debug, Clone)]
pub struct SubscriptionOptions {
    /// The delivery type for this subscription
    pub delivery_type: DeliveryType,
    
    /// Maximum number of messages to buffer
    pub buffer_size: usize,
    
    /// Should messages be persisted
    pub persistent: bool,
}

impl Default for SubscriptionOptions {
    fn default() -> Self {
        SubscriptionOptions {
            delivery_type: DeliveryType::AtMostOnce,
            buffer_size: 100,
            persistent: false,
        }
    }
}

/// Message broker for distributed pub/sub
pub struct MessageBroker {
    /// Local node ID
    local_node_id: NodeId,
    
    /// Topic subscriptions
    subscriptions: Arc<RwLock<HashMap<String, Vec<mpsc::Sender<BrokerMessage>>>>>,
    
    /// Message deduplication cache for exactly-once delivery
    deduplication_cache: Arc<RwLock<HashMap<uuid::Uuid, bool>>>,
}

impl MessageBroker {
    /// Create a new message broker
    pub fn new(local_node_id: NodeId) -> Self {
        MessageBroker {
            local_node_id,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            deduplication_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Publish a message to a topic
    pub fn publish<M>(&self, topic: &str, _message: M) -> Result<(), ClusterError> 
    where
        M: Serialize + Send + 'static,
    {
        // TODO: Implement message serialization and delivery
        log::debug!("Publishing message to topic: {}", topic);
        Ok(())
    }
    
    /// Subscribe to a topic
    pub async fn subscribe<M, A>(&self, topic: &str, actor: Addr<A>, options: SubscriptionOptions) -> Result<(), ClusterError> 
    where
        M: for<'de> Deserialize<'de> + Send + 'static + Message,
        A: Handler<M> + Actor,
    {
        // Create a channel for messages
        let (tx, mut rx) = mpsc::channel::<BrokerMessage>(options.buffer_size);
        
        // Add subscription to the topic
        {
            let mut subs = self.subscriptions.write().await;
            let topic_subs = subs.entry(topic.to_string()).or_insert_with(Vec::new);
            topic_subs.push(tx);
        }
        
        // Start a task to forward messages to the actor
        let actor_clone = actor.clone();
        actix::spawn(async move {
            while let Some(msg) = rx.recv().await {
                // TODO: Deserialize and forward message to actor
                log::debug!("Received message on topic: {}", msg.topic);
            }
        });
        
        Ok(())
    }
    
    /// Unsubscribe from a topic
    pub async fn unsubscribe(&self, topic: &str) -> Result<(), ClusterError> {
        let mut subs = self.subscriptions.write().await;
        subs.remove(topic);
        Ok(())
    }
    
    /// Get all active topics
    pub async fn get_topics(&self) -> Vec<String> {
        let subs = self.subscriptions.read().await;
        subs.keys().cloned().collect()
    }
    
    /// Handle a message received from a remote node
    pub async fn handle_remote_message(&self, msg: BrokerMessage) -> Result<(), ClusterError> {
        // Check for duplicate message if exactly-once delivery
        {
            let mut dedup = self.deduplication_cache.write().await;
            if dedup.contains_key(&msg.id) {
                // Duplicate message, ignore
                return Ok(());
            }
            
            // Add to deduplication cache
            dedup.insert(msg.id, true);
        }
        
        // Forward message to subscribers
        let subs = self.subscriptions.read().await;
        if let Some(topic_subs) = subs.get(&msg.topic) {
            for sub in topic_subs {
                if let Err(_) = sub.send(msg.clone()).await {
                    // Subscriber has dropped, but we'll clean it up on the next subscription
                    log::warn!("Failed to deliver message to subscriber");
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};
    
    struct TestActor;
    
    impl Actor for TestActor {
        type Context = Context<Self>;
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestMessage {
        pub content: String,
    }
    
    impl Message for TestMessage {
        type Result = ();
    }
    
    impl Handler<TestMessage> for TestActor {
        type Result = ();
        
        fn handle(&mut self, _msg: TestMessage, _ctx: &mut Context<Self>) -> Self::Result {
            // Test handler
        }
    }
    
    #[tokio::test]
    async fn test_broker_subscribe_unsubscribe() {
        // 创建LocalSet运行测试
        let local = tokio::task::LocalSet::new();
        
        local.run_until(async {
            let node_id = NodeId::new();
            let broker = MessageBroker::new(node_id);
            
            let actor = TestActor.start();
            
            // Subscribe
            let result = broker.subscribe::<TestMessage, _>(
                "test-topic", 
                actor.clone(), 
                SubscriptionOptions::default()
            ).await;
            
            assert!(result.is_ok());
            
            let topics = broker.get_topics().await;
            assert_eq!(topics.len(), 1);
            assert_eq!(topics[0], "test-topic");
            
            // Unsubscribe
            let result = broker.unsubscribe("test-topic").await;
            assert!(result.is_ok());
            
            let topics = broker.get_topics().await;
            assert_eq!(topics.len(), 0);
        }).await;
    }
} 