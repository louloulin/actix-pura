@cluster.md 按照这个计划实现代码，增加单元测试，测试通过后更新 @cluster.md 标记实现的功能

@oms.md 继续按照这个设计基于actix-cluster实现功能，增加单元测试，测试通过后 更新 @oms.md 标记完成的功能

@plan-akka.md 按计划实现相关功能，实现后增加测试，测试验证通过后更新 @plan-akka.md 标记实现的功能，写入代码

@plan-akka.md 按计划实现相关功能，实现后增加测试，测试验证通过后更新 @plan-akka.md 标记实现的功能，写入代码

@plan-akka.md 按计划实现相关功能，实现后增加测试，测试验证通过后更新 @plan-akka.md 标记实现的功能，写入代码


@plan-akka.md 按计划实现相关功能，实现后增加测试，测试验证通过后更新 @plan-akka.md 标记实现的功能，写入代码，目录在actix-cluster，优先验证功能


@etl.md请将etl.md文档翻译成中文，并进行以下增强：

1. 添加两种部署模式的详细说明：
   - Edge模式：适用于边缘计算环境的轻量级部署
   - Server模式：适用于中心化服务器的完整功能部署

2. 扩展数据转换功能部分，具体包括：
   - 增加更多内置转换器类型
   - 添加复杂数据转换的示例
   - 支持更多数据格式和协议

3. 添加插件系统设计，重点说明：
   - WebAssembly (WASM) 插件架构
   - 插件开发指南
   - 插件安全性和性能考量

4. 参考并整合以下数据集成框架的优秀特性：
   - Dozer的实时数据处理能力
   - Airbyte的连接器生态系统
   - Vector的可观测性功能
   - 其他相关数据集成框架的创新点

5. 在文档末尾添加"未来改进计划"部分，详细说明后续版本的功能规划和技术路线图

请将所有更新内容保存到etl.md文件中，保持文档结构清晰，并确保技术术语的准确性。@etl.md请将etl.md文档翻译成中文，并进行以下增强：

1. 添加两种部署模式的详细说明：
   - Edge模式：适用于边缘计算环境的轻量级部署
   - Server模式：适用于中心化服务器的完整功能部署

2. 扩展数据转换功能部分，具体包括：
   - 增加更多内置转换器类型
   - 添加复杂数据转换的示例
   - 支持更多数据格式和协议

3. 添加插件系统设计，重点说明：
   - WebAssembly (WASM) 插件架构
   - 插件开发指南
   - 插件安全性和性能考量

4. 参考并整合以下数据集成框架的优秀特性：
   - Dozer的实时数据处理能力
   - Airbyte的连接器生态系统
   - Vector的可观测性功能
   - 其他相关数据集成框架的创新点

5. 在文档末尾添加"未来改进计划"部分，详细说明后续版本的功能规划和技术路线图

请将所有更新内容保存到etl.md文件中，保持文档结构清晰，并确保技术术语的准确性。



请按照 etl.md.zh.md 文档中定义的计划，实现数据集成框架中的 DataFlare 功能模块。实现过程应包括以下步骤：

1. 根据文档中的架构设计，开发 DataFlare 功能的核心组件
2. 实现 DataFlare 与现有 Actix actor 架构的集成
3. 为 DataFlare 功能编写全面的单元测试和集成测试
4. 运行测试并确保所有测试用例通过
5. 测试验证通过后，更新 etl.md.zh.md 文档，在相应部分标记已实现的 DataFlare 功能
6. 在文档中添加 DataFlare 功能的实现细节、使用示例和最佳实践

请确保实现符合文档中定义的 DSL 规范，并支持全量、增量和 CDC 等多种数据采集模式。

分析整个dataflare代码改造成模块化方式包在crates下，按照docs下的 @dataflare/docs/runtime_architecture.md 和 @dataflare/docs/runtime_implementation_plan.md改造，现有dataflare代码，通过命令mv现有代码到对应的crates下，实现后更新 @dataflare/docs/runtime_implementation_plan.md 标记实现的功能


