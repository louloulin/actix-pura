//! 用户认证和授权系统

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use sha2::{Sha256, Digest};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use crate::{WasmError, WasmResult};

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// 用户ID
    pub id: String,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
    /// 显示名称
    pub display_name: String,
    /// 头像URL
    pub avatar_url: Option<String>,
    /// 用户角色
    pub role: UserRole,
    /// 账户状态
    pub status: AccountStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后登录时间
    pub last_login: Option<DateTime<Utc>>,
    /// 邮箱验证状态
    pub email_verified: bool,
    /// 两步验证启用状态
    pub two_factor_enabled: bool,
}

/// 用户角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserRole {
    /// 普通用户
    User,
    /// 开发者
    Developer,
    /// 管理员
    Admin,
    /// 超级管理员
    SuperAdmin,
}

/// 账户状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccountStatus {
    /// 活跃
    Active,
    /// 暂停
    Suspended,
    /// 已删除
    Deleted,
    /// 待验证
    PendingVerification,
}

/// API密钥
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// 密钥ID
    pub id: String,
    /// 密钥名称
    pub name: String,
    /// 密钥值（哈希后）
    pub key_hash: String,
    /// 用户ID
    pub user_id: String,
    /// 权限范围
    pub scopes: Vec<Permission>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: Option<DateTime<Utc>>,
    /// 最后使用时间
    pub last_used: Option<DateTime<Utc>>,
    /// 是否启用
    pub enabled: bool,
}

/// 权限定义
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    /// 读取插件
    ReadPlugins,
    /// 发布插件
    PublishPlugins,
    /// 更新插件
    UpdatePlugins,
    /// 删除插件
    DeletePlugins,
    /// 管理用户
    ManageUsers,
    /// 管理系统
    ManageSystem,
    /// 查看统计
    ViewStats,
    /// 管理评论
    ManageReviews,
}

/// JWT声明
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// 用户ID
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 用户角色
    pub role: UserRole,
    /// 权限列表
    pub permissions: Vec<Permission>,
    /// 签发时间
    pub iat: i64,
    /// 过期时间
    pub exp: i64,
}

/// 认证服务
pub struct AuthService {
    /// JWT编码密钥
    encoding_key: EncodingKey,
    /// JWT解码密钥
    decoding_key: DecodingKey,
    /// 用户存储
    users: HashMap<String, User>,
    /// API密钥存储
    api_keys: HashMap<String, ApiKey>,
    /// 会话存储
    sessions: HashMap<String, Session>,
}

/// 会话信息
#[derive(Debug, Clone)]
pub struct Session {
    /// 会话ID
    pub id: String,
    /// 用户ID
    pub user_id: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
    /// IP地址
    pub ip_address: String,
    /// 用户代理
    pub user_agent: String,
}

impl AuthService {
    /// 创建新的认证服务
    pub fn new(jwt_secret: &str) -> Self {
        let encoding_key = EncodingKey::from_secret(jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());

        Self {
            encoding_key,
            decoding_key,
            users: HashMap::new(),
            api_keys: HashMap::new(),
            sessions: HashMap::new(),
        }
    }

    /// 注册新用户
    pub async fn register_user(
        &mut self,
        username: String,
        email: String,
        password: String,
        display_name: String,
    ) -> WasmResult<User> {
        // 检查用户名是否已存在
        if self.users.values().any(|u| u.username == username) {
            return Err(WasmError::plugin_execution("用户名已存在".to_string()));
        }

        // 检查邮箱是否已存在
        if self.users.values().any(|u| u.email == email) {
            return Err(WasmError::plugin_execution("邮箱已存在".to_string()));
        }

        // 创建用户
        let user = User {
            id: Uuid::new_v4().to_string(),
            username,
            email,
            display_name,
            avatar_url: None,
            role: UserRole::User,
            status: AccountStatus::PendingVerification,
            created_at: Utc::now(),
            last_login: None,
            email_verified: false,
            two_factor_enabled: false,
        };

        // 存储用户（实际应用中应该存储密码哈希）
        self.users.insert(user.id.clone(), user.clone());

        Ok(user)
    }

