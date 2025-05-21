//! Adaptive batching system for DataFlare
//!
//! Implements automatic batch size adjustment based on performance metrics
//! to optimize throughput and latency.

use std::time::{Duration, Instant};
use log::{debug, info, warn};

use super::metrics::BatchingMetrics;

/// Configuration for adaptive batching
#[derive(Debug, Clone)]
pub struct AdaptiveBatchingConfig {
    /// Initial batch size
    pub initial_size: usize,
    
    /// Minimum batch size
    pub min_size: usize,
    
    /// Maximum batch size
    pub max_size: usize,
    
    /// Target throughput in records per second
    pub throughput_target: usize,
    
    /// Target latency in milliseconds
    pub latency_target_ms: u64,
    
    /// Adaptation rate (0.0-1.0) - how quickly to adjust batch size
    pub adaptation_rate: f64,
    
    /// Stability threshold - minimum number of batches before adaptation
    pub stability_threshold: usize,
}

impl Default for AdaptiveBatchingConfig {
    fn default() -> Self {
        Self {
            initial_size: 1000,
            min_size: 100,
            max_size: 10000,
            throughput_target: 100000, // 10万记录/秒
            latency_target_ms: 50,     // 50毫秒
            adaptation_rate: 0.2,      // 20%的调整率
            stability_threshold: 3,    // 连续3次测量稳定视为稳定
        }
    }
}

/// Adaptive batcher that dynamically adjusts batch size based on performance
#[derive(Debug)]
pub struct AdaptiveBatcher {
    /// Configuration
    config: AdaptiveBatchingConfig,
    
    /// Current batch size
    current_size: usize,
    
    /// Performance metrics
    metrics: BatchingMetrics,
    
    /// Last batch start time
    last_batch_start: Option<Instant>,
    
    /// Stable count
    stable_count: usize,
}

impl AdaptiveBatcher {
    /// Create a new adaptive batcher with the given configuration
    pub fn new(config: AdaptiveBatchingConfig) -> Self {
        Self {
            current_size: config.initial_size,
            config,
            metrics: BatchingMetrics::new(),
            last_batch_start: None,
            stable_count: 0,
        }
    }
    
    /// Use default configuration to create a batcher
    pub fn default() -> Self {
        Self::new(AdaptiveBatchingConfig::default())
    }
    
    /// Get the current recommended batch size
    pub fn get_batch_size(&self) -> usize {
        self.current_size
    }
    
    /// Start a new batch processing
    pub fn start_batch(&mut self) {
        self.last_batch_start = Some(Instant::now());
    }
    
    /// Complete batch processing and update batch size
    pub fn complete_batch(&mut self, records_processed: usize) {
        if let Some(start_time) = self.last_batch_start {
            let processing_time = start_time.elapsed();
            
            // Record batch processing metrics
            self.metrics.record_batch(
                records_processed, 
                processing_time,
                self.current_size
            );
            
            // Adapt batch size
            self.adapt(processing_time, records_processed);
            
            // Reset start time
            self.last_batch_start = None;
        }
    }
    
