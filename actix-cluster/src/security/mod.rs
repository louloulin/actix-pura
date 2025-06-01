//! 集群安全模块

use std::path::PathBuf;
use std::time::Duration;
use std::collections::HashSet;

use crate::node::NodeId;
use crate::transport::TransportMessage;
use crate::error::{ClusterError, ClusterResult};

/// 安全模式
#[derive(Debug, Clone)]
pub enum SecurityMode {
    /// 无安全模式
    None,
    
    /// 基础TLS
    Tls(TlsConfig),
    
    /// 双向TLS（客户端也需验证）
    MutualTls(MutualTlsConfig),
    
    /// JWT令牌认证
    Jwt(JwtConfig),
}

impl Default for SecurityMode {
    fn default() -> Self {
        Self::None
    }
}

/// TLS配置
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// 证书路径
    pub cert_path: PathBuf,
    
    /// 私钥路径
    pub key_path: PathBuf,
}

/// 双向TLS配置
#[derive(Debug, Clone)]
pub struct MutualTlsConfig {
    /// 证书路径
    pub cert_path: PathBuf,
    
    /// 私钥路径
    pub key_path: PathBuf,
    
    /// CA证书路径
    pub ca_path: PathBuf,
}

/// JWT算法
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JwtAlgorithm {
    /// HMAC SHA-256
    HS256,
    
    /// RSA SHA-256
    RS256,
    
    /// ECDSA SHA-256
    ES256,
}

impl Default for JwtAlgorithm {
    fn default() -> Self {
        Self::HS256
    }
}

/// JWT配置
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// 密钥
    pub secret_key: String,
    
    /// 算法
    pub algorithm: JwtAlgorithm,
    
    /// 过期时间
    pub expiry: Duration,
}

/// 身份验证处理器
pub struct AuthenticationHandler {
    /// 安全模式
    mode: SecurityMode,
    
    /// 授权节点列表
    authorized_nodes: HashSet<NodeId>,
}

impl AuthenticationHandler {
    /// 创建新的身份验证处理器
    pub fn new(mode: SecurityMode) -> Self {
        Self {
            mode,
            authorized_nodes: HashSet::new(),
        }
    }
    
    /// 添加授权节点
    pub fn add_authorized_node(&mut self, node_id: NodeId) {
        self.authorized_nodes.insert(node_id);
    }
    
    /// 移除授权节点
    pub fn remove_authorized_node(&mut self, node_id: &NodeId) {
        self.authorized_nodes.remove(node_id);
    }
    
    /// 检查节点是否授权
    pub fn is_node_authorized(&self, node_id: &NodeId) -> bool {
        self.authorized_nodes.contains(node_id)
    }
    
    /// 验证连接
    pub async fn authenticate_connection(&self, handshake: &TransportMessage) -> ClusterResult<bool> {
        match &self.mode {
            SecurityMode::None => Ok(true),
            SecurityMode::Tls(_) => {
                // 基本TLS验证已经由连接层处理
                Ok(true)
            }
            SecurityMode::MutualTls(_) => {
                // TODO: 在实际实现中需要检查客户端证书
                // 这里简化处理，仅检查节点是否在授权列表中
                if let TransportMessage::Handshake(node_info) = handshake {
                    Ok(self.is_node_authorized(&node_info.id))
                } else {
                    Err(ClusterError::AuthenticationError("期望握手消息".to_string()))
                }
            }
            SecurityMode::Jwt(_) => {
                // TODO: 在实际实现中需要验证JWT令牌
                // 这里简化处理，仅检查节点是否在授权列表中
                if let TransportMessage::Handshake(node_info) = handshake {
                    Ok(self.is_node_authorized(&node_info.id))
                } else {
                    Err(ClusterError::AuthenticationError("期望握手消息".to_string()))
                }
            }
        }
    }
    
