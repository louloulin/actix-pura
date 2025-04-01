pub mod order;

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
        
        struct SimpleActorRef {
            path: String,
        }
        
        impl ActorRef for SimpleActorRef {
            fn path(&self) -> String {
                self.path.clone()
            }
        }
        
        Box::new(SimpleActorRef {
            path: format!("/user/{}", uuid::Uuid::new_v4()),
        })
    }
    
    /// 向Actor发送消息并等待响应
    pub async fn ask<M, R>(&self, target: Box<dyn ActorRef>, msg: M) -> Option<R>
    where
        M: Send + 'static,
        R: 'static,
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

impl ActorRef for Box<dyn ActorRef> {
    fn path(&self) -> String {
        (**self).path()
    }
}

pub use order::OrderActor; 