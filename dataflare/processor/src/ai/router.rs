/// AI路由处理器
/// 
/// 基于AI分析结果和业务规则进行智能路由，支持：
/// - 基于内容的智能路由
/// - 多条件规则评估
/// - 优先级路由
/// - 路由决策追踪

use super::{RoutingRule, RoutingDecision, utils};
use dataflare_core::processor::{Processor, ProcessorState};
use dataflare_core::message::{DataRecord, DataRecordBatch};
use dataflare_core::error::{DataFlareError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use async_trait::async_trait;
use std::collections::HashMap;
use log::{info, debug, warn, error};

/// AI路由处理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIRouterConfig {
    /// 路由规则列表
    pub routing_rules: Vec<RoutingRule>,
    /// 输出字段名 (存储路由决策)
    pub output_field: String,
    /// 是否包含路由决策原因
    pub include_reasoning: Option<bool>,
    /// 默认路由 (当没有规则匹配时)
    pub default_route: Option<String>,
    /// 是否启用调试模式
    pub debug: Option<bool>,
}

/// AI路由处理器
pub struct AIRouterProcessor {
    /// 处理器配置
    config: AIRouterConfig,
    /// 路由统计
    route_stats: HashMap<String, u64>,
    /// 处理器状态
    state: ProcessorState,
}

impl std::fmt::Debug for AIRouterProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIRouterProcessor")
            .field("config", &self.config)
            .field("route_stats", &self.route_stats)
            .field("state", &self.state)
            .finish()
    }
}

impl AIRouterProcessor {
    /// 创建新的AI路由处理器
    pub fn new() -> Self {
        Self {
            config: AIRouterConfig {
                routing_rules: Vec::new(),
                output_field: "route".to_string(),
                include_reasoning: Some(true),
                default_route: Some("default".to_string()),
                debug: Some(false),
            },
            route_stats: HashMap::new(),
            state: ProcessorState::new("ai_router"),
        }
    }

    /// 从JSON配置创建处理器
    pub fn from_json(config: Value) -> Result<Self> {
        let mut processor = Self::new();
        processor.configure(&config)?;
        Ok(processor)
    }

    /// 评估条件表达式
    fn evaluate_condition(&self, condition: &str, record: &DataRecord) -> Result<bool> {
        // 简化的条件评估器
        if condition == "true" {
            return Ok(true);
        }
        
        if condition == "false" {
            return Ok(false);
        }

        // 解析条件表达式
        let result = self.parse_and_evaluate_condition(condition, record)?;
        
        if self.config.debug.unwrap_or(false) {
            debug!("条件评估: '{}' -> {}", condition, result);
        }
        
        Ok(result)
    }

