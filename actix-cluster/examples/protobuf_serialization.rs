use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem,
    NodeRole, SerializationFormat, DiscoveryMethod
};
use actix_cluster::serialization::{Serializer};
use actix_cluster::message::{MessageEnvelope, MessageType, DeliveryGuarantee};
use actix_cluster::node::NodeId;
use actix_cluster::proto;
use prost::Message;
use serde::{Serialize, Deserialize};
use serde_json;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

// Define a simple test message
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
struct TestMessage {
    id: u64,
    name: String,
    data: Vec<u8>,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    println!("Protocol Buffers Serialization Example");
    println!("=====================================");

    // Create test data
    let test_message = TestMessage {
        id: 42,
        name: "Test Message".to_string(),
        data: vec![0u8; 10000], // 10KB of data
    };

    // Create a message envelope
    let sender_node = NodeId::new();
    let target_node = NodeId::new();
    let envelope = MessageEnvelope::new(
        sender_node,
        target_node,
        "/user/target".to_string(),
        MessageType::ActorMessage,
        DeliveryGuarantee::AtLeastOnce,
        serde_json::to_vec(&test_message).unwrap(),
    );

    // Create serializers for different formats
    let bincode_serializer = Serializer::from_format(SerializationFormat::Bincode);
    let json_serializer = Serializer::from_format(SerializationFormat::Json);
    let _protobuf_serializer = Serializer::from_format(SerializationFormat::Protobuf);
    let compressed_bincode_serializer = Serializer::from_format(SerializationFormat::CompressedBincode);
    let compressed_json_serializer = Serializer::from_format(SerializationFormat::CompressedJson);
    let _compressed_protobuf_serializer = Serializer::from_format(SerializationFormat::CompressedProtobuf);

    // Benchmark serialization
    println!("\nSerialization Benchmark:");
    println!("------------------------");

    // Bincode
    let start = Instant::now();
    let bincode_result = bincode_serializer.serialize(&envelope).unwrap();
    let bincode_time = start.elapsed();
    println!("Bincode: {} bytes in {:?}", bincode_result.len(), bincode_time);

    // JSON
    let start = Instant::now();
    let json_result = json_serializer.serialize(&envelope).unwrap();
    let json_time = start.elapsed();
    println!("JSON: {} bytes in {:?}", json_result.len(), json_time);

    // Protocol Buffers
    // For Protocol Buffers, we need to convert to a proto message first
    let proto_envelope = proto::ProtoMessageEnvelope::try_from(envelope.clone()).unwrap();
    let start = Instant::now();
    let mut buf = Vec::new();
    buf.reserve(proto_envelope.encoded_len());
    proto_envelope.encode(&mut buf).unwrap();
    let protobuf_time = start.elapsed();
    println!("Protocol Buffers: {} bytes in {:?}", buf.len(), protobuf_time);

    // Compressed formats
    let start = Instant::now();
    let compressed_bincode_result = compressed_bincode_serializer.serialize(&envelope).unwrap();
    let compressed_bincode_time = start.elapsed();
    println!("Compressed Bincode: {} bytes in {:?}", compressed_bincode_result.len(), compressed_bincode_time);

    let start = Instant::now();
    let compressed_json_result = compressed_json_serializer.serialize(&envelope).unwrap();
    let compressed_json_time = start.elapsed();
    println!("Compressed JSON: {} bytes in {:?}", compressed_json_result.len(), compressed_json_time);

    // For Protocol Buffers with compression
    let start = Instant::now();
    let mut proto_buf = Vec::new();
    proto_buf.reserve(proto_envelope.encoded_len());
    proto_envelope.encode(&mut proto_buf).unwrap();
    let compressed_proto_result = actix_cluster::compression::compress(
        &proto_buf,
        actix_cluster::compression::CompressionAlgorithm::Gzip,
        actix_cluster::compression::CompressionLevel::Default,
    ).unwrap();
    let compressed_protobuf_time = start.elapsed();
    println!("Compressed Protocol Buffers: {} bytes in {:?}", compressed_proto_result.len(), compressed_protobuf_time);

    // Summary
    println!("\nSize Comparison:");
    println!("---------------");
    println!("Bincode: {} bytes", bincode_result.len());
    println!("JSON: {} bytes", json_result.len());
    println!("Protocol Buffers: {} bytes", buf.len());
    println!("Compressed Bincode: {} bytes", compressed_bincode_result.len());
    println!("Compressed JSON: {} bytes", compressed_json_result.len());
    println!("Compressed Protocol Buffers: {} bytes", compressed_proto_result.len());

    // Calculate compression ratios
    println!("\nCompression Ratios:");
    println!("------------------");
    println!("Bincode: {:.2}%", 100.0 * (1.0 - (compressed_bincode_result.len() as f64 / bincode_result.len() as f64)));
    println!("JSON: {:.2}%", 100.0 * (1.0 - (compressed_json_result.len() as f64 / json_result.len() as f64)));
    println!("Protocol Buffers: {:.2}%", 100.0 * (1.0 - (compressed_proto_result.len() as f64 / buf.len() as f64)));

    // Compare to original data size
    let original_data_size = test_message.data.len();
    println!("\nOriginal Data Size: {} bytes", original_data_size);
    println!("Overhead Comparison:");
    println!("-------------------");
    println!("Bincode Overhead: {:.2}%", 100.0 * ((bincode_result.len() as f64 / original_data_size as f64) - 1.0));
    println!("JSON Overhead: {:.2}%", 100.0 * ((json_result.len() as f64 / original_data_size as f64) - 1.0));
    println!("Protocol Buffers Overhead: {:.2}%", 100.0 * ((buf.len() as f64 / original_data_size as f64) - 1.0));
    println!("Compressed Bincode Overhead: {:.2}%", 100.0 * ((compressed_bincode_result.len() as f64 / original_data_size as f64) - 1.0));
    println!("Compressed JSON Overhead: {:.2}%", 100.0 * ((compressed_json_result.len() as f64 / original_data_size as f64) - 1.0));
    println!("Compressed Protocol Buffers Overhead: {:.2}%", 100.0 * ((compressed_proto_result.len() as f64 / original_data_size as f64) - 1.0));

    // Now let's create a cluster with Protocol Buffers serialization
    println!("\nStarting a cluster with Protocol Buffers serialization...");

    let config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558))
        .discovery(DiscoveryMethod::Static { seed_nodes: vec![] })
        .heartbeat_interval(Duration::from_secs(1))
        .node_timeout(Duration::from_secs(5))
        .serialization_format(SerializationFormat::Protobuf) // Now using Protocol Buffers
        .cluster_name("protobuf-example".to_string())
        .build()
        .expect("Failed to build cluster config");

    let mut system = ClusterSystem::new(config);
    let _system_addr = system.start().await.expect("Failed to start cluster system");

    println!("Cluster started with Protocol Buffers serialization");

    // Keep the system running for a while
    tokio::time::sleep(Duration::from_secs(5)).await;

    println!("Example completed");
}
