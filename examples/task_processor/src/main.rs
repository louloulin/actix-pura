use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use actix::prelude::*;
use actix_cluster::{
    ClusterSystem, ClusterConfig, Architecture, NodeRole,
    SerializationFormat, NodeId, AnyMessage
};
use log::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use structopt::StructOpt;
use actix_cluster::registry::ActorRef;
use actix_cluster::cluster::SimpleActorRef;
use rand::{thread_rng, Rng};

// Command line arguments
#[derive(StructOpt, Debug)]
#[structopt(name = "task_processor", about = "Distributed task processing example for ActixCluster")]
struct Args {
    /// Node ID
    #[structopt(short, long, default_value = "node1")]
    id: String,

    /// Bind address
    #[structopt(short, long, default_value = "127.0.0.1:8080")]
    address: SocketAddr,

    /// Seed node address
    #[structopt(short, long)]
    seed: Option<String>,

    /// Start multiple nodes (for local testing)
    #[structopt(long)]
    multi: bool,

    /// Number of nodes to start
    #[structopt(long, default_value = "3")]
    nodes: usize,

    /// Base port for multi-node mode
    #[structopt(long, default_value = "9000")]
    base_port: u16,

    /// Number of tasks to generate (for coordinator node)
    #[structopt(long, default_value = "20")]
    tasks: usize,
}

// Task definition
#[derive(Message, Serialize, Deserialize, Clone, Debug)]
#[rtype(result = "TaskResult")]
struct Task {
    id: String,
    task_type: TaskType,
    data: Vec<u8>,
    created_at: u64,
    priority: TaskPriority,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum TaskType {
    Computation,
    IO,
    LongRunning,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TaskPriority {
    Low,
    Medium,
    High,
}

#[derive(MessageResponse, Serialize, Deserialize, Clone, Debug, Message)]
#[rtype(result = "()")]
struct TaskResult {
    task_id: String,
    status: TaskStatus,
    result: Option<Vec<u8>>,
    error: Option<String>,
    execution_time_ms: u64,
    processor_node: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum TaskStatus {
    Completed,
    Failed,
    Timeout,
}

// Job dispatcher messages
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
struct RegisterWorker {
    worker_id: String,
    capabilities: Vec<TaskType>,
    max_tasks: usize,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Vec<WorkerStatus>")]
struct GetWorkerStatuses;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct WorkerStatus {
    worker_id: String,
    capabilities: Vec<TaskType>,
    max_tasks: usize,
    current_tasks: usize,
    total_completed: usize,
    last_heartbeat: u64,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Vec<TaskInfo>")]
struct GetQueuedTasks;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TaskInfo {
    id: String,
    task_type: TaskType,
    created_at: u64,
    priority: TaskPriority,
    size_bytes: usize,
}

// Heartbeat message
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
struct Heartbeat {
    worker_id: String,
    current_tasks: usize,
    total_completed: usize,
}

// Job dispatcher actor
struct JobDispatcherActor {
    node_id: String,
    tasks: Vec<Task>,
    workers: HashMap<String, WorkerStatus>,
    results: HashMap<String, TaskResult>,
    next_task_id: usize,
}

impl JobDispatcherActor {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            tasks: Vec::new(),
            workers: HashMap::new(),
            results: HashMap::new(),
            next_task_id: 0,
        }
    }

    fn now_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64
    }

    fn generate_task_id(&mut self) -> String {
        let id = format!("task-{}", self.next_task_id);
        self.next_task_id += 1;
        id
    }

    // Find a suitable worker for a task
    fn find_worker_for_task(&self, task: &Task) -> Option<String> {
        let mut candidates = self.workers.iter()
            .filter(|(_, status)| {
                // Filter workers that can handle this task type
                status.capabilities.iter().any(|cap| match cap {
                    TaskType::Computation => matches!(task.task_type, TaskType::Computation),
                    TaskType::IO => matches!(task.task_type, TaskType::IO),
                    TaskType::LongRunning => matches!(task.task_type, TaskType::LongRunning),
                }) &&
                // Filter out workers that are at capacity
                status.current_tasks < status.max_tasks &&
                // Filter out workers that haven't sent a heartbeat in the last 10 seconds
                Self::now_millis() - status.last_heartbeat < 10000
            })
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            return None;
        }

        // Sort by least loaded
        candidates.sort_by_key(|(_, status)| {
            (status.current_tasks as f64 / status.max_tasks as f64 * 100.0) as u64
        });

        // Return the least loaded worker
        candidates.first().map(|(id, _)| (*id).clone())
    }
}

impl Actor for JobDispatcherActor {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        info!("JobDispatcherActor started on node {}", self.node_id);
    }
}

