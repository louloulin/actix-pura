use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DeliveryGuarantee, DiscoveryMethod,
    MessageEnvelope, MessageType, NodeRole, SerializationFormat, AnyMessage,
    compression::{CompressionAlgorithm, CompressionConfig, CompressionLevel, get_compression_stats},
    node::NodeId,
};
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use env_logger;

// Define message types for our demo
// 1. Text message - good compression potential
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "()")]
struct TextMessage {
    content: String,        // Text content with high redundancy
    sender: String,         // Sender identifier
    timestamp: u64,         // Timestamp in milliseconds
}

// 2. Binary message - may have less compression potential
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "()")]
struct BinaryMessage {
    data: Vec<u8>,          // Binary data
    message_type: String,   // Type of binary data
    timestamp: u64,         // Timestamp in milliseconds
}

// 3. JSON message - structured data with good compression potential
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "()")]
struct JsonMessage {
    properties: serde_json::Value,  // JSON object
    sender: String,                 // Sender identifier
    timestamp: u64,                 // Timestamp in milliseconds
}

// Actor that receives messages
#[derive(Default)]
struct MessageReceiver {
    text_messages_received: usize,
    binary_messages_received: usize,
    json_messages_received: usize,
    bytes_received: usize,
}

impl Actor for MessageReceiver {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("MessageReceiver actor started");
    }
}

impl Handler<TextMessage> for MessageReceiver {
    type Result = ();

    fn handle(&mut self, msg: TextMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.text_messages_received += 1;
        self.bytes_received += msg.content.len();
        info!("Received text message from {}: {} chars", msg.sender, msg.content.len());
    }
}

impl Handler<BinaryMessage> for MessageReceiver {
    type Result = ();

    fn handle(&mut self, msg: BinaryMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.binary_messages_received += 1;
        self.bytes_received += msg.data.len();
        info!("Received binary message of type {}: {} bytes", msg.message_type, msg.data.len());
    }
}

impl Handler<JsonMessage> for MessageReceiver {
    type Result = ();

    fn handle(&mut self, msg: JsonMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.json_messages_received += 1;
        let json_str = serde_json::to_string(&msg.properties).unwrap_or_default();
        self.bytes_received += json_str.len();
        info!("Received JSON message from {}: {} bytes", msg.sender, json_str.len());
    }
}

// Implement AnyMessage handler for MessageReceiver to make it compatible with the cluster registry
impl Handler<AnyMessage> for MessageReceiver {
    type Result = ();

    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(text_msg) = msg.downcast::<TextMessage>() {
            // Clone the message to avoid borrowing issues
            let cloned_msg = TextMessage {
                content: text_msg.content.clone(),
                sender: text_msg.sender.clone(),
                timestamp: text_msg.timestamp,
            };
            self.handle(cloned_msg, ctx);
        } else if let Some(binary_msg) = msg.downcast::<BinaryMessage>() {
            // Clone the message to avoid borrowing issues
            let cloned_msg = BinaryMessage {
                data: binary_msg.data.clone(),
                message_type: binary_msg.message_type.clone(),
                timestamp: binary_msg.timestamp,
            };
            self.handle(cloned_msg, ctx);
        } else if let Some(json_msg) = msg.downcast::<JsonMessage>() {
            // Clone the message to avoid borrowing issues
            let cloned_msg = JsonMessage {
                properties: json_msg.properties.clone(),
                sender: json_msg.sender.clone(),
                timestamp: json_msg.timestamp,
            };
            self.handle(cloned_msg, ctx);
        }
    }
}

// Generate a text message with repeating content (highly compressible)
fn generate_text_message(size: usize) -> TextMessage {
    let base_text = "This is a test message with repeating content that should compress well. ";
    let repeats = (size as f64 / base_text.len() as f64).ceil() as usize;
    let content = base_text.repeat(repeats);

    TextMessage {
        content: content[0..size.min(content.len())].to_string(),
        sender: "text-sender".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    }
}

// Generate a binary message with random data (less compressible)
fn generate_binary_message(size: usize) -> BinaryMessage {
    let mut data = Vec::with_capacity(size);
    for _ in 0..size {
        data.push(rand::random::<u8>());
    }

    BinaryMessage {
        data,
        message_type: "random-binary".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    }
}

