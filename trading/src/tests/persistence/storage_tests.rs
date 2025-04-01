use std::path::PathBuf;
use tokio::test;
use uuid::Uuid;

use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};
use crate::models::account::Account;
use crate::persistence::storage::{StorageProvider, InMemoryStorage, FileSystemStorage, StorageType};

#[test]
async fn test_in_memory_storage_save_load() {
    let storage = InMemoryStorage::new();
    let order_id = Uuid::new_v4().to_string();
    
    // 创建测试订单
    let order = Order::new(
        order_id.clone(),
        "account1".to_string(),
        "AAPL".to_string(),
        OrderSide::Buy,
        OrderType::Limit,
        100.0,
        Some(150.0),
        None,
    );
    
    // 保存订单
    let result = storage.save(StorageType::Order, &order_id, &order).await;
    assert!(result.is_ok(), "保存订单应该成功");
    
    // 加载订单
    let loaded: Order = storage.load(StorageType::Order, &order_id).await.unwrap();
    assert_eq!(loaded.id, order.id, "加载的订单ID应与原始订单ID匹配");
    assert_eq!(loaded.symbol, order.symbol, "加载的订单证券代码应与原始订单证券代码匹配");
    assert_eq!(loaded.account_id, order.account_id, "加载的订单账户ID应与原始订单账户ID匹配");
}

#[test]
async fn test_in_memory_storage_list_delete() {
    let storage = InMemoryStorage::new();
    
    // 创建多个测试订单
    for i in 0..5 {
        let order_id = format!("order-{}", i);
        let order = Order::new(
            order_id.clone(),
            "account1".to_string(),
            "AAPL".to_string(),
            OrderSide::Buy,
            OrderType::Limit,
            100.0,
            Some(150.0 + i as f64),
            None,
        );
        
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
    assert!(orders_after_delete.iter().all(|o| o.id != "order-2"), "order-2应该已被删除");
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
    let order = Order::new(
        "order1".to_string(),
        "account1".to_string(),
        "AAPL".to_string(),
        OrderSide::Buy,
        OrderType::Limit,
        100.0,
        Some(150.0),
        None,
    );
    storage.save(StorageType::Order, "order1", &order).await.unwrap();
    
    // 保存账户
    let account = Account::new("account1".to_string(), "Test Account");
    storage.save(StorageType::Account, "account1", &account).await.unwrap();
    
    // 检查可以正确加载不同类型的数据
    let loaded_order: Order = storage.load(StorageType::Order, "order1").await.unwrap();
    assert_eq!(loaded_order.id, "order1");
    
    let loaded_account: Account = storage.load(StorageType::Account, "account1").await.unwrap();
    assert_eq!(loaded_account.id, "account1");
    
    // 列出不同类型的数据
    let orders: Vec<Order> = storage.list(StorageType::Order).await.unwrap();
    assert_eq!(orders.len(), 1);
    
    let accounts: Vec<Account> = storage.list(StorageType::Account).await.unwrap();
    assert_eq!(accounts.len(), 1);
} 