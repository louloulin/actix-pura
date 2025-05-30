use std::collections::HashMap;
use log::{info, warn};
use chrono::Utc;
use actix::prelude::*;
use async_trait::async_trait;

use crate::models::order::{Order, OrderType, OrderSide, OrderStatus};
use crate::models::execution::{Execution, Trade};
use crate::actor::{Actor, ActorRef, ActorContext, MessageHandler};
use crate::models::message::{Message, MessageType, ExecutionNotificationMessage, TradeNotificationMessage};
use super::order_book::OrderBook;
use super::matcher::{OrderMatcher, MatchResult};

/// 执行消息 - 用于请求订单执行
pub struct ExecuteOrderMessage {
    pub order: Order,
}

// 实现actix::Message trait
impl actix::Message for ExecuteOrderMessage {
    type Result = ();
}

// 实现自定义Message trait
impl Message for ExecuteOrderMessage {
    fn message_type(&self) -> MessageType {
        MessageType::ExecuteOrder
    }
}

/// 执行引擎 - 负责订单执行和成交管理
pub struct ExecutionEngine {
    /// 节点ID
    pub node_id: String,
    /// 订单簿集合 - 按证券代码索引
    pub order_books: HashMap<String, OrderBook>,
    /// 撮合器
    pub matcher: OrderMatcher,
    /// 执行记录 - 按ID索引
    pub executions: HashMap<String, Execution>,
    /// 成交记录 - 按ID索引
    pub trades: HashMap<String, Trade>,
    /// 账户Actor
    pub account_actor: Option<Box<dyn ActorRef>>,
}

impl ExecutionEngine {
    /// 创建新的执行引擎
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            order_books: HashMap::new(),
            matcher: OrderMatcher,
            executions: HashMap::new(),
            trades: HashMap::new(),
            account_actor: None,
        }
    }
    
    /// 设置账户Actor
    pub fn set_account_actor(&mut self, account_actor: Box<dyn ActorRef>) {
        self.account_actor = Some(account_actor);
    }
    
    /// 获取或创建订单簿
    pub fn get_or_create_order_book(&mut self, symbol: &str) -> &mut OrderBook {
        if !self.order_books.contains_key(symbol) {
            let order_book = OrderBook::new(symbol.to_string());
            self.order_books.insert(symbol.to_string(), order_book);
        }
        
        self.order_books.get_mut(symbol).unwrap()
    }
    
    /// 处理匹配结果
    pub async fn process_match_result(&mut self, result: MatchResult, ctx: &mut ActorContext) -> bool {
        if !result.has_matches() {
            return false;
        }
        
        // 存储执行记录
        let executions_clone = result.executions.clone();
        for execution in &executions_clone {
            self.executions.insert(execution.execution_id.clone(), execution.clone());
        }
        
        // 创建成交记录（将多个执行合并为成交）
        let trades = self.create_trade_from_executions(&executions_clone);
        for trade in &trades {
            self.trades.insert(trade.trade_id.clone(), trade.clone());
        }
        
        // 发送执行通知
        for execution in &executions_clone {
            // 通知买方
            if let Some(buyer_account_id) = &execution.buyer_account_id {
                let buy_notification = ExecutionNotificationMessage {
                    execution_id: execution.execution_id.clone(),
                    account_id: buyer_account_id.clone(),
                    order_id: execution.order_id.clone(),
                    symbol: execution.symbol.clone(),
                    price: execution.price,
                    quantity: execution.quantity,
                    side: OrderSide::Buy,
                    timestamp: execution.executed_at,
                };
                
                if let Some(account) = &self.account_actor {
                    let _result = ctx.tell(account.clone(), buy_notification);
                }
            }
            
            // 通知卖方
            if let Some(seller_account_id) = &execution.seller_account_id {
                let sell_notification = ExecutionNotificationMessage {
                    execution_id: execution.execution_id.clone(),
                    account_id: seller_account_id.clone(),
                    order_id: execution.order_id.clone(), // 使用主订单ID
                    symbol: execution.symbol.clone(),
                    price: execution.price,
                    quantity: execution.quantity,
                    side: OrderSide::Sell,
                    timestamp: execution.executed_at,
                };
                
                if let Some(account) = &self.account_actor {
                    let _result = ctx.tell(account.clone(), sell_notification);
                }
            }
        }
        
        // 发送成交通知
        for trade in &trades {
            let notification = TradeNotificationMessage {
                trade_id: trade.trade_id.clone(),
                symbol: trade.symbol.clone(),
                price: trade.price,
                quantity: trade.quantity,
                timestamp: trade.traded_at,
            };
            
            if let Some(account) = &self.account_actor {
                let _result = ctx.tell(account.clone(), notification);
            }
        }
        
        true
    }
    
    /// 从执行记录创建成交记录
    fn create_trade_from_executions(&self, executions: &[Execution]) -> Vec<Trade> {
        // 按证券和价格分组执行记录 - 将价格转为整数以解决f64的Eq和Hash问题
        let mut trade_groups = HashMap::<(String, i64), Vec<Execution>>::new();
        
        for exec in executions {
            // 将价格乘以10000并转为整数作为key
            let price_key = (exec.price * 10000.0) as i64;
            let key = (exec.symbol.clone(), price_key);
            let entry = trade_groups.entry(key).or_insert_with(Vec::new);
            entry.push(exec.clone());
        }
        
        // 为每个分组创建成交记录
        let mut trades = Vec::new();
        
        for ((symbol, price_key), group) in trade_groups {
            let total_quantity: f64 = group.iter().map(|e| e.quantity).sum();
            let actual_price = (price_key as f64) / 10000.0; // 还原为实际价格
            
            // 找出买方和卖方订单
            let mut buy_order_id = String::new();
            let mut sell_order_id = String::new();
            let mut buyer_account_id = String::new();
            let mut seller_account_id = String::new();
            let mut buy_execution_id = String::new();
            let mut sell_execution_id = String::new();
            
            for exec in &group {
                if exec.side == OrderSide::Buy {
                    buy_order_id = exec.order_id.clone();
                    buy_execution_id = exec.execution_id.clone();
                    if let Some(acc_id) = &exec.buyer_account_id {
                        buyer_account_id = acc_id.clone();
                    }
                } else {
                    sell_order_id = exec.order_id.clone();
                    sell_execution_id = exec.execution_id.clone();
                    if let Some(acc_id) = &exec.seller_account_id {
                        seller_account_id = acc_id.clone();
                    }
                }
            }
            
            let trade = Trade::new(
                symbol,
                actual_price,
                total_quantity,
                buy_order_id,
                sell_order_id,
                buyer_account_id,
                seller_account_id,
                buy_execution_id,
                sell_execution_id
            );
            
            trades.push(trade);
        }
        
        trades
    }
    
    /// 执行订单
    pub async fn execute_order(&mut self, order: &mut Order, ctx: &mut ActorContext) -> MatchResult {
        let symbol = order.symbol.clone();
        
        // 进行撮合和处理结果
        let mut result = {
            let order_book = self.get_or_create_order_book(&symbol);
            let match_result = OrderMatcher::match_order(order_book, order);
            
            // 处理匹配结果
            let _ = self.process_match_result(match_result.clone(), ctx).await;
            match_result
        };
        
        // 执行连续撮合
        {
            let order_book = self.get_or_create_order_book(&symbol);
            let continuous_result = OrderMatcher::continuous_match(order_book);
            if continuous_result.has_matches() {
                // 处理匹配结果
                let _ = self.process_match_result(continuous_result.clone(), ctx).await;
                
                // 合并结果
                result.merge(continuous_result);
            }
        }
        
        result
    }

    /// 执行市价单 - 未实现
    #[allow(dead_code)] // 临时禁用未实现方法的警告
    async fn execute_market_order(&mut self, _order: &Order, _ctx: &mut ActorContext) -> MatchResult {
        // 市价单撮合暂未实现
        warn!("市价单撮合功能尚未实现");
        MatchResult::new()
    }
}

