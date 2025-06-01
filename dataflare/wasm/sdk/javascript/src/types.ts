/**
 * Core types for DataFlare WASM Plugin SDK
 */

/**
 * Data record structure
 */
export interface DataRecord {
  /** Unique identifier for the record */
  id: string;
  /** JSON-serialized data payload */
  data: string;
  /** Metadata key-value pairs */
  metadata: Record<string, string>;
  /** Creation timestamp (Unix timestamp) */
  created_at?: number;
  /** Last update timestamp (Unix timestamp) */
  updated_at?: number;
}

/**
 * Plugin information
 */
export interface PluginInfo {
  /** Plugin name */
  name: string;
  /** Plugin version */
  version: string;
  /** Plugin description */
  description: string;
  /** Plugin author */
  author?: string;
  /** DataFlare version compatibility */
  dataflare_version: string;
}

/**
 * Plugin capabilities
 */
export interface PluginCapabilities {
  /** Supports async operations */
  supports_async: boolean;
  /** Supports streaming data */
  supports_streaming: boolean;
  /** Supports batch processing */
  supports_batch: boolean;
  /** Supports backpressure handling */
  supports_backpressure: boolean;
  /** Maximum batch size */
  max_batch_size?: number;
}

/**
 * Security requirements
 */
export interface SecurityRequirements {
  /** Requires network access */
  requires_network: boolean;
  /** Requires filesystem access */
  requires_filesystem: boolean;
  /** Requires environment variables */
  requires_env_vars: boolean;
  /** Maximum memory usage in MB */
  max_memory_mb?: number;
  /** Maximum execution time in milliseconds */
  max_execution_time_ms?: number;
}

/**
 * Component types
 */
export enum ComponentType {
  Source = 'source',
  Destination = 'destination',
  Processor = 'processor',
  Transformer = 'transformer',
  Filter = 'filter',
  Aggregator = 'aggregator',
  AIProcessor = 'ai-processor',
}

/**
 * Processing result
 */
export type ProcessingResult = 
  | { type: 'success'; data: DataRecord }
  | { type: 'error'; message: string }
  | { type: 'skip' };

/**
 * Batch processing result
 */
export interface BatchProcessingResult {
  /** Successfully processed records */
  success: DataRecord[];
  /** Failed records with error messages */
  errors: Array<{ record: DataRecord; error: string }>;
  /** Skipped records */
  skipped: DataRecord[];
}

/**
 * Plugin configuration
 */
export interface PluginConfig {
  /** Plugin metadata */
  plugin: PluginInfo;
  /** Plugin capabilities */
  capabilities: PluginCapabilities;
  /** Security requirements */
  security: SecurityRequirements;
  /** Custom configuration parameters */
  parameters?: Record<string, any>;
}

/**
 * Plugin context for execution
 */
export interface PluginContext {
  /** Plugin configuration */
  config: PluginConfig;
  /** Logger instance */
  logger: Logger;
  /** Metrics collector */
  metrics?: MetricsCollector;
  /** Environment variables */
  env: Record<string, string>;
}

/**
 * Logger interface
 */
export interface Logger {
  debug(message: string, ...args: any[]): void;
  info(message: string, ...args: any[]): void;
  warn(message: string, ...args: any[]): void;
  error(message: string, ...args: any[]): void;
}

/**
 * Metrics collector interface
 */
export interface MetricsCollector {
  /** Increment a counter */
  increment(name: string, value?: number, tags?: Record<string, string>): void;
  /** Record a gauge value */
  gauge(name: string, value: number, tags?: Record<string, string>): void;
  /** Record a timing value */
  timing(name: string, value: number, tags?: Record<string, string>): void;
  /** Record a histogram value */
  histogram(name: string, value: number, tags?: Record<string, string>): void;
}

/**
 * Plugin lifecycle hooks
 */
export interface PluginLifecycle {
  /** Called when plugin is initialized */
  onInit?(context: PluginContext): Promise<void> | void;
  /** Called when plugin is about to be destroyed */
  onDestroy?(context: PluginContext): Promise<void> | void;
  /** Called before processing starts */
  onStart?(context: PluginContext): Promise<void> | void;
  /** Called after processing stops */
  onStop?(context: PluginContext): Promise<void> | void;
}

/**
 * Error types
 */
export class PluginError extends Error {
  constructor(
    message: string,
    public readonly code?: string,
    public readonly details?: any
  ) {
    super(message);
    this.name = 'PluginError';
  }
}

export class ValidationError extends PluginError {
  constructor(message: string, details?: any) {
    super(message, 'VALIDATION_ERROR', details);
    this.name = 'ValidationError';
  }
}

export class ProcessingError extends PluginError {
  constructor(message: string, details?: any) {
    super(message, 'PROCESSING_ERROR', details);
    this.name = 'ProcessingError';
  }
}

export class ConfigurationError extends PluginError {
  constructor(message: string, details?: any) {
    super(message, 'CONFIGURATION_ERROR', details);
    this.name = 'ConfigurationError';
  }
}

/**
 * Stream processing interface
 */
export interface StreamProcessor {
  /** Process a stream of data records */
  processStream(
    input: AsyncIterable<DataRecord>,
    context: PluginContext
  ): AsyncIterable<ProcessingResult>;
}

/**
 * Batch processor interface
 */
export interface BatchProcessor {
  /** Process a batch of data records */
  processBatch(
    input: DataRecord[],
    context: PluginContext
  ): Promise<BatchProcessingResult>;
}

/**
 * Single record processor interface
 */
export interface RecordProcessor {
  /** Process a single data record */
  processRecord(
    input: DataRecord,
    context: PluginContext
  ): Promise<ProcessingResult>;
}
