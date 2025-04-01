use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::consensus::state_machine::{StateMachine, TradingStateMachine, OrderFilter};
use crate::models::message::LogEntry;
use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};
use crate::models::account::{Account, Position};
use crate::models::execution::{ExecutionRecord, Trade};

#[tokio::test]
async fn test_state_machine_apply_order() {
    // 创建状态机
    let mut state_machine = TradingStateMachine::new();
    
    // 创建测试订单
    let order = Order {
        order_id: "order-1".to_string(),
        account_id: "acc-123".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        price: Some(100.0),
        quantity: 10,
        filled_quantity: 0,
        order_type: OrderType::Limit,
        status: OrderStatus::New,
        created_at: 1617235200,
        updated_at: 1617235200,
    };
    
    // 创建订单日志条目
    let log_entry = LogEntry::OrderRequest(order.clone().into());
    
    // 应用日志条目
    state_machine.apply(log_entry).unwrap();
    
    // 验证订单已添加到状态机
    let stored_order = state_machine.get_order(&order.order_id);
    assert!(stored_order.is_some());
    assert_eq!(stored_order.unwrap().order_id, order.order_id);
}

#[tokio::test]
async fn test_state_machine_apply_cancel_order() {
    // 创建状态机
    let mut state_machine = TradingStateMachine::new();
    
    // 创建测试订单
    let order = Order {
        order_id: "order-1".to_string(),
        account_id: "acc-123".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        price: Some(100.0),
        quantity: 10,
        filled_quantity: 0,
        order_type: OrderType::Limit,
        status: OrderStatus::New,
        created_at: 1617235200,
        updated_at: 1617235200,
    };
    
    // 创建订单日志条目
    let log_entry = LogEntry::OrderRequest(order.into());
    
    // 应用日志条目
    state_machine.apply(log_entry).unwrap();
    
    // 创建取消订单日志条目
    let cancel_entry = LogEntry::CancelOrder(crate::models::message::CancelOrderMessage {
        order_id: "order-1".to_string(),
        account_id: "acc-123".to_string(),
    });
    
    // 应用取消日志条目
    state_machine.apply(cancel_entry).unwrap();
    
    // 验证订单状态已更新
    let stored_order = state_machine.get_order("order-1");
    assert!(stored_order.is_some());
    assert_eq!(stored_order.unwrap().status, OrderStatus::Canceled);
}

#[tokio::test]
async fn test_state_machine_apply_execution() {
    // 创建状态机
    let mut state_machine = TradingStateMachine::new();
    
    // 创建买单
    let buy_order = Order {
        order_id: "buy-1".to_string(),
        account_id: "acc-123".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        price: Some(100.0),
        quantity: 10,
        filled_quantity: 0,
        order_type: OrderType::Limit,
        status: OrderStatus::New,
        created_at: 1617235200,
        updated_at: 1617235200,
    };
    
    // 创建卖单
    let sell_order = Order {
        order_id: "sell-1".to_string(),
        account_id: "acc-456".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Sell,
        price: Some(100.0),
        quantity: 5,
        filled_quantity: 0,
        order_type: OrderType::Limit,
        status: OrderStatus::New,
        created_at: 1617235200,
        updated_at: 1617235200,
    };
    
    // 添加订单到状态机
    state_machine.apply(LogEntry::OrderRequest(buy_order.into())).unwrap();
    state_machine.apply(LogEntry::OrderRequest(sell_order.into())).unwrap();
    
    // 创建执行记录
    let execution = ExecutionRecord {
        execution_id: "exec-1".to_string(),
        buy_order_id: "buy-1".to_string(),
        sell_order_id: "sell-1".to_string(),
        account_id: "acc-123".to_string(),
        symbol: "AAPL".to_string(),
        price: 100.0,
        quantity: 5,
        timestamp: 1617235300,
    };
    
    // 应用执行记录
    state_machine.apply(LogEntry::Execution(execution.clone())).unwrap();
    
    // 验证执行记录已添加
    let stored_execution = state_machine.get_execution(&execution.execution_id);
    assert!(stored_execution.is_some());
    assert_eq!(stored_execution.unwrap().execution_id, execution.execution_id);
    
    // 验证订单状态已更新
    let buy_order = state_machine.get_order("buy-1").unwrap();
    assert_eq!(buy_order.filled_quantity, 5);
    assert_eq!(buy_order.status, OrderStatus::PartiallyFilled);
    
    let sell_order = state_machine.get_order("sell-1").unwrap();
    assert_eq!(sell_order.filled_quantity, 5);
    assert_eq!(sell_order.status, OrderStatus::Filled);
}

