//! YAML 工作流解析器
//!
//! 提供从 YAML 文件加载和解析工作流定义的功能。

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_yaml::Value as YamlValue;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use dataflare_core::error::{DataFlareError, Result};
use crate::workflow::{
    Workflow, SourceConfig, TransformationConfig, DestinationConfig,
    ScheduleConfig, ScheduleType,
};

/// YAML 工作流解析器
pub struct YamlWorkflowParser;

impl YamlWorkflowParser {
    /// 从 YAML 文件加载工作流
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Workflow> {
        // 打开文件
        let mut file = File::open(path.as_ref())
            .map_err(|e| DataFlareError::Io(e))?;

        // 读取文件内容
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| DataFlareError::Io(e))?;

        // 解析 YAML
        Self::parse_yaml(&content)
    }

    /// 从 YAML 字符串解析工作流
    pub fn parse_yaml(yaml: &str) -> Result<Workflow> {
        // 处理环境变量
        let yaml_with_env = Self::process_env_vars(yaml)?;

        // 首先尝试直接解析为 Workflow
        if let Ok(workflow) = Workflow::from_yaml(&yaml_with_env) {
            return Ok(workflow);
        }

        // 如果直接解析失败，尝试解析为 YamlWorkflowDefinition
        let definition: YamlWorkflowDefinition = serde_yaml::from_str(&yaml_with_env)
            .map_err(|e| DataFlareError::Serialization(format!("解析 YAML 工作流定义失败: {}", e)))?;

        // 转换为 Workflow
        Self::convert_definition_to_workflow(definition)
    }

    /// 处理 YAML 中的环境变量引用
    fn process_env_vars(yaml: &str) -> Result<String> {
        let mut result = String::with_capacity(yaml.len());
        let mut chars = yaml.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' && chars.peek() == Some(&'{') {
                chars.next(); // 跳过 '{'

                let mut var_name = String::new();
                let mut default_value = None;

                // 读取变量名和默认值
                while let Some(c) = chars.next() {
                    if c == '}' {
                        break;
                    } else if c == ':' && chars.peek() == Some(&'-') {
                        chars.next(); // 跳过 '-'
                        default_value = Some(String::new());
                    } else if let Some(default) = default_value.as_mut() {
                        default.push(c);
                    } else {
                        var_name.push(c);
                    }
                }

                // 获取环境变量值
                let env_value = match env::var(&var_name) {
                    Ok(value) => value,
                    Err(_) => {
                        match default_value {
                            Some(default) => default,
                            None => return Err(DataFlareError::Config(
                                format!("环境变量 '{}' 未定义且没有默认值", var_name)
                            )),
                        }
                    }
                };

                result.push_str(&env_value);
            } else {
                result.push(c);
            }
        }

        Ok(result)
    }

    /// 将 YamlWorkflowDefinition 转换为 Workflow
    fn convert_definition_to_workflow(definition: YamlWorkflowDefinition) -> Result<Workflow> {
        // 创建基本工作流
        let mut workflow = Workflow::new(
            definition.id.unwrap_or_else(|| format!("workflow-{}", Uuid::new_v4())),
            definition.name.unwrap_or_else(|| "Unnamed Workflow".to_string()),
        );

        // 设置描述和版本
        if let Some(description) = definition.description {
            workflow = workflow.with_description(description);
        }

        if let Some(version) = definition.version {
            workflow = workflow.with_version(version);
        }

        // 添加源
        if let Some(sources) = definition.sources {
            for (id, source_def) in sources {
                let config = Self::convert_to_json_value(source_def.config)?;
                let mut source_config = SourceConfig::new(source_def.r#type, config);

                if let Some(mode) = source_def.mode {
                    source_config = source_config.with_mode(mode);
                }

                workflow.add_source(id, source_config);
            }
        }

        // 添加转换
        if let Some(transformations) = definition.transformations {
            for (id, transform_def) in transformations {
                let config = Self::convert_to_json_value(transform_def.config)?;
                let mut transform_config = TransformationConfig::new(transform_def.r#type, config);

                if let Some(inputs) = transform_def.inputs {
                    for input in inputs {
                        transform_config.add_input(input);
                    }
                }

                workflow.add_transformation(id, transform_config);
            }
        }

        // 添加目标
        if let Some(destinations) = definition.destinations {
            for (id, dest_def) in destinations {
                let config = Self::convert_to_json_value(dest_def.config)?;
                let mut dest_config = DestinationConfig::new(dest_def.r#type, config);

                if let Some(inputs) = dest_def.inputs {
                    for input in inputs {
                        dest_config.add_input(input);
                    }
                }

                workflow.add_destination(id, dest_config);
            }
        }

        // 添加调度
        if let Some(schedule_def) = definition.schedule {
            let schedule_type = match schedule_def.r#type.as_str() {
                "cron" => ScheduleType::Cron,
                "interval" => ScheduleType::Interval,
                "once" => ScheduleType::Once,
                "event" => ScheduleType::Event,
                _ => return Err(DataFlareError::Config(format!("无效的调度类型: {}", schedule_def.r#type))),
            };

            let mut schedule = ScheduleConfig::new(schedule_type, schedule_def.expression);

            if let Some(timezone) = schedule_def.timezone {
                schedule = schedule.with_timezone(timezone);
            }

            if let Some(start_date) = schedule_def.start_date {
                schedule = schedule.with_start_date(start_date);
            }

            if let Some(end_date) = schedule_def.end_date {
                schedule = schedule.with_end_date(end_date);
            }

            workflow = workflow.with_schedule(schedule);
        }

        // 添加元数据
        if let Some(metadata) = definition.metadata {
            for (key, value) in metadata {
                workflow.add_metadata(key, value);
            }
        }

        // 验证工作流
        workflow.validate()?;

        Ok(workflow)
    }

    /// 将 YAML 值转换为 JSON 值
    fn convert_to_json_value(yaml_value: Option<YamlValue>) -> Result<serde_json::Value> {
        match yaml_value {
            Some(value) => {
                serde_json::to_value(value)
                    .map_err(|e| DataFlareError::Serialization(format!("转换 YAML 值到 JSON 失败: {}", e)))
            },
            None => Ok(serde_json::Value::Null),
        }
    }
}

/// YAML 工作流定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlWorkflowDefinition {
    /// 工作流 ID
    pub id: Option<String>,

    /// 工作流名称
    pub name: Option<String>,

    /// 工作流描述
    pub description: Option<String>,

    /// 工作流版本
    pub version: Option<String>,

    /// 源配置
    pub sources: Option<HashMap<String, YamlSourceDefinition>>,

    /// 转换配置
    pub transformations: Option<HashMap<String, YamlTransformationDefinition>>,

    /// 目标配置
    pub destinations: Option<HashMap<String, YamlDestinationDefinition>>,

    /// 调度配置
    pub schedule: Option<YamlScheduleDefinition>,

    /// 元数据
    pub metadata: Option<HashMap<String, String>>,
}

/// YAML 源定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlSourceDefinition {
    /// 源类型
    pub r#type: String,

    /// 提取模式
    pub mode: Option<String>,

    /// 源配置
    pub config: Option<YamlValue>,
}

/// YAML 转换定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlTransformationDefinition {
    /// 输入
    pub inputs: Option<Vec<String>>,

    /// 转换类型
    pub r#type: String,

    /// 转换配置
    pub config: Option<YamlValue>,
}

/// YAML 目标定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlDestinationDefinition {
    /// 输入
    pub inputs: Option<Vec<String>>,

    /// 目标类型
    pub r#type: String,

    /// 目标配置
    pub config: Option<YamlValue>,
}