分析整个dataflare代码改造成模块化方式包在crates下，按照docs下的 @dataflare/docs/runtime_architecture.md 和 @dataflare/docs/runtime_implementation_plan.md改造，现有dataflare代码，通过命令mv现有代码到对应的crates下，实现后更新 @dataflare/docs/runtime_implementation_plan.md 标记实现的功能


分析 DataFlare 代码库并将其重构为模块化的 crates 结构，具体要求如下：

1. 根据 `dataflare/docs/runtime_architecture.md` 和 `dataflare/docs/runtime_implementation_plan.md` 文档中定义的架构设计，将现有的 DataFlare 代码重构为多个独立的 crates

2. 创建以下 crates 并将相关代码移动到对应位置：
   - dataflare-core：核心组件和基础设施
   - dataflare-runtime：运行时引擎和执行器
   - dataflare-connector：连接器系统
   - dataflare-processor：处理器系统
   - dataflare-plugin：插件系统
   - dataflare-state：状态管理系统
   - dataflare-edge：边缘模式支持
   - dataflare-cloud：云模式支持
   - dataflare-cli：命令行工具
   - dataflare：主包，整合所有组件

3. 使用 `mv` 命令将现有代码文件移动到相应的 crates 目录下，确保代码结构清晰且符合架构设计

4. 重构完成后，更新 `dataflare/docs/runtime_implementation_plan.md` 文件，在已实现的功能项前添加 ✅ 标记，以表示该功能已经完成

5. 确保重构后的代码能够正常编译和运行



10:28 PM
请对DataFlare项目的多模块迁移进展进行全面分析。具体需要：
1. 对照 @dataflare/docs/runtime_implementation_plan.md 文档中列出的计划功能
2. 详细评估每个功能的实现状态（已完成、部分完成或未实现）
3. 明确指出哪些功能尚未实现或实现不完全
4. 提供关于未实现功能的优先级建议和可能的实现路径
5. 如可能，估计完成剩余功能所需的工作量

请以表格形式呈现分析结果，并在结论部分提供总体迁移完成度的评估。@runtime_implementation_plan.md请对DataFlare项目的多模块迁移进展进行全面分析。具体需要：
1. 对照 @dataflare/docs/runtime_implementation_plan.md 文档中列出的计划功能
2. 详细评估每个功能的实现状态（已完成、部分完成或未实现）
3. 明确指出哪些功能尚未实现或实现不完全
4. 提供关于未实现功能的优先级建议和可能的实现路径
5. 如可能，估计完成剩余功能所需的工作量

请以表格形式呈现分析结果，并在结论部分提供总体迁移完成度的评估。






基于actix实现，在 @dataflare 按计划 @plan2.md 实现相关的功能，实现后增加测试验证，验证通过后更新 @plan2.md 标记实现的功能


基于actix实现，在 @dataflare 按计划 @plan5.md 实现相关的功能，实现后增加测试验证，验证通过后更新 @plan5.md 标记实现的功能，中文说明


@xf001.md 按计划实现相关功能，实现后更新验证通过后 更新 @xf001.md 标记实现的功能

@dataflare4.md 按照计划实现相关功能，完成后增加测试验证，验证通过后，更新 @dataflare4.md 标记实现的功能并提交代码到git

分析 dataflare的相关代码，@dataflare/wasm/wasm.md  按照计划实现相关功能，完成后增加测试验证，验证通过后，更新 @dataflare/wasm/wasm.md 标记实现的功能



基于 @dataflare/wasm/plugin3.md 中详细设计方案，实施DataFlare WASM插件系统转换，遵循以下具体要求：
直接重构原来的插件，该删除的删除，该修改的修改 工作空间在dataflare下的plugin
1. **实施范围和阶段划分**：
   - **第一阶段 (P0 - 第1周)**：实现核心插件接口（plugin3.md第3.1节），包括：
      - 实现PluginRecord<'a>零拷贝数据结构，包含生命周期管理和&[u8] value字段
      - 实现SmartPlugin trait，包含process()、name()、version()方法
      - 实现SmartPluginAdapter，桥接到现有DataFlare Processor接口
      - 实现基础错误处理，包含PluginError枚举和Result类型
      - 在dataflare-plugin crate中创建完整的API结构
   - **第二阶段 (P0 - 第2周)**：实现CLI集成（plugin3.md第3.2节），扩展plan2.md现有插件命令：
      - 添加`dataflare plugin new <name> --type <filter|map|aggregate> [--lang rust]`命令
      - 添加`dataflare plugin build [--release] [--target native]`命令
      - 添加`dataflare plugin test [--data <input>] [--file <test-file>]`命令
      - 添加`dataflare plugin publish [--registry <url>]`命令（存根实现）
      - 确保与现有`plugin list/install/remove`命令完全兼容
   - **第三阶段 (P1 - 第3-4周)**：实现注解驱动系统（plugin3.md第3.3节），包含过程宏和自动代码生成

