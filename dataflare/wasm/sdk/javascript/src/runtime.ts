/**
 * WASM runtime for DataFlare plugins
 */

import { WASI } from '@wasmer/wasi';
import { WasmFs } from '@wasmer/wasmfs';
import {
  DataRecord,
  ProcessingResult,
  PluginError,
  Logger,
} from './types';

/**
 * WASM runtime for executing plugins
 */
export class WasmRuntime {
  private wasmFs: WasmFs;
  private wasi: WASI;
  private instance?: WebAssembly.Instance;
  private module?: WebAssembly.Module;

  constructor(private logger: Logger) {
    this.wasmFs = new WasmFs();
    this.wasi = new WASI({
      args: [],
      env: {},
      bindings: {
        ...this.wasmFs.getImports(),
      },
    });
  }

  /**
   * Load WASM module from bytes
   */
  async loadModule(wasmBytes: Uint8Array): Promise<void> {
    try {
      this.module = await WebAssembly.compile(wasmBytes);
      this.logger.info('WASM module compiled successfully');
    } catch (error) {
      throw new PluginError(
        `Failed to compile WASM module: ${error instanceof Error ? error.message : String(error)}`,
        'COMPILATION_ERROR'
      );
    }
  }

  /**
   * Instantiate WASM module
   */
  async instantiate(): Promise<void> {
    if (!this.module) {
      throw new PluginError('WASM module not loaded', 'MODULE_NOT_LOADED');
    }

    try {
      const imports = {
        ...this.wasi.getImports(this.module),
        env: {
          // Add custom imports here
          log: (ptr: number, len: number) => {
            const memory = this.instance?.exports.memory as WebAssembly.Memory;
            if (memory) {
              const buffer = new Uint8Array(memory.buffer, ptr, len);
              const message = new TextDecoder().decode(buffer);
              this.logger.info(`[WASM] ${message}`);
            }
          },
        },
      };

      this.instance = await WebAssembly.instantiate(this.module, imports);
      this.wasi.start(this.instance);
      this.logger.info('WASM module instantiated successfully');
    } catch (error) {
      throw new PluginError(
        `Failed to instantiate WASM module: ${error instanceof Error ? error.message : String(error)}`,
        'INSTANTIATION_ERROR'
      );
    }
  }

  /**
   * Call exported function
   */
  callFunction(name: string, ...args: any[]): any {
    if (!this.instance) {
      throw new PluginError('WASM module not instantiated', 'MODULE_NOT_INSTANTIATED');
    }

    const exports = this.instance.exports as any;
    const func = exports[name];

    if (typeof func !== 'function') {
      throw new PluginError(`Function '${name}' not found in WASM module`, 'FUNCTION_NOT_FOUND');
    }

    try {
      return func(...args);
    } catch (error) {
      throw new PluginError(
        `Error calling function '${name}': ${error instanceof Error ? error.message : String(error)}`,
        'FUNCTION_CALL_ERROR'
      );
    }
  }

  /**
   * Get memory buffer
   */
  getMemory(): WebAssembly.Memory | undefined {
    return this.instance?.exports.memory as WebAssembly.Memory;
  }

  /**
   * Write string to WASM memory
   */
  writeString(str: string): { ptr: number; len: number } {
    const memory = this.getMemory();
    if (!memory) {
      throw new PluginError('WASM memory not available', 'MEMORY_NOT_AVAILABLE');
    }

    const encoder = new TextEncoder();
    const bytes = encoder.encode(str);
    
    // Allocate memory (assuming malloc function exists)
    const ptr = this.callFunction('malloc', bytes.length);
    const buffer = new Uint8Array(memory.buffer, ptr, bytes.length);
    buffer.set(bytes);

    return { ptr, len: bytes.length };
  }

  /**
   * Read string from WASM memory
   */
  readString(ptr: number, len: number): string {
    const memory = this.getMemory();
    if (!memory) {
      throw new PluginError('WASM memory not available', 'MEMORY_NOT_AVAILABLE');
    }

    const buffer = new Uint8Array(memory.buffer, ptr, len);
    return new TextDecoder().decode(buffer);
  }

