use std::time::{Duration, Instant};
use actix_cluster::compression::{CompressionAlgorithm, CompressionConfig, CompressionLevel};
use actix_cluster::serialization::{BincodeSerializer, SerializerTrait};
use serde::{Deserialize, Serialize};

/// 测试消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestMessage {
    /// 测试数据
    data: Vec<u8>,
    /// 消息ID
    id: u32,
    /// 发送时间戳
    timestamp: u64,
}

/// 直接测试压缩功能
fn main() {
    println!("============ Message Compression Test ============");
    
    // 创建一个大消息进行测试 - 5MB 的随机数据
    let message_size = 5 * 1024 * 1024; // 5MB
    println!("Creating test message with size: {}B", message_size);
    
    let test_message = TestMessage {
        data: vec![0u8; message_size], // 实际应用中这里应该是真实数据
        id: 1,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    
    // 创建序列化器
    let serializer = BincodeSerializer::new();
    
    // 不启用压缩的序列化
    println!("\n--- Test 1: Without compression ---");
    let start = Instant::now();
    let serialized = serializer.serialize(&test_message).expect("Serialization failed");
    let duration = start.elapsed();
    println!("Serialization took: {:?}", duration);
    println!("Serialized size: {}B", serialized.len());
    
    // 压缩配置 - 使用Gzip，默认压缩级别
    let compression_config = CompressionConfig {
        enabled: true,
        algorithm: CompressionAlgorithm::Gzip,
        level: CompressionLevel::Default,
        min_size_threshold: 1024, // 对大于1KB的消息进行压缩
    };
    
    // 使用不同的压缩级别测试压缩率和性能
    test_compression(&test_message, &serializer, CompressionLevel::Fast, "Fast");
    test_compression(&test_message, &serializer, CompressionLevel::Default, "Default");
    test_compression(&test_message, &serializer, CompressionLevel::Best, "Best");
    
    println!("\n============ Test Completed ============");
}

/// 测试不同压缩级别的性能和压缩率
fn test_compression(
    message: &TestMessage, 
    serializer: &BincodeSerializer,
    level: CompressionLevel,
    level_name: &str
) {
    println!("\n--- Test with {} compression ---", level_name);
    
    // 首先序列化消息
    let start_serialize = Instant::now();
    let serialized = serializer.serialize(message).expect("Serialization failed");
    let serialize_duration = start_serialize.elapsed();
    println!("Serialization took: {:?}", serialize_duration);
    println!("Serialized size: {}B", serialized.len());
    
    // 创建压缩配置
    let compression_config = CompressionConfig {
        enabled: true,
        algorithm: CompressionAlgorithm::Gzip,
        level,
        min_size_threshold: 1024, // 对大于1KB的消息进行压缩
    };
    
    // 压缩数据
    let start_compress = Instant::now();
    let (compressed, was_compressed) = actix_cluster::compression::auto_compress(
        &serialized, 
        &compression_config
    ).expect("Compression failed");
    let compress_duration = start_compress.elapsed();
    
    if was_compressed {
        let compression_ratio = (1.0 - (compressed.len() as f64 / serialized.len() as f64)) * 100.0;
        println!("Compression took: {:?}", compress_duration);
        println!("Compressed size: {}B", compressed.len());
        println!("Compression ratio: {:.2}%", compression_ratio);
        
        // 测试解压缩
        let start_decompress = Instant::now();
        let decompressed = actix_cluster::compression::decompress(
            &compressed, 
            compression_config.algorithm
        ).expect("Decompression failed");
        let decompress_duration = start_decompress.elapsed();
        
        println!("Decompression took: {:?}", decompress_duration);
        println!("Decompressed size: {}B", decompressed.len());
        assert_eq!(decompressed, serialized, "Decompressed data does not match original");
    } else {
        println!("Auto-compression decided not to compress the data");
    }
} 