2. **具体代码实现要求**：
   - **核心接口** (dataflare-plugin crate)：
      - 实现PluginRecord<'a>结构体，包含零拷贝&[u8] value字段和&HashMap<String, String> metadata
      - 创建SmartPlugin trait，包含同步process()方法，返回PluginResult枚举
      - 构建SmartPluginAdapter，实现现有dataflare_core::processor::Processor trait
      - 实现完整的错误处理链，从PluginError到DataFlareError的转换
   - **CLI扩展** (dataflare-cli crate)：
      - 扩展plan2.md现有CLI结构，不破坏现有`plugin list/install/remove`命令
      - 添加Rust插件项目模板生成功能，包含Cargo.toml和src/lib.rs模板
      - 实现与cargo构建系统的基础集成
      - 添加插件测试框架，支持数据输入和预期输出验证
   - **集成点**：
      - 确保SmartPluginAdapter可以注册到现有处理器注册表
      - 保持与当前YAML工作流格式的兼容性，使用`processor_type: plugin`
      - 保留现有Actor系统集成，无需修改WorkflowActor或ProcessorActor

3. **测试和验证要求**：
   - **单元测试**：实现80%+代码覆盖率，包括：
      - PluginRecord创建和零拷贝行为验证
      - SmartPluginAdapter与模拟DataRecord输入的集成测试
      - CLI命令解析和执行逻辑测试
      - 错误处理路径的完整测试覆盖
   - **集成测试**：
      - 创建端到端测试：生成插件项目→构建→在DataFlare工作流中加载
      - 验证插件在现有Actor系统中的执行（WorkflowActor → ProcessorActor → SmartPluginAdapter）
      - 测试带有plugin processor类型的YAML工作流解析
      - 验证插件与现有连接器和处理器的互操作性
   - **性能基准测试**：
      - 测量并记录相对于基线Processor实现的实际性能改进
      - 通过内存分配测量验证零拷贝声明
      - 比较插件执行开销与原生处理器实现的差异
      - 提供具体的性能指标和基准数据

4. **文档和进度跟踪**：
   - **实时更新**：每个阶段完成后，更新plugin3.md：
      - 在第3.1-3.5节中用✅标记已完成任务
      - 记录实际实施时间与估计时间的对比
      - 记录任何架构决策或与原计划的偏差
   - **实施说明**：添加新章节记录：
      - 代码组织和模块结构
      - 集成挑战和解决方案
      - 具体指标的性能测试结果
   - **API文档**：使用实际实现的API签名更新第9.1节

5. **验证和验收标准**：
   - **功能验证**：
      - 成功使用`dataflare plugin new`创建简单过滤器插件
      - 使用`dataflare plugin build`无错误构建插件
      - 在DataFlare工作流中执行插件并验证正确的数据处理
      - 通过现有CLI `dataflare run workflow.yaml`演示插件加载和执行
   - **性能验证**：
      - 实现可测量的性能改进（目标：比等效原生处理器快70%）
      - 通过内存分析演示零拷贝行为
      - 验证插件开销最小（<总处理时间的5%）
   - **集成验证**：
      - 现有DataFlare功能保持不受影响
      - 新CLI命令与plan2.md的命令结构无缝集成
      - 插件系统与现有Actor模型和错误处理协同工作

