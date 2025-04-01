# Distributed Benchmark Example

A tool for benchmarking Actix-Pura's distributed actor system performance.

## Features

* Performance testing
* Multi-node measurements
* Configurable workload
* Result aggregation

## Running the Example

```bash
cargo run
```

Multiple nodes:

```bash
cargo run -- --multi --nodes 5
```

## Architecture

This example provides a framework for testing the performance of Actix-Pura in a distributed environment. The main components are:

1. **BenchmarkCoordinatorActor** - Manages the benchmark process
2. **WorkerActor** - Executes benchmark tasks
3. **MetricsCollectorActor** - Aggregates performance metrics
