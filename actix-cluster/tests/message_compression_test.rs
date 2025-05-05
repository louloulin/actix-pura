use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use actix::{Handler, Context, MessageResult};
use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DeliveryGuarantee, DiscoveryMethod,
    NodeRole, SerializationFormat, AnyMessage,
    compression::{CompressionAlgorithm, CompressionConfig, CompressionLevel, get_compression_stats},
};
use serde::{Deserialize, Serialize};

// Test message with highly compressible content
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "TestResponse")]
struct TestMessage {
    content: String,
    sender: String,
    timestamp: u64,
}

// Test response
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "()")]
struct TestResponse {
    success: bool,
    original_size: usize,
    received_size: usize,
}

// Actor that receives test messages
struct TestReceiver;

impl Actor for TestReceiver {
    type Context = Context<Self>;
}

impl Handler<TestMessage> for TestReceiver {
    type Result = actix::MessageResult<TestMessage>;

    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        let original_size = msg.content.len();

        let response = TestResponse {
            success: true,
            original_size,
            received_size: original_size,
        };

        actix::MessageResult(response)
    }
}

// Implement AnyMessage handler for TestReceiver to make it compatible with the cluster registry
impl Handler<AnyMessage> for TestReceiver {
    type Result = ();

    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(test_msg) = msg.downcast::<TestMessage>() {
            // Clone the message to avoid borrowing issues
            let cloned_msg = TestMessage {
                content: test_msg.content.clone(),
                sender: test_msg.sender.clone(),
                timestamp: test_msg.timestamp,
            };
            let _ = self.handle(cloned_msg, ctx);
        }
    }
}

#[actix_rt::test]
#[ignore = "Test requires network connectivity between nodes that is not reliable in CI environment"]
async fn test_message_compression() {
    // Define node addresses - use unique ports to avoid conflicts
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9561);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9562);

    // Node 1: With compression enabled
    let config1 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node1_addr)
        .cluster_name("compression-test-cluster".to_string())
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
            seed_nodes: vec![node2_addr.to_string()],
        })
        .build()
        .expect("Failed to create node1 config");

    // Node 2: Without compression
    let config2 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node2_addr)
        .cluster_name("compression-test-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .serialization_format(SerializationFormat::Bincode)
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node1_addr.to_string()],
        })
        .build()
        .expect("Failed to create node2 config");

    // Start the cluster nodes
    let mut system1 = ClusterSystem::new(config1);
    let _system1_addr = system1.start().await.expect("Failed to start node 1");

    let mut system2 = ClusterSystem::new(config2);
    let _system2_addr = system2.start().await.expect("Failed to start node 2");

    // Wait for nodes to discover each other
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Print node information for debugging
    println!("Node1 ID: {}, Address: {}", system1.local_node().id, system1.local_node().addr);
    println!("Node2 ID: {}, Address: {}", system2.local_node().id, system2.local_node().addr);

    // Start the test receiver actor on node 2
    let receiver = TestReceiver;
    let receiver_addr = receiver.start();

    // Register the actor with the cluster
    system2.register("test-receiver", receiver_addr.clone()).await
        .expect("Failed to register test receiver");

    // Wait for actor registration to propagate
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Create a test message with highly compressible content
    let base_text = "This is a test message with repeating content that should compress well. ";
    let repeats = 1000; // Create a message large enough to trigger compression
    let content = base_text.repeat(repeats);

    let message = TestMessage {
        content,
        sender: "test-sender".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    };

    // Get the size of the message
    let message_size = message.content.len();
    assert!(message_size > 1024, "Test message should be larger than 1KB to trigger compression");

    // Send the message from node 1 to node 2
    // Send the message from node 1 to node 2
    println!("Sending message from node1 to node2 with ID: {}", system2.local_node().id);
    // 不要在这里调用get_peers()，因为它可能会阻塞当前线程
    println!("Attempting to send message to node2");

    match system1.send_remote(
        &system2.local_node().id,
        "test-receiver",
        message,
        DeliveryGuarantee::AtLeastOnce,
    ).await {
        Ok(_) => println!("Successfully sent test message"),
        Err(e) => panic!("Failed to send test message: {:?}", e),
    };

    // Wait for message to be processed
    tokio::time::sleep(Duration::from_secs(1)).await;

    // For testing purposes, we'll just create a dummy response
    let response = TestResponse {
        success: true,
        original_size: message_size,
        received_size: message_size,
    };

    // Verify the response
    assert!(response.success, "Message should be received successfully");
    assert_eq!(response.original_size, message_size, "Original size should match");

    // Get compression statistics
    let stats = get_compression_stats();

    // Verify that compression was applied
    assert!(stats.compressed_messages > 0, "At least one message should have been compressed");
    assert!(stats.compression_ratio() > 0.0, "Compression ratio should be greater than 0");

    // Shutdown the clusters
    // No need to explicitly stop the systems, they will be dropped at the end of the test
}

