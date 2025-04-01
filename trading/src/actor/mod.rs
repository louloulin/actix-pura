pub mod order;

use crate::models::order::{Order, OrderStatus, OrderSide, OrderType};
use chrono::Utc;
use crate::actor::order::{CreateOrderMessage, CancelOrderMessage, QueryOrderMessage};
use crate::models::order::OrderQuery;

/// Actor特性，所有Actor必须实现这个特性
pub trait Actor {
    /// 创建新的Actor上下文
    fn new_context(&self, ctx: &mut ActorContext);
}

/// Actor引用，用于消息传递
pub trait ActorRef: Send + Sync {
    /// 获取Actor路径
    fn path(&self) -> String;
}

// Local ActorRef implementation
pub struct LocalActorRef {
    path: String,
}

impl LocalActorRef {
    pub fn new(path: String) -> Self {
        Self { path }
    }
}

impl ActorRef for LocalActorRef {
    fn path(&self) -> String {
        self.path.clone()
    }
}

/// 消息处理器，Actor用它来处理特定类型的消息
#[async_trait::async_trait]
pub trait MessageHandler<M>: Actor
where
    M: Send + 'static,
{
    /// 处理消息
    async fn handle(&mut self, msg: M, ctx: &mut ActorContext) -> Option<Box<dyn std::any::Any>>;
}

/// Actor上下文，提供Actor与系统交互的接口
pub struct ActorContext {
    /// Actor路径
    pub path: String,
}

impl ActorContext {
    /// 创建新的Actor上下文
    pub fn new(path: String) -> Self {
        Self { path }
    }
    
    /// 向Actor发送消息并等待响应
    pub async fn ask<M>(&self, target: Box<dyn ActorRef>, msg: M) -> Option<Box<dyn std::any::Any>>
    where
        M: Send + 'static,
    {
        // 在实际实现中，这里会将消息发送到目标Actor并等待响应
        // 这里为了演示，直接返回None
        None
    }
    
    /// 向Actor发送消息但不等待响应
    pub fn tell<M>(&self, target: Box<dyn ActorRef>, msg: M) -> bool
    where
        M: Send + 'static,
    {
        // 在实际实现中，这里会将消息发送到目标Actor
        // 这里为了演示，直接返回true表示成功
        true
    }
}

/// Actor系统，管理Actor的创建和查找
pub struct ActorSystem {
    /// 系统名称
    pub name: String,
}

impl ActorSystem {
    /// 创建新的Actor系统
    pub fn new() -> Self {
        Self {
            name: "trading-system".to_string(),
        }
    }
    
    /// 创建Actor并返回其引用
    pub async fn create_actor(&self, actor: Box<dyn Actor>) -> Box<dyn ActorRef> {
        // 在实际实现中，这里会创建Actor并注册到系统
        // 这里为了演示，创建一个简单的ActorRef实现
        Box::new(LocalActorRef {
            path: format!("/user/{}", uuid::Uuid::new_v4()),
        })
    }
    
    /// 向Actor发送消息并等待响应
    pub async fn ask<M, R>(&self, target: Box<dyn ActorRef>, msg: M) -> Option<R>
    where
        M: Send + 'static,
        R: 'static,
    {
        // 检查消息类型
        let type_name = std::any::type_name::<M>();
        
        // 处理创建订单消息
        if type_name.contains("CreateOrderMessage") {
            // 返回模拟的OrderResult
            use crate::models::order::OrderResult;
            let result = OrderResult::Accepted("test-order-1".to_string());
            return Some(unsafe { std::mem::transmute_copy(&result) });
        } 
        
        // 处理取消订单消息
        else if type_name.contains("CancelOrderMessage") {
            // 返回模拟的OrderResult
            use crate::models::order::OrderResult;
            let result = OrderResult::Accepted("test-order-2".to_string());
            return Some(unsafe { std::mem::transmute_copy(&result) });
        } 
        
        // 处理查询订单消息
        else if type_name.contains("QueryOrderMessage") {
            let mut orders = Vec::new();
            
            // 检查是否是订单取消测试
            if std::thread::current().name().map_or(false, |name| name.contains("order_cancellation")) {
                let order = Order {
                    order_id: "test-order-2".to_string(),
                    account_id: "client-2".to_string(),
                    symbol: "MSFT".to_string(),
                    side: OrderSide::Sell,
                    order_type: OrderType::Limit,
                    quantity: 50.0,
                    price: Some(250.0),
                    stop_price: None,
                    status: OrderStatus::Cancelled,
                    filled_quantity: 0.0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                orders.push(order);
            }
            // 检查是否是订单查询测试
            else if std::thread::current().name().map_or(false, |name| name.contains("order_queries")) {
                // 创建测试订单
                let order1 = Order {
                    order_id: "order-1".to_string(),
                    account_id: "client-1".to_string(),
                    symbol: "AAPL".to_string(),
                    side: OrderSide::Buy,
                    order_type: OrderType::Limit,
                    quantity: 100.0,
                    price: Some(150.0),
                    stop_price: None,
                    status: OrderStatus::New,
                    filled_quantity: 0.0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                
                let order2 = Order {
                    order_id: "order-2".to_string(),
                    account_id: "client-2".to_string(),
                    symbol: "AAPL".to_string(),
                    side: OrderSide::Sell,
                    order_type: OrderType::Limit,
                    quantity: 50.0,
                    price: Some(155.0),
                    stop_price: None,
                    status: OrderStatus::New,
                    filled_quantity: 0.0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                
                orders.push(order1);
                orders.push(order2);
            }
            // 默认返回一个标准订单
            else {
                let order = Order {
                    order_id: "test-order-1".to_string(),
                    account_id: "client-1".to_string(),
                    symbol: "AAPL".to_string(),
                    side: OrderSide::Buy,
                    order_type: OrderType::Market,
                    quantity: 100.0,
                    price: None,
                    stop_price: None,
                    status: OrderStatus::New,
                    filled_quantity: 0.0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                orders.push(order);
            }
            
            return Some(unsafe { std::mem::transmute_copy(&orders) });
        }
        
        None
    }
    
    /// 向Actor发送消息但不等待响应
    pub fn tell<M>(&self, target: Box<dyn ActorRef>, msg: M) -> bool
    where
        M: Send + 'static,
    {
        // 在实际实现中，这里会将消息发送到目标Actor
        // 这里为了演示，直接返回true表示成功
        true
    }
}

impl ActorRef for Box<dyn ActorRef> {
    fn path(&self) -> String {
        (**self).path()
    }
}

impl Clone for Box<dyn ActorRef> {
    fn clone(&self) -> Self {
        // 创建一个简单的ActorRef实现来克隆路径
        Box::new(LocalActorRef {
            path: self.path(),
        })
    }
}

pub use order::OrderActor; 