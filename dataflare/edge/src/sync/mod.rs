//! Sync manager module
//!
//! This module provides functionality for synchronizing data with cloud services.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use dataflare_core::error::Result;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time;

/// Sync manager for synchronizing data with cloud services
pub struct SyncManager {
    /// Sync interval in seconds
    sync_interval: u64,
    /// Whether the sync manager is running
    running: AtomicBool,
    /// Sync task handle
    sync_task: Mutex<Option<JoinHandle<()>>>,
}

impl SyncManager {
    /// Create a new sync manager
    pub fn new(sync_interval: u64) -> Self {
        Self {
            sync_interval,
            running: AtomicBool::new(false),
            sync_task: Mutex::new(None),
        }
    }

    /// Start the sync manager
    pub async fn start(&self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.running.store(true, Ordering::SeqCst);

        // Start sync task
        let running = Arc::new(AtomicBool::new(self.running.load(Ordering::SeqCst)));
        let sync_interval = self.sync_interval;

        let task = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(sync_interval));

            while running.load(Ordering::SeqCst) {
                interval.tick().await;

                // Perform sync
                if let Err(e) = Self::sync_data().await {
                    log::error!("Failed to sync data: {}", e);
                }
            }
        });

        // Store task handle
        let mut sync_task = self.sync_task.lock().await;
        *sync_task = Some(task);

        Ok(())
    }

    /// Stop the sync manager
    pub async fn stop(&self) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.running.store(false, Ordering::SeqCst);

        // Wait for sync task to finish
        let mut sync_task = self.sync_task.lock().await;
        if let Some(task) = sync_task.take() {
            let _ = task.await;
        }

        Ok(())
    }

    /// Sync data with cloud services
    async fn sync_data() -> Result<()> {
        // This is a placeholder - actual implementation would depend on the cloud service
        log::info!("Syncing data with cloud services");

        // Simulate some work
        time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Trigger a manual sync
    pub async fn sync_now(&self) -> Result<()> {
        Self::sync_data().await
    }

    /// Get the sync interval
    pub fn sync_interval(&self) -> u64 {
        self.sync_interval
    }

    /// Set the sync interval
    pub fn set_sync_interval(&mut self, sync_interval: u64) {
        self.sync_interval = sync_interval;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sync_manager() {
        let sync_manager = SyncManager::new(60);

        assert_eq!(sync_manager.sync_interval(), 60);
        assert_eq!(sync_manager.running.load(Ordering::SeqCst), false);

        sync_manager.start().await.unwrap();
        assert_eq!(sync_manager.running.load(Ordering::SeqCst), true);

        // Trigger a manual sync
        sync_manager.sync_now().await.unwrap();

        // Sleep for a short time to let the sync task run
        time::sleep(Duration::from_millis(100)).await;

        sync_manager.stop().await.unwrap();
        assert_eq!(sync_manager.running.load(Ordering::SeqCst), false);
    }
}
