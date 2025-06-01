use std::collections::{BTreeMap, HashMap};
use log::warn;

use crate::models::order::{Order, OrderSide, OrderStatus};

/// 订单簿 - 管理单一证券的买卖订单
pub struct OrderBook {
    /// 证券代码
    pub symbol: String,
    /// 买单 - 按价格降序排列 (价格->订单列表)
    pub buy_orders: BTreeMap<i64, Vec<Order>>,
    /// 卖单 - 按价格升序排列 (价格->订单列表)
    pub sell_orders: BTreeMap<i64, Vec<Order>>,
    /// 所有订单 - 通过ID索引
    pub orders_by_id: HashMap<String, Order>,
}

impl OrderBook {
    /// 创建新的订单簿
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            buy_orders: BTreeMap::new(),
            sell_orders: BTreeMap::new(),
            orders_by_id: HashMap::new(),
        }
    }
    
    /// 添加订单到订单簿
    pub fn add_order(&mut self, order: Order) -> bool {
        // 验证证券代码
        if order.symbol != self.symbol {
            warn!("Order symbol {} does not match order book symbol {}", 
                order.symbol, self.symbol);
            return false;
        }
        
        // 检查订单状态
        if !order.is_active() {
            warn!("Cannot add inactive order {} to order book", order.order_id);
            return false;
        }
        
        // 市价单必须立即执行，不进入订单簿
        let has_price = match order.price {
            Some(price) if price > 0.0 => true,
            _ => false,
        };
        
        if has_price {
            // 转换为价格整数表示 (乘以100避免浮点数精度问题)
            let price_key = (order.price.unwrap() * 100.0) as i64;
            
            // 按照买卖方向添加到对应的订单列表
            match order.side {
                OrderSide::Buy => {
                    self.buy_orders.entry(price_key)
                        .or_insert_with(Vec::new)
                        .push(order.clone());
                },
                OrderSide::Sell => {
                    self.sell_orders.entry(price_key)
                        .or_insert_with(Vec::new)
                        .push(order.clone());
                },
            }
        }
        
        // 添加到订单ID索引
        self.orders_by_id.insert(order.order_id.clone(), order);
        
        true
    }
    
    /// 从订单簿中移除订单
    pub fn remove_order(&mut self, order_id: &str) -> Option<Order> {
        // 查找订单
        if let Some(order) = self.orders_by_id.remove(order_id) {
            // 从对应的价格列表中移除
            let price_key = (order.price.unwrap_or(0.0) * 100.0) as i64;
            
            match order.side {
                OrderSide::Buy => {
                    if let Some(orders) = self.buy_orders.get_mut(&price_key) {
                        orders.retain(|o| o.order_id != order_id);
                        if orders.is_empty() {
                            self.buy_orders.remove(&price_key);
                        }
                    }
                },
                OrderSide::Sell => {
                    if let Some(orders) = self.sell_orders.get_mut(&price_key) {
                        orders.retain(|o| o.order_id != order_id);
                        if orders.is_empty() {
                            self.sell_orders.remove(&price_key);
                        }
                    }
                },
            }
            
            Some(order)
        } else {
            None
        }
    }
    
    /// 更新订单状态
    pub fn update_order(&mut self, order_id: &str, status: OrderStatus, filled_quantity: Option<f64>) -> Option<Order> {
        // First, check if the order exists
        if !self.orders_by_id.contains_key(order_id) {
            return None;
        }
        
        // Clone the order to avoid multiple mutable borrows
        let mut order = self.orders_by_id.get(order_id).cloned().unwrap();
        
        // 更新状态
        order.update_status(status);
        
        // 更新成交数量
        if let Some(qty) = filled_quantity {
            order.update_filled_quantity(qty);
        }
        
        // If order is no longer active, remove it from price lists
        let is_active = order.is_active();
        
        // Update the order in the orders_by_id map
        self.orders_by_id.insert(order_id.to_string(), order.clone());
        
        // 如果订单不再活跃，从价格列表中移除
        if !is_active {
            self.remove_order(order_id);
        }
        
        Some(order)
    }
    
    /// 获取最优价格买单
    pub fn best_buy_price(&self) -> Option<f64> {
        self.buy_orders.iter().next_back()
            .map(|(price_key, _)| *price_key as f64 / 100.0)
    }
    
    /// 获取最优价格卖单
    pub fn best_sell_price(&self) -> Option<f64> {
        self.sell_orders.iter().next()
            .map(|(price_key, _)| *price_key as f64 / 100.0)
    }
    
    /// 获取订单簿买方深度 (价格 -> 数量累计)
    pub fn buy_depth(&self) -> Vec<(f64, f64)> {
        let mut depth = Vec::new();
        let mut total_qty = 0.0;
        
        // 从高价到低价遍历买单
        for (price_key, orders) in self.buy_orders.iter().rev() {
            let price = *price_key as f64 / 100.0;
            let qty: f64 = orders.iter().map(|o| o.remaining_quantity()).sum();
            
            total_qty += qty;
            depth.push((price, total_qty));
        }
        
        depth
    }
    
    /// 获取订单簿卖方深度 (价格 -> 数量累计)
    pub fn sell_depth(&self) -> Vec<(f64, f64)> {
        let mut depth = Vec::new();
        let mut total_qty = 0.0;
        
        // 从低价到高价遍历卖单
        for (price_key, orders) in self.sell_orders.iter() {
            let price = *price_key as f64 / 100.0;
            let qty: f64 = orders.iter().map(|o| o.remaining_quantity()).sum();
            
            total_qty += qty;
            depth.push((price, total_qty));
        }
        
        depth
    }
    
    /// 获取特定价格的买单
    pub fn get_buy_orders_at_price(&self, price: f64) -> Vec<Order> {
        let price_key = (price * 100.0) as i64;
        self.buy_orders.get(&price_key)
            .cloned()
            .unwrap_or_default()
    }
    
    /// 获取特定价格的卖单
    pub fn get_sell_orders_at_price(&self, price: f64) -> Vec<Order> {
        let price_key = (price * 100.0) as i64;
        self.sell_orders.get(&price_key)
            .cloned()
            .unwrap_or_default()
    }
    
    /// 获取所有可以与指定价格匹配的买单
    pub fn get_matching_buy_orders(&self, price: f64) -> Vec<Order> {
        let target_price_key = (price * 100.0) as i64;
        let mut result = Vec::new();
        
        // 收集所有价格大于等于目标价格的买单
        for (price_key, orders) in self.buy_orders.range(target_price_key..) {
            result.extend(orders.clone());
        }
        
        result
    }
    
    /// 获取所有可以与指定价格匹配的卖单
    pub fn get_matching_sell_orders(&self, price: f64) -> Vec<Order> {
        let target_price_key = (price * 100.0) as i64;
        let mut result = Vec::new();
        
        // 收集所有价格小于等于目标价格的卖单
        for (price_key, orders) in self.sell_orders.range(..=target_price_key) {
            result.extend(orders.clone());
        }
        
        result
    }
    
    /// 获取订单簿中的所有活跃订单
    pub fn get_all_active_orders(&self) -> Vec<Order> {
        self.orders_by_id.values()
            .filter(|o| o.is_active())
            .cloned()
            .collect()
    }
    
    /// 获取订单簿中的活跃买单
    pub fn get_active_buy_orders(&self) -> Vec<Order> {
        self.orders_by_id.values()
            .filter(|o| o.is_active() && o.side == OrderSide::Buy)
            .cloned()
            .collect()
    }
    
    /// 获取订单簿中的活跃卖单
    pub fn get_active_sell_orders(&self) -> Vec<Order> {
        self.orders_by_id.values()
            .filter(|o| o.is_active() && o.side == OrderSide::Sell)
            .cloned()
            .collect()
    }
    
    /// 获取订单簿统计信息
    pub fn get_summary(&self) -> OrderBookSummary {
        OrderBookSummary {
            symbol: self.symbol.clone(),
            buy_orders_count: self.get_active_buy_orders().len(),
            sell_orders_count: self.get_active_sell_orders().len(),
            best_buy_price: self.best_buy_price(),
            best_sell_price: self.best_sell_price(),
            buy_volume: self.buy_volume(),
            sell_volume: self.sell_volume(),
        }
    }
    
    /// 计算买方总量
    fn buy_volume(&self) -> f64 {
        self.get_active_buy_orders()
            .iter()
            .map(|o| o.remaining_quantity())
            .sum()
    }
    
    /// 计算卖方总量
    fn sell_volume(&self) -> f64 {
        self.get_active_sell_orders()
            .iter()
            .map(|o| o.remaining_quantity())
            .sum()
    }
    
    /// 清空订单簿
    pub fn clear(&mut self) {
        self.buy_orders.clear();
        self.sell_orders.clear();
        self.orders_by_id.clear();
    }
}

