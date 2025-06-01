use actix::prelude::*;
use serde::{Serialize, Deserialize};

// User message
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
pub struct ChatMessage {
    pub from: String,
    pub room: String,
    pub content: String,
    pub timestamp: u64,
}

// Room management messages
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
pub struct JoinRoom {
    pub user_id: String,
    pub room: String,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "()")]
pub struct LeaveRoom {
    pub user_id: String,
    pub room: String,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Vec<String>")]
pub struct GetRoomUsers {
    pub room: String,
}

#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "Vec<String>")]
pub struct GetAllRooms; 