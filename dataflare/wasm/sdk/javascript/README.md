# DataFlare WASM Plugin SDK for JavaScript

A comprehensive TypeScript/JavaScript SDK for developing DataFlare WASM plugins.

## Features

- 🚀 **Easy Plugin Development** - Simple base classes for different plugin types
- 🔧 **TypeScript Support** - Full type safety and IntelliSense
- 🏃 **WASM Runtime** - Built-in WebAssembly runtime for plugin execution
- 📊 **Metrics & Logging** - Integrated observability features
- 🛡️ **Error Handling** - Comprehensive error types and validation
- 🔄 **Async Support** - Full async/await support for modern JavaScript

## Installation

```bash
npm install @dataflare/plugin-sdk
```

## Quick Start

### Creating a Simple Processor Plugin

```typescript
import { DataProcessor, DataRecord, PluginContext, ProcessingResult } from '@dataflare/plugin-sdk';

export class MyProcessor extends DataProcessor {
  getInfo() {
    return {
      name: 'my-processor',
      version: '1.0.0',
      description: 'A simple data processor',
      dataflare_version: '4.0.0',
    };
  }

  getCapabilities() {
    return {
      supports_async: true,
      supports_streaming: false,
      supports_batch: true,
      supports_backpressure: false,
    };
  }

  getSecurityRequirements() {
    return {
      requires_network: false,
      requires_filesystem: false,
      requires_env_vars: false,
    };
  }

  async processRecord(input: DataRecord, context: PluginContext): Promise<ProcessingResult> {
    try {
      const data = JSON.parse(input.data);
      
      // Process the data
      const processed = {
        ...data,
        processed_at: Date.now(),
        processed_by: this.getInfo().name,
      };

      return {
        type: 'success',
        data: {
          ...input,
          data: JSON.stringify(processed),
          updated_at: Date.now(),
        },
      };
    } catch (error) {
      return {
        type: 'error',
        message: `Processing failed: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }
}
```

### Using the WASM Runtime

```typescript
import { PluginLoader, createConsoleLogger } from '@dataflare/plugin-sdk';

async function loadAndRunPlugin() {
  const logger = createConsoleLogger('info');
  const loader = new PluginLoader(logger);

  // Load plugin from URL
  const plugin = await loader.loadPluginFromUrl('https://example.com/my-plugin.wasm');

  // Process a record
  const record = {
    id: 'test-1',
    data: JSON.stringify({ message: 'Hello, World!' }),
    metadata: {},
    created_at: Date.now(),
  };

  const result = await plugin.processRecord(record);
  console.log('Processing result:', result);

  // Cleanup
  plugin.destroy();
  loader.destroy();
}
```

## Plugin Types

The SDK provides base classes for different types of plugins:

### DataProcessor
For general data processing and transformation.

### DataTransformer
For data format transformations.

### DataSource
For reading data from external sources.

### DataDestination
For writing data to external destinations.

### DataFilter
For filtering data based on conditions.

### DataAggregator
For aggregating multiple records.

### AIProcessor
For AI/ML-powered data processing.

## Utilities

The SDK includes helpful utilities:

```typescript
import {
  createDataRecord,
  validateDataRecord,
  measureTime,
  retry,
  createConsoleLogger,
  createConsoleMetrics,
} from '@dataflare/plugin-sdk';

// Create a data record
const record = createDataRecord('id-1', { message: 'Hello' }, { source: 'api' });

// Validate a record
if (validateDataRecord(record)) {
  console.log('Valid record');
}

// Measure execution time
const result = await measureTime(async () => {
  // Some async operation
  return await processData();
}, (duration) => {
  console.log(`Operation took ${duration}ms`);
});

// Retry with exponential backoff
const data = await retry(async () => {
  return await fetchDataFromAPI();
}, {
  maxAttempts: 3,
  baseDelay: 1000,
  backoffFactor: 2,
});
```

## Error Handling

The SDK provides specific error types:

```typescript
import { PluginError, ValidationError, ProcessingError, ConfigurationError } from '@dataflare/plugin-sdk';

try {
  // Plugin operation
} catch (error) {
  if (error instanceof ValidationError) {
    console.error('Validation failed:', error.message);
  } else if (error instanceof ProcessingError) {
    console.error('Processing failed:', error.message);
  } else if (error instanceof ConfigurationError) {
    console.error('Configuration error:', error.message);
  }
}
```

## Development

```bash
# Install dependencies
npm install

# Build the SDK
npm run build

# Run tests
npm test

# Type checking
npm run type-check

# Linting
npm run lint
```

## License

MIT License - see LICENSE file for details.
