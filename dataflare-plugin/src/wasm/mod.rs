//! WASM 插件模块
//!
//! 本模块提供了 WASM 插件的接口和实现。

use std::sync::Arc;
use serde_json::Value;
use wasmtime::{Memory, Store, Val};

use dataflare_core::error::{DataFlareError, Result};
use dataflare_core::message::DataRecord;
use crate::plugin::{Plugin, PluginMetadata, PluginState, ProcessorPlugin};

/// WASM 内存接口
pub struct WasmMemory<'a> {
    /// 内存实例
    memory: Memory,
    /// 存储
    store: &'a mut Store<PluginState>,
}

impl<'a> WasmMemory<'a> {
    /// 创建新的 WASM 内存接口
    pub fn new(memory: Memory, store: &'a mut Store<PluginState>) -> Self {
        Self { memory, store }
    }

    /// 从 WASM 内存中读取字符串
    pub fn read_string(&mut self, ptr: i32, len: i32) -> Result<String> {
        if ptr < 0 || len < 0 {
            return Err(DataFlareError::Plugin("无效的内存指针或长度".to_string()));
        }

        let data = self.memory.data(&*self.store)
            .get(ptr as usize..(ptr as usize + len as usize))
            .ok_or_else(|| DataFlareError::Plugin("内存访问越界".to_string()))?;

        let string = String::from_utf8(data.to_vec())
            .map_err(|e| DataFlareError::Plugin(format!("解析字符串时出错: {}", e)))?;

        Ok(string)
    }

    /// 将字符串写入 WASM 内存
    pub fn write_string(&mut self, string: &str) -> Result<(i32, i32)> {
        // 获取分配函数
        let alloc_name = self.store.data().config.get("alloc_function")
            .and_then(|v| v.as_str())
            .unwrap_or("alloc");

        // 获取实例
        let instance = self.memory.data(&*self.store).len();

        // 从预定义的内存位置开始分配
        // 在实际实现中，应该调用 WASM 模块的分配函数
        let ptr = 1024; // 从 1024 开始分配，这只是一个简单的实现
        let len = string.len() as i32;

        // 检查内存边界
        if (ptr as usize + string.len()) > instance {
            return Err(DataFlareError::Plugin("内存不足".to_string()));
        }

        // 写入字符串
        let memory_data = self.memory.data_mut(&mut *self.store);
        let start = ptr as usize;
        let end = start + string.len();

        memory_data[start..end].copy_from_slice(string.as_bytes());

        Ok((ptr as i32, len))
    }
}

/// WASM 处理器插件
#[derive(Debug)]
pub struct WasmProcessor {
    /// 插件实例
    plugin: Arc<Plugin>,
}

impl WasmProcessor {
    /// 创建新的 WASM 处理器插件
    pub fn new(plugin: Arc<Plugin>) -> Self {
        Self { plugin }
    }

    /// 获取 WASM 内存
    fn get_memory<'a>(&self, store: &'a mut Store<PluginState>) -> Result<Memory> {
        let instance = self.plugin.instance.instantiate(&mut *store)
            .map_err(|e| DataFlareError::Plugin(format!("实例化 WASM 模块时出错: {}", e)))?;

        instance.get_memory(&mut *store, "memory")
            .ok_or_else(|| DataFlareError::Plugin("插件没有导出内存".to_string()))
    }

    /// 调用 WASM 函数
    fn call_function(&self, store: &mut Store<PluginState>, name: &str, args: &[Value]) -> Result<Value> {
        // 获取函数
        let func = self.plugin.instance.instantiate(&mut *store)
            .map_err(|e| DataFlareError::Plugin(format!("实例化 WASM 模块时出错: {}", e)))?
            .get_func(&mut *store, name)
            .ok_or_else(|| DataFlareError::Plugin(format!("插件没有导出 {} 函数", name)))?;

        // 检查函数类型
        let func_type = func.ty(&*store);

        // 将 JSON 参数转换为 WASM 参数
        let mut wasm_args = Vec::new();
        let mut wasm_results = Vec::new();

        // 根据函数类型准备参数和结果
        for _ in func_type.params() {
            wasm_args.push(Val::I32(0));
        }

        for _ in func_type.results() {
            wasm_results.push(Val::I32(0));
        }

        // 获取内存
        let memory = self.get_memory(&mut *store)?;

        // 如果有参数，将其序列化并写入 WASM 内存
        if !args.is_empty() && !wasm_args.is_empty() {
            // 序列化参数
            let args_json = serde_json::to_string(args)
                .map_err(|e| DataFlareError::Plugin(format!("序列化参数时出错: {}", e)))?;

            // 获取分配函数
            let instance = self.plugin.instance.instantiate(&mut *store)
                .map_err(|e| DataFlareError::Plugin(format!("实例化 WASM 模块时出错: {}", e)))?;

            let alloc = instance.get_func(&mut *store, "alloc")
                .ok_or_else(|| DataFlareError::Plugin("插件没有导出 alloc 函数".to_string()))?;

            // 调用分配函数
            let mut alloc_results = [Val::I32(0)];
            let alloc_args = [Val::I32(args_json.len() as i32)];
            alloc.call(&mut *store, &alloc_args, &mut alloc_results)
                .map_err(|e| DataFlareError::Plugin(format!("调用 alloc 函数时出错: {}", e)))?;

            // 获取分配的内存指针
            let ptr = if let Val::I32(val) = alloc_results[0] { val } else { 0 };

            if ptr <= 0 {
                return Err(DataFlareError::Plugin("内存分配失败".to_string()));
            }

            // 写入参数数据
            let memory_data = memory.data_mut(&mut *store);
            let start = ptr as usize;
            let end = start + args_json.len();

            if end > memory_data.len() {
                return Err(DataFlareError::Plugin("内存访问越界".to_string()));
            }

            memory_data[start..end].copy_from_slice(args_json.as_bytes());

            // 设置参数
            if wasm_args.len() >= 2 {
                wasm_args[0] = Val::I32(ptr);
                wasm_args[1] = Val::I32(args_json.len() as i32);
            }
        }

        // 调用函数
        func.call(&mut *store, &wasm_args, &mut wasm_results)
            .map_err(|e| DataFlareError::Plugin(format!("调用 {} 函数时出错: {}", name, e)))?;

        // 如果有返回值，从 WASM 内存中读取
        if wasm_results.len() >= 2 {
            let ptr = if let Val::I32(val) = wasm_results[0] { val } else { 0 };
            let len = if let Val::I32(val) = wasm_results[1] { val } else { 0 };

            if ptr > 0 && len > 0 {
                // 读取返回值
                let memory_data = memory.data(&*store);
                let start = ptr as usize;
                let end = start + len as usize;

                if end > memory_data.len() {
                    return Err(DataFlareError::Plugin("内存访问越界".to_string()));
                }

                let result_json = String::from_utf8(memory_data[start..end].to_vec())
                    .map_err(|e| DataFlareError::Plugin(format!("解析返回值时出错: {}", e)))?;

                // 解析返回值
                let result: Value = serde_json::from_str(&result_json)
                    .map_err(|e| DataFlareError::Plugin(format!("解析返回值 JSON 时出错: {}", e)))?;

                return Ok(result);
            }
        }

        // 如果没有有效的返回值，返回空值
        Ok(Value::Null)
    }
}

