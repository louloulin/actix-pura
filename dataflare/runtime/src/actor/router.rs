//! # Message Router
//!
//! Message routing system for simplifying communication between actors.
//! This module reduces Actor nesting by providing a central routing mechanism.

use std::sync::Arc;

use actix::prelude::*;
use log::{debug, error, warn};
use uuid::Uuid;

use dataflare_core::error::{DataFlareError, Result};
use crate::actor::message_bus::{MessageBus, DataFlareMessage, ActorId};
use crate::actor::pool::StopWorker;

/// Configuration for the message router
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Whether to log all messages for debugging
    pub debug_logging: bool,
    /// Whether to collect message metrics
    pub collect_metrics: bool,
    /// Maximum number of routing retries
    pub max_retries: usize,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            debug_logging: false,
            collect_metrics: true,
            max_retries: 3,
            retry_delay_ms: 100,
        }
    }
}

/// Message routing statistics
#[derive(Debug, Default, Clone)]
pub struct RouterStats {
    /// Total messages routed
    pub total_messages: usize,
    /// Failed routing attempts
    pub failed_messages: usize,
    /// Successful routing attempts
    pub successful_messages: usize,
    /// Messages that required retries
    pub retried_messages: usize,
    /// Average routing time in milliseconds
    pub avg_routing_time_ms: f64,
}

/// Message router that simplifies communication between actors
pub struct MessageRouter {
    /// The message bus used for communication
    message_bus: Arc<MessageBus>,
    /// Router configuration
    config: RouterConfig,
    /// Router statistics
    stats: RouterStats,
    /// Optional tracer for message tracing
    tracer: Option<Box<dyn Tracer + Send>>,
}

/// Trait for message tracing
pub trait Tracer: 'static {
    /// Trace a message through the system
    fn trace_message(&self, message: &DataFlareMessage);
    /// Record a message delivery result
    fn record_result(&self, message_id: Uuid, successful: bool);
}

impl MessageRouter {
    /// Create a new message router
    pub fn new(message_bus: Arc<MessageBus>, config: RouterConfig) -> Self {
        Self {
            message_bus,
            config,
            stats: RouterStats::default(),
            tracer: None,
        }
    }
    
    /// Set a tracer for message tracing
    pub fn with_tracer<T: Tracer + Send + 'static>(mut self, tracer: T) -> Self {
        self.tracer = Some(Box::new(tracer));
        self
    }
    
    /// Route a message to a target actor
    pub fn route<M: 'static + Send + Sync>(&mut self, sender: impl Into<ActorId>, target: impl Into<ActorId>, message: M) -> Result<()> {
        let sender_id = sender.into();
        let target_id = target.into();
        
        // Create the message
        let wrapped_msg = DataFlareMessage::new(sender_id.clone(), target_id.clone(), message);
        
        // Log if debug logging is enabled
        if self.config.debug_logging {
            debug!(
                "Routing message from {:?} to {:?} [id: {}]",
                sender_id, target_id, wrapped_msg.message_id
            );
        }
        
        // Trace the message
        if let Some(tracer) = &self.tracer {
            tracer.trace_message(&wrapped_msg);
        }
        
        // Update statistics
        if self.config.collect_metrics {
            self.stats.total_messages += 1;
        }
        
        // Send the message directly without retry for now to simplify
        let result = self.message_bus.send(target_id.clone(), wrapped_msg.clone());
        
        match result {
            Ok(()) => {
                if self.config.collect_metrics {
                    self.stats.successful_messages += 1;
                }
                
                if let Some(tracer) = &self.tracer {
                    tracer.record_result(wrapped_msg.message_id, true);
                }
                
                Ok(())
            },
            Err(e) => {
                if self.config.collect_metrics {
                    self.stats.failed_messages += 1;
                }
                
                if let Some(tracer) = &self.tracer {
                    tracer.record_result(wrapped_msg.message_id, false);
                }
                
                error!("Failed to route message to {:?}: {}", target_id, e);
                Err(e)
            }
        }
    }
    
    /// Send a message with retry logic
    fn send_with_retry(&self, target: ActorId, message: DataFlareMessage, retry_count: usize) -> Result<()> {
        // Try to send the message
        let result = self.message_bus.send(target.clone(), message.clone());
        
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                // Check if we should retry
                if retry_count < self.config.max_retries {
                    warn!(
                        "Message routing failed, retrying ({}/{}): {:?}",
                        retry_count + 1, self.config.max_retries, e
                    );
                    
                    if self.config.collect_metrics {
                        // Access mutable stats - slightly hacky in this design
                        let stats_ptr = &self.stats as *const RouterStats as *mut RouterStats;
                        unsafe {
                            (*stats_ptr).retried_messages += 1;
                        }
                    }
                    
                    // Schedule a retry
                    let router = self.clone();
                    let message = message.clone();
                    let target = target.clone();
                    let retry_count = retry_count + 1;
                    
                    actix::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(router.config.retry_delay_ms)).await;
                        if let Err(e) = router.send_with_retry(target.clone(), message, retry_count) {
                            error!("Message retry failed: {}", e);
                        }
                    });
                    
                    Ok(())
                } else {
                    Err(DataFlareError::Actor(format!(
                        "Failed to route message from {:?} to {:?} after {} retries: {}",
                        message.sender, target, retry_count, e
                    )))
                }
            }
        }
    }
    
    /// Get current router statistics
    pub fn get_stats(&self) -> RouterStats {
        self.stats.clone()
    }
    
    /// Reset router statistics
    pub fn reset_stats(&mut self) {
        self.stats = RouterStats::default();
    }
}

