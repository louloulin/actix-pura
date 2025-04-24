use std::sync::Arc;
use std::time::Duration;
use actix::prelude::*;
use actix_cluster::prelude::*;
use actix_cluster::actor::DistributedActorExt;
use actix_cluster::node::{NodeId, NodeInfo, PlacementStrategy};
use actix_cluster::config::{ClusterConfig, NodeRole, DiscoveryMethod};
use actix_cluster::message::{MessageEnvelope, DeliveryGuarantee};
use actix_cluster::serialization::SerializationFormat;

// 定义一个简单的消息
#[derive(Message, Clone, serde::Serialize, serde::Deserialize)]
#[rtype(result = "String")]
struct Ping(String);

// 定义第一个使用本地亲和性的 Actor
#[derive(Clone)]
struct PrimaryActor {
    group_id: String,
}

impl Actor for PrimaryActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("PrimaryActor started in group: {}", self.group_id);
    }
}

impl DistributedActor for PrimaryActor {
    fn actor_path(&self) -> String {
        format!("/user/{}/primary", self.group_id)
    }

    fn placement_strategy(&self) -> PlacementStrategy {
        // 使用本地亲和性策略，如果本地节点不可用，回退到随机策略
        PlacementStrategy::LocalAffinity {
            fallback: Box::new(PlacementStrategy::Random),
            group: Some(self.group_id.clone()),
        }
    }

    fn serialization_format(&self) -> SerializationFormat {
        SerializationFormat::Json
    }
}

impl Handler<Ping> for PrimaryActor {
    type Result = String;

    fn handle(&mut self, msg: Ping, _ctx: &mut Self::Context) -> Self::Result {
        println!("PrimaryActor received: {}", msg.0);
        format!("Primary in group {} received: {}", self.group_id, msg.0)
    }
}

// 定义第二个使用相同亲和性组的 Actor
#[derive(Clone)]
struct SecondaryActor {
    group_id: String,
    primary_path: String,
}

impl Actor for SecondaryActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("SecondaryActor started in group: {}", self.group_id);

        // 启动后与主Actor通信
        ctx.run_later(Duration::from_secs(1), move |_act, ctx| {
            println!("Secondary actor sending ping to primary");
            // 在真实环境中，这里会通过集群找到主节点并发送消息
            // 为简化示例，这里省略了查找步骤
        });
    }
}

impl DistributedActor for SecondaryActor {
    fn actor_path(&self) -> String {
        format!("/user/{}/secondary", self.group_id)
    }

    fn placement_strategy(&self) -> PlacementStrategy {
        // 使用相同的本地亲和性组，尝试与主Actor部署在同一节点
        PlacementStrategy::LocalAffinity {
            fallback: Box::new(PlacementStrategy::Random),
            group: Some(self.group_id.clone()),
        }
    }

    fn serialization_format(&self) -> SerializationFormat {
        SerializationFormat::Json
    }
}

impl Handler<Ping> for SecondaryActor {
    type Result = String;

    fn handle(&mut self, msg: Ping, _ctx: &mut Self::Context) -> Self::Result {
        println!("SecondaryActor received: {}", msg.0);
        format!("Secondary in group {} received: {}", self.group_id, msg.0)
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    println!("Starting local affinity example...");

    // 创建集群配置
    let config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .discovery(DiscoveryMethod::Static { seed_nodes: vec![] })
        .bind_addr("127.0.0.1:8000".parse().unwrap())
        .build()
        .expect("Failed to build config");

    // 创建并启动集群系统
    let mut cluster = ClusterSystem::new(config);
    let system_addr = cluster.start().await.unwrap();

    println!("Cluster system started");

    // 创建一个特定分组的主Actor
    let group_id = "service-group-1".to_string();
    let primary = PrimaryActor { group_id: group_id.clone() };
    let primary_addr = primary.start_distributed();
    let primary_path = "/user/primary";

    println!("Started primary actor at path: {}", primary_path);

    // 创建同一分组的次要Actor（会被部署在同一节点上）
    let secondary = SecondaryActor {
        group_id: group_id.clone(),
        primary_path: primary_path.to_string(),
    };
    let secondary_addr = secondary.start_distributed();

    println!("Started secondary actor at path: /user/secondary");

    // 发送消息测试
    println!("Sending message to primary actor...");
    let primary_result = primary_addr.send(Ping("Hello from main".to_string())).await;
    println!("Primary response: {:?}", primary_result);

    println!("Sending message to secondary actor...");
    let secondary_result = secondary_addr.send(Ping("Hello from main".to_string())).await;
    println!("Secondary response: {:?}", secondary_result);

    // 等待一段时间，确保消息能够处理
    actix_rt::time::sleep(Duration::from_secs(2)).await;

    println!("Shutting down...");

    // 关闭系统
    System::current().stop();

    Ok(())
}