impl ProcessorPlugin for WasmProcessor {
    fn configure(&mut self, config: Value) -> Result<()> {
        // 创建新的 Store 实例
        let mut store = Store::new(&self.plugin.engine, self.plugin.state.clone());

        // 调用配置函数
        self.call_function(&mut store, "configure", &[config])?;

        Ok(())
    }

    fn process(&self, record: DataRecord) -> Result<DataRecord> {
        // 创建新的 Store 实例
        let mut store = Store::new(&self.plugin.engine, self.plugin.state.clone());

        // 调用处理函数
        let result = self.call_function(&mut store, "process", &[serde_json::to_value(&record)?])?;

        // 反序列化结果
        let processed_record: DataRecord = serde_json::from_value(result)
            .map_err(|e| DataFlareError::Plugin(format!("反序列化处理结果时出错: {}", e)))?;

        Ok(processed_record)
    }

    fn get_metadata(&self) -> &PluginMetadata {
        &self.plugin.metadata
    }
}

/// 创建示例 WASM 模块
pub fn create_example_wasm_module() -> Result<Vec<u8>> {
    // 使用 wat 格式定义一个简单的 WASM 模块
    let wat = r#"
    (module
      ;; 导入日志函数
      (import "env" "log" (func $log (param i32 i32 i32)))

      ;; 导出内存
      (memory (export "memory") 1)

      ;; 全局变量存储配置
      (global $config (mut i32) (i32.const 0))

      ;; 分配内存
      (func $alloc (param i32) (result i32)
        (local $ptr i32)
        (local.set $ptr (i32.const 1024))  ;; 从 1024 开始分配
        (return (local.get $ptr))
      )

      ;; 获取元数据
      (func $get_metadata (result i32 i32)
        ;; 直接返回数据段中的元数据 JSON
        (i32.const 1024)  ;; 元数据 JSON 的内存位置
        (i32.const 98)    ;; 元数据 JSON 的长度
      )

      ;; 配置函数
      (func $configure (param i32 i32) (result i32)
        ;; 存储配置指针
        (global.set $config (local.get 0))

        ;; 记录日志
        (call $log (i32.const 3) (i32.const 2048) (i32.const 18))

        ;; 返回成功
        (return (i32.const 0))
      )

      ;; 处理函数
      (func $process (param i32 i32) (result i32 i32)
        ;; 处理记录
        (local $result i32)
        (local $result_len i32)

        ;; 记录日志
        (call $log (i32.const 3) (i32.const 2048) (i32.const 18))

        ;; 返回处理结果
        (local.set $result (i32.const 3072))
        (local.set $result_len (i32.const 189))
        (return (local.get $result) (local.get $result_len))
      )

      ;; 导出函数
      (export "alloc" (func $alloc))
      (export "get_metadata" (func $get_metadata))
      (export "configure" (func $configure))
      (export "process" (func $process))

      ;; 数据段
      (data (i32.const 2048) "Plugin initialized")
      (data (i32.const 1024) "{\"name\":\"example-processor\",\"version\":\"1.0.0\",\"author\":\"DataFlare Team\",\"plugin_type\":\"Processor\"}")
      (data (i32.const 3072) "{\"id\":\"00000000-0000-0000-0000-000000000001\",\"data\":{\"name\":\"Test Record\",\"value\":42,\"processed\":true},\"metadata\":{},\"created_at\":\"2023-01-01T00:00:00Z\",\"updated_at\":\"2023-01-01T00:00:00Z\"}")
    )
    "#;

    // 将 WAT 转换为 WASM 二进制
    let wasm = wat::parse_str(wat)
        .map_err(|e| DataFlareError::Plugin(format!("解析 WAT 时出错: {}", e)))?;

    Ok(wasm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_example_wasm_module() {
        let wasm = create_example_wasm_module();
        assert!(wasm.is_ok());
        assert!(!wasm.unwrap().is_empty());
    }

    // 更多测试将在实现完成后添加
}
