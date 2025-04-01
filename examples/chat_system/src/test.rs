#[cfg(test)]
mod tests {
    use actix::prelude::*;
    use std::time::Duration;
    use crate::actors::{ChatServiceActor, UserActor};
    use crate::messages::{ChatMessage, JoinRoom, GetRoomUsers, GetAllRooms};

    #[test]
    fn test_chat_service() {
        let rt = actix_rt::System::new();
        rt.block_on(async {
            // Create chat service actor
            let chat_service = ChatServiceActor::new("test_node".to_string()).start();
            
            // Create a few test users
            let user1 = UserActor::new(
                "user1".to_string(),
                "test_node".to_string(),
                chat_service.clone(),
            ).start();
            
            let user2 = UserActor::new(
                "user2".to_string(),
                "test_node".to_string(),
                chat_service.clone(),
            ).start();
            
            // Join a room
            let join_msg1 = JoinRoom {
                user_id: "user1".to_string(),
                room: "test_room".to_string(),
            };
            
            user1.send(join_msg1).await.unwrap();
            
            // Brief delay for actor processing
            actix_rt::time::sleep(Duration::from_millis(100)).await;
            
            // Join another user to the same room
            let join_msg2 = JoinRoom {
                user_id: "user2".to_string(),
                room: "test_room".to_string(),
            };
            
            user2.send(join_msg2).await.unwrap();
            
            // Brief delay for actor processing
            actix_rt::time::sleep(Duration::from_millis(100)).await;
            
            // Check room users
            let users = chat_service.send(GetRoomUsers {
                room: "test_room".to_string(),
            }).await.unwrap();
            
            assert_eq!(users.len(), 2);
            assert!(users.contains(&"user1".to_string()));
            assert!(users.contains(&"user2".to_string()));
            
            // Check available rooms
            let rooms = chat_service.send(GetAllRooms).await.unwrap();
            assert_eq!(rooms.len(), 1);
            assert!(rooms.contains(&"test_room".to_string()));
            
            // Send a message
            let msg = ChatMessage {
                from: "user1".to_string(),
                room: "test_room".to_string(),
                content: "Hello, user2!".to_string(),
                timestamp: 123456789,
            };
            
            user1.send(msg).await.unwrap();
            
            // Wait briefly to allow message propagation
            actix_rt::time::sleep(Duration::from_millis(100)).await;
            
            // Clean up by sending stop messages
            drop(user1);
            drop(user2);
            drop(chat_service);
            
            // Let the system clean up
            actix_rt::time::sleep(Duration::from_millis(10)).await;
        });
    }
} 