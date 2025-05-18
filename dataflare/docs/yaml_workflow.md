# YAML 工作流定义指南

本文档介绍如何使用 YAML 定义 DataFlare 工作流。

## 概述

DataFlare 支持使用 YAML 格式定义数据集成工作流，这提供了一种声明式的方式来描述数据流和转换逻辑。YAML 工作流定义包括以下主要部分：

- 工作流元数据（ID、名称、描述等）
- 数据源配置
- 转换配置
- 目标配置
- 调度配置
- 其他元数据

## 基本结构

一个完整的 YAML 工作流定义的基本结构如下：

```yaml
# 工作流元数据
id: my-workflow
name: My Workflow
description: 工作流描述
version: 1.0.0

# 数据源配置
sources:
  source1:
    type: source_type
    mode: extraction_mode  # 可选
    config:
      # 源特定配置

# 转换配置
transformations:
  transform1:
    inputs:
      - source1
    type: transformation_type
    config:
      # 转换特定配置

# 目标配置
destinations:
  dest1:
    inputs:
      - transform1
    type: destination_type
    config:
      # 目标特定配置

# 调度配置
schedule:
  type: schedule_type
  expression: schedule_expression
  timezone: timezone  # 可选

# 其他元数据
metadata:
  key1: value1
  key2: value2
```

## 工作流元数据

工作流元数据部分定义了工作流的基本信息：

- `id`：工作流的唯一标识符
- `name`：工作流的显示名称
- `description`：工作流的描述（可选）
- `version`：工作流的版本号

示例：

```yaml
id: user-data-sync
name: User Data Synchronization
description: 将用户数据从 PostgreSQL 同步到 Elasticsearch
version: 1.0.0
```

## 数据源配置

`sources` 部分定义了工作流的数据源：

- 每个源都有一个唯一的 ID
- `type`：源类型（如 postgres、mysql、csv、memory 等）
- `mode`：提取模式（可选，如 full、incremental、cdc）
- `config`：源特定的配置参数

示例：

```yaml
sources:
  # PostgreSQL 源
  postgres_users:
    type: postgres
    mode: incremental
    config:
      host: localhost
      port: 5432
      database: mydb
      username: user
      password: pass
      table: users
      incremental:
        cursor_field: updated_at
        cursor_value: "2023-01-01T00:00:00Z"

  # CSV 源
  csv_orders:
    type: csv
    config:
      file_path: /data/orders.csv
      delimiter: ","
      has_header: true
```

### 提取模式

DataFlare 支持以下提取模式：

- `full`：全量提取，每次提取所有数据
- `incremental`：增量提取，基于游标字段提取新数据
- `cdc`：变更数据捕获，捕获数据库中的变更事件

对于增量模式，需要在配置中指定游标字段和初始游标值：

```yaml
mode: incremental
config:
  # ...其他配置
  incremental:
    cursor_field: updated_at
    cursor_value: "2023-01-01T00:00:00Z"
```

对于 CDC 模式，需要在配置中指定 CDC 相关参数：

```yaml
mode: cdc
config:
  # ...其他配置
  cdc:
    slot_name: my_slot
    publication_name: my_pub
    plugin: pgoutput
```

## 转换配置

`transformations` 部分定义了数据转换步骤：

- 每个转换都有一个唯一的 ID
- `inputs`：转换的输入源或其他转换
- `type`：转换类型（如 mapping、filter、aggregate、enrichment 等）
- `config`：转换特定的配置参数

示例：

```yaml
transformations:
  # 映射转换
  user_mapping:
    inputs:
      - postgres_users
    type: mapping
    config:
      mappings:
        - source: name
          destination: user.name
        - source: email
          destination: user.email
          transform: lowercase

  # 过滤转换
  active_users:
    inputs:
      - user_mapping
    type: filter
    config:
      condition: "user.active == true"

  # 聚合转换
  order_stats:
    inputs:
      - csv_orders
    type: aggregate
    config:
      group_by:
        - user_id
      aggregations:
        - source: amount
          destination: total_amount
          function: sum
        - source: id
          destination: order_count
          function: count
```

### 支持的转换类型

DataFlare 支持以下转换类型：

- `mapping`：字段映射和转换
- `filter`：基于条件过滤数据
- `aggregate`：数据聚合
- `enrichment`：数据丰富
- `join`：数据连接

## 目标配置

`destinations` 部分定义了数据输出目标：

- 每个目标都有一个唯一的 ID
- `inputs`：目标的输入源或转换
- `type`：目标类型（如 postgres、elasticsearch、csv、memory 等）
- `config`：目标特定的配置参数

示例：

