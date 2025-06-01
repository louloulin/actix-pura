/**
 * DataFlare WASM Plugin SDK for JavaScript
 * 
 * This SDK provides TypeScript/JavaScript bindings for developing DataFlare WASM plugins.
 */

export * from './types';
export * from './plugin';
export * from './runtime';
export * from './utils';

// Re-export commonly used types for convenience
export type {
  DataRecord,
  PluginInfo,
  PluginCapabilities,
  SecurityRequirements,
  ProcessingResult,
  ComponentType,
  PluginContext,
  Logger,
  MetricsCollector,
} from './types';

export {
  PluginBase,
  DataProcessor,
  DataTransformer,
  DataSource,
  DataDestination,
  DataFilter,
  DataAggregator,
  AIProcessor,
} from './plugin';

export {
  WasmRuntime,
  PluginLoader,
  WasmPlugin,
} from './runtime';

export {
  validateDataRecord,
  createDataRecord,
  serializeResult,
  deserializeInput,
  createConsoleLogger,
  createNoOpMetrics,
  createConsoleMetrics,
  measureTime,
  retry,
} from './utils';

// Version information
export const VERSION = '1.0.0';
export const SDK_NAME = '@dataflare/plugin-sdk';

/**
 * Initialize the DataFlare Plugin SDK
 * 
 * @param options - SDK initialization options
 */
export function initializeSDK(options?: {
  logLevel?: 'debug' | 'info' | 'warn' | 'error';
  enableMetrics?: boolean;
}) {
  const config = {
    logLevel: options?.logLevel || 'info',
    enableMetrics: options?.enableMetrics || false,
  };
  
  if (config.logLevel === 'debug') {
    console.log(`[DataFlare SDK] Initialized v${VERSION} with config:`, config);
  }
  
  return config;
}