6. **实施指导原则**：
   - 每个阶段从最小可行实现开始
   - 使用现有DataFlare模式和约定（错误处理、日志记录、配置）
   - 优先考虑兼容性而非新功能
   - 实现全面的错误消息和用户反馈
   - 遵循Rust最佳实践进行零拷贝数据处理和生命周期管理
   - 在实施过程中保持与plugin3.md设计文档的一致性
   - 每个阶段完成后进行代码审查和文档更新

7. **交付成果**：
   - **第一阶段交付**：可工作的核心插件接口，包含完整单元测试
   - **第二阶段交付**：集成的CLI命令，能够创建和构建基础插件
   - **第三阶段交付**：注解驱动的插件开发体验，包含完整文档和示例
   - **最终交付**：更新的plugin3.md文档，包含实际实施结果和性能数据

@dataflare/docs/plan3.md 按计划实现相关的功能，实现后增加测试验证，验证通过后更新 @dataflare/docs/plan3.md 标记实现的功能

根据 @dataflare/docs/plan5.md 中制定的DataFlare插件系统重构计划，按照以下优先级顺序实施改造：

1. **阶段1：架构重构 (P0 - 2周)**
    - 实现新的3层插件架构：Plugin Interface → Plugin Runtime → Native/WASM Backend
    - 创建统一的DataFlarePlugin trait接口
    - 实现零拷贝的PluginRecord<'a>数据结构
    - 重构现有的SmartPluginAdapter以适配新架构
    - 删除过时的6层抽象结构（WasmSystem → WasmRuntime → WasmPlugin → WasmComponent → WasmProcessor → SmartPlugin）

2. **阶段2：性能优化 (P0 - 1周)**
    - 实现WasmInstancePool实例池管理
    - 添加WASM编译缓存机制
    - 实现MemoryPool内存池管理
    - 支持BatchPlugin批处理接口

3. **阶段3：CLI工具统一 (P1)**
    - 将分散的CLI工具（dataflare plugin、dataflare-plugin-cli、dataflare-wasm-cli）统一为单一的dataflare plugin命令
    - 实现标准化的plugin.toml配置格式
    - 添加完整的插件生命周期管理命令

4. **测试验证要求**：
    - 为每个实现的功能添加单元测试
    - 实现集成测试验证新旧系统兼容性
    - 添加性能基准测试，验证达到预期的性能目标（70%延迟减少，3倍吞吐量提升）
    - 确保测试覆盖率达到90%以上

5. **实施后更新文档**：
    - 在plan5.md中标记已完成的功能（使用✅符号）
    - 更新实施进度和完成状态
    - 记录实际性能提升数据
    - 添加迁移指南和向后兼容性说明

请按照P0优先级开始实施，确保每个阶段完成后都有完整的测试验证，并及时更新plan5.md文档反映实际进展。


based on the detailed development plan in `@dataflare/wasm/plot.md`, implement the DataFlare Enterprise transformation features according to the specified timeline and milestones. For each implemented feature or module:

1. **Implementation Requirements**:
    - Follow the exact technical specifications outlined in plot.md
    - Implement features in the order specified by the development phases (Week 1-10)
    - Ensure integration with `@dataflare/plugin` as the foundation
    - Remove duplicate functionality that conflicts with the existing plugin system
    - Maintain the enterprise-grade performance targets (>1M records/sec, <10ms P99 latency)

2. **Testing and Validation**:
    - Write comprehensive unit tests for each implemented module
    - Create integration tests that verify enterprise-level functionality
    - Run performance benchmarks to validate performance targets
    - Test security and compliance features thoroughly
    - Verify distributed computing capabilities across multiple nodes

3. **Documentation Updates**:
    - After successful testing, update `@dataflare/wasm/plot.md` to mark completed features with ✅
    - Update progress tracking for each milestone and phase
    - Document any deviations from the original plan with explanations
    - Add implementation notes and lessons learned
    - Update the success metrics and KPIs based on actual results

4. **Quality Gates**:
    - All tests must pass before marking a feature as complete
    - Performance benchmarks must meet or exceed the specified targets
    - Security scans must pass for enterprise-grade compliance
    - Code review and approval required for each major component

Focus on implementing Phase 1 (Weeks 1-2) first: module renaming, cleanup of duplicate functionality, and establishment of enterprise-grade core modules, then proceed systematically through subsequent phases.