// Handle task submission
impl Handler<Task> for JobDispatcherActor {
    type Result = TaskResult;

    fn handle(&mut self, mut task: Task, ctx: &mut Self::Context) -> Self::Result {
        // If task ID is empty, generate one
        if task.id.is_empty() {
            task.id = self.generate_task_id();
        }

        info!("Received task {}: {:?}", task.id, task.task_type);

        // Add timestamp if not present
        if task.created_at == 0 {
            task.created_at = Self::now_millis();
        }

        // Find a worker for this task
        if let Some(worker_id) = self.find_worker_for_task(&task) {
            info!("Assigning task {} to worker {}", task.id, worker_id);

            if let Some(worker_status) = self.workers.get_mut(&worker_id) {
                worker_status.current_tasks += 1;
            }

            // Forward task to worker
            // In a real implementation, we would store pending tasks and retry if needed
            self.tasks.push(task.clone());

            // Return a pending result
            TaskResult {
                task_id: task.id,
                status: TaskStatus::Completed, // This would normally be "Pending"
                result: None,
                error: None,
                execution_time_ms: 0,
                processor_node: worker_id,
            }
        } else {
            warn!("No suitable worker found for task {}", task.id);

            // Queue the task for later processing
            self.tasks.push(task.clone());

            // Return a queued result
            TaskResult {
                task_id: task.id,
                status: TaskStatus::Failed, // This would normally be "Queued"
                result: None,
                error: Some("No suitable worker available".to_string()),
                execution_time_ms: 0,
                processor_node: "none".to_string(),
            }
        }
    }
}

// Handle worker registration
impl Handler<RegisterWorker> for JobDispatcherActor {
    type Result = ();

    fn handle(&mut self, msg: RegisterWorker, _: &mut Self::Context) -> Self::Result {
        info!("Worker {} registered with capabilities: {:?}", msg.worker_id, msg.capabilities);

        // Create or update worker status
        let worker_status = WorkerStatus {
            worker_id: msg.worker_id.clone(),
            capabilities: msg.capabilities,
            max_tasks: msg.max_tasks,
            current_tasks: 0,
            total_completed: 0,
            last_heartbeat: Self::now_millis(),
        };

        self.workers.insert(msg.worker_id, worker_status);

        // Assign queued tasks that match this worker's capabilities
        self.assign_queued_tasks();
    }
}

// Handle worker heartbeats
impl Handler<Heartbeat> for JobDispatcherActor {
    type Result = ();

    fn handle(&mut self, msg: Heartbeat, _: &mut Self::Context) -> Self::Result {
        if let Some(worker) = self.workers.get_mut(&msg.worker_id) {
            worker.last_heartbeat = Self::now_millis();
            worker.current_tasks = msg.current_tasks;
            worker.total_completed = msg.total_completed;

            debug!("Heartbeat from worker {}: {} active tasks, {} completed",
                   msg.worker_id, msg.current_tasks, msg.total_completed);
        } else {
            warn!("Heartbeat from unknown worker: {}", msg.worker_id);
        }

        // Check for timed-out workers and reassign their tasks
        self.check_worker_timeouts();

        // Try to assign any queued tasks
        self.assign_queued_tasks();
    }
}

// Handle status requests
impl Handler<GetWorkerStatuses> for JobDispatcherActor {
    type Result = Vec<WorkerStatus>;

    fn handle(&mut self, _: GetWorkerStatuses, _: &mut Self::Context) -> Self::Result {
        self.workers.values().cloned().collect()
    }
}

impl Handler<GetQueuedTasks> for JobDispatcherActor {
    type Result = Vec<TaskInfo>;

    fn handle(&mut self, _: GetQueuedTasks, _: &mut Self::Context) -> Self::Result {
        self.tasks.iter().map(|task| {
            TaskInfo {
                id: task.id.clone(),
                task_type: task.task_type.clone(),
                created_at: task.created_at,
                priority: task.priority.clone(),
                size_bytes: task.data.len(),
            }
        }).collect()
    }
}

