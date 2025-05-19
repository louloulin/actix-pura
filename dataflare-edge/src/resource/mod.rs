//! Resource monitor module
//!
//! This module provides functionality for monitoring system resources.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use rand::Rng;

/// Resource monitor for tracking system resource usage
pub struct ResourceMonitor {
    /// Maximum memory usage in MB
    max_memory_mb: usize,
    /// Maximum CPU usage (0.0 - 1.0)
    max_cpu_usage: f32,
    /// Whether the monitor is running
    running: AtomicBool,
    /// Monitor thread handle
    monitor_thread: Option<thread::JoinHandle<()>>,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(max_memory_mb: usize, max_cpu_usage: f32) -> Self {
        Self {
            max_memory_mb,
            max_cpu_usage,
            running: AtomicBool::new(false),
            monitor_thread: None,
        }
    }

    /// Start monitoring resources
    pub fn start(&self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(true, Ordering::SeqCst);

        // Start monitoring in a separate thread
        let running = self.running.clone();
        let max_memory_mb = self.max_memory_mb;
        let max_cpu_usage = self.max_cpu_usage;

        let handle = thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                // Check memory usage
                let memory_usage = Self::get_memory_usage();
                if memory_usage > max_memory_mb {
                    log::warn!("Memory usage exceeds limit: {} MB > {} MB", memory_usage, max_memory_mb);
                }

                // Check CPU usage
                let cpu_usage = Self::get_cpu_usage();
                if cpu_usage > max_cpu_usage {
                    log::warn!("CPU usage exceeds limit: {:.2} > {:.2}", cpu_usage, max_cpu_usage);
                }

                // Sleep for a while
                thread::sleep(Duration::from_secs(5));
            }
        });

        // Store thread handle
        let mut_self = unsafe { &mut *(self as *const Self as *mut Self) };
        mut_self.monitor_thread = Some(handle);
    }

    /// Stop monitoring resources
    pub fn stop(&self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(false, Ordering::SeqCst);

        // Wait for monitor thread to finish
        let mut_self = unsafe { &mut *(self as *const Self as *mut Self) };
        if let Some(handle) = mut_self.monitor_thread.take() {
            let _ = handle.join();
        }
    }

    /// Get current memory usage in MB
    pub fn get_memory_usage() -> usize {
        // This is a placeholder - actual implementation would depend on the platform
        // For now, just return a random value for testing
        let mut rng = rand::thread_rng();
        rand::Rng::gen_range(&mut rng, 100..500)
    }

    /// Get current CPU usage (0.0 - 1.0)
    pub fn get_cpu_usage() -> f32 {
        // This is a placeholder - actual implementation would depend on the platform
        // For now, just return a random value for testing
        let mut rng = rand::thread_rng();
        rand::Rng::gen_range(&mut rng, 0.1..0.9)
    }

    /// Get the number of available CPU cores
    pub fn get_cpu_cores() -> usize {
        num_cpus::get()
    }
}

impl Drop for ResourceMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_monitor() {
        let monitor = ResourceMonitor::new(256, 0.8);

        assert_eq!(monitor.max_memory_mb, 256);
        assert_eq!(monitor.max_cpu_usage, 0.8);
        assert_eq!(monitor.running.load(Ordering::SeqCst), false);

        monitor.start();
        assert_eq!(monitor.running.load(Ordering::SeqCst), true);

        // Sleep for a short time to let the monitor run
        thread::sleep(Duration::from_millis(100));

        monitor.stop();
        assert_eq!(monitor.running.load(Ordering::SeqCst), false);
    }
}
