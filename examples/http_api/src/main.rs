use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use actix::prelude::*;
use actix_web::{web, App, HttpResponse, HttpServer, Responder, middleware, Error};
use actix_cluster::{
    ClusterSystem, ClusterConfig, Architecture, NodeRole, 
    SerializationFormat, NodeId, AnyMessage
};
use log::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use structopt::StructOpt;
use actix_cluster::registry::ActorRef;
use futures::StreamExt;

// Command line arguments
#[derive(StructOpt, Debug)]
#[structopt(name = "http_api", about = "HTTP API with Actix actor backend")]
struct Args {
    /// Node ID
    #[structopt(short, long, default_value = "node1")]
    id: String,

    /// Cluster address
    #[structopt(short, long, default_value = "127.0.0.1:8080")]
    cluster_address: SocketAddr,

    /// HTTP address
    #[structopt(long, default_value = "127.0.0.1:3000")]
    http_address: SocketAddr,

    /// Seed node address
    #[structopt(short, long)]
    seed: Option<String>,
}

// Data models
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id: String,
    name: String,
    email: String,
    created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
}

// Actor messages
#[derive(Message, Serialize, Deserialize)]
#[rtype(result = "Result<User, String>")]
struct CreateUser {
    name: String,
    email: String,
}

#[derive(Message, Serialize, Deserialize)]
#[rtype(result = "Result<User, String>")]
struct GetUser {
    id: String,
}

#[derive(Message, Serialize, Deserialize)]
#[rtype(result = "Result<Vec<User>, String>")]
struct ListUsers;

#[derive(Message, Serialize, Deserialize)]
#[rtype(result = "Result<bool, String>")]
struct DeleteUser {
    id: String,
}

// User service actor
struct UserServiceActor {
    node_id: String,
    users: HashMap<String, User>,
    next_id: usize,
}

impl UserServiceActor {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            users: HashMap::new(),
            next_id: 1,
        }
    }
    
    fn now_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64
    }
}

impl Actor for UserServiceActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("UserServiceActor started on node {}", self.node_id);
    }
}

// Create user handler
impl Handler<CreateUser> for UserServiceActor {
    type Result = Result<User, String>;
    
    fn handle(&mut self, msg: CreateUser, _: &mut Self::Context) -> Self::Result {
        let id = format!("user-{}", self.next_id);
        self.next_id += 1;
        
        let user = User {
            id: id.clone(),
            name: msg.name,
            email: msg.email,
            created_at: Self::now_millis(),
        };
        
        info!("Creating user: {}", id);
        self.users.insert(id, user.clone());
        
        Ok(user)
    }
}

// Get user handler
impl Handler<GetUser> for UserServiceActor {
    type Result = Result<User, String>;
    
    fn handle(&mut self, msg: GetUser, _: &mut Self::Context) -> Self::Result {
        match self.users.get(&msg.id) {
            Some(user) => {
                info!("Retrieved user: {}", msg.id);
                Ok(user.clone())
            },
            None => {
                warn!("User not found: {}", msg.id);
                Err(format!("User not found: {}", msg.id))
            }
        }
    }
}

// List users handler
impl Handler<ListUsers> for UserServiceActor {
    type Result = Result<Vec<User>, String>;
    
    fn handle(&mut self, _: ListUsers, _: &mut Self::Context) -> Self::Result {
        info!("Listing {} users", self.users.len());
        Ok(self.users.values().cloned().collect())
    }
}

// Delete user handler
impl Handler<DeleteUser> for UserServiceActor {
    type Result = Result<bool, String>;
    
    fn handle(&mut self, msg: DeleteUser, _: &mut Self::Context) -> Self::Result {
        if self.users.remove(&msg.id).is_some() {
            info!("Deleted user: {}", msg.id);
            Ok(true)
        } else {
            warn!("Failed to delete user: {}", msg.id);
            Err(format!("User not found: {}", msg.id))
        }
    }
}

// Handle AnyMessage for cluster communication
impl Handler<AnyMessage> for UserServiceActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        // Handle each message type, extracting by reference from the AnyMessage
        // and creating a new instance of the appropriate message type to handle
        if let Some(create_msg) = msg.downcast::<CreateUser>() {
            let create_msg = CreateUser {
                name: create_msg.name.clone(),
                email: create_msg.email.clone(),
            };
            let _ = UserServiceActor::handle(self, create_msg, ctx);
        } else if let Some(get_msg) = msg.downcast::<GetUser>() {
            let get_msg = GetUser {
                id: get_msg.id.clone(),
            };
            let _ = UserServiceActor::handle(self, get_msg, ctx);
        } else if let Some(_) = msg.downcast::<ListUsers>() {
            let _ = UserServiceActor::handle(self, ListUsers, ctx);
        } else if let Some(delete_msg) = msg.downcast::<DeleteUser>() {
            let delete_msg = DeleteUser {
                id: delete_msg.id.clone(),
            };
            let _ = UserServiceActor::handle(self, delete_msg, ctx);
        } else {
            warn!("UserServiceActor received unknown message type");
        }
    }
}

// App state holding actor address and cluster system
struct AppState {
    user_service: Addr<UserServiceActor>,
    cluster: Arc<StdMutex<Option<ClusterSystem>>>,
}

