//! Backpressure control system for DataFlare
//!
//! Implements credit-based backpressure to prevent overload situations
//! in data processing pipelines.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use log::{debug, info, warn};

/// Credit allocation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditMode {
    /// Fixed credit allocation
    Fixed,
    
    /// Dynamic credit allocation based on processing rate
    Dynamic,
    
    /// Adaptive credit allocation based on system metrics
    Adaptive,
}

/// Credit allocation configuration
#[derive(Debug, Clone)]
pub struct CreditConfig {
    /// Credit allocation mode
    pub mode: CreditMode,
    
    /// Initial credit allocation per task
    pub initial_credits: usize,
    
    /// Minimum credit allocation per task
    pub min_credits: usize,
    
    /// Maximum credit allocation per task
    pub max_credits: usize,
    
    /// Credit refill rate (credits per second)
    pub refill_rate: f64,
    
    /// Credit refill interval
    pub refill_interval: Duration,
    
    /// Memory pressure threshold (0.0-1.0)
    pub memory_threshold: f64,
}

impl Default for CreditConfig {
    fn default() -> Self {
        Self {
            mode: CreditMode::Dynamic,
            initial_credits: 1000,
            min_credits: 10,
            max_credits: 10000,
            refill_rate: 500.0,  // 500 credits per second
            refill_interval: Duration::from_millis(100),
            memory_threshold: 0.7,  // 70% memory usage
        }
    }
}

/// Credit allocation for a task
#[derive(Debug, Clone)]
struct CreditAllocation {
    /// Current credits
    credits: usize,
    
    /// Maximum credits
    max_credits: usize,
    
    /// Last refill time
    last_refill: Instant,
    
    /// Processing rate (records per second)
    processing_rate: f64,
    
    /// Refill rate (credits per second)
    refill_rate: f64,
}

/// Backpressure controller using credit-based flow control
#[derive(Debug)]
pub struct BackpressureController {
    /// Configuration
    config: CreditConfig,
    
    /// Credit allocations by task ID
    allocations: HashMap<String, CreditAllocation>,
    
    /// System memory pressure (0.0-1.0)
    memory_pressure: f64,
    
    /// Last update time
    last_update: Instant,
}

impl BackpressureController {
    /// Create a new backpressure controller
    pub fn new(config: CreditConfig) -> Self {
        Self {
            config,
            allocations: HashMap::new(),
            memory_pressure: 0.0,
            last_update: Instant::now(),
        }
    }
    
    /// Register a task with the controller
    pub fn register_task(&mut self, task_id: &str) {
        if !self.allocations.contains_key(task_id) {
            self.allocations.insert(
                task_id.to_string(),
                CreditAllocation {
                    credits: self.config.initial_credits,
                    max_credits: self.config.initial_credits,
                    last_refill: Instant::now(),
                    processing_rate: 0.0,
                    refill_rate: self.config.refill_rate,
                },
            );
        }
    }
    
    /// Request credits for a task
    /// Returns the number of credits granted
    pub fn request_credits(&mut self, task_id: &str, requested: usize) -> usize {
        // Auto-register if not registered
        if !self.allocations.contains_key(task_id) {
            self.register_task(task_id);
        }
        
        // Refill credits if it's time
        self.refill_credits(task_id);
        
        // Get current allocation
        if let Some(allocation) = self.allocations.get_mut(task_id) {
            // Grant up to available credits
            let granted = requested.min(allocation.credits);
            allocation.credits -= granted;
            
            // Debug logging
            debug!(
                "Task {} requested {} credits, granted {}. Remaining: {}",
                task_id, requested, granted, allocation.credits
            );
            
            granted
        } else {
            0
        }
    }
    