/// 订单簿摘要信息
#[derive(Debug, Clone)]
pub struct OrderBookSummary {
    pub symbol: String,
    pub buy_orders_count: usize,
    pub sell_orders_count: usize,
    pub best_buy_price: Option<f64>,
    pub best_sell_price: Option<f64>,
    pub buy_volume: f64,
    pub sell_volume: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::order::{OrderType};
    use chrono::Utc;
    
    fn create_test_order(id: &str, side: OrderSide, price: f64, quantity: f64) -> Order {
        Order {
            order_id: id.to_string(),
            account_id: "test_account".to_string(),
            symbol: "AAPL".to_string(),
            side,
            order_type: OrderType::Limit,
            quantity,
            price: Some(price),
            stop_price: None,
            status: OrderStatus::New,
            filled_quantity: 0.0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
    
    #[test]
    fn test_add_and_remove_orders() {
        let mut book = OrderBook::new("AAPL".to_string());
        
        // 添加买单
        let buy_order1 = create_test_order("buy1", OrderSide::Buy, 150.0, 10.0);
        let buy_order2 = create_test_order("buy2", OrderSide::Buy, 155.0, 5.0);
        
        assert!(book.add_order(buy_order1.clone()));
        assert!(book.add_order(buy_order2.clone()));
        
        // 添加卖单
        let sell_order1 = create_test_order("sell1", OrderSide::Sell, 160.0, 8.0);
        let sell_order2 = create_test_order("sell2", OrderSide::Sell, 165.0, 3.0);
        
        assert!(book.add_order(sell_order1.clone()));
        assert!(book.add_order(sell_order2.clone()));
        
        // 验证订单数量
        assert_eq!(book.orders_by_id.len(), 4);
        assert_eq!(book.buy_orders.len(), 2); // 两个不同价格的买单
        assert_eq!(book.sell_orders.len(), 2); // 两个不同价格的卖单
        
        // 验证最优价格
        assert_eq!(book.best_buy_price(), Some(155.0)); // 买单最高价
        assert_eq!(book.best_sell_price(), Some(160.0)); // 卖单最低价
        
        // 移除订单
        let removed = book.remove_order("buy1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().order_id, "buy1");
        
        // 验证移除后的状态
        assert_eq!(book.orders_by_id.len(), 3);
        assert_eq!(book.buy_orders.len(), 1);
        assert_eq!(book.best_buy_price(), Some(155.0));
        
        // 移除不存在的订单
        let removed = book.remove_order("nonexistent");
        assert!(removed.is_none());
    }
    
    #[test]
    fn test_update_order() {
        let mut book = OrderBook::new("AAPL".to_string());
        
        // 添加一个买单
        let buy_order = create_test_order("buy1", OrderSide::Buy, 150.0, 10.0);
        book.add_order(buy_order);
        
        // 更新订单状态
        let updated = book.update_order("buy1", OrderStatus::PartiallyFilled, Some(5.0));
        assert!(updated.is_some());
        
        // 验证更新后的状态
        let order = book.orders_by_id.get("buy1").unwrap();
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.filled_quantity, 5.0);
        assert_eq!(order.remaining_quantity(), 5.0);
        
        // 更新为已完成状态
        let updated = book.update_order("buy1", OrderStatus::Filled, Some(5.0));
        assert!(updated.is_some());
        
        // 验证订单已从活跃列表中移除
        assert!(!book.orders_by_id.contains_key("buy1"));
        assert_eq!(book.buy_orders.len(), 0);
    }
    
    #[test]
    fn test_order_book_depth() {
        let mut book = OrderBook::new("AAPL".to_string());
        
        // 添加买单
        book.add_order(create_test_order("buy1", OrderSide::Buy, 150.0, 10.0));
        book.add_order(create_test_order("buy2", OrderSide::Buy, 155.0, 5.0));
        book.add_order(create_test_order("buy3", OrderSide::Buy, 150.0, 3.0));
        
        // 添加卖单
        book.add_order(create_test_order("sell1", OrderSide::Sell, 160.0, 8.0));
        book.add_order(create_test_order("sell2", OrderSide::Sell, 165.0, 3.0));
        book.add_order(create_test_order("sell3", OrderSide::Sell, 160.0, 4.0));
        
        // 获取买方深度
        let buy_depth = book.buy_depth();
        assert_eq!(buy_depth.len(), 2); // 两个不同价格
        assert_eq!(buy_depth[0], (155.0, 5.0)); // 价格155有5.0数量
        assert_eq!(buy_depth[1], (150.0, 18.0)); // 价格150增加13.0，总共18.0
        
        // 获取卖方深度
        let sell_depth = book.sell_depth();
        assert_eq!(sell_depth.len(), 2); // 两个不同价格
        assert_eq!(sell_depth[0], (160.0, 12.0)); // 价格160有12.0数量
        assert_eq!(sell_depth[1], (165.0, 15.0)); // 价格165增加3.0，总共15.0
    }
    
    #[test]
    fn test_matching_orders() {
        let mut book = OrderBook::new("AAPL".to_string());
        
        // 添加买单
        book.add_order(create_test_order("buy1", OrderSide::Buy, 150.0, 10.0));
        book.add_order(create_test_order("buy2", OrderSide::Buy, 155.0, 5.0));
        book.add_order(create_test_order("buy3", OrderSide::Buy, 160.0, 3.0));
        
        // 添加卖单
        book.add_order(create_test_order("sell1", OrderSide::Sell, 158.0, 8.0));
        book.add_order(create_test_order("sell2", OrderSide::Sell, 165.0, 3.0));
        book.add_order(create_test_order("sell3", OrderSide::Sell, 170.0, 4.0));
        
        // 获取匹配价格158的买单（价格大于等于158）
        let matching_buys = book.get_matching_buy_orders(158.0);
        assert_eq!(matching_buys.len(), 1);
        assert_eq!(matching_buys[0].order_id, "buy3");
        
        // 获取匹配价格160的卖单（价格小于等于160）
        let matching_sells = book.get_matching_sell_orders(160.0);
        assert_eq!(matching_sells.len(), 1);
        assert_eq!(matching_sells[0].order_id, "sell1");
    }
} 