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

/// Compression statistics for monitoring and debugging
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Total number of messages processed
    pub total_messages: u64,
    /// Number of messages that were compressed
    pub compressed_messages: u64,
    /// Total bytes before compression
    pub total_bytes_before: u64,
    /// Total bytes after compression
    pub total_bytes_after: u64,
    /// Largest compression ratio achieved (percentage)
    pub max_compression_ratio: f64,
    /// Average compression ratio (percentage)
    pub avg_compression_ratio: f64,
    /// Number of messages that were not compressed due to size threshold
    pub skipped_small_messages: u64,
    /// Number of messages that were not compressed because compression was ineffective
    pub skipped_ineffective: u64,
}

impl CompressionStats {
    /// Create a new CompressionStats instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Update stats with a new compression result
    pub fn update(&mut self, original_size: usize, compressed_size: usize, was_compressed: bool) {
        self.total_messages += 1;
        self.total_bytes_before += original_size as u64;

        if was_compressed {
            self.compressed_messages += 1;
            self.total_bytes_after += compressed_size as u64;

            // Calculate compression ratio
            let ratio = 100.0 * (1.0 - (compressed_size as f64 / original_size as f64));

            // Update max ratio if this is better
            if ratio > self.max_compression_ratio {
                self.max_compression_ratio = ratio;
            }

            // Recalculate average ratio
            if self.compressed_messages > 0 {
                self.avg_compression_ratio = 100.0 * (1.0 - (self.total_bytes_after as f64 / self.total_bytes_before as f64));
            }
        } else {
            // Not compressed, add original size to after bytes
            self.total_bytes_after += original_size as u64;

            // Track why it wasn't compressed
            if original_size < 1024 { // Assuming default threshold
                self.skipped_small_messages += 1;
            } else {
                self.skipped_ineffective += 1;
            }
        }
    }

    /// Get the current compression ratio
    pub fn compression_ratio(&self) -> f64 {
        if self.total_bytes_before == 0 {
            return 0.0;
        }
        100.0 * (1.0 - (self.total_bytes_after as f64 / self.total_bytes_before as f64))
    }

    /// Get the percentage of messages that were compressed
    pub fn compression_percentage(&self) -> f64 {
        if self.total_messages == 0 {
            return 0.0;
        }
        100.0 * (self.compressed_messages as f64 / self.total_messages as f64)
    }

    /// Get a summary of the compression statistics
    pub fn summary(&self) -> String {
        format!(
            "Compression Stats:\n\
             - Total messages: {}\n\
             - Compressed messages: {} ({:.2}%)\n\
             - Total bytes before: {}\n\
             - Total bytes after: {}\n\
             - Overall compression ratio: {:.2}%\n\
             - Max compression ratio: {:.2}%\n\
             - Skipped (small): {}\n\
             - Skipped (ineffective): {}",
            self.total_messages,
            self.compressed_messages,
            self.compression_percentage(),
            self.total_bytes_before,
            self.total_bytes_after,
            self.compression_ratio(),
            self.max_compression_ratio,
            self.skipped_small_messages,
            self.skipped_ineffective
        )
    }
}

use std::sync::atomic::{AtomicU64, Ordering};

/// Global compression statistics counters
static TOTAL_MESSAGES: AtomicU64 = AtomicU64::new(0);
static COMPRESSED_MESSAGES: AtomicU64 = AtomicU64::new(0);
static TOTAL_BYTES_BEFORE: AtomicU64 = AtomicU64::new(0);
static TOTAL_BYTES_AFTER: AtomicU64 = AtomicU64::new(0);
static MAX_COMPRESSION_RATIO: AtomicU64 = AtomicU64::new(0); // Stored as ratio * 100
static SKIPPED_SMALL_MESSAGES: AtomicU64 = AtomicU64::new(0);
static SKIPPED_INEFFECTIVE: AtomicU64 = AtomicU64::new(0);

/// Update the compression statistics
fn update_compression_stats(original_size: usize, compressed_size: usize, was_compressed: bool) {
    TOTAL_MESSAGES.fetch_add(1, Ordering::Relaxed);
    TOTAL_BYTES_BEFORE.fetch_add(original_size as u64, Ordering::Relaxed);

    if was_compressed {
        COMPRESSED_MESSAGES.fetch_add(1, Ordering::Relaxed);
        TOTAL_BYTES_AFTER.fetch_add(compressed_size as u64, Ordering::Relaxed);

        // Calculate compression ratio (as percentage * 100)
        if original_size > 0 {
            let ratio = ((1.0 - (compressed_size as f64 / original_size as f64)) * 10000.0) as u64;
            let current_max = MAX_COMPRESSION_RATIO.load(Ordering::Relaxed);
            if ratio > current_max {
                MAX_COMPRESSION_RATIO.store(ratio, Ordering::Relaxed);
            }
        }
    } else {
        TOTAL_BYTES_AFTER.fetch_add(original_size as u64, Ordering::Relaxed);

        // Track why it wasn't compressed
        if original_size < 1024 { // Assuming default threshold
            SKIPPED_SMALL_MESSAGES.fetch_add(1, Ordering::Relaxed);
        } else {
            SKIPPED_INEFFECTIVE.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Get a copy of the current compression statistics
pub fn get_compression_stats() -> CompressionStats {
    let total_messages = TOTAL_MESSAGES.load(Ordering::Relaxed);
    let compressed_messages = COMPRESSED_MESSAGES.load(Ordering::Relaxed);
    let total_bytes_before = TOTAL_BYTES_BEFORE.load(Ordering::Relaxed);
    let total_bytes_after = TOTAL_BYTES_AFTER.load(Ordering::Relaxed);
    let max_compression_ratio = MAX_COMPRESSION_RATIO.load(Ordering::Relaxed) as f64 / 100.0;
    let skipped_small_messages = SKIPPED_SMALL_MESSAGES.load(Ordering::Relaxed);
    let skipped_ineffective = SKIPPED_INEFFECTIVE.load(Ordering::Relaxed);

    // Calculate average compression ratio
    let avg_compression_ratio = if total_bytes_before > 0 && total_bytes_after <= total_bytes_before {
        100.0 * (1.0 - (total_bytes_after as f64 / total_bytes_before as f64))
    } else {
        0.0
    };

    CompressionStats {
        total_messages,
        compressed_messages,
        total_bytes_before,
        total_bytes_after,
        max_compression_ratio,
        avg_compression_ratio,
        skipped_small_messages,
        skipped_ineffective,
    }
}

/// Automatically compress data if it meets the threshold
pub fn auto_compress(
    data: &[u8],
    config: &CompressionConfig
) -> ClusterResult<(Vec<u8>, bool)> {
    let original_size = data.len();
    let mut was_compressed = false;
    let result;

    if config.should_compress(original_size) {
        let compressed = compress(data, config.algorithm, config.level)?;
        // 只有当压缩确实减小了数据大小时才使用压缩结果
        if compressed.len() < original_size {
            was_compressed = true;
            result = compressed;
        } else {
            result = data.to_vec();
        }
    } else {
        result = data.to_vec();
    }

    // Update compression statistics
    update_compression_stats(original_size, result.len(), was_compressed);

    Ok((result, was_compressed))
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