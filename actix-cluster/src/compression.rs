//! Compression module for message payloads.
//!
//! This module provides compression and decompression functionality 
//! for message payloads to reduce network traffic.

use std::io::{self, Read, Write};
use flate2::read::{GzDecoder, GzEncoder};
use flate2::Compression;
use serde::{Serialize, Deserialize};
use crate::error::{ClusterError, ClusterResult};

/// Compression level for message payloads
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionLevel {
    /// No compression
    None,
    /// Fast compression (less CPU, larger size)
    Fast,
    /// Default compression (balanced)
    Default,
    /// Best compression (more CPU, smaller size)
    Best,
}

impl Default for CompressionLevel {
    fn default() -> Self {
        CompressionLevel::Default
    }
}

impl CompressionLevel {
    /// Convert to flate2 Compression level
    pub fn to_flate2_level(&self) -> Compression {
        match self {
            CompressionLevel::None => Compression::none(),
            CompressionLevel::Fast => Compression::fast(),
            CompressionLevel::Default => Compression::default(),
            CompressionLevel::Best => Compression::best(),
        }
    }
}

/// Compression algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    /// Gzip compression
    Gzip,
    // 可以在未来添加更多压缩算法
}

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        CompressionAlgorithm::Gzip
    }
}

/// Configuration for compression
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Whether compression is enabled
    pub enabled: bool,
    /// Compression algorithm to use
    pub algorithm: CompressionAlgorithm,
    /// Compression level
    pub level: CompressionLevel,
    /// Minimum message size for auto-compression (in bytes)
    pub min_size_threshold: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            algorithm: CompressionAlgorithm::Gzip,
            level: CompressionLevel::Default,
            min_size_threshold: 1024, // 默认1KB及以上消息才压缩
        }
    }
}

impl CompressionConfig {
    /// Create a new compression configuration
    pub fn new(
        enabled: bool, 
        algorithm: CompressionAlgorithm,
        level: CompressionLevel,
        min_size_threshold: usize,
    ) -> Self {
        Self {
            enabled,
            algorithm,
            level,
            min_size_threshold,
        }
    }
    
    /// Check if a message should be compressed based on its size
    pub fn should_compress(&self, data_size: usize) -> bool {
        self.enabled && data_size >= self.min_size_threshold
    }
}

/// Compress data using the configured algorithm and level
pub fn compress(
    data: &[u8], 
    algorithm: CompressionAlgorithm, 
    level: CompressionLevel
) -> ClusterResult<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::Gzip => {
            let mut compressed = Vec::new();
            let mut encoder = GzEncoder::new(data, level.to_flate2_level());
            encoder.read_to_end(&mut compressed)
                .map_err(|e| ClusterError::CompressionError(e.to_string()))?;
            Ok(compressed)
        }
    }
}

/// Decompress data using the specified algorithm
pub fn decompress(
    data: &[u8], 
    algorithm: CompressionAlgorithm
) -> ClusterResult<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::Gzip => {
            let mut decompressed = Vec::new();
            let mut decoder = GzDecoder::new(data);
            decoder.read_to_end(&mut decompressed)
                .map_err(|e| ClusterError::DecompressionError(e.to_string()))?;
            Ok(decompressed)
        }
    }
}

/// Automatically compress data if it meets the threshold
pub fn auto_compress(
    data: &[u8], 
    config: &CompressionConfig
) -> ClusterResult<(Vec<u8>, bool)> {
    if config.should_compress(data.len()) {
        let compressed = compress(data, config.algorithm, config.level)?;
        // 只有当压缩确实减小了数据大小时才使用压缩结果
        if compressed.len() < data.len() {
            return Ok((compressed, true));
        }
    }
    
    // 如果不需要压缩或压缩无效，返回原始数据
    Ok((data.to_vec(), false))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compression_config() {
        let config = CompressionConfig::default();
        assert!(config.enabled);
        assert_eq!(config.algorithm, CompressionAlgorithm::Gzip);
        assert_eq!(config.level, CompressionLevel::Default);
        
        assert!(config.should_compress(2048));
        assert!(!config.should_compress(512));
        
        let custom_config = CompressionConfig::new(
            true,
            CompressionAlgorithm::Gzip,
            CompressionLevel::Best,
            4096
        );
        assert!(custom_config.should_compress(8192));
        assert!(!custom_config.should_compress(2048));
    }
    
    #[test]
    fn test_compression_decompression() {
        let test_data = vec![b'a'; 10000]; // 创建一个可压缩的测试数据
        
        let compressed = compress(
            &test_data, 
            CompressionAlgorithm::Gzip, 
            CompressionLevel::Default
        ).unwrap();
        
        // 验证压缩是有效的
        assert!(compressed.len() < test_data.len());
        
        let decompressed = decompress(
            &compressed,
            CompressionAlgorithm::Gzip
        ).unwrap();
        
        // 验证解压后与原始数据一致
        assert_eq!(decompressed, test_data);
    }
    
    #[test]
    fn test_auto_compress() {
        let config = CompressionConfig::default();
        
        // 测试小数据不压缩
        let small_data = vec![1, 2, 3, 4, 5];
        let (result, compressed) = auto_compress(&small_data, &config).unwrap();
        assert!(!compressed);
        assert_eq!(result, small_data);
        
        // 测试大数据自动压缩
        let large_data = vec![b'a'; 10000]; // 创建一个可压缩的大数据
        let (result, compressed) = auto_compress(&large_data, &config).unwrap();
        assert!(compressed);
        assert!(result.len() < large_data.len());
        
        // 测试不可压缩的数据
        let random_data = (0..2000).map(|_| rand::random::<u8>()).collect::<Vec<_>>();
        let (result, compressed) = auto_compress(&random_data, &config).unwrap();
        
        // 对于随机数据，可能无法有效压缩，所以我们只需验证结果是正确的
        if compressed {
            let decompressed = decompress(&result, CompressionAlgorithm::Gzip).unwrap();
            assert_eq!(decompressed, random_data);
        } else {
            assert_eq!(result, random_data);
        }
    }
    
    #[test]
    fn test_disabled_compression() {
        let mut config = CompressionConfig::default();
        config.enabled = false;
        
        let data = vec![b'a'; 10000]; // 创建一个可压缩的测试数据
        let (result, compressed) = auto_compress(&data, &config).unwrap();
        
        assert!(!compressed);
        assert_eq!(result, data);
    }
} 