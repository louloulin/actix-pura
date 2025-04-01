use actix::prelude::*;
use log::{info, debug, error};

use crate::messages::{ChatMessage, JoinRoom, LeaveRoom};
use crate::actors::chat_service::ChatServiceActor;

// User actor
pub struct UserActor {
    pub user_id: String,
    pub node_id: String,
    pub chat_service: Addr<ChatServiceActor>,
    pub current_rooms: Vec<String>,
}

impl UserActor {
    pub fn new(user_id: String, node_id: String, chat_service: Addr<ChatServiceActor>) -> Self {
        Self {
            user_id,
            node_id,
            chat_service,
            current_rooms: Vec::new(),
        }
    }
}

impl Actor for UserActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("UserActor started for user {} on node {}", self.user_id, self.node_id);
    }
    
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // Leave all rooms when stopping
        for room in self.current_rooms.clone() {
            let leave_msg = LeaveRoom {
                user_id: self.user_id.clone(),
                room,
            };
            self.chat_service.do_send(leave_msg);
        }
        
        Running::Stop
    }
}

// Handle chat messages
impl Handler<ChatMessage> for UserActor {
    type Result = ();
    
    fn handle(&mut self, msg: ChatMessage, _: &mut Self::Context) -> Self::Result {
        // In a real application, this would send the message to the user's
        // client connection. For this example, we just log it.
        info!(
            "[User:{}] Received message from {} in room {}: {}",
            self.user_id, msg.from, msg.room, msg.content
        );
    }
}

// Handle join room
impl Handler<JoinRoom> for UserActor {
    type Result = ();
    
    fn handle(&mut self, msg: JoinRoom, _: &mut Self::Context) -> Self::Result {
        debug!("User {} is joining room {}", self.user_id, msg.room);
        
        // Only join if not already in the room
        if !self.current_rooms.contains(&msg.room) {
            self.current_rooms.push(msg.room.clone());
            
            // Forward to chat service
            let join_msg = JoinRoom {
                user_id: self.user_id.clone(),
                room: msg.room.clone(),
            };
            if let Err(e) = self.chat_service.try_send(join_msg) {
                error!("Failed to join room: {}", e);
                // Remove from local rooms list
                self.current_rooms.retain(|r| *r != msg.room);
            }
        }
    }
}

// Handle leave room
impl Handler<LeaveRoom> for UserActor {
    type Result = ();
    
    fn handle(&mut self, msg: LeaveRoom, _: &mut Self::Context) -> Self::Result {
        debug!("User {} is leaving room {}", self.user_id, msg.room);
        
        // Remove from local list
        self.current_rooms.retain(|r| *r != msg.room);
        
        // Forward to chat service
        let leave_msg = LeaveRoom {
            user_id: self.user_id.clone(),
            room: msg.room,
        };
        if let Err(e) = self.chat_service.try_send(leave_msg) {
            error!("Failed to leave room: {}", e);
        }
    }
} 