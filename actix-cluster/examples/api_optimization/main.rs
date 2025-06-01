//! API Optimization Example
//!
//! This example demonstrates the improved actor API with:
//! - Fluent builder pattern for actor configuration
//! - Simplified actor creation
//! - Better message handling with timeout support
//! - Various placement strategies
//!
//! To run this example:
//! ```
//! cargo run --example api_optimization
//! ```

use actix::prelude::*;
use serde::{Serialize, Deserialize};
use actix_cluster::{
    actor::{DistributedActor, DistributedActorExt, actor},
    node::{PlacementStrategy, NodeId},
    serialization::SerializationFormat,
    error::ClusterResult,
    migration::MigratableActor,
};
use std::time::Duration;

// Define a simple counter actor
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CounterActor {
    name: String,
    count: i32,
}

impl CounterActor {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            count: 0,
        }
    }
}

impl Actor for CounterActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("Counter actor '{}' started!", self.name);
    }
}

// Make our actor compatible with distributed deployment
impl DistributedActor for CounterActor {
    fn actor_path(&self) -> String {
        format!("/user/counter/{}", self.name)
    }
}

// Add migration capability to the actor
impl MigratableActor for CounterActor {
    fn get_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(bincode::serialize(self).unwrap())
    }
    
    fn restore_state(&mut self, state: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        let actor: CounterActor = bincode::deserialize(&state)?;
        self.name = actor.name;
        self.count = actor.count;
        Ok(())
    }
    
    fn before_migration(&mut self, _ctx: &mut Self::Context) {
        println!("Preparing to migrate counter '{}'", self.name);
    }
    
    fn after_migration(&mut self, _ctx: &mut Self::Context) {
        println!("Counter '{}' migrated successfully", self.name);
    }
    
    fn can_migrate(&self) -> bool {
        true
    }
}

// Define messages for our counter actor
#[derive(Message)]
#[rtype(result = "i32")]
struct GetCount;

#[derive(Message)]
#[rtype(result = "i32")]
struct Increment(i32);

#[derive(Message)]
#[rtype(result = "String")]
struct GetStatus;

// Implement message handlers
impl Handler<GetCount> for CounterActor {
    type Result = i32;
    
    fn handle(&mut self, _msg: GetCount, _ctx: &mut Self::Context) -> Self::Result {
        self.count
    }
}

impl Handler<Increment> for CounterActor {
    type Result = i32;
    
    fn handle(&mut self, msg: Increment, _ctx: &mut Self::Context) -> Self::Result {
        self.count += msg.0;
        println!("Counter '{}' incremented by {}. New value: {}", 
            self.name, msg.0, self.count);
        self.count
    }
}

impl Handler<GetStatus> for CounterActor {
    type Result = String;
    
    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        format!("Counter '{}' status: {}", self.name, self.count)
    }
}

#[actix_rt::main]
async fn main() -> ClusterResult<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    println!("=== API Optimization Example ===");
    println!("Demonstrating the improved actor API");
    
    // EXAMPLE 1: Using the traditional start approach
    println!("\n--- Example 1: Traditional Start ---");
    let counter1 = CounterActor::new("counter1");
    let addr1 = counter1.start_distributed();
    let count1 = addr1.send(Increment(5)).await?;
    println!("Counter1 value: {}", count1);
    
    // EXAMPLE 2: Using the new builder pattern
    println!("\n--- Example 2: Builder Pattern ---");
    let counter2 = CounterActor::new("counter2");
    let addr2 = counter2.props()
        .with_path("/user/custom-counter")
        .with_format(SerializationFormat::Json)
        .with_round_robin()
        .start();
    
    let count2 = addr2.send(Increment(10)).await?;
    println!("Counter2 value: {}", count2);
    
    // EXAMPLE 3: Using the simplified actor creation function
    println!("\n--- Example 3: Simplified Creation ---");
    let addr3 = actor(CounterActor::new("counter3"))
        .with_local_affinity(Some("counters".to_string()), PlacementStrategy::RoundRobin)
        .start();
    
    let count3 = addr3.send(Increment(3)).await?;
    let status3 = addr3.send(GetStatus).await?;
    println!("Counter3 value: {}, Status: {}", count3, status3);
    
    // EXAMPLE 4: Using the ask pattern with timeout
    println!("\n--- Example 4: Ask Pattern with Timeout ---");
    let counter4 = CounterActor::new("counter4");
    // Need to clone before moving into start_distributed
    let counter4_ref = counter4.clone();
    let addr4 = counter4.start_distributed();
    
    // Increment a few times
    addr4.send(Increment(1)).await?;
    addr4.send(Increment(2)).await?;
    
    // Use the ask_async helper with timeout
    let result = counter4_ref.ask_async(
        &addr4, 
        Increment(7), 
        Duration::from_secs(1)
    ).await?;
    
    println!("Counter4 final value (using ask_async): {}", result);
    
    println!("\nExample completed successfully!");
    Ok(())
} 