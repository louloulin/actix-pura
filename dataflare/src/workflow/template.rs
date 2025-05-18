//! 工作流模板
//!
//! 提供工作流模板功能，支持创建和使用可重用的工作流模板。

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml::Value as YamlValue;

use crate::{
    error::{DataFlareError, Result},
    workflow::{Workflow, YamlWorkflowParser},
};

/// 工作流模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    /// 模板 ID
    pub id: String,
    
    /// 模板名称
    pub name: String,
    
    /// 模板描述
    pub description: Option<String>,
    
    /// 模板版本
    pub version: String,
    
    /// 模板参数
    pub parameters: HashMap<String, TemplateParameter>,
    
    /// 模板内容
    pub template: String,
}

/// 模板参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParameter {
    /// 参数名称
    pub name: String,
    
    /// 参数描述
    pub description: Option<String>,
    
    /// 参数类型
    pub r#type: ParameterType,
    
    /// 是否必需
    pub required: bool,
    
    /// 默认值
    pub default: Option<YamlValue>,
    
    /// 可选值（用于枚举类型）
    pub options: Option<Vec<YamlValue>>,
}

/// 参数类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    /// 字符串
    String,
    
    /// 数字
    Number,
    
    /// 布尔值
    Boolean,
    
    /// 对象
    Object,
    
    /// 数组
    Array,
    
    /// 枚举
    Enum,
}

/// 模板参数值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParameterValues {
    /// 参数值
    pub values: HashMap<String, YamlValue>,
}

/// 工作流模板管理器
pub struct WorkflowTemplateManager {
    /// 模板目录
    template_dir: PathBuf,
    
    /// 模板缓存
    templates: HashMap<String, WorkflowTemplate>,
}

impl WorkflowTemplateManager {
    /// 创建新的工作流模板管理器
    pub fn new<P: AsRef<Path>>(template_dir: P) -> Result<Self> {
        let template_dir = template_dir.as_ref().to_path_buf();
        
        // 确保模板目录存在
        if !template_dir.exists() {
            fs::create_dir_all(&template_dir)
                .map_err(|e| DataFlareError::Io(e))?;
        }
        
        Ok(Self {
            template_dir,
            templates: HashMap::new(),
        })
    }
    
    /// 加载所有模板
    pub fn load_templates(&mut self) -> Result<()> {
        self.templates.clear();
        
        // 遍历模板目录
        for entry in fs::read_dir(&self.template_dir)
            .map_err(|e| DataFlareError::Io(e))? {
            let entry = entry.map_err(|e| DataFlareError::Io(e))?;
            let path = entry.path();
            
            // 只处理 YAML 文件
            if path.is_file() && path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
                // 加载模板
                let template = self.load_template_from_file(&path)?;
                self.templates.insert(template.id.clone(), template);
            }
        }
        
        Ok(())
    }
    
    /// 从文件加载模板
    pub fn load_template_from_file<P: AsRef<Path>>(&self, path: P) -> Result<WorkflowTemplate> {
        // 打开文件
        let mut file = File::open(path.as_ref())
            .map_err(|e| DataFlareError::Io(e))?;
        
        // 读取文件内容
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| DataFlareError::Io(e))?;
        
        // 解析模板
        let template: WorkflowTemplate = serde_yaml::from_str(&content)
            .map_err(|e| DataFlareError::Serialization(format!("解析工作流模板失败: {}", e)))?;
        
        Ok(template)
    }
    
    /// 保存模板
    pub fn save_template(&self, template: &WorkflowTemplate) -> Result<()> {
        // 构建模板文件路径
        let file_path = self.template_dir.join(format!("{}.yaml", template.id));
        
        // 序列化模板
        let yaml = serde_yaml::to_string(template)
            .map_err(|e| DataFlareError::Serialization(format!("序列化工作流模板失败: {}", e)))?;
        
        // 写入文件
        let mut file = File::create(file_path)
            .map_err(|e| DataFlareError::Io(e))?;
        file.write_all(yaml.as_bytes())
            .map_err(|e| DataFlareError::Io(e))?;
        
        Ok(())
    }
    
    /// 获取模板
    pub fn get_template(&self, id: &str) -> Option<&WorkflowTemplate> {
        self.templates.get(id)
    }
    
    /// 获取所有模板
    pub fn get_templates(&self) -> &HashMap<String, WorkflowTemplate> {
        &self.templates
    }
    
    /// 应用模板
    pub fn apply_template(&self, template_id: &str, params: &TemplateParameterValues) -> Result<Workflow> {
        // 获取模板
        let template = self.templates.get(template_id)
            .ok_or_else(|| DataFlareError::Config(format!("模板 '{}' 不存在", template_id)))?;
        
        // 验证参数
        self.validate_parameters(template, params)?;
        
        // 替换参数
        let yaml = self.replace_parameters(template, params)?;
        
        // 解析工作流
        YamlWorkflowParser::parse_yaml(&yaml)
    }
    
    /// 验证参数
    fn validate_parameters(&self, template: &WorkflowTemplate, params: &TemplateParameterValues) -> Result<()> {
        // 检查必需参数
        for (param_name, param) in &template.parameters {
            if param.required && !params.values.contains_key(param_name) && param.default.is_none() {
                return Err(DataFlareError::Config(
                    format!("缺少必需参数 '{}'", param_name)
                ));
            }
        }
        
        // 检查未知参数
        for param_name in params.values.keys() {
            if !template.parameters.contains_key(param_name) {
                return Err(DataFlareError::Config(
                    format!("未知参数 '{}'", param_name)
                ));
            }
        }
        
        // 检查枚举参数
        for (param_name, param_value) in &params.values {
            if let Some(param) = template.parameters.get(param_name) {
                if param.r#type == ParameterType::Enum {
                    if let Some(options) = &param.options {
                        if !options.contains(param_value) {
                            return Err(DataFlareError::Config(
                                format!("参数 '{}' 的值 '{:?}' 不在允许的选项中", param_name, param_value)
                            ));
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 替换参数
    fn replace_parameters(&self, template: &WorkflowTemplate, params: &TemplateParameterValues) -> Result<String> {
        let mut result = template.template.clone();
        
        // 替换参数
        for (param_name, param) in &template.parameters {
            let param_value = if let Some(value) = params.values.get(param_name) {
                value.clone()
            } else if let Some(default) = &param.default {
                default.clone()
            } else {
                continue;
            };
            
            // 将参数值序列化为 YAML
            let param_yaml = serde_yaml::to_string(&param_value)
                .map_err(|e| DataFlareError::Serialization(format!("序列化参数值失败: {}", e)))?;
            
            // 去除 YAML 文档分隔符和尾部换行符
            let param_yaml = param_yaml.trim_start_matches("---\n").trim_end();
            
            // 替换参数
            let placeholder = format!("{{{{ {} }}}}", param_name);
            result = result.replace(&placeholder, param_yaml);
        }
        
        Ok(result)
    }
}