```yaml
destinations:
  # Elasticsearch 目标
  es_users:
    inputs:
      - active_users
    type: elasticsearch
    config:
      host: localhost
      port: 9200
      index: users
      id_field: user.id

  # CSV 目标
  csv_output:
    inputs:
      - order_stats
    type: csv
    config:
      file_path: /data/user_orders.csv
      delimiter: ","
      write_header: true
```

## 调度配置

`schedule` 部分定义了工作流的执行调度：

- `type`：调度类型（cron、interval、once、event）
- `expression`：调度表达式
- `timezone`：时区（可选）
- `start_date`：开始日期（可选）
- `end_date`：结束日期（可选）

示例：

```yaml
# Cron 调度
schedule:
  type: cron
  expression: "0 0 * * *"  # 每天午夜执行
  timezone: UTC

# 间隔调度
schedule:
  type: interval
  expression: "3600"  # 每小时执行一次
  start_date: "2023-01-01T00:00:00Z"

# 一次性执行
schedule:
  type: once
  expression: "2023-12-31T23:59:59Z"
```

## 元数据

`metadata` 部分可以包含任意的键值对，用于存储工作流的附加信息：

```yaml
metadata:
  owner: data-team
  department: analytics
  priority: high
  tags: etl,production
```

## 完整示例

以下是一个完整的 YAML 工作流定义示例：

```yaml
id: user-order-sync
name: User and Order Synchronization
description: 同步用户和订单数据到数据仓库
version: 1.0.0

sources:
  users:
    type: postgres
    mode: incremental
    config:
      host: localhost
      port: 5432
      database: app_db
      username: app_user
      password: app_pass
      table: users
      incremental:
        cursor_field: updated_at
        cursor_value: "2023-01-01T00:00:00Z"

  orders:
    type: mysql
    mode: incremental
    config:
      host: localhost
      port: 3306
      database: orders_db
      username: orders_user
      password: orders_pass
      table: orders
      incremental:
        cursor_field: order_date
        cursor_value: "2023-01-01"

transformations:
  user_transform:
    inputs:
      - users
    type: mapping
    config:
      mappings:
        - source: id
          destination: user.id
        - source: name
          destination: user.name
        - source: email
          destination: user.email
          transform: lowercase

  order_transform:
    inputs:
      - orders
    type: mapping
    config:
      mappings:
        - source: id
          destination: order.id
        - source: user_id
          destination: order.userId
        - source: amount
          destination: order.amount

  order_aggregation:
    inputs:
      - order_transform
    type: aggregate
    config:
      group_by:
        - order.userId
      aggregations:
        - source: order.amount
          destination: user.totalSpent
          function: sum
        - source: order.id
          destination: user.orderCount
          function: count

  user_enrichment:
    inputs:
      - user_transform
      - order_aggregation
    type: enrichment
    config:
      source_field: user.id
      target_field: user.orders
      lookup_key: user.userId
      keep_original_fields: true

destinations:
  data_warehouse:
    inputs:
      - user_enrichment
    type: postgres
    config:
      host: localhost
      port: 5432
      database: data_warehouse
      username: dw_user
      password: dw_pass
      table: enriched_users
      batch_size: 1000

schedule:
  type: cron
  expression: "0 0 * * *"
  timezone: UTC

metadata:
  owner: data-engineering
  department: analytics
  priority: high
  environment: production
```

## 使用命令行工具

DataFlare 提供了命令行工具，用于验证和执行 YAML 工作流：

```bash
# 验证工作流
cargo run --example workflow_cli -- validate -f examples/workflows/simple_workflow.yaml

# 生成工作流 DOT 图
cargo run --example workflow_cli -- validate -f examples/workflows/simple_workflow.yaml -d

# 执行工作流
cargo run --example workflow_cli -- execute -f examples/workflows/simple_workflow.yaml
```

## 最佳实践

1. **使用有意义的 ID**：为工作流、源、转换和目标使用有意义的 ID，以便于理解和维护。

2. **模块化转换**：将复杂的转换逻辑拆分为多个小的转换步骤，以提高可读性和可维护性。

3. **验证工作流**：在执行工作流之前，使用验证工具检查工作流定义是否正确。

4. **使用版本控制**：将 YAML 工作流定义存储在版本控制系统中，以跟踪变更历史。

5. **使用环境变量**：对于敏感信息（如密码和 API 密钥），使用环境变量或密钥管理系统，而不是直接在 YAML 中硬编码。

6. **添加元数据**：使用元数据字段添加有用的信息，如所有者、部门、优先级等。

7. **使用注释**：在 YAML 文件中添加注释，解释复杂的配置和转换逻辑。
