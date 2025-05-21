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
    pub throughput_target: Option<usize>,
    
    /// Target latency in milliseconds
    pub latency_target_ms: Option<u64>,
    
    /// Adaptation rate (0.0-1.0) - how quickly to adjust batch size
    pub adaptation_rate: f64,
    
    /// Evaluation interval - how often to adjust batch size
    pub evaluation_interval: Duration,
    
    /// Stability threshold - minimum number of batches before adaptation
    pub stability_threshold: usize,
}

impl Default for AdaptiveBatchingConfig {
    fn default() -> Self {
        Self {
            initial_size: 1000,
            min_size: 10,
            max_size: 100000,
            throughput_target: None,
            latency_target_ms: Some(50), // Default to 50ms target latency
            adaptation_rate: 0.2,
            evaluation_interval: Duration::from_secs(5),
            stability_threshold: 10,
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
    
    /// Last evaluation time
    last_evaluation: Instant,
    
    /// Number of batches processed since last adjustment
    batches_since_adjustment: usize,
}

impl AdaptiveBatcher {
    /// Create a new adaptive batcher with the given configuration
    pub fn new(config: AdaptiveBatchingConfig) -> Self {
        Self {
            current_size: config.initial_size,
            config,
            metrics: BatchingMetrics::new(),
            last_evaluation: Instant::now(),
            batches_since_adjustment: 0,
        }
    }
    
    /// Get the current recommended batch size
    pub fn batch_size(&self) -> usize {
        self.current_size
    }
    
    /// Record a batch processing event
    pub fn record_batch(&mut self, batch_size: usize, processing_time: Duration) {
        self.metrics.record_batch(batch_size, processing_time);
        self.batches_since_adjustment += 1;
        
        // Check if it's time to evaluate and adapt
        if self.last_evaluation.elapsed() >= self.config.evaluation_interval && 
           self.batches_since_adjustment >= self.config.stability_threshold {
            self.adapt();
            self.last_evaluation = Instant::now();
            self.batches_since_adjustment = 0;
        }
    }
    
    /// Adapt the batch size based on performance metrics
    pub fn adapt(&mut self) {
        // Get current performance metrics
        let current_throughput = self.metrics.throughput();
        let current_latency = self.metrics.average_latency();
        
        // Calculate adjustment factor
        let mut adjustment_factor = 1.0; // No change by default
        
        // Adjust for throughput if target is specified
        if let Some(target_throughput) = self.config.throughput_target {
            if let Some(throughput) = current_throughput {
                // If throughput is below target, increase batch size
                // If throughput is above target, we're good
                if throughput < target_throughput as f64 {
                    let throughput_ratio = target_throughput as f64 / throughput;
                    // Cap the adjustment to avoid extreme changes
                    let capped_ratio = throughput_ratio.min(2.0);
                    adjustment_factor *= capped_ratio;
                }
            }
        }
        
        // Adjust for latency if target is specified
        if let Some(target_latency_ms) = self.config.latency_target_ms {
            if let Some(latency) = current_latency {
                let latency_ms = latency.as_millis() as u64;
                // If latency is above target, decrease batch size
                // If latency is below target and we're not already increasing for throughput,
                // we can increase batch size
                if latency_ms > target_latency_ms {
                    let latency_ratio = target_latency_ms as f64 / latency_ms as f64;
                    // Cap the adjustment to avoid extreme changes
                    let capped_ratio = latency_ratio.max(0.5);
                    adjustment_factor *= capped_ratio;
                } else if latency_ms < target_latency_ms / 2 && adjustment_factor == 1.0 {
                    // If latency is less than half the target, we can process larger batches
                    adjustment_factor *= 1.1; // Increase by 10%
                }
            }
        }
        
        // Apply smoothing using adaptation rate
        adjustment_factor = 1.0 + (adjustment_factor - 1.0) * self.config.adaptation_rate;
        
        // Calculate new batch size
        let new_size = (self.current_size as f64 * adjustment_factor).round() as usize;
        
        // Clamp to min/max
        let new_size = new_size.clamp(self.config.min_size, self.config.max_size);
        
        // Log the adjustment
        if new_size != self.current_size {
            info!(
                "Adapting batch size from {} to {} (adjustment factor: {:.2}, throughput: {:?} rps, latency: {:?})",
                self.current_size,
                new_size,
                adjustment_factor,
                current_throughput.map(|t| t.round() as usize),
                current_latency.map(|d| format!("{}ms", d.as_millis())),
            );
        }
        
        self.current_size = new_size;
        
        // Reset metrics for the next evaluation period
        self.metrics.reset();
    }
    
    /// Get current metrics
    pub fn metrics(&self) -> &BatchingMetrics {
        &self.metrics
    }
    
    /// Reset metrics and adaptation state
    pub fn reset(&mut self) {
        self.metrics.reset();
        self.current_size = self.config.initial_size;
        self.last_evaluation = Instant::now();
        self.batches_since_adjustment = 0;
    }
} 