// Handle task results
impl Handler<TaskResult> for JobDispatcherActor {
    type Result = ();

    fn handle(&mut self, result: TaskResult, _: &mut Self::Context) -> Self::Result {
        info!("Received result for task {}: {:?}", result.task_id, result.status);

        // Store the result
        self.results.insert(result.task_id.clone(), result.clone());

        // Update worker status if known
        if let Some(worker) = self.workers.get_mut(&result.processor_node) {
            worker.current_tasks = worker.current_tasks.saturating_sub(1);
            if matches!(result.status, TaskStatus::Completed) {
                worker.total_completed += 1;
            }
        }

        // Remove task from queue if present
        self.tasks.retain(|t| t.id != result.task_id);

        // Try to assign more tasks
        self.assign_queued_tasks();
    }
}

// Helper methods for JobDispatcherActor
impl JobDispatcherActor {
    fn check_worker_timeouts(&mut self) {
        let now = Self::now_millis();
        let timeout_threshold = 15000; // 15 seconds

        let timed_out_workers: Vec<String> = self.workers.iter()
            .filter(|(_, status)| now - status.last_heartbeat > timeout_threshold)
            .map(|(id, _)| id.clone())
            .collect();

        for worker_id in timed_out_workers {
            warn!("Worker {} timed out, removing", worker_id);
            self.workers.remove(&worker_id);
        }
    }

    fn assign_queued_tasks(&mut self) {
        // Sort tasks by priority (higher priority first)
        self.tasks.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut assigned_tasks = Vec::new();

        for task in &self.tasks {
            if let Some(worker_id) = self.find_worker_for_task(task) {
                info!("Assigning queued task {} to worker {}", task.id, worker_id);

                if let Some(worker_status) = self.workers.get_mut(&worker_id) {
                    worker_status.current_tasks += 1;
                }

                // In a real implementation, we would dispatch the task to the worker here
                assigned_tasks.push(task.id.clone());
            }
        }

        // Remove assigned tasks from the queue
        self.tasks.retain(|t| !assigned_tasks.contains(&t.id));
    }
}

// Implement AnyMessage handler to make it discoverable in the cluster
impl Handler<AnyMessage> for JobDispatcherActor {
    type Result = ();

    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(task) = msg.downcast::<Task>() {
            let task_clone = task.clone();
            self.handle(task_clone, ctx);
        } else if let Some(reg_msg) = msg.downcast::<RegisterWorker>() {
            let reg_msg_clone = reg_msg.clone();
            self.handle(reg_msg_clone, ctx);
        } else if let Some(hb_msg) = msg.downcast::<Heartbeat>() {
            let hb_msg_clone = hb_msg.clone();
            self.handle(hb_msg_clone, ctx);
        } else if let Some(result) = msg.downcast::<TaskResult>() {
            let result_clone = result.clone();
            self.handle(result_clone, ctx);
        } else {
            warn!("Received unknown message type");
        }
    }
}

// Worker actor that processes tasks
struct WorkerActor {
    node_id: String,
    worker_id: String,
    capabilities: Vec<TaskType>,
    max_tasks: usize,
    current_tasks: usize,
    total_completed: usize,
    dispatcher: Option<Box<dyn ActorRef>>,
}

impl WorkerActor {
    fn new(node_id: String, worker_id: String, capabilities: Vec<TaskType>, max_tasks: usize) -> Self {
        Self {
            node_id,
            worker_id,
            capabilities,
            max_tasks,
            current_tasks: 0,
            total_completed: 0,
            dispatcher: None,
        }
    }

    fn set_dispatcher(&mut self, dispatcher: Box<dyn ActorRef>) {
        self.dispatcher = Some(dispatcher);
    }

    fn now_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64
    }

    fn send_heartbeat(&self, ctx: &mut <Self as Actor>::Context) {
        if let Some(dispatcher) = &self.dispatcher {
            let heartbeat = Heartbeat {
                worker_id: self.worker_id.clone(),
                current_tasks: self.current_tasks,
                total_completed: self.total_completed,
            };

            let boxed_msg = Box::new(heartbeat) as Box<dyn std::any::Any + Send>;
            if let Err(e) = dispatcher.send_any(boxed_msg) {
                warn!("Failed to send heartbeat: {}", e);
            }
        }

        // Schedule next heartbeat
        ctx.run_later(Duration::from_secs(5), |actor, ctx| {
            actor.send_heartbeat(ctx);
        });
    }
}

