//! Message broker module for pub/sub messaging in the cluster.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use actix::prelude::*;
use actix::dev::ToEnvelope;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::interval;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use log::{debug, error, info, warn};

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo, NodeStatus};
use crate::message::{MessageEnvelope, DeliveryGuarantee};
use crate::transport::{P2PTransport, TransportMessage};
use crate::serialization::{deserialize, serialize};

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
    
    /// Message sequence number for ordering
    pub sequence: u64,
    
    /// Delivery guarantee level
    pub delivery_guarantee: DeliveryGuarantee,
}

/// Message acknowledgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAck {
    /// Message ID being acknowledged
    pub message_id: uuid::Uuid,
    
    /// Topic the message belongs to
    pub topic: String,
    
    /// Node that is acknowledging the message
    pub node_id: NodeId,
    
    /// Timestamp of acknowledgment
    pub timestamp: u64,
}

/// Topic subscription options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionOptions {
    /// The delivery guarantee for this subscription
    pub delivery_guarantee: DeliveryGuarantee,
    
    /// Maximum number of messages to buffer
    pub buffer_size: usize,
    
    /// Should messages be persisted
    pub persistent: bool,
    
    /// Message TTL in seconds
    pub ttl: Option<u64>,
}

impl Default for SubscriptionOptions {
    fn default() -> Self {
        SubscriptionOptions {
            delivery_guarantee: DeliveryGuarantee::AtMostOnce,
            buffer_size: 100,
            persistent: false,
            ttl: None,
        }
    }
}

/// Pending message waiting for acknowledgment
struct PendingMessage {
    /// The original message
    message: BrokerMessage,
    
    /// When the message was first sent
    first_sent: Instant,
    
    /// When the message was last retried
    last_retry: Instant,
    
    /// Number of retries
    retry_count: u8,
    
    /// Nodes that have acknowledged this message
    acked_by: HashSet<NodeId>,
    
    /// Nodes that should receive this message
    target_nodes: HashSet<NodeId>,
}

/// Remote subscription information
#[derive(Debug, Clone)]
struct RemoteSubscription {
    /// Node ID where the subscription exists
    node_id: NodeId,
    
    /// Topic being subscribed to
    topic: String,
    
    /// When the subscription was created
    created_at: Instant,
    
    /// Last activity on this subscription
    last_activity: Instant,
    
    /// Subscription options
    options: SubscriptionOptions,
}

/// Message broker for distributed pub/sub
pub struct MessageBroker {
    /// Local node ID
    local_node_id: NodeId,
    
    /// Topic subscriptions (local)
    subscriptions: Arc<RwLock<HashMap<String, Vec<mpsc::Sender<BrokerMessage>>>>>,
    
    /// Message deduplication cache for exactly-once delivery
    deduplication_cache: Arc<RwLock<HashMap<uuid::Uuid, Instant>>>,
    
    /// Sequence number counters per topic
    sequence_counters: Arc<RwLock<HashMap<String, u64>>>,
}