  /**
   * Free memory
   */
  free(ptr: number): void {
    this.callFunction('free', ptr);
  }

  /**
   * Destroy runtime
   */
  destroy(): void {
    this.instance = undefined;
    this.module = undefined;
    this.logger.info('WASM runtime destroyed');
  }
}

/**
 * Plugin loader for WASM plugins
 */
export class PluginLoader {
  private runtime: WasmRuntime;
  private logger: Logger;

  constructor(logger: Logger) {
    this.logger = logger;
    this.runtime = new WasmRuntime(logger);
  }

  /**
   * Load plugin from WASM bytes
   */
  async loadPlugin(wasmBytes: Uint8Array): Promise<WasmPlugin> {
    await this.runtime.loadModule(wasmBytes);
    await this.runtime.instantiate();

    return new WasmPlugin(this.runtime, this.logger);
  }

  /**
   * Load plugin from URL
   */
  async loadPluginFromUrl(url: string): Promise<WasmPlugin> {
    this.logger.info(`Loading plugin from URL: ${url}`);
    
    const response = await fetch(url);
    if (!response.ok) {
      throw new PluginError(`Failed to fetch plugin: ${response.statusText}`, 'FETCH_ERROR');
    }

    const wasmBytes = new Uint8Array(await response.arrayBuffer());
    return this.loadPlugin(wasmBytes);
  }

  /**
   * Destroy loader
   */
  destroy(): void {
    this.runtime.destroy();
  }
}

/**
 * WASM plugin wrapper
 */
export class WasmPlugin {
  constructor(
    private runtime: WasmRuntime,
    private logger: Logger
  ) {}

  /**
   * Get plugin info
   */
  getInfo(): any {
    try {
      const infoPtr = this.runtime.callFunction('get_plugin_info');
      const infoJson = this.runtime.readString(infoPtr, 1024); // Assume max 1KB
      this.runtime.free(infoPtr);
      return JSON.parse(infoJson);
    } catch (error) {
      throw new PluginError(
        `Failed to get plugin info: ${error instanceof Error ? error.message : String(error)}`,
        'GET_INFO_ERROR'
      );
    }
  }

  /**
   * Process data record
   */
  async processRecord(record: DataRecord): Promise<ProcessingResult> {
    try {
      const inputJson = JSON.stringify(record);
      const { ptr: inputPtr, len: inputLen } = this.runtime.writeString(inputJson);

      const resultPtr = this.runtime.callFunction('process_record', inputPtr, inputLen);
      const resultJson = this.runtime.readString(resultPtr, 4096); // Assume max 4KB result
      
      this.runtime.free(inputPtr);
      this.runtime.free(resultPtr);

      const result = JSON.parse(resultJson);
      
      if (result.type === 'success') {
        return { type: 'success', data: result.data };
      } else if (result.type === 'error') {
        return { type: 'error', message: result.message };
      } else {
        return { type: 'skip' };
      }
    } catch (error) {
      return {
        type: 'error',
        message: `Processing error: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }

  /**
   * Process batch of records
   */
  async processBatch(records: DataRecord[]): Promise<ProcessingResult[]> {
    const results: ProcessingResult[] = [];
    
    for (const record of records) {
      const result = await this.processRecord(record);
      results.push(result);
    }

    return results;
  }

  /**
   * Validate plugin
   */
  validate(): boolean {
    try {
      return this.runtime.callFunction('validate_plugin') === 1;
    } catch (error) {
      this.logger.error(`Plugin validation failed: ${error}`);
      return false;
    }
  }

  /**
   * Destroy plugin
   */
  destroy(): void {
    try {
      this.runtime.callFunction('cleanup');
    } catch (error) {
      this.logger.warn(`Plugin cleanup failed: ${error}`);
    }
    this.runtime.destroy();
  }
}
