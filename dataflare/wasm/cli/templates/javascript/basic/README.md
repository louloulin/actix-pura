# {{plugin_name}}

{{description}}

## Overview

This is a DataFlare {{plugin_type}} plugin written in JavaScript. It processes data records and {{#if (eq plugin_type "filter")}}filters them based on specified criteria{{/if}}{{#if (eq plugin_type "map")}}transforms them according to the implemented logic{{/if}}{{#if (eq plugin_type "aggregate")}}aggregates them to produce summary statistics{{/if}}{{#if (eq plugin_type "processor")}}processes them according to the implemented logic{{/if}}.

## Features

- ✅ JavaScript/TypeScript support
- ✅ WebAssembly compilation
- ✅ DataFlare plugin system integration
- ✅ Comprehensive error handling
- ✅ Performance optimized
- ✅ Easy to customize and extend

## Quick Start

### Prerequisites

- Node.js 18+ 
- DataFlare CLI tools
- DataFlare Plugin CLI

### Installation

```bash
# Install dependencies
npm install

# Build the plugin
npm run build

# Test the plugin
npm run test

# Run benchmarks
npm run benchmark
```

### Usage

```bash
# Validate the plugin
npm run validate

# Package for distribution
npm run package

# Install in DataFlare
dataflare-plugin install ./{{plugin_name}}-0.1.0.dfpkg
```

## Development

### Project Structure

```
{{plugin_name}}/
├── src/
│   └── index.js          # Main plugin implementation
├── tests/
│   ├── test_data/        # Test data files
│   └── integration/      # Integration tests
├── package.json          # Node.js configuration
├── plugin.toml          # Plugin configuration
└── README.md            # This file
```

### Plugin Implementation

The main plugin logic is in `src/index.js`. Key functions:

{{#if (eq plugin_type "filter")}}
- `process(record)` - Filter function that returns true/false
{{/if}}
{{#if (eq plugin_type "map")}}
- `process(record)` - Transform function that returns modified data
{{/if}}
{{#if (eq plugin_type "aggregate")}}
- `process(record)` - Aggregate function that accumulates data
{{/if}}
{{#if (eq plugin_type "processor")}}
- `process(record)` - General processing function
{{/if}}
- `info()` - Returns plugin name and version
- `capabilities()` - Returns plugin capabilities
- `initialize(config)` - Optional initialization function
- `cleanup()` - Optional cleanup function

### Testing

```bash
# Run unit tests
npm test

# Run with coverage
npm run test -- --coverage

# Run integration tests
npm run test -- --integration

# Run specific test
npm run test -- --filter "my_test"
```

### Benchmarking

```bash
# Run performance benchmarks
npm run benchmark

# Run with specific iterations
npm run benchmark -- --iterations 1000

# Run specific benchmark
npm run benchmark -- --filter "performance"
```

## Configuration

Plugin behavior can be configured through `plugin.toml`:

```toml
[plugin.runtime]
memory_limit = "16MB"      # Memory limit for the plugin
timeout_ms = 10000         # Execution timeout
sandbox = true             # Enable sandboxing

[plugin.capabilities]
supports_batch = false     # Batch processing support
supports_streaming = true  # Streaming support
supports_state = {{#if (eq plugin_type "aggregate")}}true{{else}}false{{/if}}      # Stateful processing
```

## API Reference

### Input Record Format

```javascript
{
  value: Uint8Array,        // Record data as bytes
  metadata: {               // Record metadata
    timestamp: "2024-01-01T00:00:00Z",
    source: "input_source",
    // ... other metadata
  }
}
```

### Output Format

{{#if (eq plugin_type "filter")}}
- **Filter**: Returns `boolean` (true to keep, false to filter)
{{/if}}
{{#if (eq plugin_type "map")}}
- **Map**: Returns `Uint8Array` (transformed data)
{{/if}}
{{#if (eq plugin_type "aggregate")}}
- **Aggregate**: Returns `Uint8Array` (aggregated result)
{{/if}}
{{#if (eq plugin_type "processor")}}
- **Processor**: Returns `Uint8Array` (processed data)
{{/if}}

## Performance

This plugin is optimized for:

- **Memory Usage**: Low memory footprint
- **CPU Usage**: Efficient processing algorithms
- **Throughput**: High-performance data processing
- **Latency**: Minimal processing delay

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Run the test suite
6. Submit a pull request

## License

MIT License - see LICENSE file for details.

## Support

For support and questions:

- DataFlare Documentation: https://docs.dataflare.io
- Plugin Development Guide: https://docs.dataflare.io/plugins
- Community Forum: https://community.dataflare.io
- Issue Tracker: https://github.com/dataflare/dataflare/issues