impl MessageBroker {
    /// Create a new message broker
    pub fn new(local_node_id: NodeId) -> Self {
        MessageBroker {
            local_node_id,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            deduplication_cache: Arc::new(RwLock::new(HashMap::new())),
            sequence_counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Publish a message to a topic
    pub async fn publish<M>(&self, topic: &str, message: M, guarantee: DeliveryGuarantee) -> Result<uuid::Uuid, ClusterError> 
    where
        M: Serialize + Send + 'static,
    {
        let message_id = uuid::Uuid::new_v4();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Get sequence number for this topic
        let sequence = {
            let mut sequence_counters = self.sequence_counters.write().await;
            let counter = sequence_counters.entry(topic.to_string()).or_insert(0);
            *counter += 1;
            *counter
        };
        
        // Serialize the message
        let payload = serialize(&message)
            .map_err(|_| ClusterError::SerializationError("Failed to serialize message".into()))?;
        
        let broker_message = BrokerMessage {
            topic: topic.to_string(),
            payload,
            id: message_id,
            source_node: self.local_node_id.clone(),
            timestamp: now,
            sequence,
            delivery_guarantee: guarantee,
        };
        
        // Deliver to local subscribers
        self.deliver_local(broker_message.clone()).await?;
        
        log::debug!("Published message to topic: {} with ID: {}", topic, message_id);
        Ok(message_id)
    }
    
    /// Subscribe to a topic
    pub async fn subscribe<M, A>(&self, topic: &str, actor: Addr<A>, options: SubscriptionOptions) -> Result<(), ClusterError> 
    where
        M: for<'de> Deserialize<'de> + Send + 'static + Message,
        A: Handler<M> + Actor,
        M::Result: Send,
        A::Context: ToEnvelope<A, M>,
    {
        // Create channel for messages
        let (tx, mut rx) = mpsc::channel(options.buffer_size);
        
        // Register subscription
        {
            let mut subs = self.subscriptions.write().await;
            if let Some(topic_subs) = subs.get_mut(topic) {
                topic_subs.push(tx);
            } else {
                subs.insert(topic.to_string(), vec![tx]);
            }
        }
        
        // Actor clone for the async task
        let actor_clone = actor.clone();
        
        // Start task to forward messages from channel to actor
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                // Deserialize the message
                match deserialize::<M>(&msg.payload) {
                    Ok(message) => {
                        // Send to actor
                        if let Err(err) = actor_clone.send(message).await {
                            log::error!("Failed to deliver message to actor: {:?}", err);
                        }
                    },
                    Err(err) => {
                        log::error!("Failed to deserialize message: {:?}", err);
                    }
                }
            }
        });
        
        log::debug!("Subscribed to topic: {}", topic);
        Ok(())
    }
    
    /// Unsubscribe from a topic
    pub async fn unsubscribe(&self, topic: &str) -> Result<(), ClusterError> {
        let mut subs = self.subscriptions.write().await;
        subs.remove(topic);
        log::debug!("Unsubscribed from topic: {}", topic);
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
        if msg.delivery_guarantee == DeliveryGuarantee::ExactlyOnce {
            let mut dedup = self.deduplication_cache.write().await;
            if dedup.contains_key(&msg.id) {
                // Duplicate message, ignore
                log::debug!("Ignoring duplicate message: {}", msg.id);
                return Ok(());
            }
            
            // Add to deduplication cache with current timestamp
            dedup.insert(msg.id, Instant::now());
        }
        
        // Deliver message to local subscribers
        self.deliver_local(msg).await
    }
    
    /// Deliver a message to local subscribers
    async fn deliver_local(&self, msg: BrokerMessage) -> Result<(), ClusterError> {
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
    
    /// Clean up expired deduplication cache entries
    pub async fn clean_deduplication_cache(&self, ttl: Duration) {
        let now = Instant::now();
        let mut dedup = self.deduplication_cache.write().await;
        
        dedup.retain(|_, timestamp| {
            now.duration_since(*timestamp) < ttl
        });
        
        log::debug!("Cleaned deduplication cache, remaining entries: {}", dedup.len());
    }
}

/// NetworkBroker extends MessageBroker with distributed capabilities
pub struct NetworkBroker {
    /// The underlying message broker
    broker: MessageBroker,
    
    /// Transport layer for network communication
    transport: Arc<Mutex<P2PTransport>>,
    
    /// Remote subscriptions (topic -> node_ids)
    remote_subscriptions: Arc<RwLock<HashMap<String, HashSet<NodeId>>>>,
    
    /// Pending messages waiting for acknowledgment
    pending_messages: Arc<RwLock<HashMap<uuid::Uuid, PendingMessage>>>,
    
    /// Known cluster nodes
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
}

impl NetworkBroker {
    /// Create a new network broker
    pub fn new(
        local_node_id: NodeId, 
        transport: Arc<Mutex<P2PTransport>>, 
        nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>
    ) -> Self {
        let broker = MessageBroker::new(local_node_id.clone());
        
        let network_broker = NetworkBroker {
            broker,
            transport,
            remote_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            pending_messages: Arc::new(RwLock::new(HashMap::new())),
            nodes,
        };
        
        // Start background tasks
        network_broker.start_background_tasks();
        
        network_broker
    }
    