    /// 生成授权令牌（用于JWT模式）
    pub fn generate_token(&self, node_id: &NodeId) -> ClusterResult<String> {
        match &self.mode {
            SecurityMode::Jwt(config) => {
                // TODO: 实际实现应该生成真正的JWT令牌
                // 这里简化返回一个占位符
                Ok(format!("jwt-token-for-{}-using-{:?}", node_id, config.algorithm))
            }
            _ => Err(ClusterError::InvalidOperation("当前安全模式不支持生成令牌".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::node::{NodeId, NodeInfo};
    use crate::transport::TransportMessage;
    use crate::config::NodeRole;
    use std::net::SocketAddr;
    
    #[test]
    fn test_security_mode_default() {
        let mode = SecurityMode::default();
        assert!(matches!(mode, SecurityMode::None));
    }
    
    #[test]
    fn test_jwt_algorithm_default() {
        let algo = JwtAlgorithm::default();
        assert_eq!(algo, JwtAlgorithm::HS256);
    }
    
    #[test]
    fn test_authentication_handler_authorized_nodes() {
        let mode = SecurityMode::None;
        let mut handler = AuthenticationHandler::new(mode);
        
        let node1 = NodeId::new();
        let node2 = NodeId::new();
        
        // 初始状态，节点未授权
        assert!(!handler.is_node_authorized(&node1));
        assert!(!handler.is_node_authorized(&node2));
        
        // 添加授权节点
        handler.add_authorized_node(node1.clone());
        assert!(handler.is_node_authorized(&node1));
        assert!(!handler.is_node_authorized(&node2));
        
        // 移除授权节点
        handler.remove_authorized_node(&node1);
        assert!(!handler.is_node_authorized(&node1));
    }
    
    #[tokio::test]
    async fn test_authenticate_connection() {
        // 测试None模式
        let handler = AuthenticationHandler::new(SecurityMode::None);
        let node_id = NodeId::new();
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let node_info = NodeInfo::new(node_id.clone(), "test-node".to_string(), NodeRole::Peer, addr);
        let handshake = TransportMessage::Handshake(node_info);
        
        let result = handler.authenticate_connection(&handshake).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // 测试JWT模式
        let jwt_config = JwtConfig {
            secret_key: "test-secret".to_string(),
            algorithm: JwtAlgorithm::HS256,
            expiry: Duration::from_secs(3600),
        };
        let mut handler = AuthenticationHandler::new(SecurityMode::Jwt(jwt_config));
        
        // 创建一个新的NodeInfo实例
        let node_info2 = NodeInfo::new(node_id.clone(), "test-node".to_string(), NodeRole::Peer, addr);
        let handshake2 = TransportMessage::Handshake(node_info2);
        
        // 未授权时应该返回false
        let result = handler.authenticate_connection(&handshake2).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
        
        // 授权后应该返回true
        handler.add_authorized_node(node_id.clone());
        
        // 创建另一个新的NodeInfo实例
        let node_info3 = NodeInfo::new(node_id.clone(), "test-node".to_string(), NodeRole::Peer, addr);
        let handshake3 = TransportMessage::Handshake(node_info3);
        
        let result = handler.authenticate_connection(&handshake3).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // 非握手消息应该返回错误
        let node_info4 = NodeInfo::new(node_id.clone(), "test-node".to_string(), NodeRole::Peer, addr);
        let other_message = TransportMessage::Heartbeat(node_info4);
        let result = handler.authenticate_connection(&other_message).await;
        assert!(result.is_err());
    }
    
    #[test]
    fn test_generate_token() {
        let jwt_config = JwtConfig {
            secret_key: "test-secret".to_string(),
            algorithm: JwtAlgorithm::HS256,
            expiry: Duration::from_secs(3600),
        };
        let handler = AuthenticationHandler::new(SecurityMode::Jwt(jwt_config));
        let node_id = NodeId::new();
        
        let token_result = handler.generate_token(&node_id);
        assert!(token_result.is_ok());
        let token = token_result.unwrap();
        assert!(token.contains(&node_id.to_string()));
        assert!(token.contains("HS256"));
        
        // 不支持的模式
        let handler = AuthenticationHandler::new(SecurityMode::None);
        let token_result = handler.generate_token(&node_id);
        assert!(token_result.is_err());
    }
} 