impl Actor for ExecutionEngine {
    fn new_context(&self, ctx: &mut ActorContext) {}
}

#[async_trait]
impl MessageHandler<ExecuteOrderMessage> for ExecutionEngine {
    async fn handle(&mut self, msg: ExecuteOrderMessage, ctx: &mut ActorContext) -> Option<Box<dyn std::any::Any>> {
        let mut order = msg.order;
        let result = self.execute_order(&mut order, ctx).await;
        
        if result.has_matches() {
            info!("Order execution completed: {} with {} matches", 
                order.order_id, result.executions.len());
        } else {
            info!("Order added to book: {}", order.order_id);
        }
        
        Some(Box::new(result))
    }
}

#[async_trait]
impl MessageHandler<ExecutionNotificationMessage> for ExecutionEngine {
    async fn handle(&mut self, msg: ExecutionNotificationMessage, _ctx: &mut ActorContext) -> Option<Box<dyn std::any::Any>> {
        info!("Execution notification: {} {} @ {} for account {}", 
            msg.quantity, msg.symbol, msg.price, msg.account_id);
        None
    }
}

#[async_trait]
impl MessageHandler<TradeNotificationMessage> for ExecutionEngine {
    async fn handle(&mut self, msg: TradeNotificationMessage, _ctx: &mut ActorContext) -> Option<Box<dyn std::any::Any>> {
        info!("Trade notification: {} {} @ {}", 
            msg.quantity, msg.symbol, msg.price);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::ActorSystem;
    
    #[tokio::test]
    async fn test_execution_engine() {
        let actor_system = ActorSystem::new();
        
        // 创建执行引擎
        let engine = ExecutionEngine::new("test-node".to_string());
        let engine_addr = actor_system.create_actor(Box::new(engine)).await;
        
        // 创建测试订单
        let buy_order = Order {
            order_id: "buy1".to_string(),
            account_id: "account1".to_string(),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 10.0,
            price: Some(150.0),
            stop_price: None,
            status: OrderStatus::New,
            filled_quantity: 0.0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // 发送订单到执行引擎
        let msg = ExecuteOrderMessage { order: buy_order };
        let _: Option<MatchResult> = actor_system.ask(engine_addr.clone(), msg).await;
        
        // 创建匹配的卖单
        let sell_order = Order {
            order_id: "sell1".to_string(),
            account_id: "account2".to_string(),
            symbol: "AAPL".to_string(),
            side: OrderSide::Sell,
            order_type: OrderType::Limit,
            quantity: 5.0,
            price: Some(148.0),
            stop_price: None,
            status: OrderStatus::New,
            filled_quantity: 0.0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // 发送卖单到执行引擎
        let msg = ExecuteOrderMessage { order: sell_order };
        let result = actor_system.ask::<ExecuteOrderMessage, MatchResult>(engine_addr.clone(), msg).await;
        
        // 验证结果
        if let Some(match_result) = result {
            assert!(match_result.has_matches());
            assert_eq!(match_result.executions.len(), 1);
            assert_eq!(match_result.executions[0].price, 150.0);
            assert_eq!(match_result.executions[0].quantity, 5.0);
        }
    }
} 