/// YAML 调度定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlScheduleDefinition {
    /// 调度类型
    pub r#type: String,

    /// 调度表达式
    pub expression: String,

    /// 时区
    pub timezone: Option<String>,

    /// 开始日期
    pub start_date: Option<DateTime<Utc>>,

    /// 结束日期
    pub end_date: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::WorkflowParser;

    #[test]
    fn test_parse_yaml_workflow() {
        let yaml = r#"
id: test-workflow
name: Test Workflow
description: A test workflow
version: 1.0.0
sources:
  users:
    type: postgres
    mode: incremental
    config:
      host: localhost
      port: 5432
      database: test
      table: users
transformations:
  user_transform:
    inputs:
      - users
    type: mapping
    config:
      mappings:
        - source: name
          destination: user.name
destinations:
  es_users:
    inputs:
      - user_transform
    type: elasticsearch
    config:
      host: localhost
      port: 9200
      index: users
schedule:
  type: cron
  expression: 0 0 * * *
  timezone: UTC
metadata:
  owner: test-user
"#;

        // 解析 YAML
        let workflow = YamlWorkflowParser::parse_yaml(yaml).unwrap();

        // 验证工作流
        assert_eq!(workflow.id, "test-workflow");
        assert_eq!(workflow.name, "Test Workflow");
        assert_eq!(workflow.description, Some("A test workflow".to_string()));
        assert_eq!(workflow.version, "1.0.0");

        // 验证源
        assert_eq!(workflow.sources.len(), 1);
        assert!(workflow.sources.contains_key("users"));
        let source = &workflow.sources["users"];
        assert_eq!(source.r#type, "postgres");
        assert_eq!(source.mode, Some("incremental".to_string()));

        // 验证转换
        assert_eq!(workflow.transformations.len(), 1);
        assert!(workflow.transformations.contains_key("user_transform"));
        let transform = &workflow.transformations["user_transform"];
        assert_eq!(transform.r#type, "mapping");
        assert_eq!(transform.inputs.len(), 1);
        assert_eq!(transform.inputs[0], "users");

        // 验证目标
        assert_eq!(workflow.destinations.len(), 1);
        assert!(workflow.destinations.contains_key("es_users"));
        let dest = &workflow.destinations["es_users"];
        assert_eq!(dest.r#type, "elasticsearch");
        assert_eq!(dest.inputs.len(), 1);
        assert_eq!(dest.inputs[0], "user_transform");

        // 验证调度
        assert!(workflow.schedule.is_some());
        let schedule = workflow.schedule.as_ref().unwrap();
        assert_eq!(schedule.r#type, ScheduleType::Cron);
        assert_eq!(schedule.expression, "0 0 * * *");
        assert_eq!(schedule.timezone, Some("UTC".to_string()));

        // 验证元数据
        assert_eq!(workflow.metadata.len(), 1);
        assert_eq!(workflow.metadata.get("owner"), Some(&"test-user".to_string()));

        // 验证工作流图
        let mut parser = WorkflowParser::new(workflow);
        parser.parse().unwrap();
        assert!(!parser.has_cycles());
        assert!(parser.all_components_reachable());
    }

    #[test]
    fn test_parse_invalid_yaml_workflow() {
        // 缺少必要的目标
        let yaml = r#"
id: test-workflow
name: Test Workflow
sources:
  users:
    type: postgres
    config:
      host: localhost
"#;

        let result = YamlWorkflowParser::parse_yaml(yaml);
        assert!(result.is_err());

        // 无效的转换输入
        let yaml = r#"
id: test-workflow
name: Test Workflow
sources:
  users:
    type: postgres
    config:
      host: localhost
transformations:
  user_transform:
    inputs:
      - invalid_source
    type: mapping
    config: {}
destinations:
  es_users:
    inputs:
      - user_transform
    type: elasticsearch
    config: {}
"#;

        let result = YamlWorkflowParser::parse_yaml(yaml);
        assert!(result.is_err());
    }
}
