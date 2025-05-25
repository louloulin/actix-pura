//! 工作流解析单元测试
//!
//! 测试YAML工作流文件的解析功能

use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;
use dataflare_runtime::workflow::YamlWorkflowParser;

#[test]
fn test_simple_workflow_parsing() {
    let workflow_yaml = r#"
id: test-workflow
name: Test Workflow
description: Simple test workflow
version: 1.0.0

sources:
  csv_source:
    type: csv
    config:
      file_path: "test.csv"
      has_header: true
      delimiter: ","
    collection_mode: full

transformations:
  passthrough:
    inputs:
      - csv_source
    type: mapping
    config:
      mappings:
        - source: id
          destination: id
        - source: name
          destination: name

destinations:
  csv_output:
    inputs:
      - passthrough
    type: csv
    config:
      file_path: "output.csv"
      delimiter: ","
      write_header: true

metadata:
  owner: test
  department: test
  created: "2025-01-01"
"#;

    // 创建临时文件
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(workflow_yaml.as_bytes()).unwrap();

    // 解析工作流
    let result = YamlWorkflowParser::load_from_file(temp_file.path());

    assert!(result.is_ok(), "工作流解析失败: {:?}", result.err());

    let workflow = result.unwrap();
    assert_eq!(workflow.id, "test-workflow");
    assert_eq!(workflow.name, "Test Workflow");
    assert_eq!(workflow.sources.len(), 1);
    assert_eq!(workflow.transformations.len(), 1);
    assert_eq!(workflow.destinations.len(), 1);

    // 验证源配置
    let csv_source = workflow.sources.get("csv_source").unwrap();
    assert_eq!(csv_source.r#type, "csv");
    assert_eq!(csv_source.config.get("file_path").unwrap().as_str().unwrap(), "test.csv");

    // 验证转换配置
    let passthrough = workflow.transformations.get("passthrough").unwrap();
    assert_eq!(passthrough.r#type, "mapping");
    assert_eq!(passthrough.inputs, vec!["csv_source"]);

    // 验证目标配置
    let csv_output = workflow.destinations.get("csv_output").unwrap();
    assert_eq!(csv_output.r#type, "csv");
    assert_eq!(csv_output.inputs, vec!["passthrough"]);
}

#[test]
fn test_workflow_with_windows_paths() {
    let workflow_yaml = r#"
id: windows-test
name: Windows Path Test
version: 1.0.0

sources:
  csv_source:
    type: csv
    config:
      file_path: "examples\\data\\test.csv"
      has_header: true
      delimiter: ","
    collection_mode: full

destinations:
  csv_output:
    inputs:
      - csv_source
    type: csv
    config:
      file_path: "examples\\data\\output.csv"
      delimiter: ","
      write_header: true
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(workflow_yaml.as_bytes()).unwrap();

    let result = YamlWorkflowParser::load_from_file(temp_file.path());
    assert!(result.is_ok(), "Windows路径工作流解析失败: {:?}", result.err());

    let workflow = result.unwrap();
    let csv_source = workflow.sources.get("csv_source").unwrap();
    assert_eq!(csv_source.config.get("file_path").unwrap().as_str().unwrap(), "examples\\data\\test.csv");
}

#[test]
fn test_invalid_workflow_parsing() {
    let invalid_yaml = r#"
id: invalid-workflow
# 缺少必需字段
sources:
  invalid_source:
    # 缺少type字段
    config:
      file_path: "test.csv"
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(invalid_yaml.as_bytes()).unwrap();

    let result = YamlWorkflowParser::load_from_file(temp_file.path());
    assert!(result.is_err(), "无效工作流应该解析失败");
}

#[test]
fn test_workflow_file_not_found() {
    let result = YamlWorkflowParser::load_from_file(Path::new("nonexistent.yaml"));
    assert!(result.is_err(), "不存在的文件应该返回错误");
}

#[test]
fn test_workflow_validation() {
    let workflow_yaml = r#"
id: validation-test
name: Validation Test
version: 1.0.0

sources:
  csv_source:
    type: csv
    config:
      file_path: "test.csv"
      has_header: true
      delimiter: ","
    collection_mode: full

transformations:
  filter_transform:
    inputs:
      - csv_source
    type: filter
    config:
      condition: "age > 18"

  mapping_transform:
    inputs:
      - filter_transform
    type: mapping
    config:
      mappings:
        - source: id
          destination: user_id

destinations:
  csv_output:
    inputs:
      - mapping_transform
    type: csv
    config:
      file_path: "output.csv"
      delimiter: ","
      write_header: true
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(workflow_yaml.as_bytes()).unwrap();

    let result = YamlWorkflowParser::load_from_file(temp_file.path());
    assert!(result.is_ok(), "复杂工作流解析失败: {:?}", result.err());

    let workflow = result.unwrap();

    // 验证数据流连接
    let filter_transform = workflow.transformations.get("filter_transform").unwrap();
    assert_eq!(filter_transform.inputs, vec!["csv_source"]);

    let mapping_transform = workflow.transformations.get("mapping_transform").unwrap();
    assert_eq!(mapping_transform.inputs, vec!["filter_transform"]);

    let csv_output = workflow.destinations.get("csv_output").unwrap();
    assert_eq!(csv_output.inputs, vec!["mapping_transform"]);
}

use std::io::Write;
