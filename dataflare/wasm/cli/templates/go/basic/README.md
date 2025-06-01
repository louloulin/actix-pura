# {{plugin_name}}

{{description}}

## Overview

This is a DataFlare {{plugin_type}} plugin written in Go. It processes data records and {{#if (eq plugin_type "filter")}}filters them based on specified criteria{{/if}}{{#if (eq plugin_type "map")}}transforms them according to the implemented logic{{/if}}{{#if (eq plugin_type "aggregate")}}aggregates them to produce summary statistics{{/if}}{{#if (eq plugin_type "processor")}}processes them according to the implemented logic{{/if}}.

## Features

- ✅ Go language support with TinyGo compiler
- ✅ WebAssembly compilation
- ✅ DataFlare plugin system integration
- ✅ Comprehensive error handling
- ✅ Performance optimized
- ✅ Easy to customize and extend

## Quick Start

### Prerequisites

- Go 1.21+
- TinyGo 0.30+
- DataFlare CLI tools
- DataFlare Plugin CLI

### Installation

```bash
# Install dependencies
go mod tidy

# Build the plugin
dataflare-plugin build

# Test the plugin
dataflare-plugin test

# Run benchmarks
dataflare-plugin benchmark
```

### Usage

```bash
# Validate the plugin
dataflare-plugin validate

# Package for distribution
dataflare-plugin package

# Install in DataFlare
dataflare-plugin install ./{{plugin_name}}-0.1.0.dfpkg
```

## Development

### Project Structure

```
{{plugin_name}}/
├── main.go              # Main plugin implementation
├── go.mod               # Go module configuration
├── plugin.toml          # Plugin configuration
├── tests/
│   ├── test_data/       # Test data files
│   └── integration/     # Integration tests
└── README.md            # This file
```

### Plugin Implementation

The main plugin logic is in `main.go`. Key functions:

{{#if (eq plugin_type "filter")}}
- `Process(data []byte) (bool, error)` - Filter function that returns true/false
{{/if}}
{{#if (eq plugin_type "map")}}
- `Process(data []byte) ([]byte, error)` - Transform function that returns modified data
{{/if}}
{{#if (eq plugin_type "aggregate")}}
- `Process(data []byte) ([]byte, error)` - Aggregate function that accumulates data
{{/if}}
{{#if (eq plugin_type "processor")}}
- `Process(data []byte) ([]byte, error)` - General processing function
{{/if}}
- `Info() (string, string)` - Returns plugin name and version
- `Capabilities() PluginCapabilities` - Returns plugin capabilities
- `Initialize(config map[string]interface{}) error` - Optional initialization function
- `Cleanup() error` - Optional cleanup function

### Testing

```bash
# Run the plugin locally for testing
go run main.go

# Build and test with DataFlare CLI
dataflare-plugin test

# Run with coverage
dataflare-plugin test --coverage

# Run integration tests
dataflare-plugin test --integration

# Run specific test
dataflare-plugin test --filter "my_test"
```

### Benchmarking

```bash
# Run performance benchmarks
dataflare-plugin benchmark

# Run with specific iterations
dataflare-plugin benchmark --iterations 1000

# Run specific benchmark
dataflare-plugin benchmark --filter "performance"
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

[build.go]
compiler = "tinygo"        # Use TinyGo for WASM compilation
gc = "leaking"             # GC strategy for smaller binaries
scheduler = "none"         # Disable scheduler for WASM
```

## API Reference

### Input Data Format

```go
type DataRecord struct {
    Value    []byte            // Record data as bytes
    Metadata map[string]string // Record metadata
}
```

### Output Format

{{#if (eq plugin_type "filter")}}
- **Filter**: Returns `(bool, error)` (true to keep, false to filter)
{{/if}}
{{#if (eq plugin_type "map")}}
- **Map**: Returns `([]byte, error)` (transformed data)
{{/if}}
{{#if (eq plugin_type "aggregate")}}
- **Aggregate**: Returns `([]byte, error)` (aggregated result)
{{/if}}
{{#if (eq plugin_type "processor")}}
- **Processor**: Returns `([]byte, error)` (processed data)
{{/if}}

### Plugin Capabilities

```go
type PluginCapabilities struct {
    Name               string // Plugin name
    Version            string // Plugin version
    Type               string // Plugin type
    Language           string // Programming language
    Description        string // Plugin description
    Author             string // Plugin author
    SupportsBatch      bool   // Batch processing support
    SupportsStreaming  bool   // Streaming support
    MemoryRequirements string // Memory requirements
    CPURequirements    string // CPU requirements
}
```

## Performance

This plugin is optimized for:

- **Memory Usage**: Low memory footprint with TinyGo
- **Binary Size**: Small WASM binary size
- **CPU Usage**: Efficient processing algorithms
- **Throughput**: High-performance data processing
- **Latency**: Minimal processing delay

## TinyGo Considerations

This plugin uses TinyGo for WebAssembly compilation, which has some limitations:

- **Reflection**: Limited reflection support
- **Goroutines**: Limited goroutine support in WASM
- **Standard Library**: Subset of Go standard library
- **GC**: Simplified garbage collection

For more information, see the [TinyGo documentation](https://tinygo.org/docs/).

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
- TinyGo Documentation: https://tinygo.org/docs/
- Community Forum: https://community.dataflare.io
- Issue Tracker: https://github.com/dataflare/dataflare/issues