// API handlers
async fn create_user(
    data: web::Data<AppState>,
    req: web::Json<CreateUserRequest>,
) -> Result<impl Responder, Error> {
    let msg = CreateUser {
        name: req.name.clone(),
        email: req.email.clone(),
    };
    
    match data.user_service.send(msg).await {
        Ok(result) => match result {
            Ok(user) => Ok(HttpResponse::Ok().json(user)),
            Err(e) => Ok(HttpResponse::InternalServerError().body(e)),
        },
        Err(e) => {
            error!("Actor mailbox error: {}", e);
            Ok(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
}

async fn get_user(
    data: web::Data<AppState>,
    path: web::Path<(String,)>,
) -> Result<impl Responder, Error> {
    let id = path.0.clone();
    let msg = GetUser { id };
    
    match data.user_service.send(msg).await {
        Ok(result) => match result {
            Ok(user) => Ok(HttpResponse::Ok().json(user)),
            Err(e) => Ok(HttpResponse::NotFound().body(e)),
        },
        Err(e) => {
            error!("Actor mailbox error: {}", e);
            Ok(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
}

async fn list_users(
    data: web::Data<AppState>,
) -> Result<impl Responder, Error> {
    match data.user_service.send(ListUsers).await {
        Ok(result) => match result {
            Ok(users) => Ok(HttpResponse::Ok().json(users)),
            Err(e) => Ok(HttpResponse::InternalServerError().body(e)),
        },
        Err(e) => {
            error!("Actor mailbox error: {}", e);
            Ok(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
}

async fn delete_user(
    data: web::Data<AppState>,
    path: web::Path<(String,)>,
) -> Result<impl Responder, Error> {
    let id = path.0.clone();
    let msg = DeleteUser { id };
    
    match data.user_service.send(msg).await {
        Ok(result) => match result {
            Ok(_) => Ok(HttpResponse::NoContent().finish()),
            Err(e) => Ok(HttpResponse::NotFound().body(e)),
        },
        Err(e) => {
            error!("Actor mailbox error: {}", e);
            Ok(HttpResponse::InternalServerError().body(e.to_string()))
        }
    }
}

async fn health(
    data: web::Data<AppState>,
) -> Result<impl Responder, Error> {
    // Check if user service actor is alive
    if data.user_service.connected() {
        // Check if cluster is healthy
        let cluster_connected = {
            let cluster_guard = data.cluster.lock().unwrap();
            cluster_guard.is_some() // Presence of cluster means it's initialized
        };
        
        if cluster_connected {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "status": "healthy",
                "cluster": "connected",
                "timestamp": UserServiceActor::now_millis()
            })))
        } else {
            Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "status": "degraded",
                "cluster": "disconnected",
                "timestamp": UserServiceActor::now_millis()
            })))
        }
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "unhealthy",
            "user_service": "disconnected",
            "timestamp": UserServiceActor::now_millis()
        })))
    }
}

// Start the HTTP server and actor system
async fn start_server(args: Args) -> std::io::Result<()> {
    // Create user service actor
    let user_service = UserServiceActor::new(args.id.clone()).start();
    
    // Set up cluster configuration
    let mut config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(args.cluster_address)
        .cluster_name("http-api-cluster".to_string())
        .serialization_format(SerializationFormat::Bincode);
    
    // Add seed node if specified
    if let Some(seed) = args.seed {
        info!("Adding seed node: {}", seed);
        config = config.seed_nodes(vec![seed]);
    }
    
    let config = config.build().expect("Failed to create cluster configuration");
    
    // Create and start cluster system
    let mut sys = ClusterSystem::new(&args.id, config);
    let cluster_start_result = sys.start().await;
    
    match cluster_start_result {
        Ok(_) => info!("Cluster started at {}", args.cluster_address),
        Err(e) => error!("Failed to start cluster: {}", e),
    }
    
    // Register user service actor with the cluster
    let service_path = "/user/services/user";
    
    match sys.register(service_path, user_service.clone()).await {
        Ok(_) => info!("User service registered at {}", service_path),
        Err(e) => error!("Failed to register user service: {}", e),
    }
    
    // Create shared app state
    let cluster_mutex = Arc::new(StdMutex::new(Some(sys)));
    let app_state = web::Data::new(AppState {
        user_service,
        cluster: cluster_mutex.clone(),
    });
    
    // Start HTTP server
    info!("Starting HTTP server at {}", args.http_address);
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/api")
                    .route("/users", web::post().to(create_user))
                    .route("/users", web::get().to(list_users))
                    .route("/users/{id}", web::get().to(get_user))
                    .route("/users/{id}", web::delete().to(delete_user))
            )
            .route("/health", web::get().to(health))
    })
    .bind(args.http_address)?
    .run()
    .await?;
    
    // Clean up
    {
        let mut cluster_guard = cluster_mutex.lock().unwrap();
        if let Some(mut cluster) = cluster_guard.take() {
            info!("Shutting down cluster");
            // Just drop the cluster instance
        }
    }
    
    Ok(())
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // Parse command line arguments
    let args = Args::from_args();
    
    info!("Starting node ID: {}", args.id);
    info!("Cluster address: {}", args.cluster_address);
    info!("HTTP address: {}", args.http_address);
    
    if let Some(seed) = &args.seed {
        info!("Seed node: {}", seed);
    }
    
    // Start server
    start_server(args).await
} 