impl Actor for WorkerActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("WorkerActor {} started on node {}", self.worker_id, self.node_id);

        // Start heartbeat
        self.send_heartbeat(ctx);
    }
}

// Handle tasks
impl Handler<Task> for WorkerActor {
    type Result = TaskResult;

    fn handle(&mut self, task: Task, _: &mut Self::Context) -> Self::Result {
        if self.current_tasks >= self.max_tasks {
            return TaskResult {
                task_id: task.id,
                status: TaskStatus::Failed,
                result: None,
                error: Some("Worker at capacity".to_string()),
                execution_time_ms: 0,
                processor_node: self.worker_id.clone(),
            };
        }

        // Check if we can handle this task type
        let can_handle = self.capabilities.iter().any(|cap| match cap {
            TaskType::Computation => matches!(task.task_type, TaskType::Computation),
            TaskType::IO => matches!(task.task_type, TaskType::IO),
            TaskType::LongRunning => matches!(task.task_type, TaskType::LongRunning),
        });

        if !can_handle {
            return TaskResult {
                task_id: task.id,
                status: TaskStatus::Failed,
                result: None,
                error: Some("Worker does not support this task type".to_string()),
                execution_time_ms: 0,
                processor_node: self.worker_id.clone(),
            };
        }

        info!("Processing task {}: {:?}", task.id, task.task_type);

        self.current_tasks += 1;

        let start_time = Self::now_millis();

        // Simulate processing time based on task type
        let (status, result, error) = match task.task_type {
            TaskType::Computation => {
                // Simulate computational task
                std::thread::sleep(Duration::from_millis(50));

                // "Process" the data
                let result = task.data.iter().map(|b| b.wrapping_add(1)).collect();
                (TaskStatus::Completed, Some(result), None)
            },
            TaskType::IO => {
                // Simulate I/O task with occasional failures
                std::thread::sleep(Duration::from_millis(100));

                let mut rng = thread_rng();
                if rng.gen_bool(0.1) {
                    (TaskStatus::Failed, None, Some("I/O Error".to_string()))
                } else {
                    (TaskStatus::Completed, Some(task.data), None)
                }
            },
            TaskType::LongRunning => {
                // Simulate long running task
                std::thread::sleep(Duration::from_millis(200));

                // Generate some result data
                let result = vec![42; task.data.len()];
                (TaskStatus::Completed, Some(result), None)
            }
        };

        let end_time = Self::now_millis();
        let execution_time = end_time - start_time;

        if matches!(status, TaskStatus::Completed) {
            self.total_completed += 1;
        }

        self.current_tasks -= 1;

        TaskResult {
            task_id: task.id,
            status,
            result,
            error,
            execution_time_ms: execution_time,
            processor_node: self.worker_id.clone(),
        }
    }
}

// Handle AnyMessage for cluster communication
impl Handler<AnyMessage> for WorkerActor {
    type Result = ();

    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(task) = msg.downcast::<Task>() {
            let task_clone = task.clone();
            let result = self.handle(task_clone, ctx);

            // Send result back to dispatcher
            if let Some(dispatcher) = &self.dispatcher {
                let boxed_msg = Box::new(result) as Box<dyn std::any::Any + Send>;
                if let Err(e) = dispatcher.send_any(boxed_msg) {
                    error!("Failed to send task result: {}", e);
                }
            }
        } else {
            warn!("Worker received unknown message type");
        }
    }
}