#[tokio::test]
async fn test_state_machine_query_orders() {
    // 创建状态机
    let mut state_machine = TradingStateMachine::new();
    
    // 创建多个订单
    let orders = vec![
        Order {
            order_id: "order-1".to_string(),
            account_id: "acc-123".to_string(),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            price: Some(100.0),
            quantity: 10,
            filled_quantity: 0,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
            created_at: 1617235200,
            updated_at: 1617235200,
        },
        Order {
            order_id: "order-2".to_string(),
            account_id: "acc-123".to_string(),
            symbol: "MSFT".to_string(),
            side: OrderSide::Sell,
            price: Some(200.0),
            quantity: 5,
            filled_quantity: 0,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
            created_at: 1617235200,
            updated_at: 1617235200,
        },
        Order {
            order_id: "order-3".to_string(),
            account_id: "acc-456".to_string(),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            price: Some(101.0),
            quantity: 20,
            filled_quantity: 0,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
            created_at: 1617235200,
            updated_at: 1617235200,
        },
    ];
    
    // 添加订单到状态机
    for order in orders {
        state_machine.apply(LogEntry::OrderRequest(order.into())).unwrap();
    }
    
    // 测试按账户查询
    let account_filter = OrderFilter::new().with_account_id("acc-123".to_string());
    let account_orders = state_machine.query_orders(account_filter);
    assert_eq!(account_orders.len(), 2);
    
    // 测试按股票查询
    let symbol_filter = OrderFilter::new().with_symbol("AAPL".to_string());
    let symbol_orders = state_machine.query_orders(symbol_filter);
    assert_eq!(symbol_orders.len(), 2);
    
    // 测试组合查询
    let combined_filter = OrderFilter::new()
        .with_account_id("acc-123".to_string())
        .with_symbol("AAPL".to_string());
    let combined_orders = state_machine.query_orders(combined_filter);
    assert_eq!(combined_orders.len(), 1);
    assert_eq!(combined_orders[0].order_id, "order-1");
}

#[tokio::test]
async fn test_state_machine_snapshot_restore() {
    // 创建状态机
    let mut state_machine = TradingStateMachine::new();
    
    // 添加测试数据
    let order = Order {
        order_id: "order-1".to_string(),
        account_id: "acc-123".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        price: Some(100.0),
        quantity: 10,
        filled_quantity: 5,
        order_type: OrderType::Limit,
        status: OrderStatus::PartiallyFilled,
        created_at: 1617235200,
        updated_at: 1617235200,
    };
    
    let account = Account {
        account_id: "acc-123".to_string(),
        balance: 10000.0,
        positions: {
            let mut positions = HashMap::new();
            positions.insert("AAPL".to_string(), Position {
                symbol: "AAPL".to_string(),
                quantity: 5,
                average_price: 100.0,
            });
            positions
        },
        status: "active".to_string(),
        created_at: 1617235200,
    };
    
    // 应用日志条目
    state_machine.apply(LogEntry::OrderRequest(order.into())).unwrap();
    state_machine.apply(LogEntry::AccountUpdate(account.clone())).unwrap();
    
    // 创建快照
    let snapshot = state_machine.get_snapshot().unwrap();
    
    // 创建新的状态机
    let mut new_state_machine = TradingStateMachine::new();
    
    // 从快照恢复
    new_state_machine.restore_from_snapshot(snapshot).unwrap();
    
    // 验证恢复后的状态
    let restored_order = new_state_machine.get_order("order-1");
    assert!(restored_order.is_some());
    assert_eq!(restored_order.unwrap().filled_quantity, 5);
    
    let restored_account = new_state_machine.get_account(&account.account_id);
    assert!(restored_account.is_some());
    assert_eq!(restored_account.unwrap().balance, 10000.0);
    assert!(restored_account.unwrap().positions.contains_key("AAPL"));
}

#[tokio::test]
async fn test_run_state_machine() {
    // 创建通道
    let (tx, rx) = mpsc::channel::<LogEntry>(100);
    
    // 创建状态机
    let state_machine = Arc::new(RwLock::new(TradingStateMachine::new()));
    let state_machine_clone = state_machine.clone();
    
    // 启动状态机应用器
    let handle = tokio::spawn(async move {
        let mut lock = state_machine_clone.write().unwrap();
        crate::consensus::state_machine::run_state_machine(&mut *lock, rx).await.unwrap();
    });
    
    // 创建测试数据
    let order = Order {
        order_id: "order-1".to_string(),
        account_id: "acc-123".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        price: Some(100.0),
        quantity: 10,
        filled_quantity: 0,
        order_type: OrderType::Limit,
        status: OrderStatus::New,
        created_at: 1617235200,
        updated_at: 1617235200,
    };
    
    // 发送日志条目
    tx.send(LogEntry::OrderRequest(order.into())).await.unwrap();
    
    // 给状态机一些时间处理
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 验证订单已应用
    let lock = state_machine.read().unwrap();
    let stored_order = lock.get_order("order-1");
    assert!(stored_order.is_some());
    
    // 关闭通道，结束应用器
    drop(tx);
    handle.await.unwrap();
} 