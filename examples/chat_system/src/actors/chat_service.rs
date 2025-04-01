use std::collections::{HashMap, HashSet};
use actix::prelude::*;
use actix_cluster::registry::ActorRef;
use log::{info, warn};

use crate::messages::{ChatMessage, JoinRoom, LeaveRoom, GetRoomUsers, GetAllRooms};
use crate::utils::now_micros;

// Chat service actor
pub struct ChatServiceActor {
    node_id: String,
    // room -> set of user_ids
    rooms: HashMap<String, HashSet<String>>,
    // user_id -> actor_ref for direct messaging
    users: HashMap<String, Box<dyn ActorRef>>,
}

impl ChatServiceActor {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            rooms: HashMap::new(),
            users: HashMap::new(),
        }
    }
    
    pub fn register_user(&mut self, user_id: String, user_ref: Box<dyn ActorRef>) {
        info!("Registering user {}", user_id);
        self.users.insert(user_id, user_ref);
    }
    
    pub fn unregister_user(&mut self, user_id: &str) {
        info!("Unregistering user {}", user_id);
        self.users.remove(user_id);
        
        // Remove user from all rooms
        for room in self.rooms.values_mut() {
            room.remove(user_id);
        }
        
        // Clean up empty rooms
        self.rooms.retain(|room_name, users| {
            let is_empty = users.is_empty();
            if is_empty {
                info!("Room {} is now empty and has been removed", room_name);
            }
            !is_empty
        });
    }
}

impl Actor for ChatServiceActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("ChatServiceActor started on node {}", self.node_id);
    }
}

// Handle chat messages
impl Handler<ChatMessage> for ChatServiceActor {
    type Result = ();
    
    fn handle(&mut self, msg: ChatMessage, _: &mut Self::Context) -> Self::Result {
        info!("Message in room '{}' from {}: {}", msg.room, msg.from, msg.content);
        
        // Get users in the room
        if let Some(users) = self.rooms.get(&msg.room) {
            // Forward message to all users in the room
            for user_id in users {
                if let Some(user_ref) = self.users.get(user_id) {
                    // Skip sending to the original sender
                    if *user_id != msg.from {
                        let boxed_msg = Box::new(msg.clone()) as Box<dyn std::any::Any + Send>;
                        if let Err(e) = user_ref.send_any(boxed_msg) {
                            warn!("Failed to send message to {}: {}", user_id, e);
                        }
                    }
                }
            }
        } else {
            warn!("Message sent to non-existent room: {}", msg.room);
        }
    }
}

// Room management
impl Handler<JoinRoom> for ChatServiceActor {
    type Result = ();
    
    fn handle(&mut self, msg: JoinRoom, _: &mut Self::Context) -> Self::Result {
        info!("User {} is joining room {}", msg.user_id, msg.room);
        
        // Create room if it doesn't exist
        let room_users = self.rooms.entry(msg.room.clone()).or_insert_with(HashSet::new);
        room_users.insert(msg.user_id.clone());
        
        // Broadcast join notification to room
        let join_notification = ChatMessage {
            from: "SYSTEM".to_string(),
            room: msg.room.clone(),
            content: format!("{} has joined the room", msg.user_id),
            timestamp: now_micros(),
        };
        
        self.handle(join_notification, &mut Context::new());
    }
}

impl Handler<LeaveRoom> for ChatServiceActor {
    type Result = ();
    
    fn handle(&mut self, msg: LeaveRoom, _: &mut Self::Context) -> Self::Result {
        info!("User {} is leaving room {}", msg.user_id, msg.room);
        
        if let Some(room_users) = self.rooms.get_mut(&msg.room) {
            room_users.remove(&msg.user_id);
            
            // Remove room if empty
            if room_users.is_empty() {
                self.rooms.remove(&msg.room);
                info!("Room {} is now empty and has been removed", msg.room);
            } else {
                // Broadcast leave notification
                let leave_notification = ChatMessage {
                    from: "SYSTEM".to_string(),
                    room: msg.room.clone(),
                    content: format!("{} has left the room", msg.user_id),
                    timestamp: now_micros(),
                };
                
                self.handle(leave_notification, &mut Context::new());
            }
        }
    }
}

impl Handler<GetRoomUsers> for ChatServiceActor {
    type Result = Vec<String>;
    
    fn handle(&mut self, msg: GetRoomUsers, _: &mut Self::Context) -> Self::Result {
        if let Some(users) = self.rooms.get(&msg.room) {
            users.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }
}

impl Handler<GetAllRooms> for ChatServiceActor {
    type Result = Vec<String>;
    
    fn handle(&mut self, _: GetAllRooms, _: &mut Self::Context) -> Self::Result {
        self.rooms.keys().cloned().collect()
    }
}