    /// Start background maintenance tasks
    fn start_background_tasks(&self) {
        // Start acknowledgment tracker
        let pending_clone = self.pending_messages.clone();
        let transport_clone = self.transport.clone();
        
        // Use a new thread to avoid tokio::spawn Send issues
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut interval = interval(Duration::from_secs(5));
                loop {
                    interval.tick().await;
                    Self::process_pending_messages(pending_clone.clone(), transport_clone.clone()).await;
                }
            });
        });
        
        // Start deduplication cache cleaner
        let broker_clone = self.broker.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut interval = interval(Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    broker_clone.clean_deduplication_cache(Duration::from_secs(3600)).await;
                }
            });
        });
    }
    
    /// Process pending messages that need retransmission
    async fn process_pending_messages(
        pending_messages: Arc<RwLock<HashMap<uuid::Uuid, PendingMessage>>>,
        transport: Arc<Mutex<P2PTransport>>
    ) {
        log::debug!("Processing pending messages");
        let now = Instant::now();
        let retry_after = Duration::from_secs(10);
        let max_retries = 5;

        // First, collect all data we need under read lock
        let mut to_retry: Vec<(uuid::Uuid, BrokerMessage, Vec<NodeId>, HashSet<NodeId>)> = Vec::new();
        let mut to_remove: Vec<uuid::Uuid> = Vec::new();
        let mut to_update: Vec<(uuid::Uuid, u8)> = Vec::new();
        
        // Scope for the read lock
        {
            let pending = pending_messages.read().await;
            for (id, msg) in pending.iter() {
                if now.duration_since(msg.last_retry) > retry_after && msg.retry_count < max_retries {
                    // Collect all data needed for retry: id, message, target nodes, and acked nodes
                    to_retry.push((
                        id.clone(),
                        msg.message.clone(),
                        msg.target_nodes.iter().cloned().collect(),
                        msg.acked_by.clone()
                    ));
                    to_update.push((id.clone(), msg.retry_count + 1));
                } else if msg.retry_count >= max_retries {
                    to_remove.push(id.clone());
                    log::warn!("Message delivery failed after max retries: {}", id);
                }
            }
        } // Read lock is released here
        
        // Update retry counts
        if !to_update.is_empty() {
            let mut pending = pending_messages.write().await;
            for (id, retry_count) in to_update {
                if let Some(pending_msg) = pending.get_mut(&id) {
                    pending_msg.retry_count = retry_count;
                    pending_msg.last_retry = now;
                }
            }
        }
        
        // Process retries with all the data we collected
        if !to_retry.is_empty() {
            let mut transport_guard = transport.lock().await;
            for (id, broker_msg, target_nodes, acked_by) in to_retry {
                for node_id in &target_nodes {
                    if !acked_by.contains(node_id) {
                        let trans_msg = TransportMessage::BrokerMessage(broker_msg.clone());
                        if let Err(err) = transport_guard.send_message(node_id, trans_msg).await {
                            log::error!("Failed to retransmit message to node {}: {:?}", node_id, err);
                        }
                    }
                }
            }
        }
        
        // Remove expired messages
        if !to_remove.is_empty() {
            let mut pending = pending_messages.write().await;
            for id in to_remove {
                pending.remove(&id);
            }
        }
    }
    
    /// Publish a message to a topic across the network
    pub async fn publish<M>(&self, topic: &str, message: M, guarantee: DeliveryGuarantee) -> Result<uuid::Uuid, ClusterError> 
    where
        M: Serialize + Send + Clone + 'static,
    {
        // Publish locally first
        let message_id = self.broker.publish(topic, message.clone(), guarantee).await?;
        
        // Find remote nodes that have subscribed to this topic
        let remote_nodes = {
            let remote_subs = self.remote_subscriptions.read().await;
            match remote_subs.get(topic) {
                Some(nodes) => nodes.clone(),
                None => HashSet::new(),
            }
        };
        
        // If we have remote subscribers, distribute the message
        if !remote_nodes.is_empty() {
            // Get the message that was just created
            let mut sequenced_counters = self.broker.sequence_counters.write().await;
            let sequence = *sequenced_counters.get(topic).unwrap_or(&0);
            
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
                
            // Create the broker message
            let payload = serialize(&message)
                .map_err(|_| ClusterError::SerializationError("Failed to serialize message".into()))?;
                
            let broker_message = BrokerMessage {
                topic: topic.to_string(),
                payload,
                id: message_id,
                source_node: self.broker.local_node_id.clone(),
                timestamp: now,
                sequence,
                delivery_guarantee: guarantee,
            };
            
            // Send to all remote subscribers
            let mut transport_guard = self.transport.lock().await;
            
            // Track message if delivery guarantee requires it
            if guarantee != DeliveryGuarantee::AtMostOnce {
                let pending_message = PendingMessage {
                    message: broker_message.clone(),
                    first_sent: Instant::now(),
                    last_retry: Instant::now(),
                    retry_count: 0,
                    acked_by: HashSet::new(),
                    target_nodes: remote_nodes.clone(),
                };
                
                self.pending_messages.write().await.insert(message_id, pending_message);
            }
            
            // Send to all remote subscribers
            for node_id in &remote_nodes {
                let trans_msg = TransportMessage::BrokerMessage(broker_message.clone());
                if let Err(err) = transport_guard.send_message(node_id, trans_msg).await {
                    log::error!("Failed to send message to node {}: {:?}", node_id, err);
                }
            }
        }
        
        Ok(message_id)
    }
    
    /// Subscribe an actor to a topic
    pub async fn subscribe<M, A>(
        &self, 
        topic: &str, 
        actor: Addr<A>, 
        options: SubscriptionOptions,
        remote: bool
    ) -> Result<(), ClusterError>
    where
        M: for<'de> Deserialize<'de> + Send + 'static + Message<Result = ()>,
        A: Handler<M> + Actor,
        A::Context: ToEnvelope<A, M>,
    {
        // First, subscribe locally
        self.broker.subscribe::<M, A>(topic, actor, options.clone()).await?;
        
        // Then, if we should notify remote nodes
        if remote {
            // Get active nodes
            let nodes = {
                let nodes_guard = self.nodes.read().await;
                nodes_guard.keys().cloned().collect::<Vec<_>>()
            };
            
            // Add to our remote subscriptions
            {
                let mut remote_subs = self.remote_subscriptions.write().await;
                let topic_nodes = remote_subs.entry(topic.to_string()).or_insert_with(HashSet::new);
                
                for node_id in &nodes {
                    if node_id != &self.broker.local_node_id {
                        topic_nodes.insert(node_id.clone());
                    }
                }
            }
            
            // Send subscription message to all remote nodes
            let mut transport = self.transport.lock().await;
            for node_id in nodes {
                if node_id != self.broker.local_node_id {
                    let sub_msg = TransportMessage::Subscribe {
                        topic: topic.to_string(),
                        options: options.clone(),
                    };
                    
                    if let Err(err) = transport.send_message(&node_id, sub_msg).await {
                        log::error!("Failed to send subscription message to node {}: {:?}", node_id, err);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle a subscription from a remote node
    pub async fn handle_remote_subscription(
        &self,
        topic: &str,
        node_id: &NodeId,
        _options: SubscriptionOptions
    ) -> Result<(), ClusterError> {
        let mut remote_subs = self.remote_subscriptions.write().await;
        
        let subscribers = remote_subs.entry(topic.to_string()).or_insert_with(HashSet::new);
        subscribers.insert(node_id.clone());
        
        log::debug!("Registered remote subscription from node {} for topic {}", node_id, topic);
        Ok(())
    }
    
    /// Handle an unsubscribe request from a remote node
    pub async fn handle_remote_unsubscribe(&self, topic: &str, node_id: &NodeId) -> Result<(), ClusterError> {
        let mut remote_subs = self.remote_subscriptions.write().await;
        
        if let Some(subscribers) = remote_subs.get_mut(topic) {
            subscribers.remove(node_id);
            
            // If no more subscribers, remove the topic
            if subscribers.is_empty() {
                remote_subs.remove(topic);
            }
        }
        
        log::debug!("Removed remote subscription from node {} for topic {}", node_id, topic);
        Ok(())
    }
    
    /// Handle an acknowledgment from a remote node
    pub async fn handle_message_ack(&self, ack: MessageAck) -> Result<(), ClusterError> {
        let mut pending = self.pending_messages.write().await;
        
        if let Some(msg) = pending.get_mut(&ack.message_id) {
            // Add the node to the acknowledged list
            msg.acked_by.insert(ack.node_id.clone());
            
            // If all nodes have acknowledged, remove from pending
            if msg.acked_by.len() == msg.target_nodes.len() {
                pending.remove(&ack.message_id);
                log::debug!("Message {} fully acknowledged by all nodes", ack.message_id);
            }
        }
        
        Ok(())
    }
    
    /// Send an acknowledgment for a received message
    pub async fn send_message_ack(&self, message: &BrokerMessage) -> Result<(), ClusterError> {
        // Only acknowledge if required by delivery guarantee
        if message.delivery_guarantee == DeliveryGuarantee::AtLeastOnce || 
           message.delivery_guarantee == DeliveryGuarantee::ExactlyOnce {
            
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
                
            let ack = MessageAck {
                message_id: message.id,
                topic: message.topic.clone(),
                node_id: self.broker.local_node_id.clone(),
                timestamp: now,
            };
            
            let mut transport_guard = self.transport.lock().await;
            let ack_msg = TransportMessage::BrokerAck(ack);
            
            transport_guard.send_message(&message.source_node, ack_msg).await?;
            log::debug!("Sent acknowledgment for message {} to node {}", message.id, message.source_node);
        }
        
        Ok(())
    }
    
    /// Unsubscribe from a topic, including remote notification
    pub async fn unsubscribe(&self, topic: &str, notify_remote: bool) -> Result<(), ClusterError> {
        // Unsubscribe locally
        self.broker.unsubscribe(topic).await?;
        
        // If requested, notify remote nodes
        if notify_remote {
            let mut transport_guard = self.transport.lock().await;
            
            // Get list of active nodes
            let active_nodes = {
                let nodes_guard = self.nodes.read().await;
                nodes_guard
                    .iter()
                    .filter(|(_, info)| info.status == NodeStatus::Up && info.id != self.broker.local_node_id)
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>()
            };
            
            // Send unsubscribe announcement to all active nodes
            for node_id in active_nodes {
                let unsubscribe_msg = TransportMessage::BrokerUnsubscribe {
                    topic: topic.to_string(),
                    node_id: self.broker.local_node_id.clone(),
                };
                
                if let Err(err) = transport_guard.send_message(&node_id, unsubscribe_msg).await {
                    log::error!("Failed to announce unsubscribe to node {}: {:?}", node_id, err);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get a reference to the underlying message broker
    pub fn local_broker(&self) -> &MessageBroker {
        &self.broker
    }
    
    /// Get a list of all topics including remote subscriptions
    pub async fn get_all_topics(&self) -> Vec<String> {
        let local_topics = self.broker.get_topics().await;
        let remote_topics = {
            let remote_subs = self.remote_subscriptions.read().await;
            remote_subs.keys().cloned().collect::<Vec<_>>()
        };
        
        // Combine and deduplicate
        let mut all_topics = local_topics;
        for topic in remote_topics {
            if !all_topics.contains(&topic) {
                all_topics.push(topic);
            }
        }
        
        all_topics
    }
    
    /// Handle a broker message received from the network
    pub async fn handle_broker_message(&self, message: BrokerMessage) -> Result<(), ClusterError> {
        // First send an acknowledgment if needed
        self.send_message_ack(&message).await?;
        
        // Then process the message locally
        self.broker.handle_remote_message(message).await
    }
}

// Make MessageBroker cloneable
impl Clone for MessageBroker {
    fn clone(&self) -> Self {
        MessageBroker {
            local_node_id: self.local_node_id.clone(),
            subscriptions: self.subscriptions.clone(),
            deduplication_cache: self.deduplication_cache.clone(),
            sequence_counters: self.sequence_counters.clone(),
        }
    }
}

// Actor implementation for NetworkBroker
pub struct NetworkBrokerActor {
    network_broker: Arc<NetworkBroker>,
}

impl NetworkBrokerActor {
    pub fn new(network_broker: Arc<NetworkBroker>) -> Self {
        Self { network_broker }
    }
}

impl Actor for NetworkBrokerActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("NetworkBrokerActor started");
    }
}

/// Message to publish to a topic
#[derive(Message)]
#[rtype(result = "Result<uuid::Uuid, ClusterError>")]
pub struct PublishMessage<M: Serialize + Clone + Send + 'static> {
    pub topic: String,
    pub message: M,
    pub guarantee: DeliveryGuarantee,
}

impl<M: Serialize + Clone + Send + 'static> Handler<PublishMessage<M>> for NetworkBrokerActor {
    type Result = ResponseFuture<Result<uuid::Uuid, ClusterError>>;
    
    fn handle(&mut self, msg: PublishMessage<M>, _ctx: &mut Self::Context) -> Self::Result {
        let broker = self.network_broker.clone();
        
        Box::pin(async move {
            broker.publish(&msg.topic, msg.message, msg.guarantee).await
        })
    }
}

/// Message to handle a broker message from the network
#[derive(Message)]
#[rtype(result = "Result<(), ClusterError>")]
pub struct HandleBrokerMessage {
    pub message: BrokerMessage,
}

impl Handler<HandleBrokerMessage> for NetworkBrokerActor {
    type Result = ResponseFuture<Result<(), ClusterError>>;
    
    fn handle(&mut self, msg: HandleBrokerMessage, _ctx: &mut Self::Context) -> Self::Result {
        let broker = self.network_broker.clone();
        
        Box::pin(async move {
            broker.handle_broker_message(msg.message).await
        })
    }
}

/// Message to handle a subscription from a remote node
#[derive(Message)]
#[rtype(result = "Result<(), ClusterError>")]
pub struct HandleRemoteSubscription {
    pub topic: String,
    pub node_id: NodeId,
    pub options: SubscriptionOptions,
}

impl Handler<HandleRemoteSubscription> for NetworkBrokerActor {
    type Result = ResponseFuture<Result<(), ClusterError>>;
    
    fn handle(&mut self, msg: HandleRemoteSubscription, _ctx: &mut Self::Context) -> Self::Result {
        let broker = self.network_broker.clone();
        
        Box::pin(async move {
            broker.handle_remote_subscription(&msg.topic, &msg.node_id, msg.options).await
        })
    }
}

/// Message to handle an unsubscribe from a remote node
#[derive(Message)]
#[rtype(result = "Result<(), ClusterError>")]
pub struct HandleRemoteUnsubscribe {
    pub topic: String,
    pub node_id: NodeId,
}

impl Handler<HandleRemoteUnsubscribe> for NetworkBrokerActor {
    type Result = ResponseFuture<Result<(), ClusterError>>;
    
    fn handle(&mut self, msg: HandleRemoteUnsubscribe, _ctx: &mut Self::Context) -> Self::Result {
        let broker = self.network_broker.clone();
        
        Box::pin(async move {
            broker.handle_remote_unsubscribe(&msg.topic, &msg.node_id).await
        })
    }
}

/// Message to handle an acknowledgment from a remote node
#[derive(Message)]
#[rtype(result = "Result<(), ClusterError>")]
pub struct HandleMessageAck {
    pub ack: MessageAck,
}

impl Handler<HandleMessageAck> for NetworkBrokerActor {
    type Result = ResponseFuture<Result<(), ClusterError>>;
    
    fn handle(&mut self, msg: HandleMessageAck, _ctx: &mut Self::Context) -> Self::Result {
        let broker = self.network_broker.clone();
        
        Box::pin(async move {
            broker.handle_message_ack(msg.ack).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};
    use std::time::Duration;
    
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
        // Create LocalSet to run test
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
    
    #[tokio::test]
    async fn test_message_publish_local() {
        // Create LocalSet to run test
        let local = tokio::task::LocalSet::new();
        
        local.run_until(async {
            let node_id = NodeId::new();
            let broker = MessageBroker::new(node_id);
            
            // Publish a message
            let message = TestMessage { content: "Hello World".to_string() };
            let result = broker.publish("test-topic", message, DeliveryGuarantee::AtMostOnce).await;
            
            assert!(result.is_ok());
            assert!(uuid::Uuid::nil() != result.unwrap());
        }).await;
    }
    
    // Additional tests for NetworkBroker would require mock P2PTransport
} 