// Start a node in the task processor system
async fn start_node(node_id: String, addr: SocketAddr, seed_nodes: Vec<String>,
                   is_coordinator: bool, num_tasks: usize) -> std::io::Result<()> {
    // Create cluster configuration
    let mut config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(if is_coordinator { NodeRole::Peer } else { NodeRole::Peer })
        .bind_addr(addr)
        .cluster_name("task-processor-cluster".to_string())
        .serialization_format(SerializationFormat::Bincode);

    // Add seed nodes
    if !seed_nodes.is_empty() {
        let mut seed_addrs = Vec::new();
        for seed in seed_nodes {
            info!("Adding seed node: {}", seed);
            seed_addrs.push(seed);
        }
        config = config.seed_nodes(seed_addrs);
    }

    let config = config.build().expect("Failed to create cluster configuration");

    // Create and start cluster system
    let mut sys = ClusterSystem::new(config);
    sys.start().await.expect("Failed to start cluster");

    info!("Node {} started at address {}", node_id, addr);

    // Coordinator node (job dispatcher)
    if is_coordinator {
        let dispatcher = JobDispatcherActor::new(node_id.clone()).start();

        // Register dispatcher to the cluster
        let dispatcher_path = "/user/task/dispatcher";
        info!("Registering job dispatcher: {}", dispatcher_path);

        // Ensure cluster system is initialized
        tokio::time::sleep(Duration::from_secs(2)).await;

        match sys.register(dispatcher_path, dispatcher.clone()).await {
            Ok(_) => info!("Successfully registered job dispatcher"),
            Err(e) => error!("Failed to register job dispatcher: {}", e),
        }

        // Wait for the service to be fully registered
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Generate test tasks
        if num_tasks > 0 {
            info!("Generating {} test tasks...", num_tasks);

            let mut rng = thread_rng();
            for i in 0..num_tasks {
                let task_type = match i % 3 {
                    0 => TaskType::Computation,
                    1 => TaskType::IO,
                    _ => TaskType::LongRunning,
                };

                let priority = match i % 3 {
                    0 => TaskPriority::High,
                    1 => TaskPriority::Medium,
                    _ => TaskPriority::Low,
                };

                // Create random data
                let data_size = rng.gen_range(10..1000);
                let data: Vec<u8> = (0..data_size).map(|_| rng.gen()).collect();

                let task = Task {
                    id: format!("test-{}", i),
                    task_type,
                    data,
                    created_at: JobDispatcherActor::now_millis(),
                    priority,
                };

                // Submit task to local dispatcher directly
                if let Err(e) = dispatcher.send(task).await {
                    error!("Failed to send task: {}", e);
                }

                // Space out task submissions
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    } else {
        // Worker node
        // Wait a bit for the cluster to be ready
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Find the dispatcher
        info!("Looking for job dispatcher...");
        if let Some(dispatcher_ref) = sys.lookup("/user/task/dispatcher").await {
            info!("Found job dispatcher");

            // Create worker actor with random capabilities
            let mut rng = thread_rng();
            let mut capabilities = Vec::new();

            // Randomly select task types this worker can handle
            if rng.gen_bool(0.8) {
                capabilities.push(TaskType::Computation);
            }
            if rng.gen_bool(0.7) {
                capabilities.push(TaskType::IO);
            }
            if rng.gen_bool(0.5) {
                capabilities.push(TaskType::LongRunning);
            }

            // Ensure at least one capability
            if capabilities.is_empty() {
                capabilities.push(TaskType::Computation);
            }

            // Random max tasks (1-5)
            let max_tasks = rng.gen_range(1..6);

            let worker_id = format!("worker-{}", node_id);
            let worker = WorkerActor::new(
                node_id.clone(),
                worker_id.clone(),
                capabilities.clone(),
                max_tasks
            ).start();

            // Register worker actor to the cluster
            let worker_path = format!("/user/task/workers/{}", worker_id);
            info!("Registering worker {}: {:?}, max tasks: {}", worker_id, capabilities, max_tasks);

            match sys.register(&worker_path, worker.clone()).await {
                Ok(_) => info!("Successfully registered worker {}", worker_id),
                Err(e) => error!("Failed to register worker {}: {}", worker_id, e),
            }

            // Set dispatcher reference
            worker.do_send(SystemMessage::SetDispatcher(dispatcher_ref.clone()));

            // Register with the dispatcher
            let register_msg = RegisterWorker {
                worker_id: worker_id.clone(),
                capabilities,
                max_tasks,
            };

            let boxed_msg = Box::new(register_msg) as Box<dyn std::any::Any + Send>;
            match dispatcher_ref.send_any(boxed_msg) {
                Ok(_) => info!("Worker {} registered with dispatcher", worker_id),
                Err(e) => error!("Failed to register worker: {}", e),
            }
        } else {
            error!("Could not find job dispatcher");
        }
    }

    // Keep the node running
    let wait_time = if is_coordinator { 60 } else { 120 };
    info!("Node {} running for {} seconds", node_id, wait_time);
    tokio::time::sleep(Duration::from_secs(wait_time)).await;

    info!("Node {} shutting down", node_id);
    Ok(())
}

// System messages for worker configuration
#[derive(Message)]
#[rtype(result = "()")]
enum SystemMessage {
    SetDispatcher(Box<dyn ActorRef>),
}

impl Handler<SystemMessage> for WorkerActor {
    type Result = ();

    fn handle(&mut self, msg: SystemMessage, _: &mut Self::Context) -> Self::Result {
        match msg {
            SystemMessage::SetDispatcher(dispatcher) => {
                info!("Setting dispatcher for worker {}", self.worker_id);
                self.set_dispatcher(dispatcher);
            }
        }
    }
}

// Run multi-node test locally
async fn run_multi_node_test(node_count: usize, base_port: u16, num_tasks: usize) -> std::io::Result<()> {
    info!("Starting task processor with {} nodes", node_count);

    if node_count < 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Node count must be at least 2 (1 coordinator and 1 worker)"
        ));
    }

    // Create a LocalSet for running local tasks
    let local = tokio::task::LocalSet::new();

    // Set up the coordinator node
    let coordinator_addr_str = format!("127.0.0.1:{}", base_port);

    // Run all node tasks in the LocalSet
    local.run_until(async move {
        // Start the coordinator node first
        let coordinator_handle = tokio::task::spawn_local({
            let addr_str = coordinator_addr_str.clone();
            async move {
                match start_node(
                    "coordinator".to_string(),
                    addr_str.parse().unwrap(),
                    Vec::new(),
                    true,
                    num_tasks
                ).await {
                    Ok(_) => info!("Coordinator node completed"),
                    Err(e) => error!("Coordinator node failed: {:?}", e),
                }
            }
        });

        // Let the coordinator start up first
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Start worker nodes
        let mut worker_handles = Vec::new();
        for i in 1..node_count {
            let port = base_port + i as u16;
            let node_id = format!("node{}", i);
            let addr = format!("127.0.0.1:{}", port);
            let seed = coordinator_addr_str.clone();

            info!("Starting worker node {}, address: {}, seed: {}", node_id, addr, seed);

            let worker_handle = tokio::task::spawn_local(async move {
                match start_node(
                    node_id.clone(),
                    addr.parse().unwrap(),
                    vec![seed],
                    false,
                    0
                ).await {
                    Ok(_) => info!("Worker node {} completed", node_id),
                    Err(e) => error!("Worker node {} failed: {:?}", node_id, e),
                }
            });

            worker_handles.push(worker_handle);

            // Space out worker starts to avoid overloading
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Wait for coordinator to complete (it will finish first)
        if let Err(e) = coordinator_handle.await {
            error!("Coordinator task error: {:?}", e);
        }

        // Wait for all workers to complete
        for (i, handle) in worker_handles.into_iter().enumerate() {
            if let Err(e) = handle.await {
                error!("Worker task {} error: {:?}", i+1, e);
            }
        }

        info!("Task processor test completed");
    }).await;

    Ok(())
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Parse command line arguments
    let args = Args::from_args();

    if args.multi {
        // Multi-node mode - automatically start multiple nodes
        info!("Starting in multi-node mode with {} nodes, base port: {}", args.nodes, args.base_port);
        run_multi_node_test(args.nodes, args.base_port, args.tasks).await?;
    } else {
        // Single node mode - manual connection
        let mut seed_nodes = Vec::new();
        if let Some(seed) = args.seed {
            seed_nodes.push(seed);
        }

        // If no seed nodes, this is a coordinator, otherwise a worker
        let is_coordinator = seed_nodes.is_empty();

        info!("Starting {} node ID: {}, address: {}",
            if is_coordinator { "coordinator" } else { "worker" },
            args.id,
            args.address
        );

        if !seed_nodes.is_empty() {
            info!("Seed nodes: {:?}", seed_nodes);
        }

        // Start node
        start_node(args.id, args.address, seed_nodes, is_coordinator, args.tasks).await?;
    }

    Ok(())
}