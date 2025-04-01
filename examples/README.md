# Actix-Pura Examples

This directory contains examples for the Actix-Pura framework.

## Examples

1. **Chat System** (`chat_system/`) - A real-time distributed chat system with rooms and messaging.
2. **Task Processor** (`task_processor/`) - A distributed job processing system with dynamic worker allocation.
3. **HTTP API** (`http_api/`) - A RESTful API backed by distributed actors.
4. **File Sync** (`file_sync/`) - A distributed file synchronization service.
5. **Distributed Benchmark** (`distributed_benchmark/`) - A tool for benchmarking distributed actor performance.

## Running the Examples

Each example is a separate crate that can be run independently. Navigate to the example's directory and use Cargo to run it:

```bash
# Run the chat system example
cd chat_system
cargo run

# Run the task processor example
cd ../task_processor
cargo run

# For multi-node examples, use the --multi flag (where supported)
cargo run -- --multi --nodes 3
```

See the individual example's documentation for specific command-line options and features.

## Example Features

| Example | Features |
|---------|----------|
| Chat System | User and room management, real-time messaging, actor discovery |
| Task Processor | Coordinator-worker architecture, task scheduling, load balancing |
| HTTP API | RESTful API, actor backend, distributed state, health monitoring |
| File Sync | File system monitoring, peer-to-peer synchronization |
| Distributed Benchmark | Performance testing, multi-node measurements |

## Docker Support

Some examples include Docker support for easy deployment in a distributed environment. For instance, the Chat System example provides both a Dockerfile and a docker-compose.yml file:

```bash
# Build and run with Docker Compose
cd chat_system
docker-compose up --build
```

This will start a multi-node cluster automatically. See each example's README for specific Docker instructions.

## Testing

Examples with tests demonstrate how to test actor systems. Run the tests with:

```bash
cd chat_system
cargo test
```

## Code Organization

The examples follow best practices for organizing Rust code:

- **Modular Structure**: Code is organized into modules (`actors`, `messages`, etc.)
- **Clear Separation**: Actors, messages, and utilities are separated
- **Well-Documented**: Comments explain the purpose and usage
- **Testable**: Tests demonstrate functionality

## Installation

These examples use local dependencies from the Actix-Pura workspace. The examples directory is configured as a workspace with all examples as members.

To run any example, you need to have Rust installed. If you don't have Rust installed, you can get it from [https://rustup.rs/](https://rustup.rs/).

### Dependencies

All examples share common dependencies:

- actix (path: ../../actix)
- actix-cluster (path: ../../actix-cluster)
- log and env_logger for logging
- structopt for command-line arguments
- serde for serialization
- tokio for async runtime

## Example-Specific Dependencies

Some examples have additional dependencies:

- **Chat System**: Standard actix dependencies
- **Task Processor**: Includes rand for randomized task generation
- **HTTP API**: Uses actix-web for the HTTP server and serde_json for JSON processing
- **File Sync**: Standard actix dependencies
- **Distributed Benchmark**: Focused on performance testing with minimal dependencies

## Contributing

Contributions to these examples are welcome! If you find a bug or want to add a new example, please open an issue or submit a pull request on the repository.

When adding a new example, please follow the existing structure:

1. Create a new directory for your example
2. Add a Cargo.toml file with appropriate dependencies
3. Document your example's functionality in comments
4. Add your example to the workspace in examples/Cargo.toml
5. Update this README.md file with details about your example

## Example Details

### Chat System

A real-time distributed chat application that demonstrates actor-based communication. Users can join chat rooms and exchange messages in a distributed environment.

### Task Processor

A distributed task processing system that shows how to implement work distribution across multiple nodes. Features a coordinator that dispatches tasks and workers that process them.

### HTTP API

Integrates Actix actors with an HTTP API using actix-web. Shows how to build RESTful services backed by an actor system.

### File Sync

A file synchronization service that demonstrates how to use actors for distributed file operations and synchronization between nodes.

### Distributed Benchmark

A performance testing tool designed to benchmark the Actix-Pura distributed actor system under various loads and configurations.
