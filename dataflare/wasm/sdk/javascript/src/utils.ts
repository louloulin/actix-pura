/**
 * Utility functions for DataFlare WASM Plugin SDK
 */

import {
  DataRecord,
  ProcessingResult,
  ValidationError,
  Logger,
  MetricsCollector,
} from './types';

/**
 * Validate data record structure
 */
export function validateDataRecord(record: any): record is DataRecord {
  if (!record || typeof record !== 'object') {
    return false;
  }

  if (typeof record.id !== 'string' || record.id.length === 0) {
    return false;
  }

  if (typeof record.data !== 'string') {
    return false;
  }

  if (record.metadata && typeof record.metadata !== 'object') {
    return false;
  }

  if (record.created_at !== undefined && typeof record.created_at !== 'number') {
    return false;
  }

  if (record.updated_at !== undefined && typeof record.updated_at !== 'number') {
    return false;
  }

  return true;
}

/**
 * Create a new data record
 */
export function createDataRecord(
  id: string,
  data: any,
  metadata: Record<string, string> = {}
): DataRecord {
  if (!id || typeof id !== 'string') {
    throw new ValidationError('Record ID must be a non-empty string');
  }

  return {
    id,
    data: typeof data === 'string' ? data : JSON.stringify(data),
    metadata,
    created_at: Date.now(),
  };
}

/**
 * Serialize processing result to JSON
 */
export function serializeResult(result: ProcessingResult): string {
  return JSON.stringify(result);
}

/**
 * Deserialize input from JSON
 */
export function deserializeInput(json: string): DataRecord {
  try {
    const parsed = JSON.parse(json);
    
    if (!validateDataRecord(parsed)) {
      throw new ValidationError('Invalid data record format');
    }

    return parsed;
  } catch (error) {
    if (error instanceof ValidationError) {
      throw error;
    }
    throw new ValidationError(`Failed to parse input JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/**
 * Parse JSON data safely
 */
export function parseJsonData(data: string): any {
  try {
    return JSON.parse(data);
  } catch (error) {
    throw new ValidationError(`Invalid JSON data: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/**
 * Stringify data safely
 */
export function stringifyData(data: any): string {
  try {
    return JSON.stringify(data);
  } catch (error) {
    throw new ValidationError(`Failed to stringify data: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/**
 * Deep clone an object
 */
export function deepClone<T>(obj: T): T {
  if (obj === null || typeof obj !== 'object') {
    return obj;
  }

  if (obj instanceof Date) {
    return new Date(obj.getTime()) as unknown as T;
  }

  if (obj instanceof Array) {
    return obj.map(item => deepClone(item)) as unknown as T;
  }

  if (typeof obj === 'object') {
    const cloned = {} as T;
    for (const key in obj) {
      if (obj.hasOwnProperty(key)) {
        cloned[key] = deepClone(obj[key]);
      }
    }
    return cloned;
  }

  return obj;
}

/**
 * Merge metadata objects
 */
export function mergeMetadata(
  base: Record<string, string>,
  additional: Record<string, string>
): Record<string, string> {
  return { ...base, ...additional };
}

/**
 * Generate unique ID
 */
export function generateId(prefix: string = 'record'): string {
  const timestamp = Date.now();
  const random = Math.random().toString(36).substr(2, 9);
  return `${prefix}-${timestamp}-${random}`;
}

/**
 * Format timestamp
 */
export function formatTimestamp(timestamp: number): string {
  return new Date(timestamp).toISOString();
}

/**
 * Parse timestamp
 */
export function parseTimestamp(timestamp: string): number {
  const parsed = Date.parse(timestamp);
  if (isNaN(parsed)) {
    throw new ValidationError(`Invalid timestamp format: ${timestamp}`);
  }
  return parsed;
}

/**
 * Create a console logger
 */
export function createConsoleLogger(level: 'debug' | 'info' | 'warn' | 'error' = 'info'): Logger {
  const levels = { debug: 0, info: 1, warn: 2, error: 3 };
  const currentLevel = levels[level];

  return {
    debug: (message: string, ...args: any[]) => {
      if (currentLevel <= 0) console.debug(`[DEBUG] ${message}`, ...args);
    },
    info: (message: string, ...args: any[]) => {
      if (currentLevel <= 1) console.info(`[INFO] ${message}`, ...args);
    },
    warn: (message: string, ...args: any[]) => {
      if (currentLevel <= 2) console.warn(`[WARN] ${message}`, ...args);
    },
    error: (message: string, ...args: any[]) => {
      if (currentLevel <= 3) console.error(`[ERROR] ${message}`, ...args);
    },
  };
}

/**
 * Create a no-op metrics collector
 */
export function createNoOpMetrics(): MetricsCollector {
  return {
    increment: () => {},
    gauge: () => {},
    timing: () => {},
    histogram: () => {},
  };
}

/**
 * Create a console metrics collector
 */
export function createConsoleMetrics(logger: Logger): MetricsCollector {
  return {
    increment: (name: string, value = 1, tags?: Record<string, string>) => {
      logger.debug(`[METRIC] Counter ${name} += ${value}`, tags);
    },
    gauge: (name: string, value: number, tags?: Record<string, string>) => {
      logger.debug(`[METRIC] Gauge ${name} = ${value}`, tags);
    },
    timing: (name: string, value: number, tags?: Record<string, string>) => {
      logger.debug(`[METRIC] Timing ${name} = ${value}ms`, tags);
    },
    histogram: (name: string, value: number, tags?: Record<string, string>) => {
      logger.debug(`[METRIC] Histogram ${name} = ${value}`, tags);
    },
  };
}

/**
 * Measure execution time
 */
export async function measureTime<T>(
  fn: () => Promise<T>,
  onComplete?: (duration: number) => void
): Promise<T> {
  const start = performance.now();
  try {
    const result = await fn();
    const duration = performance.now() - start;
    onComplete?.(duration);
    return result;
  } catch (error) {
    const duration = performance.now() - start;
    onComplete?.(duration);
    throw error;
  }
}

/**
 * Retry function with exponential backoff
 */
export async function retry<T>(
  fn: () => Promise<T>,
  options: {
    maxAttempts?: number;
    baseDelay?: number;
    maxDelay?: number;
    backoffFactor?: number;
  } = {}
): Promise<T> {
  const {
    maxAttempts = 3,
    baseDelay = 1000,
    maxDelay = 10000,
    backoffFactor = 2,
  } = options;

  let lastError: Error;

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      
      if (attempt === maxAttempts) {
        throw lastError;
      }

      const delay = Math.min(baseDelay * Math.pow(backoffFactor, attempt - 1), maxDelay);
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  throw lastError!;
}
