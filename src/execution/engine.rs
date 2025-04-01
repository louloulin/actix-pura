use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use log::{debug, info, warn, error};
use uuid::Uuid;
use chrono::Utc;

use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};
use crate::models::execution::ExecutionRecord;
use crate::models::trade::Trade;
use crate::models::message::{Message, MessageType, ExecutionNotificationMessage, TradeNotificationMessage};
use crate::actor::{Actor, ActorRef, ActorContext, MessageHandler};
use crate::consensus::raft::{RaftClient, AppendLogRequest};
use super::order_book::OrderBook;
use super::matcher::{OrderMatcher, MatchResult};

/// 执行消息 - 用于请求订单执行
pub struct ExecuteOrderMessage {
    pub order: Order,
}

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
    pub executions: HashMap<String, ExecutionRecord>,
    /// 成交记录 - 按ID索引
    pub trades: HashMap<String, Trade>,
    /// Raft客户端
    pub raft_client: Option<Box<dyn ActorRef>>,
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
            raft_client: None,
            account_actor: None,
        }
    }
    
    /// 设置Raft客户端
    pub fn set_raft_client(&mut self, raft_client: Box<dyn ActorRef>) {
        self.raft_client = Some(raft_client);
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
        for execution in &result.executions {
            self.executions.insert(execution.execution_id.clone(), execution.clone());
        }
        
        // 创建成交记录（将多个执行合并为成交）
        let trades = self.create_trade_from_executions(&result.executions);
        for trade in &trades {
            self.trades.insert(trade.trade_id.clone(), trade.clone());
        }
        
        // 添加日志到Raft
        if let Some(raft) = &self.raft_client {
            for execution in &result.executions {
                let req = AppendLogRequest {
                    log_type: "execution".to_string(),
                    data: serde_json::to_string(execution).unwrap(),
                };
                
                let _result = ctx.ask(raft.clone(), req).await;
            }
            
            for trade in &trades {
                let req = AppendLogRequest {
                    log_type: "trade".to_string(),
                    data: serde_json::to_string(trade).unwrap(),
                };
                
                let _result = ctx.ask(raft.clone(), req).await;
            }
        }
        
        // 发送执行通知
        for execution in &result.executions {
            // 通知买方
            let buy_notification = ExecutionNotificationMessage {
                execution_id: execution.execution_id.clone(),
                account_id: execution.buyer_account_id.clone(),
                order_id: execution.order_id.clone(),
                symbol: execution.symbol.clone(),
                price: execution.price,
                quantity: execution.quantity,
                side: OrderSide::Buy,
                timestamp: execution.timestamp,
            };
            
            // 通知卖方
            let sell_notification = ExecutionNotificationMessage {
                execution_id: execution.execution_id.clone(),
                account_id: execution.seller_account_id.clone(),
                order_id: execution.counter_order_id.clone(),
                symbol: execution.symbol.clone(),
                price: execution.price,
                quantity: execution.quantity,
                side: OrderSide::Sell,
                timestamp: execution.timestamp,
            };
            
            if let Some(account) = &self.account_actor {
                let _result = ctx.tell(account.clone(), buy_notification);
                let _result = ctx.tell(account.clone(), sell_notification);
            }
        }
        
        // 发送成交通知
        for trade in &trades {
            let notification = TradeNotificationMessage {
                trade_id: trade.trade_id.clone(),
                symbol: trade.symbol.clone(),
                price: trade.price,
                quantity: trade.quantity,
                timestamp: trade.timestamp,
            };
            
            if let Some(account) = &self.account_actor {
                let _result = ctx.tell(account.clone(), notification);
            }
        }
        
        true
    }
    
    /// 从执行记录创建成交记录
    fn create_trade_from_executions(&self, executions: &[ExecutionRecord]) -> Vec<Trade> {
        // 按证券和价格分组执行记录
        let mut trade_groups: HashMap<(String, f64), Vec<&ExecutionRecord>> = HashMap::new();
        
        for exec in executions {
            let key = (exec.symbol.clone(), exec.price);
            trade_groups.entry(key).or_default().push(exec);
        }
        
        // 为每个分组创建成交记录
        let mut trades = Vec::new();
        
        for ((symbol, price), group) in trade_groups {
            let total_quantity: f64 = group.iter().map(|e| e.quantity).sum();
            let timestamp = group.iter().map(|e| e.timestamp).max().unwrap_or_else(Utc::now);
            
            let trade = Trade {
                trade_id: Uuid::new_v4().to_string(),
                symbol,
                price,
                quantity: total_quantity,
                timestamp,
                execution_ids: group.iter().map(|e| e.execution_id.clone()).collect(),
            };
            
            trades.push(trade);
        }
        
        trades
    }
    
    /// 执行订单
    pub async fn execute_order(&mut self, order: &mut Order, ctx: &mut ActorContext) -> MatchResult {
        let symbol = order.symbol.clone();
        let order_book = self.get_or_create_order_book(&symbol);
        
        let result = OrderMatcher::match_order(order_book, order);
        
        // 处理匹配结果
        let _ = self.process_match_result(result.clone(), ctx).await;
        
        // 执行连续撮合（检查订单簿中是否还有可以撮合的订单）
        let continuous_result = OrderMatcher::continuous_match(order_book);
        if continuous_result.has_matches() {
            let _ = self.process_match_result(continuous_result.clone(), ctx).await;
            
            // 合并结果
            let mut combined = result;
            combined.merge(continuous_result);
            return combined;
        }
        
        result
    }
}

impl Actor for ExecutionEngine {
    fn new_context(&self, ctx: &mut ActorContext) {}
}

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

impl MessageHandler<ExecutionNotificationMessage> for ExecutionEngine {
    async fn handle(&mut self, msg: ExecutionNotificationMessage, _ctx: &mut ActorContext) -> Option<Box<dyn std::any::Any>> {
        info!("Execution notification: {} {} @ {} for account {}", 
            msg.quantity, msg.symbol, msg.price, msg.account_id);
        None
    }
}

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
        let mut buy_order = Order {
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
        let _ = actor_system.ask(&engine_addr, msg).await;
        
        // 创建匹配的卖单
        let mut sell_order = Order {
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
        let result = actor_system.ask::<ExecuteOrderMessage, MatchResult>(&engine_addr, msg).await;
        
        // 验证结果
        if let Some(match_result) = result {
            assert!(match_result.has_matches());
            assert_eq!(match_result.executions.len(), 1);
            assert_eq!(match_result.executions[0].price, 150.0);
            assert_eq!(match_result.executions[0].quantity, 5.0);
        }
    }
} 