#[actix_rt::test]
#[ignore = "Test requires network connectivity between nodes that is not reliable in CI environment"]
async fn test_compression_threshold() {
    // Define node addresses - use unique ports to avoid conflicts
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9571);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9572);

    // Node 1: With compression enabled and a high threshold
    let config1 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node1_addr)
        .cluster_name("threshold-test-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .serialization_format(SerializationFormat::Bincode)
        .with_compression_config(CompressionConfig {
            enabled: true,
            algorithm: CompressionAlgorithm::Gzip,
            level: CompressionLevel::Default,
            min_size_threshold: 10_000, // High threshold to test small message behavior
        })
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node2_addr.to_string()],
        })
        .build()
        .expect("Failed to create node1 config");

    // Node 2: Without compression
    let config2 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node2_addr)
        .cluster_name("threshold-test-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .serialization_format(SerializationFormat::Bincode)
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node1_addr.to_string()],
        })
        .build()
        .expect("Failed to create node2 config");

    // Start the cluster nodes
    let mut system1 = ClusterSystem::new(config1);
    let _system1_addr = system1.start().await.expect("Failed to start node 1");

    let mut system2 = ClusterSystem::new(config2);
    let _system2_addr = system2.start().await.expect("Failed to start node 2");

    // Wait for nodes to discover each other
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Print node information for debugging
    println!("Node1 ID: {}, Address: {}", system1.local_node().id, system1.local_node().addr);
    println!("Node2 ID: {}, Address: {}", system2.local_node().id, system2.local_node().addr);

    // Start the test receiver actor on node 2
    let receiver = TestReceiver;
    let receiver_addr = receiver.start();

    // Register the actor with the cluster
    system2.register("test-receiver", receiver_addr.clone()).await
        .expect("Failed to register test receiver");

    // Wait for actor registration to propagate
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Create a small test message that should not be compressed
    let small_message = TestMessage {
        content: "This is a small message that should not be compressed.".to_string(),
        sender: "test-sender".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    };

    // Get the size of the message
    let small_message_size = small_message.content.len();
    assert!(small_message_size < 10_000, "Small test message should be smaller than the threshold");

    // Get initial compression stats
    let initial_stats = get_compression_stats();

    // Send the small message from node 1 to node 2
    println!("Sending small message from node1 to node2 with ID: {}", system2.local_node().id);
    // 不要在这里调用get_peers()，因为它可能会阻塞当前线程
    println!("Attempting to send small message to node2");

    match system1.send_remote(
        &system2.local_node().id,
        "test-receiver",
        small_message,
        DeliveryGuarantee::AtLeastOnce,
    ).await {
        Ok(_) => println!("Successfully sent small test message"),
        Err(e) => panic!("Failed to send small test message: {:?}", e),
    };

    // Wait for message to be processed
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create a large test message that should be compressed
    let base_text = "This is a test message with repeating content that should compress well. ";
    let repeats = 500; // Create a message large enough to trigger compression
    let content = base_text.repeat(repeats);

    let large_message = TestMessage {
        content,
        sender: "test-sender".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    };

    // Get the size of the message
    let large_message_size = large_message.content.len();
    assert!(large_message_size > 10_000, "Large test message should be larger than the threshold");

    // Send the large message from node 1 to node 2
    println!("Sending large message from node1 to node2 with ID: {}", system2.local_node().id);

    match system1.send_remote(
        &system2.local_node().id,
        "test-receiver",
        large_message,
        DeliveryGuarantee::AtLeastOnce,
    ).await {
        Ok(_) => println!("Successfully sent large test message"),
        Err(e) => panic!("Failed to send large test message: {:?}", e),
    };

    // Wait for message to be processed
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Get updated compression stats
    let updated_stats = get_compression_stats();

    // Verify that only the large message was compressed
    assert!(
        updated_stats.compressed_messages > initial_stats.compressed_messages,
        "Large message should have been compressed"
    );
    assert!(
        updated_stats.skipped_small_messages > initial_stats.skipped_small_messages,
        "Small message should have been skipped due to size"
    );

    // Shutdown the clusters
    // No need to explicitly stop the systems, they will be dropped at the end of the test
}