    /// Report records processed by a task
    pub fn report_processing(&mut self, task_id: &str, records: usize, duration: Duration) {
        if let Some(allocation) = self.allocations.get_mut(task_id) {
            // Update processing rate
            let duration_secs = duration.as_secs_f64();
            if duration_secs > 0.0 {
                // Use exponential moving average for stability
                let rate = records as f64 / duration_secs;
                allocation.processing_rate = 
                    0.7 * allocation.processing_rate + 0.3 * rate;
                
                // In dynamic mode, adjust refill rate based on processing rate
                if self.config.mode == CreditMode::Dynamic {
                    // Set refill rate to match processing rate with some headroom
                    allocation.refill_rate = 
                        (allocation.processing_rate * 1.2).clamp(
                            self.config.min_credits as f64,
                            self.config.max_credits as f64
                        );
                }
            }
        }
    }
    
    /// Update system memory pressure (0.0-1.0)
    pub fn update_memory_pressure(&mut self, pressure: f64) {
        self.memory_pressure = pressure.clamp(0.0, 1.0);
        
        // In adaptive mode, adjust credit allocations based on memory pressure
        if self.config.mode == CreditMode::Adaptive {
            self.adapt_to_pressure();
        }
    }
    
    /// Refill credits for a task based on elapsed time and refill rate
    fn refill_credits(&mut self, task_id: &str) {
        if let Some(allocation) = self.allocations.get_mut(task_id) {
            let elapsed = allocation.last_refill.elapsed();
            
            // Only refill if enough time has passed
            if elapsed >= self.config.refill_interval {
                let elapsed_secs = elapsed.as_secs_f64();
                
                // Calculate credits to add
                let credits_to_add = 
                    (allocation.refill_rate * elapsed_secs) as usize;
                
                if credits_to_add > 0 {
                    // Add credits, but don't exceed max
                    allocation.credits = 
                        (allocation.credits + credits_to_add)
                            .min(allocation.max_credits);
                    
                    // Update last refill time
                    allocation.last_refill = Instant::now();
                }
            }
        }
    }
    
    /// Adapt credit allocations based on memory pressure
    fn adapt_to_pressure(&mut self) {
        // If memory pressure is above threshold, reduce max credits
        if self.memory_pressure > self.config.memory_threshold {
            // Calculate reduction factor based on how far above threshold
            let excess = (self.memory_pressure - self.config.memory_threshold)
                / (1.0 - self.config.memory_threshold);
            
            // Reduce max credits by up to 50% based on excess
            let reduction_factor = 1.0 - (0.5 * excess);
            
            for (id, allocation) in &mut self.allocations {
                let new_max = (allocation.max_credits as f64 * reduction_factor) as usize;
                allocation.max_credits = new_max.max(self.config.min_credits);
                
                // Ensure current credits doesn't exceed new max
                allocation.credits = allocation.credits.min(allocation.max_credits);
                
                info!(
                    "Reducing credits for task {} due to memory pressure: {} -> {}",
                    id, allocation.credits, allocation.max_credits
                );
            }
        } else {
            // If memory pressure is below threshold, gradually increase max credits
            for allocation in self.allocations.values_mut() {
                // Increase by up to 10% if we're well below threshold
                let headroom = (self.config.memory_threshold - self.memory_pressure)
                    / self.config.memory_threshold;
                
                let increase_factor = 1.0 + (0.1 * headroom);
                
                let new_max = (allocation.max_credits as f64 * increase_factor) as usize;
                allocation.max_credits = new_max.min(self.config.max_credits);
            }
        }
    }
    
    /// Get current credit allocation for a task
    pub fn get_credits(&self, task_id: &str) -> Option<usize> {
        self.allocations.get(task_id).map(|a| a.credits)
    }
    
    /// Get current processing rate for a task
    pub fn get_processing_rate(&self, task_id: &str) -> Option<f64> {
        self.allocations.get(task_id).map(|a| a.processing_rate)
    }
}

/// Shared backpressure controller that can be used across threads
pub type SharedBackpressureController = Arc<Mutex<BackpressureController>>;

/// Create a new shared backpressure controller
pub fn create_shared_controller(config: CreditConfig) -> SharedBackpressureController {
    Arc::new(Mutex::new(BackpressureController::new(config)))
} 