//! Simple workflow executor for demonstration purposes

use std::path::Path;
use std::fs;
use serde_json::{Value, Map};
use anyhow::{Result, Context};

/// Execute a simple workflow for demonstration
pub fn execute_simple_workflow(workflow_path: &Path) -> Result<()> {
    println!("执行工作流: {}", workflow_path.display());

    // 读取工作流配置
    let workflow_content = fs::read_to_string(workflow_path)
        .context("无法读取工作流文件")?;

    // 简单解析YAML（这里使用简化的方法）
    if workflow_content.contains("simple-wasm-demo") {
        execute_simple_wasm_demo()?;
    } else {
        println!("⚠ 未知的工作流类型，跳过执行");
    }

    Ok(())
}

/// Execute the simple WASM demo workflow
fn execute_simple_wasm_demo() -> Result<()> {
    println!("🔄 执行简单WASM演示工作流...");

    // 步骤1: 读取源数据
    println!("📖 读取源数据: examples/data/sample_users.json");
    let input_path = "examples/data/sample_users.json";

    if !Path::new(input_path).exists() {
        println!("⚠ 源数据文件不存在，创建示例数据...");
        create_sample_data()?;
    }

    let input_data = fs::read_to_string(input_path)
        .context("无法读取输入数据")?;

    let users: Vec<Value> = serde_json::from_str(&input_data)
        .context("无法解析JSON数据")?;

    println!("✓ 成功读取 {} 条用户记录", users.len());

    // 步骤2: 数据转换
    println!("🔄 执行数据转换...");
    let mut transformed_users = Vec::new();

    for user in &users {
        if let Some(transformed) = transform_user_data(&user)? {
            transformed_users.push(transformed);
        }
    }

    println!("✓ 成功转换 {} 条记录", transformed_users.len());

    // 步骤3: 输出结果
    println!("💾 保存转换结果...");
    let output_path = "examples/output/simple_transformed.json";

    // 确保输出目录存在
    if let Some(parent) = Path::new(output_path).parent() {
        fs::create_dir_all(parent)?;
    }

    let output_json = serde_json::to_string_pretty(&transformed_users)
        .context("无法序列化输出数据")?;

    fs::write(output_path, output_json)
        .context("无法写入输出文件")?;

    println!("✓ 结果已保存到: {}", output_path);

    // 显示处理统计
    println!("\n📊 处理统计:");
    println!("  输入记录: {}", users.len());
    println!("  输出记录: {}", transformed_users.len());
    println!("  输出文件: {}", output_path);

    Ok(())
}

/// Transform user data according to the workflow rules
fn transform_user_data(user: &Value) -> Result<Option<Value>> {
    let mut transformed = Map::new();

    // 提取姓名
    if let (Some(first_name), Some(last_name)) = (
        user.pointer("/personal_info/first_name").and_then(|v| v.as_str()),
        user.pointer("/personal_info/last_name").and_then(|v| v.as_str())
    ) {
        transformed.insert("name".to_string(), Value::String(format!("{} {}", first_name, last_name)));
    }

    // 提取位置
    if let (Some(city), Some(country)) = (
        user.pointer("/address/city").and_then(|v| v.as_str()),
        user.pointer("/address/country").and_then(|v| v.as_str())
    ) {
        transformed.insert("location".to_string(), Value::String(format!("{}, {}", city, country)));
    }

    // 计算年薪
    if let Some(monthly_salary) = user.pointer("/profile/salary").and_then(|v| v.as_u64()) {
        let annual_salary = monthly_salary * 12;
        transformed.insert("salary_annual".to_string(), Value::Number(annual_salary.into()));
    }

    // 提取其他有用信息
    if let Some(age) = user.pointer("/profile/age") {
        transformed.insert("age".to_string(), age.clone());
    }

    if let Some(occupation) = user.pointer("/profile/occupation") {
        transformed.insert("occupation".to_string(), occupation.clone());
    }

    if let Some(email) = user.pointer("/personal_info/email") {
        transformed.insert("email".to_string(), email.clone());
    }

    // 提取状态
    if let Some(status) = user.pointer("/metadata/status").and_then(|v| v.as_str()) {
        transformed.insert("is_active".to_string(), Value::Bool(status == "active"));
    }

    // 提取标签
    if let Some(tags) = user.pointer("/metadata/tags") {
        transformed.insert("tags".to_string(), tags.clone());
    }

    if transformed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(transformed)))
    }
}

/// Create sample data if it doesn't exist
fn create_sample_data() -> Result<()> {
    let sample_data = r#"[
  {
    "id": 1,
    "personal_info": {
      "first_name": "张",
      "last_name": "三",
      "email": "zhang.san@example.com"
    },
    "address": {
      "city": "北京",
      "country": "中国"
    },
    "profile": {
      "age": 28,
      "occupation": "软件工程师",
      "salary": 15000
    },
    "metadata": {
      "status": "active",
      "tags": ["premium", "developer"]
    }
  },
  {
    "id": 2,
    "personal_info": {
      "first_name": "李",
      "last_name": "四",
      "email": "li.si@example.com"
    },
    "address": {
      "city": "上海",
      "country": "中国"
    },
    "profile": {
      "age": 32,
      "occupation": "产品经理",
      "salary": 18000
    },
    "metadata": {
      "status": "active",
      "tags": ["premium", "manager"]
    }
  }
]"#;

    // 确保目录存在
    if let Some(parent) = Path::new("examples/data/sample_users.json").parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write("examples/data/sample_users.json", sample_data)
        .context("无法创建示例数据文件")?;

    println!("✓ 已创建示例数据文件");
    Ok(())
}
