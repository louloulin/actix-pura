/**
 * Base plugin classes for DataFlare WASM plugins
 */

import {
  DataRecord,
  PluginInfo,
  PluginCapabilities,
  SecurityRequirements,
  ComponentType,
  ProcessingResult,
  BatchProcessingResult,
  PluginContext,
  PluginLifecycle,
  RecordProcessor,
  BatchProcessor,
  StreamProcessor,
  ProcessingError,
  ValidationError,
} from './types';

/**
 * Base class for all DataFlare plugins
 */
export abstract class PluginBase implements PluginLifecycle {
  protected context?: PluginContext;

  /**
   * Get plugin metadata
   */
  abstract getInfo(): PluginInfo;

  /**
   * Get component type
   */
  abstract getComponentType(): ComponentType;

  /**
   * Get plugin capabilities
   */
  abstract getCapabilities(): PluginCapabilities;

  /**
   * Get security requirements
   */
  abstract getSecurityRequirements(): SecurityRequirements;

  /**
   * Initialize the plugin
   */
  async initialize(context: PluginContext): Promise<void> {
    this.context = context;
    await this.onInit?.(context);
  }

  /**
   * Destroy the plugin
   */
  async destroy(): Promise<void> {
    if (this.context) {
      await this.onDestroy?.(this.context);
      this.context = undefined;
    }
  }

  /**
   * Validate plugin configuration
   */
  protected validateConfig(): void {
    if (!this.context) {
      throw new ValidationError('Plugin not initialized');
    }
  }

  /**
   * Get logger instance
   */
  protected get logger() {
    this.validateConfig();
    return this.context!.logger;
  }

  /**
   * Get metrics collector
   */
  protected get metrics() {
    this.validateConfig();
    return this.context!.metrics;
  }
}

/**
 * Data processor plugin base class
 */
export abstract class DataProcessor extends PluginBase implements RecordProcessor, BatchProcessor {
  getComponentType(): ComponentType {
    return ComponentType.Processor;
  }

  /**
   * Process a single data record
   */
  abstract processRecord(input: DataRecord, context: PluginContext): Promise<ProcessingResult>;

  /**
   * Process a batch of data records
   */
  async processBatch(input: DataRecord[], context: PluginContext): Promise<BatchProcessingResult> {
    const result: BatchProcessingResult = {
      success: [],
      errors: [],
      skipped: [],
    };

    for (const record of input) {
      try {
        const processResult = await this.processRecord(record, context);

        switch (processResult.type) {
          case 'success':
            result.success.push(processResult.data);
            break;
          case 'error':
            result.errors.push({ record, error: processResult.message });
            break;
          case 'skip':
            result.skipped.push(record);
            break;
        }
      } catch (error) {
        result.errors.push({
          record,
          error: error instanceof Error ? error.message : String(error),
        });
      }
    }

    return result;
  }
}

/**
 * Data transformer plugin base class
 */
export abstract class DataTransformer extends PluginBase implements RecordProcessor {
  getComponentType(): ComponentType {
    return ComponentType.Transformer;
  }

  /**
   * Transform a data record
   */
  abstract processRecord(input: DataRecord, context: PluginContext): Promise<ProcessingResult>;

  /**
   * Transform data payload
   */
  protected async transformData(data: any): Promise<any> {
    // Default implementation - override in subclasses
    return data;
  }
}

/**
 * Data source plugin base class
 */
export abstract class DataSource extends PluginBase {
  getComponentType(): ComponentType {
    return ComponentType.Source;
  }

  /**
   * Read data from source
   */
  abstract read(context: PluginContext): Promise<DataRecord[]>;

  /**
   * Stream data from source
   */
  abstract stream(context: PluginContext): AsyncIterable<DataRecord>;

  /**
   * Check if source has more data
   */
  abstract hasMore(): Promise<boolean>;
}

/**
 * Data destination plugin base class
 */
export abstract class DataDestination extends PluginBase {
  getComponentType(): ComponentType {
    return ComponentType.Destination;
  }

  /**
   * Write a single record to destination
   */
  abstract write(record: DataRecord, context: PluginContext): Promise<void>;

  /**
   * Write multiple records to destination
   */
  async writeBatch(records: DataRecord[], context: PluginContext): Promise<void> {
    for (const record of records) {
      await this.write(record, context);
    }
  }

  /**
   * Flush any pending writes
   */
  async flush(context: PluginContext): Promise<void> {
    // Default implementation - override if needed
  }
}

/**
 * Data filter plugin base class
 */
export abstract class DataFilter extends PluginBase implements RecordProcessor {
  getComponentType(): ComponentType {
    return ComponentType.Filter;
  }

  /**
   * Check if record should be included
   */
  protected abstract shouldInclude(data: any, context: PluginContext): Promise<boolean>;

  /**
   * Filter a data record - default implementation that uses shouldInclude
   */
  async processRecord(input: DataRecord, context: PluginContext): Promise<ProcessingResult> {
    try {
      const data = JSON.parse(input.data);
      const include = await this.shouldInclude(data, context);

      if (include) {
        return { type: 'success', data: input };
      } else {
        return { type: 'skip' };
      }
    } catch (error) {
      return {
        type: 'error',
        message: `Filter error: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }
}

/**
 * Data aggregator plugin base class
 */
export abstract class DataAggregator extends PluginBase implements BatchProcessor {
  getComponentType(): ComponentType {
    return ComponentType.Aggregator;
  }

  /**
   * Aggregate a batch of records
   */
  abstract processBatch(input: DataRecord[], context: PluginContext): Promise<BatchProcessingResult>;

  /**
   * Aggregate data values
   */
  protected abstract aggregate(values: any[], context: PluginContext): Promise<any>;
}

/**
 * AI processor plugin base class
 */
export abstract class AIProcessor extends PluginBase implements RecordProcessor {
  getComponentType(): ComponentType {
    return ComponentType.AIProcessor;
  }

  /**
   * Process record with AI
   */
  abstract processRecord(input: DataRecord, context: PluginContext): Promise<ProcessingResult>;

  /**
   * Load AI model
   */
  protected abstract loadModel(context: PluginContext): Promise<void>;

  /**
   * Predict using AI model
   */
  protected abstract predict(input: any, context: PluginContext): Promise<any>;

  /**
   * Initialize AI processor
   */
  async onInit(context: PluginContext): Promise<void> {
    await this.loadModel(context);
  }
}