impl Clone for MessageRouter {
    fn clone(&self) -> Self {
        Self {
            message_bus: self.message_bus.clone(),
            config: self.config.clone(),
            stats: self.stats.clone(),
            tracer: None, // Tracer is not cloned
        }
    }
}

/// Actor implementation of message router
pub struct RouterActor {
    /// Internal message router
    router: MessageRouter,
}

impl RouterActor {
    /// Create a new router actor
    pub fn new(message_bus: Arc<MessageBus>, config: RouterConfig) -> Self {
        Self {
            router: MessageRouter::new(message_bus, config),
        }
    }
    
    /// Set a tracer for message tracing
    pub fn with_tracer<T: Tracer + Send + 'static>(mut self, tracer: T) -> Self {
        self.router = self.router.with_tracer(tracer);
        self
    }
}

impl Actor for RouterActor {
    type Context = Context<Self>;
}

/// Message to route through the router actor
#[derive(Debug)]
pub struct RouteMessage<M: 'static + Send + Sync> {
    /// Sender actor ID
    pub sender: ActorId,
    /// Target actor ID
    pub target: ActorId,
    /// Message to route
    pub message: M,
}

impl<M: 'static + Send + Sync> Message for RouteMessage<M> {
    type Result = Result<()>;
}

impl<M: 'static + Send + Sync> Handler<RouteMessage<M>> for RouterActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: RouteMessage<M>, _ctx: &mut Self::Context) -> Self::Result {
        // 更新统计信息
        if self.router.config.collect_metrics {
            self.router.stats.total_messages += 1;
            self.router.stats.successful_messages += 1;
        }
        
        // 直接使用message_bus发送消息
        self.router.message_bus.send(msg.target.clone(), msg.message)
    }
}

/// Message to get router statistics
#[derive(Debug)]
pub struct GetRouterStats;

impl Message for GetRouterStats {
    type Result = RouterStats;
}

impl Handler<GetRouterStats> for RouterActor {
    type Result = MessageResult<GetRouterStats>;
    
    fn handle(&mut self, _msg: GetRouterStats, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.router.get_stats())
    }
}

/// Message to reset router statistics
#[derive(Debug)]
pub struct ResetRouterStats;

impl Message for ResetRouterStats {
    type Result = ();
}

impl Handler<ResetRouterStats> for RouterActor {
    type Result = ();
    
    fn handle(&mut self, _msg: ResetRouterStats, _ctx: &mut Self::Context) -> Self::Result {
        self.router.reset_stats();
    }
}

/// Implementation of Handler<StopWorker> for RouterActor
impl Handler<StopWorker> for RouterActor {
    type Result = ();
    
    fn handle(&mut self, _: StopWorker, ctx: &mut Self::Context) {
        ctx.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Simple test tracer
    struct TestTracer {
        traced_messages: std::sync::Mutex<Vec<Uuid>>,
        results: std::sync::Mutex<Vec<(Uuid, bool)>>,
    }
    
    impl TestTracer {
        fn new() -> Self {
            Self {
                traced_messages: std::sync::Mutex::new(Vec::new()),
                results: std::sync::Mutex::new(Vec::new()),
            }
        }
    }
    
    impl Tracer for TestTracer {
        fn trace_message(&self, message: &DataFlareMessage) {
            if let Ok(mut traced) = self.traced_messages.lock() {
                traced.push(message.message_id);
            }
        }
        
        fn record_result(&self, message_id: Uuid, successful: bool) {
            if let Ok(mut results) = self.results.lock() {
                results.push((message_id, successful));
            }
        }
    }
} 