    /// 解析和评估条件表达式
    fn parse_and_evaluate_condition(&self, condition: &str, record: &DataRecord) -> Result<bool> {
        // 处理逻辑操作符
        if condition.contains(" && ") {
            let parts: Vec<&str> = condition.split(" && ").collect();
            for part in parts {
                if !self.parse_and_evaluate_condition(part.trim(), record)? {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        
        if condition.contains(" || ") {
            let parts: Vec<&str> = condition.split(" || ").collect();
            for part in parts {
                if self.parse_and_evaluate_condition(part.trim(), record)? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }

        // 处理比较操作
        if let Some(result) = self.evaluate_comparison(condition, record)? {
            return Ok(result);
        }

        // 处理字段存在性检查
        if condition.starts_with("exists(") && condition.ends_with(")") {
            let field = &condition[7..condition.len()-1];
            return Ok(record.get_value(field).is_some());
        }

        // 处理数组长度检查
        if condition.contains(".length()") {
            return self.evaluate_array_length_condition(condition, record);
        }

        // 默认返回false
        warn!("无法解析条件表达式: {}", condition);
        Ok(false)
    }

    /// 评估比较操作
    fn evaluate_comparison(&self, condition: &str, record: &DataRecord) -> Result<Option<bool>> {
        let operators = ["==", "!=", ">=", "<=", ">", "<"];
        
        for op in &operators {
            if let Some(pos) = condition.find(op) {
                let left = condition[..pos].trim();
                let right = condition[pos + op.len()..].trim();
                
                let left_value = self.get_field_or_literal_value(left, record)?;
                let right_value = self.get_field_or_literal_value(right, record)?;
                
                return Ok(Some(self.compare_values(&left_value, &right_value, op)?));
            }
        }
        
        Ok(None)
    }

    /// 评估数组长度条件
    fn evaluate_array_length_condition(&self, condition: &str, record: &DataRecord) -> Result<bool> {
        // 解析类似 "similar_docs.length() > 0" 的条件
        if let Some(dot_pos) = condition.find(".length()") {
            let field_name = &condition[..dot_pos];
            let rest = &condition[dot_pos + 9..].trim();
            
            if let Some(field_value) = record.get_value(field_name) {
                if let Value::Array(arr) = field_value {
                    let length = arr.len();
                    
                    // 解析比较操作
                    for op in &[">=", "<=", "==", "!=", ">", "<"] {
                        if let Some(op_pos) = rest.find(op) {
                            let right_str = rest[op_pos + op.len()..].trim();
                            if let Ok(right_num) = right_str.parse::<usize>() {
                                return Ok(match *op {
                                    "==" => length == right_num,
                                    "!=" => length != right_num,
                                    ">" => length > right_num,
                                    "<" => length < right_num,
                                    ">=" => length >= right_num,
                                    "<=" => length <= right_num,
                                    _ => false,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(false)
    }

    /// 获取字段值或字面量值
    fn get_field_or_literal_value(&self, expr: &str, record: &DataRecord) -> Result<Value> {
        let expr = expr.trim();
        
        // 字符串字面量
        if (expr.starts_with('"') && expr.ends_with('"')) || 
           (expr.starts_with('\'') && expr.ends_with('\'')) {
            return Ok(Value::String(expr[1..expr.len()-1].to_string()));
        }
        
        // 数字字面量
        if let Ok(num) = expr.parse::<f64>() {
            return Ok(Value::Number(
                serde_json::Number::from_f64(num)
                    .unwrap_or_else(|| serde_json::Number::from(0))
            ));
        }
        
        // 布尔字面量
        if expr == "true" {
            return Ok(Value::Bool(true));
        }
        if expr == "false" {
            return Ok(Value::Bool(false));
        }
        
        // 字段引用
        if let Some(value) = record.get_value(expr) {
            return Ok(value.clone());
        }
        
        // 数组索引访问 (如 similar_docs[0].score)
        if expr.contains('[') && expr.contains(']') {
            return self.get_array_indexed_value(expr, record);
        }
        
        Ok(Value::Null)
    }

    /// 获取数组索引值
    fn get_array_indexed_value(&self, expr: &str, record: &DataRecord) -> Result<Value> {
        // 解析类似 "similar_docs[0].score" 的表达式
        if let Some(bracket_start) = expr.find('[') {
            if let Some(bracket_end) = expr.find(']') {
                let array_field = &expr[..bracket_start];
                let index_str = &expr[bracket_start + 1..bracket_end];
                let rest = &expr[bracket_end + 1..];
                
                if let Ok(index) = index_str.parse::<usize>() {
                    if let Some(Value::Array(arr)) = record.get_value(array_field) {
                        if let Some(item) = arr.get(index) {
                            if rest.is_empty() {
                                return Ok(item.clone());
                            } else if rest.starts_with('.') {
                                let sub_field = &rest[1..];
                                if let Value::Object(obj) = item {
                                    if let Some(sub_value) = obj.get(sub_field) {
                                        return Ok(sub_value.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(Value::Null)
    }

    /// 比较两个值
    fn compare_values(&self, left: &Value, right: &Value, op: &str) -> Result<bool> {
        match (left, right) {
            (Value::String(l), Value::String(r)) => {
                Ok(match op {
                    "==" => l == r,
                    "!=" => l != r,
                    ">" => l > r,
                    "<" => l < r,
                    ">=" => l >= r,
                    "<=" => l <= r,
                    _ => false,
                })
            },
            (Value::Number(l), Value::Number(r)) => {
                let l_f64 = l.as_f64().unwrap_or(0.0);
                let r_f64 = r.as_f64().unwrap_or(0.0);
                Ok(match op {
                    "==" => (l_f64 - r_f64).abs() < f64::EPSILON,
                    "!=" => (l_f64 - r_f64).abs() >= f64::EPSILON,
                    ">" => l_f64 > r_f64,
                    "<" => l_f64 < r_f64,
                    ">=" => l_f64 >= r_f64,
                    "<=" => l_f64 <= r_f64,
                    _ => false,
                })
            },
            (Value::Bool(l), Value::Bool(r)) => {
                Ok(match op {
                    "==" => l == r,
                    "!=" => l != r,
                    _ => false,
                })
            },
            _ => Ok(false),
        }
    }

    /// 执行路由决策
    fn make_routing_decision(&mut self, record: &DataRecord) -> Result<RoutingDecision> {
        // 按优先级排序规则
        let mut sorted_rules = self.config.routing_rules.clone();
        sorted_rules.sort_by_key(|rule| rule.priority);

        // 评估路由规则
        for rule in &sorted_rules {
            if self.evaluate_condition(&rule.condition, record)? {
                // 更新路由统计
                *self.route_stats.entry(rule.destination.clone()).or_insert(0) += 1;
                
                let decision = RoutingDecision {
                    route: rule.destination.clone(),
                    matched_rule: Some(rule.name.clone()),
                    reason: Some(format!("匹配规则: {} (条件: {})", rule.name, rule.condition)),
                    priority: Some(rule.priority),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                
                debug!("路由决策: {} -> {} (规则: {})", 
                       rule.condition, rule.destination, rule.name);
                
                return Ok(decision);
            }
        }

        // 使用默认路由
        let default_route = self.config.default_route.clone().unwrap_or_else(|| "default".to_string());
        *self.route_stats.entry(default_route.clone()).or_insert(0) += 1;
        
        Ok(RoutingDecision {
            route: default_route,
            matched_rule: None,
            reason: Some("使用默认路由".to_string()),
            priority: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// 将路由决策转换为JSON值
    fn decision_to_json(&self, decision: &RoutingDecision) -> Value {
        if self.config.include_reasoning.unwrap_or(true) {
            serde_json::to_value(decision).unwrap_or_else(|_| Value::String(decision.route.clone()))
        } else {
            Value::String(decision.route.clone())
        }
    }
}

#[async_trait]
impl Processor for AIRouterProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = serde_json::from_value(config.clone())
            .map_err(|e| DataFlareError::Config(format!("无效的AI路由处理器配置: {}", e)))?;

        // 验证配置
        if self.config.routing_rules.is_empty() {
            return Err(DataFlareError::Config("至少需要配置一个路由规则".to_string()));
        }

        // 验证规则优先级唯一性
        let mut priorities = std::collections::HashSet::new();
        for rule in &self.config.routing_rules {
            if !priorities.insert(rule.priority) {
                warn!("发现重复的规则优先级: {}", rule.priority);
            }
        }

        info!("AI路由处理器配置成功，规则数: {}, 输出字段: {}", 
              self.config.routing_rules.len(), self.config.output_field);
        Ok(())
    }

    async fn initialize(&mut self) -> Result<()> {
        info!("初始化AI路由处理器");
        
        // 初始化路由统计
        self.route_stats.clear();
        
        // 预注册所有可能的路由
        for rule in &self.config.routing_rules {
            self.route_stats.insert(rule.destination.clone(), 0);
        }
        
        if let Some(ref default_route) = self.config.default_route {
            self.route_stats.insert(default_route.clone(), 0);
        }
        
        info!("路由统计初始化完成，注册 {} 个路由", self.route_stats.len());
        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        let mut new_record = record.clone();

        match self.make_routing_decision(record) {
            Ok(decision) => {
                // 设置路由决策结果
                let decision_json = self.decision_to_json(&decision);
                new_record.set_value(&self.config.output_field, decision_json)?;
                
                debug!("记录路由决策: {}", decision.route);
            },
            Err(e) => {
                error!("路由决策失败: {}", e);
                // 使用默认路由
                let default_route = self.config.default_route.clone().unwrap_or_else(|| "error".to_string());
                new_record.set_value(&self.config.output_field, Value::String(default_route))?;
            }
        }

        // 更新处理器状态
        self.state.records_processed += 1;

        Ok(new_record)
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::with_capacity(batch.records.len());

        for record in &batch.records {
            let processed = self.process_record(record).await?;
            processed_records.push(processed);
        }

        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        info!("批量路由决策完成: {} 条记录", batch.records.len());

        Ok(new_batch)
    }

    async fn finalize(&mut self) -> Result<()> {
        info!("AI路由处理器完成，共处理 {} 条记录", self.state.records_processed);
        info!("路由统计: {:?}", self.route_stats);
        Ok(())
    }

    fn get_state(&self) -> ProcessorState {
        self.state.clone()
    }

    fn get_input_schema(&self) -> Option<dataflare_core::Schema> {
        None // TODO: 实现输入模式定义
    }

    fn get_output_schema(&self) -> Option<dataflare_core::Schema> {
        None // TODO: 实现输出模式定义
    }
}

impl Default for AIRouterProcessor {
    fn default() -> Self {
        Self::new()
    }
}