// Generate a JSON message with repeating structure (compressible)
fn generate_json_message(num_properties: usize) -> JsonMessage {
    let mut map = serde_json::Map::new();

    for i in 0..num_properties {
        map.insert(
            format!("property_{}", i),
            serde_json::Value::String(format!("This is the value for property {}", i)),
        );

        // Add nested objects for better compression potential
        if i % 10 == 0 {
            let mut nested = serde_json::Map::new();
            for j in 0..5 {
                nested.insert(
                    format!("nested_{}", j),
                    serde_json::Value::String("Nested property value with repeating content".to_string()),
                );
            }
            map.insert(format!("nested_obj_{}", i), serde_json::Value::Object(nested));
        }
    }

    JsonMessage {
        properties: serde_json::Value::Object(map),
        sender: "json-sender".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Define node addresses
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8551);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8552);

    // Node 1: With compression enabled
    let config1 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node1_addr)
        .cluster_name("compression-demo-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .serialization_format(SerializationFormat::Bincode)
        .with_compression_config(CompressionConfig {
            enabled: true,
            algorithm: CompressionAlgorithm::Gzip,
            level: CompressionLevel::Default,
            min_size_threshold: 1024, // Compress messages larger than 1KB
        })
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![],
        })
        .build()
        .expect("Failed to create node1 config");

    // Node 2: Without compression
    let config2 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node2_addr)
        .cluster_name("compression-demo-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .serialization_format(SerializationFormat::Bincode)
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node1_addr.to_string()],
        })
        .build()
        .expect("Failed to create node2 config");

    // Start the cluster nodes
    info!("Starting node 1 (with compression)");
    let mut system1 = ClusterSystem::new(config1);
    let system1_addr = system1.start().await.expect("Failed to start node 1");

    info!("Starting node 2 (without compression)");
    let mut system2 = ClusterSystem::new(config2);
    let system2_addr = system2.start().await.expect("Failed to start node 2");

    // Wait for nodes to discover each other
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Start the message receiver actor on node 2
    let receiver = MessageReceiver::default();
    let receiver_addr = receiver.start();

    // Register the actor with the cluster
    system2.register("message-receiver", receiver_addr.clone()).await
        .expect("Failed to register message receiver");

    info!("Message receiver registered on node 2");

    // Wait for actor registration to propagate
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Send messages from node 1 to node 2
    info!("Starting message sending test");

    // Test parameters
    let text_message_sizes = vec![500, 5_000, 50_000, 500_000];
    let binary_message_sizes = vec![500, 5_000, 50_000, 500_000];
    let json_message_property_counts = vec![10, 100, 1_000, 10_000];

    // Send text messages (highly compressible)
    info!("Sending text messages (highly compressible)");
    for &size in &text_message_sizes {
        let message = generate_text_message(size);
        let start = Instant::now();

        system1.send_remote(
            &system2.local_node().id,
            "message-receiver",
            message,
            DeliveryGuarantee::AtLeastOnce,
        ).await.expect("Failed to send text message");

        let elapsed = start.elapsed();
        info!("Sent text message of size {} bytes in {:?}", size, elapsed);

        // Wait a bit between messages
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Send binary messages (less compressible)
    info!("Sending binary messages (less compressible)");
    for &size in &binary_message_sizes {
        let message = generate_binary_message(size);
        let start = Instant::now();

        system1.send_remote(
            &system2.local_node().id,
            "message-receiver",
            message,
            DeliveryGuarantee::AtLeastOnce,
        ).await.expect("Failed to send binary message");

        let elapsed = start.elapsed();
        info!("Sent binary message of size {} bytes in {:?}", size, elapsed);

        // Wait a bit between messages
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Send JSON messages (structured, compressible)
    info!("Sending JSON messages (structured, compressible)");
    for &count in &json_message_property_counts {
        let message = generate_json_message(count);
        let json_size = serde_json::to_string(&message.properties).unwrap_or_default().len();
        let start = Instant::now();

        system1.send_remote(
            &system2.local_node().id,
            "message-receiver",
            message,
            DeliveryGuarantee::AtLeastOnce,
        ).await.expect("Failed to send JSON message");

        let elapsed = start.elapsed();
        info!("Sent JSON message with {} properties ({} bytes) in {:?}", count, json_size, elapsed);

        // Wait a bit between messages
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for all messages to be processed
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Display compression statistics
    let stats = get_compression_stats();
    info!("\n{}", stats.summary());

        // Shutdown the clusters
    info!("Shutting down clusters");
    // No need to explicitly stop the systems, they will be dropped at the end of the example

    Ok(())
}
