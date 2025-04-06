use std::time::Duration;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use actix::prelude::*;
use actix_cluster::prelude::*;
use actix_cluster::supervision::*;
use actix_cluster::error::{ClusterError, ClusterResult};
use actix_cluster::node::{NodeId, PlacementStrategy};
use log::{info, error, warn};

// Define a simple actor that can fail on demand
#[derive(Clone, Debug)]
struct CounterActor {
    name: String,
    count: usize,
    fail_on: usize,
    restart_count: Arc<AtomicUsize>,
}

impl Actor for CounterActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("CounterActor {} started with count {}", self.name, self.count);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("CounterActor {} stopped at count {}", self.name, self.count);
    }
}

impl DistributedActor for CounterActor {
    fn actor_path(&self) -> String {
        format!("/counter/{}", self.name)
    }
}

// Implement the supervised trait for the counter actor
impl SupervisedDistributedActor for CounterActor {
    fn supervision_strategy(&self) -> SupervisionStrategy {
        // Different actors can have different supervision strategies
        match self.name.as_str() {
            "restart" => SupervisionStrategy::Restart {
                max_restarts: 3,
                window: Duration::from_secs(60),
                delay: Duration::from_millis(500),
            },
            "stop" => SupervisionStrategy::Stop,
            "escalate" => SupervisionStrategy::Escalate,
            "matcher" => {
                // A more complex strategy that applies different strategies based on error type
                let io_matcher = SupervisionMatcher {
                    error_type: "std::io::Error".to_string(),
                    strategy: Box::new(SupervisionStrategy::Restart {
                        max_restarts: 5,
                        window: Duration::from_secs(60),
                        delay: Duration::from_millis(100),
                    }),
                };
                
                let runtime_matcher = SupervisionMatcher {
                    error_type: "std::runtime::Error".to_string(),
                    strategy: Box::new(SupervisionStrategy::Stop),
                };
                
                SupervisionStrategy::Match {
                    default: Box::new(SupervisionStrategy::Escalate),
                    matchers: vec![io_matcher, runtime_matcher],
                }
            },
            _ => SupervisionStrategy::default(),
        }
    }
    
    fn before_restart(&mut self, _ctx: &mut <Self as Actor>::Context, failure: Option<FailureInfo>) {
        if let Some(failure) = &failure {
            info!("CounterActor {} preparing to restart after failure: {}", self.name, failure.error);
        } else {
            info!("CounterActor {} preparing to restart", self.name);
        }
    }
    
    fn after_restart(&mut self, _ctx: &mut <Self as Actor>::Context, _failure: Option<FailureInfo>) {
        info!("CounterActor {} restarted, resetting count", self.name);
        self.count = 0;
        self.restart_count.fetch_add(1, Ordering::SeqCst);
    }
}

// Messages that our actor can handle
#[derive(Message)]
#[rtype(result = "ClusterResult<usize>")]
struct Increment(usize);

#[derive(Message)]
#[rtype(result = "usize")]
struct GetCount;

// Message handlers
impl Handler<Increment> for CounterActor {
    type Result = ClusterResult<usize>;
    
    fn handle(&mut self, msg: Increment, ctx: &mut Self::Context) -> Self::Result {
        self.count += msg.0;
        info!("CounterActor {} incremented by {} to {}", self.name, msg.0, self.count);
        
        // Fail on specific count if set
        if self.fail_on > 0 && self.count >= self.fail_on {
            error!("CounterActor {} failing on count {}", self.name, self.count);
            return Err(ClusterError::ActorFailure(format!("Actor {} failed at count {}", self.name, self.count)));
        }
        
        Ok(self.count)
    }
}

impl Handler<GetCount> for CounterActor {
    type Result = usize;
    
    fn handle(&mut self, _: GetCount, _ctx: &mut Self::Context) -> Self::Result {
        self.count
    }
}

// Run several supervised actors with different strategies
#[actix_rt::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // Create actor with restart strategy
    let restart_count = Arc::new(AtomicUsize::new(0));
    let restart_actor = CounterActor {
        name: "restart".to_string(),
        count: 0,
        fail_on: 5,
        restart_count: restart_count.clone(),
    };
    
    // Create props for the actor
    let restart_props = ActorProps::new(restart_actor.clone())
        .with_supervision(SupervisionStrategy::Restart {
            max_restarts: 3,
            window: Duration::from_secs(60),
            delay: Duration::from_millis(500),
        });
    
    // Start the supervised actor
    let restart_supervisor = restart_props.start();
    
    // Get the actor address
    let actor_addr = restart_supervisor.send(GetActorAddr::new()).await?
        .expect("Actor should be running");
    
    // Increment the actor a few times
    for _ in 1..=10 {
        match actor_addr.send(Increment(1)).await {
            Ok(Ok(count)) => {
                info!("Increment succeeded, new count: {}", count);
            },
            Ok(Err(e)) => {
                warn!("Actor operation failed: {}", e);
                
                // Report the failure to supervisor
                let error: Box<dyn std::error::Error + Send> = Box::new(e);
                let result = restart_supervisor.send(ReportFailure::new(error)).await?;
                
                if let Some(new_addr) = result {
                    info!("Actor was restarted successfully");
                } else {
                    error!("Actor could not be restarted");
                    break;
                }
            },
            Err(e) => {
                error!("Communication with actor failed: {}", e);
                break;
            }
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    // Check restart count
    info!("Actor was restarted {} times", restart_count.load(Ordering::SeqCst));
    
    // Get failure history
    let history = restart_supervisor.send(GetFailureHistory::new()).await?;
    for (i, failure) in history.iter().enumerate() {
        info!("Failure {}: {} at actor path: {}", i+1, failure.error, failure.actor_path);
    }
    
    // Create another actor with stop strategy
    let stop_actor = CounterActor {
        name: "stop".to_string(),
        count: 0,
        fail_on: 3,
        restart_count: Arc::new(AtomicUsize::new(0)),
    };
    
    // Create props with stop strategy
    let stop_props = ActorProps::new(stop_actor)
        .with_supervision(SupervisionStrategy::Stop);
    
    // Start the supervised actor
    let stop_supervisor = stop_props.start();
    let stop_addr = stop_supervisor.send(GetActorAddr::new()).await?
        .expect("Actor should be running");
    
    // This will eventually fail and stop
    for _ in 1..=5 {
        match stop_addr.send(Increment(1)).await {
            Ok(Ok(count)) => {
                info!("Stop actor increment succeeded, new count: {}", count);
            },
            Ok(Err(e)) => {
                warn!("Stop actor operation failed: {}", e);
                
                // Report the failure to supervisor
                let error: Box<dyn std::error::Error + Send> = Box::new(e);
                let result = stop_supervisor.send(ReportFailure::new(error)).await?;
                
                if result.is_none() {
                    info!("Stop actor was stopped as expected");
                    break;
                }
            },
            Err(e) => {
                info!("Communication with stop actor failed as expected: {}", e);
                break;
            }
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    info!("Supervision example completed");
    System::current().stop();
    
    Ok(())
} 