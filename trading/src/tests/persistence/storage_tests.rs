use std::path::PathBuf;
use tokio::test;
use uuid::Uuid;

use crate::models::order::{Order, OrderSide, OrderType, OrderRequest};
use crate::models::account::Account;
use crate::persistence::storage::{StorageProvider, InMemoryStorage, FileSystemStorage, StorageType};
use chrono::Utc;

#[test]
async fn test_in_memory_storage_save_load() {
    let storage = InMemoryStorage::new();
    let order_id = Uuid::new_v4().to_string();
    
    // 创建测试订单
    let request = OrderRequest {
        order_id: Some(order_id.clone()),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        quantity: 100,
        price: Some(150.0),
        client_id: "account1".to_string(),
    };
    
    let order = Order::from_request(request);
    
    // 保存订单
    let result = storage.save(StorageType::Order, &order_id, &order).await;
    assert!(result.is_ok(), "保存订单应该成功");
    
    // 加载订单
    let loaded: Order = storage.load(StorageType::Order, &order_id).await.unwrap();
    assert_eq!(loaded.order_id, order.order_id, "加载的订单ID应与原始订单ID匹配");
    assert_eq!(loaded.symbol, order.symbol, "加载的订单证券代码应与原始订单证券代码匹配");
    assert_eq!(loaded.account_id, order.account_id, "加载的订单账户ID应与原始订单账户ID匹配");
}

#[test]
async fn test_in_memory_storage_list_delete() {
    let storage = InMemoryStorage::new();
    
    // 创建多个测试订单
    for i in 0..5 {
        let order_id = format!("order-{}", i);
        let request = OrderRequest {
            order_id: Some(order_id.clone()),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 100,
            price: Some(150.0 + i as f64),
            client_id: "account1".to_string(),
        };
        
        let order = Order::from_request(request);
        storage.save(StorageType::Order, &order_id, &order).await.unwrap();
    }
    
    // 列出所有订单
    let orders: Vec<Order> = storage.list(StorageType::Order).await.unwrap();
    assert_eq!(orders.len(), 5, "应该有5个订单");
    
    // 删除一个订单
    let delete_result = storage.delete(StorageType::Order, "order-2").await;
    assert!(delete_result.is_ok(), "删除订单应该成功");
    
    // 再次列出所有订单
    let orders_after_delete: Vec<Order> = storage.list(StorageType::Order).await.unwrap();
    assert_eq!(orders_after_delete.len(), 4, "删除后应该有4个订单");
    
    // 确认order-2被删除
    assert!(orders_after_delete.iter().all(|o| o.order_id != "order-2"), "order-2应该已被删除");
}

#[test]
async fn test_in_memory_storage_not_found() {
    let storage = InMemoryStorage::new();
    
    // 尝试加载不存在的订单
    let result = storage.load::<Order>(StorageType::Order, "non-existent").await;
    assert!(result.is_err(), "加载不存在的订单应该失败");
    
    if let Err(e) = result {
        let error_string = e.to_string();
        assert!(error_string.contains("not found"), "错误消息应该包含'not found'");
    }
}

