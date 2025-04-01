use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};
use chrono::Utc;
use crate::execution::matcher::OrderMatcher;
use crate::execution::order_book::OrderBook;

#[test]
fn test_remaining_quantity_calculation() {
    // 创建测试订单
    let mut order = Order {
        order_id: "test-order".to_string(),
        account_id: "test-account".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Sell,
        order_type: OrderType::Limit,
        quantity: 10.0,
        price: Some(150.0),
        stop_price: None,
        status: OrderStatus::New,
        filled_quantity: 0.0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    
    // 初始状态应该是 10.0 - 0.0 = 10.0
    assert_eq!(order.remaining_quantity(), 10.0);
    
    // 部分成交后
    order.filled_quantity = 3.0;
    assert_eq!(order.remaining_quantity(), 7.0);
    assert_eq!(order.quantity - order.filled_quantity, 7.0);
    println!("量: {}, 已成交: {}, 计算的剩余: {}, remaining_quantity(): {}", 
             order.quantity, order.filled_quantity, 
             order.quantity - order.filled_quantity, order.remaining_quantity());
    
    // 完全成交后
    order.filled_quantity = 10.0;
    assert_eq!(order.remaining_quantity(), 0.0);
}

#[test]
fn test_debug_market_order_match_remaining_quantity() {
    // Create an order book
    let mut order_book = OrderBook::new("AAPL".to_string());
    
    // Create and add sell orders to the book
    let sell_order1 = Order {
        order_id: "sell1".to_string(),
        account_id: "test_account".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Sell,
        order_type: OrderType::Limit,
        quantity: 5.0,
        price: Some(150.0),
        stop_price: None,
        status: OrderStatus::New,
        filled_quantity: 0.0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    
    let sell_order2 = Order {
        order_id: "sell2".to_string(),
        account_id: "test_account".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Sell,
        order_type: OrderType::Limit,
        quantity: 10.0,
        price: Some(155.0),
        stop_price: None,
        status: OrderStatus::New,
        filled_quantity: 0.0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    
    println!("\n----- TEST STARTING -----");
    println!("Initial sell_order2 quantity: {}, filled: 0.0, expected remaining: 10.0", sell_order2.quantity);
    
    // Clone the orders before adding them to the order book
    order_book.add_order(sell_order1.clone());
    order_book.add_order(sell_order2.clone());
    
    // Create a market buy order that will partially fill sell_order2
    let mut buy_order = Order {
        order_id: "buy1".to_string(),
        account_id: "test_account".to_string(),
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        order_type: OrderType::Market,
        quantity: 8.0,
        price: None,
        stop_price: None,
        status: OrderStatus::New,
        filled_quantity: 0.0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    
    // Match the orders
    let result = OrderMatcher::match_order(&mut order_book, &mut buy_order);
    
    // Get the remaining sell order
    let remaining_sell = order_book.orders_by_id.get("sell2").unwrap();
    
    // Print detailed debugging information
    println!("\nAfter match:");
    println!("sell_order2 quantity: {}", remaining_sell.quantity);
    println!("sell_order2 filled_quantity: {}", remaining_sell.filled_quantity);
    println!("Manual calculation: {} - {} = {}", 
             remaining_sell.quantity, 
             remaining_sell.filled_quantity, 
             remaining_sell.quantity - remaining_sell.filled_quantity);
    println!("remaining_quantity() method returns: {}", remaining_sell.remaining_quantity());
    
    // Look at all executions
    println!("\nExecution records:");
    for (i, exec) in result.executions.iter().enumerate() {
        println!("Execution {}: order={}, counter={:?}, price={}, qty={}", 
                i, exec.order_id, exec.counter_order_id, exec.price, exec.quantity);
    }
    
    // Check all updated orders
    println!("\nUpdated orders:");
    for (i, order) in result.updated_orders.iter().enumerate() {
        println!("Updated order {}: id={}, status={:?}, qty={}, filled={}, remaining={}", 
                i, order.order_id, order.status, order.quantity, 
                order.filled_quantity, order.remaining_quantity());
    }
    
    // Assert the expected behavior
    assert_eq!(remaining_sell.filled_quantity, 3.0);
    
    // The issue: This is failing in the matcher test but works in the direct test
    assert_eq!(remaining_sell.remaining_quantity(), remaining_sell.quantity - remaining_sell.filled_quantity);
    
    // Check updated orders in the match result
    for order in result.updated_orders.iter() {
        if order.order_id == "sell2" {
            println!("\nSell2 order from result - id: {}, quantity: {}, filled: {}, remaining method: {}, manual calc: {}",
                     order.order_id, order.quantity, order.filled_quantity, 
                     order.remaining_quantity(), order.quantity - order.filled_quantity);
        }
    }
    println!("----- TEST ENDING -----\n");
} 