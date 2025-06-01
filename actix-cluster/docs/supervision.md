# Enhanced Supervision for Actix-Pura

This document provides an overview of the Enhanced Supervision feature in Actix-Pura, which offers sophisticated actor failure recovery mechanisms for distributed systems.

## Overview

The Enhanced Supervision system provides various strategies for handling actor failures in a distributed environment. Inspired by Akka's supervision model, it allows actors to recover gracefully from failures based on configurable policies.

## Key Features

### 1. Multiple Supervision Strategies

The system offers several built-in strategies:

- **Restart**: Restarts the actor on the same node with configurable retry limits and backoff
- **Stop**: Stops the actor without recovery attempts
- **Escalate**: Escalates failure handling to a parent supervisor
- **Relocate**: Restarts the actor on a different node based on placement policy
- **Resume**: Ignores the failure and continues actor execution
- **Match**: Applies different strategies based on error type

### 2. Error Pattern Matching

The `Match` strategy allows for detailed control over how different error types are handled:

```rust
let strategy = SupervisionStrategy::Match {
    default: Box::new(SupervisionStrategy::Restart { 
        max_restarts: 3, 
        window: Duration::from_secs(60), 
        delay: Duration::from_millis(100) 
    }),
    matchers: vec![
        SupervisionMatcher {
            error_type: "std::io::Error".to_string(),
            strategy: Box::new(SupervisionStrategy::Relocate { 
                placement: PlacementStrategy::RoundRobin,
                max_relocations: 3, 
                delay: Duration::from_millis(200)
            }),
        },
        SupervisionMatcher {
            error_type: "DatabaseError".to_string(),
            strategy: Box::new(SupervisionStrategy::Stop),
        },
    ],
};
```

### 3. Lifecycle Hooks

The `SupervisedDistributedActor` trait provides hooks for key lifecycle events:

```rust
impl SupervisedDistributedActor for MyActor {
    // Define a custom supervision strategy
    fn supervision_strategy(&self) -> SupervisionStrategy {
        SupervisionStrategy::Restart {
            max_restarts: 5,
            window: Duration::from_secs(60),
            delay: Duration::from_millis(500),
        }
    }
    
    // Called before actor restart
    fn before_restart(&mut self, ctx: &mut Context<Self>, failure: Option<FailureInfo>) {
        // Cleanup resources, log failure, etc.
    }
    
    // Called after actor restart
    fn after_restart(&mut self, ctx: &mut Context<Self>, failure: Option<FailureInfo>) {
        // Initialize state, restore connections, etc.
    }
    
    // Called before actor relocation
    fn before_relocate(&mut self, ctx: &mut Context<Self>, target_node: NodeId, failure: Option<FailureInfo>) {
        // Prepare for migration to new node
    }
    
    // Called after actor relocation
    fn after_relocate(&mut self, ctx: &mut Context<Self>, source_node: NodeId, failure: Option<FailureInfo>) {
        // Setup after migration from another node
    }
}
```

### 4. Failure Tracking

The supervision system maintains detailed failure information:

```rust
pub struct FailureInfo {
    pub actor_path: String,     // Path of the failed actor
    pub node_id: NodeId,        // Node where failure occurred
    pub time: u64,              // Failure timestamp
    pub error: String,          // Error message
    pub error_type: String,     // Type of error
    pub restart_count: usize,   // Number of restarts attempted
    pub failure_id: Uuid,       // Unique failure identifier
}
```

This information is used both internally for decisions and can be queried for monitoring and diagnostics.

## Usage Examples

### Creating a Supervised Actor

```rust
// Define an actor
#[derive(Clone)]
struct MyActor {
    // Actor state
}

impl Actor for MyActor {
    type Context = Context<Self>;
}

impl DistributedActor for MyActor {
    fn actor_path(&self) -> String {
        "/user/my-actor".to_string()
    }
}

// Implement supervision behavior
impl SupervisedDistributedActor for MyActor {
    fn supervision_strategy(&self) -> SupervisionStrategy {
        SupervisionStrategy::Restart {
            max_restarts: 5,
            window: Duration::from_secs(60),
            delay: Duration::from_millis(100),
        }
    }
}

// Create and start the supervised actor
let actor = MyActor { /* ... */ };
let supervised = ActorProps::new(actor)
    .with_supervision(SupervisionStrategy::Restart {
        max_restarts: 3,
        window: Duration::from_secs(30),
        delay: Duration::from_millis(100),
    })
    .start();

// Get the actor address
let actor_addr = supervised.send(GetActorAddr::new()).await?.expect("Actor should be running");
```

### Handling Failures

```rust
// Report a failure to the supervisor
let error: Box<dyn std::error::Error + Send> = Box::new(std::io::Error::new(
    std::io::ErrorKind::Other,
    "An error occurred",
));

// Report the failure to the supervisor
let result = supervisor_addr.send(ReportFailure::new(error)).await?;

// Check if the actor was restarted
if let Some(new_addr) = result {
    println!("Actor was restarted successfully");
} else {
    println!("Actor couldn't be restarted (stopped or escalated)");
}

// Get failure history
let history = supervisor_addr.send(GetFailureHistory::new()).await?;
for (i, failure) in history.iter().enumerate() {
    println!("Failure {}: {} at {}", i+1, failure.error, failure.time);
}
```

## Complete Example

See the `supervised_actor.rs` example in the examples directory for a complete demonstration of using the supervision system with different strategies.

## Best Practices

1. **Choose Appropriate Strategies**: Different actor types and failure modes require different strategies:
   - Use `Restart` for transient errors that might resolve with a fresh start
   - Use `Relocate` for node-specific issues (hardware, connectivity)
   - Use `Stop` for persistent errors that won't be fixed by restarting
   - Use `Match` for complex actors with different error handling needs

2. **Implement Lifecycle Methods**: Always implement the lifecycle methods to ensure proper resource cleanup and initialization during recovery.

3. **Set Reasonable Limits**: Configure `max_restarts` and timing windows to prevent restart storms or resource exhaustion.

4. **Monitor Failure History**: Regularly check failure history for patterns that might indicate systemic issues.

## Conclusion

The Enhanced Supervision feature in Actix-Pura provides a robust framework for building resilient distributed systems. By offering multiple recovery strategies and detailed failure handling, it allows applications to maintain availability and gracefully handle various error conditions. 