#[test]
async fn test_multiple_storage_types() {
    let storage = InMemoryStorage::new();
    
    // 保存订单
    let request = OrderRequest {
        order_id: Some("order1".to_string()),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        order_type: OrderType::Limit,
        quantity: 100,
        price: Some(150.0),
        client_id: "account1".to_string(),
    };
    
    let order = Order::from_request(request);
    storage.save(StorageType::Order, "order1", &order).await.unwrap();
    
    // 保存账户
    let account = Account::new(
        "account1".to_string(), 
        "Test Account".to_string(),
        1000.0 // 添加初始余额参数
    );
    storage.save(StorageType::Account, "account1", &account).await.unwrap();
    
    // 检查可以正确加载不同类型的数据
    let loaded_order: Order = storage.load(StorageType::Order, "order1").await.unwrap();
    assert_eq!(loaded_order.order_id, "order1");
    
    let loaded_account: Account = storage.load(StorageType::Account, "account1").await.unwrap();
    assert_eq!(loaded_account.id, "account1");
    
    // 列出不同类型的数据
    let orders: Vec<Order> = storage.list(StorageType::Order).await.unwrap();
    assert_eq!(orders.len(), 1);
    
    let accounts: Vec<Account> = storage.list(StorageType::Account).await.unwrap();
    assert_eq!(accounts.len(), 1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::order::{Order, OrderRequest, OrderType, OrderSide, OrderStatus};
    use crate::models::account::Account;
    use std::collections::HashMap;

    #[actix_rt::test]
    async fn test_in_memory_store_and_load() {
        let storage = InMemoryStorage::new();
        
        // Create an order from a request
        let request = OrderRequest {
            order_id: Some("order1".to_string()),
            symbol: "BTC/USD".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 100,
            price: Some(50000.0),
            client_id: "account1".to_string(),
        };
        
        let order = Order::from_request(request);
        
        // Store the order
        storage.save(StorageType::Order, &order.order_id, &order).await.unwrap();
        
        // Load the order
        let loaded = storage.load::<Order>(StorageType::Order, &order.order_id).await.unwrap();
        
        // Verify fields match
        assert_eq!(loaded.order_id, order.order_id, "加载的订单ID应与原始订单ID匹配");
        assert_eq!(loaded.symbol, "BTC/USD");
        assert_eq!(loaded.quantity, 100.0);
        assert_eq!(loaded.price, Some(50000.0));
    }

    #[actix_rt::test]
    async fn test_in_memory_delete() {
        let storage = InMemoryStorage::new();
        
        // Create and store multiple orders
        for i in 1..=3 {
            let request = OrderRequest {
                order_id: Some(format!("order-{}", i)),
                symbol: "BTC/USD".to_string(),
                side: OrderSide::Buy,
                order_type: OrderType::Limit,
                quantity: 100,
                price: Some(50000.0),
                client_id: "account1".to_string(),
            };
            
            let order = Order::from_request(request);
            storage.save(StorageType::Order, &order.order_id, &order).await.unwrap();
        }
        
        // Verify all orders exist
        let orders = storage.list::<Order>(StorageType::Order).await.unwrap();
        assert_eq!(orders.len(), 3, "应该有3个订单");
        
        // Delete one order
        storage.delete(StorageType::Order, "order-2").await.unwrap();
        
        // Verify deleted order is gone
        let orders_after_delete = storage.list::<Order>(StorageType::Order).await.unwrap();
        assert_eq!(orders_after_delete.len(), 2, "删除后应该有2个订单");
        assert!(orders_after_delete.iter().all(|o| o.order_id != "order-2"), "order-2应该已被删除");
    }

    #[actix_rt::test]
    async fn test_in_memory_store_different_types() {
        let storage = InMemoryStorage::new();
        
        // Create and store an order
        let request = OrderRequest {
            order_id: Some("order1".to_string()),
            symbol: "BTC/USD".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 100,
            price: Some(50000.0),
            client_id: "account1".to_string(),
        };
        
        let order = Order::from_request(request);
        
        // Create and store an account
        let account = Account::new(
            "account1".to_string(), 
            "Test Account".to_string(),
            10000.0
        );
        
        // Store both
        storage.save(StorageType::Order, &order.order_id, &order).await.unwrap();
        storage.save(StorageType::Account, &account.id, &account).await.unwrap();
        
        // Load both back
        let loaded_order = storage.load::<Order>(StorageType::Order, "order1").await.unwrap();
        let loaded_account = storage.load::<Account>(StorageType::Account, "account1").await.unwrap();
        
        // Verify data
        assert_eq!(loaded_order.order_id, "order1");
        assert_eq!(loaded_account.id, "account1");
        assert_eq!(loaded_account.name, "Test Account");
    }
} 