    /// 用户登录
    pub async fn login(
        &mut self,
        username: String,
        password: String,
        ip_address: String,
        user_agent: String,
    ) -> WasmResult<(String, User)> {
        // 查找用户
        let user_id = {
            let user = self.users.values_mut()
                .find(|u| u.username == username)
                .ok_or_else(|| WasmError::plugin_execution("用户名或密码错误".to_string()))?;

            // 验证密码（简化实现）
            // 实际应用中应该验证密码哈希

            // 检查账户状态
            if user.status != AccountStatus::Active {
                return Err(WasmError::plugin_execution("账户已被暂停或删除".to_string()));
            }

            // 更新最后登录时间
            user.last_login = Some(Utc::now());
            user.id.clone()
        };

        // 获取用户信息用于生成令牌
        let user = self.users.get(&user_id).unwrap().clone();

        // 生成JWT令牌
        let token = self.generate_jwt_token(&user)?;

        // 创建会话
        let session = Session {
            id: Uuid::new_v4().to_string(),
            user_id: user.id.clone(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            ip_address,
            user_agent,
        };

        self.sessions.insert(session.id.clone(), session);

        Ok((token, user.clone()))
    }

    /// 生成JWT令牌
    pub fn generate_jwt_token(&self, user: &User) -> WasmResult<String> {
        let permissions = self.get_user_permissions(&user.role);

        let claims = Claims {
            sub: user.id.clone(),
            username: user.username.clone(),
            role: user.role.clone(),
            permissions,
            iat: Utc::now().timestamp(),
            exp: (Utc::now() + Duration::hours(24)).timestamp(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| WasmError::plugin_execution(format!("JWT生成失败: {}", e)))
    }

    /// 验证JWT令牌
    pub fn verify_jwt_token(&self, token: &str) -> WasmResult<Claims> {
        let validation = Validation::new(Algorithm::HS256);

        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| WasmError::plugin_execution(format!("JWT验证失败: {}", e)))
    }

    /// 创建API密钥
    pub async fn create_api_key(
        &mut self,
        user_id: String,
        name: String,
        scopes: Vec<Permission>,
        expires_at: Option<DateTime<Utc>>,
    ) -> WasmResult<(String, ApiKey)> {
        // 生成API密钥
        let key_value = format!("df_{}", Uuid::new_v4().to_string().replace("-", ""));
        let key_hash = self.hash_api_key(&key_value);

        let api_key = ApiKey {
            id: Uuid::new_v4().to_string(),
            name,
            key_hash,
            user_id,
            scopes,
            created_at: Utc::now(),
            expires_at,
            last_used: None,
            enabled: true,
        };

        self.api_keys.insert(api_key.id.clone(), api_key.clone());

        Ok((key_value, api_key))
    }

    /// 验证API密钥
    pub async fn verify_api_key(&mut self, key: &str) -> WasmResult<(User, Vec<Permission>)> {
        let key_hash = self.hash_api_key(key);

        // 查找API密钥
        let api_key = self.api_keys.values_mut()
            .find(|k| k.key_hash == key_hash && k.enabled)
            .ok_or_else(|| WasmError::plugin_execution("无效的API密钥".to_string()))?;

        // 检查过期时间
        if let Some(expires_at) = api_key.expires_at {
            if Utc::now() > expires_at {
                return Err(WasmError::plugin_execution("API密钥已过期".to_string()));
            }
        }

        // 更新最后使用时间
        api_key.last_used = Some(Utc::now());

        // 获取用户信息
        let user = self.users.get(&api_key.user_id)
            .ok_or_else(|| WasmError::plugin_execution("用户不存在".to_string()))?
            .clone();

        Ok((user, api_key.scopes.clone()))
    }

    /// 获取用户权限
    fn get_user_permissions(&self, role: &UserRole) -> Vec<Permission> {
        match role {
            UserRole::User => vec![
                Permission::ReadPlugins,
            ],
            UserRole::Developer => vec![
                Permission::ReadPlugins,
                Permission::PublishPlugins,
                Permission::UpdatePlugins,
                Permission::ViewStats,
            ],
            UserRole::Admin => vec![
                Permission::ReadPlugins,
                Permission::PublishPlugins,
                Permission::UpdatePlugins,
                Permission::DeletePlugins,
                Permission::ManageReviews,
                Permission::ViewStats,
            ],
            UserRole::SuperAdmin => vec![
                Permission::ReadPlugins,
                Permission::PublishPlugins,
                Permission::UpdatePlugins,
                Permission::DeletePlugins,
                Permission::ManageUsers,
                Permission::ManageSystem,
                Permission::ManageReviews,
                Permission::ViewStats,
            ],
        }
    }

    /// 哈希API密钥
    fn hash_api_key(&self, key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 检查权限
    pub fn check_permission(&self, user_permissions: &[Permission], required: Permission) -> bool {
        user_permissions.contains(&required)
    }
}