    /// Adapt the batch size based on performance metrics
    pub fn adapt(&mut self, processing_time: Duration, records_processed: usize) {
        // Calculate current throughput (records per second)
        let throughput = if processing_time.as_secs_f64() > 0.0 {
            records_processed as f64 / processing_time.as_secs_f64()
        } else {
            0.0 // Avoid division by zero
        };
        
        // Calculate average processing time per record (milliseconds)
        let avg_record_time_ms = if records_processed > 0 {
            processing_time.as_millis() as f64 / records_processed as f64
        } else {
            0.0
        };
        
        // Processing time (milliseconds)
        let processing_ms = processing_time.as_millis() as u64;
        
        // Determine current performance against target
        let throughput_ratio = throughput / self.config.throughput_target as f64;
        let latency_ratio = processing_ms as f64 / self.config.latency_target_ms as f64;
        
        // Decision logic: whether to adjust batch size
        let mut new_size = self.current_size;
        let mut is_stable = false;
        
        if throughput_ratio < 0.8 && latency_ratio > 1.2 {
            // Low throughput and high latency: decrease batch size
            debug!(
                "Batch processing performance poor: throughput={:.2} records/sec, processing time={}ms, decreasing batch size", 
                throughput, processing_ms
            );
            new_size = (self.current_size as f64 * (1.0 - self.config.adaptation_rate)) as usize;
            self.stable_count = 0;
        } else if throughput_ratio < 0.8 && latency_ratio <= 1.0 {
            // Low throughput but normal latency: increase batch size to improve throughput
            debug!(
                "Attempting to improve throughput: current={:.2} records/sec, target={} records/sec, increasing batch size", 
                throughput, self.config.throughput_target
            );
            new_size = (self.current_size as f64 * (1.0 + self.config.adaptation_rate)) as usize;
            self.stable_count = 0;
        } else if throughput_ratio >= 0.8 && latency_ratio > 1.2 {
            // Normal throughput but high latency: decrease batch size
            debug!(
                "Attempting to reduce latency: current={}ms, target={}ms, decreasing batch size",
                processing_ms, self.config.latency_target_ms
            );
            new_size = (self.current_size as f64 * (1.0 - self.config.adaptation_rate)) as usize;
            self.stable_count = 0;
        } else {
            // Throughput and latency within acceptable range: maintain current batch size
            debug!(
                "Batch processing performance good: throughput={:.2} records/sec, processing time={}ms, maintaining batch size={}",
                throughput, processing_ms, self.current_size
            );
            self.stable_count += 1;
            is_stable = self.stable_count >= self.config.stability_threshold;
        }
        
        // Ensure batch size is within configured range
        new_size = new_size.max(self.config.min_size).min(self.config.max_size);
        
        // Update if batch size changes by more than 5%
        if (new_size as f64 - self.current_size as f64).abs() / self.current_size as f64 > 0.05 {
            info!(
                "Adjusting batch size: {} -> {} (throughput={:.2} records/sec, latency={}ms, average record processing time={:.2}ms)",
                self.current_size, new_size, throughput, processing_ms, avg_record_time_ms
            );
            self.current_size = new_size;
        }
        
        // Record stability status
        if is_stable && !self.metrics.is_system_stable() {
            info!("Batch processing system stabilized at size {}, throughput {:.2} records/sec", self.current_size, throughput);
            self.metrics.set_system_stable(true);
        } else if !is_stable && self.metrics.is_system_stable() {
            info!("Batch processing system no longer stable, re-adjusting");
            self.metrics.set_system_stable(false);
        }
    }
    
    /// Get current metrics
    pub fn get_metrics(&self) -> &BatchingMetrics {
        &self.metrics
    }
    
    /// Reset batch processor state
    pub fn reset(&mut self) {
        self.current_size = self.config.initial_size;
        self.metrics.reset();
        self.last_batch_start = None;
        self.stable_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_adaptive_batcher_initialization() {
        let config = AdaptiveBatchingConfig {
            initial_size: 500,
            min_size: 50,
            max_size: 5000,
            throughput_target: 50000,
            latency_target_ms: 100,
            adaptation_rate: 0.1,
            stability_threshold: 5,
        };
        
        let batcher = AdaptiveBatcher::new(config.clone());
        
        // Verify initialization
        assert_eq!(batcher.current_size, config.initial_size);
        assert_eq!(batcher.get_batch_size(), config.initial_size);
    }
    
    #[test]
    fn test_adaptive_batcher_adaptation() {
        let mut batcher = AdaptiveBatcher::default();
        
        // Simulate high latency scenario
        batcher.start_batch();
        // Simulate processing time of 200ms, exceeding target of 50ms
        batcher.complete_batch(1000);
        batcher.adapt(Duration::from_millis(200), 1000);
        
        // Batch size should decrease
        assert!(batcher.get_batch_size() < 1000);
        
        // Reset and simulate low throughput but normal latency scenario
        batcher.reset();
        batcher.start_batch();
        // Simulate processing time of 25ms, but only processed 100 records
        batcher.complete_batch(100);
        batcher.adapt(Duration::from_millis(25), 100);
        
        // Batch size should increase
        assert!(batcher.get_batch_size() > 1000);
    }
    
    #[test]
    fn test_adaptive_batcher_stability() {
        let config = AdaptiveBatchingConfig {
            initial_size: 1000,
            min_size: 100,
            max_size: 10000,
            throughput_target: 100000,
            latency_target_ms: 50,
            adaptation_rate: 0.2,
            stability_threshold: 3,
        };
        
        let mut batcher = AdaptiveBatcher::new(config);
        
        // Simulate ideal performance scenario continuous multiple times
        for _ in 0..4 {
            batcher.start_batch();
            // Process 1000 records, time 40ms, within target
            batcher.complete_batch(1000);
            batcher.adapt(Duration::from_millis(40), 1000);
        }
        
        // System should be marked as stable
        assert!(batcher.get_metrics().is_system_stable());
        
        // Simulate performance decline
        batcher.start_batch();
        batcher.complete_batch(1000);
        batcher.adapt(Duration::from_millis(150), 1000);
        
        // System should no longer be stable
        assert!(!batcher.get_metrics().is_system_stable());
    }
} 