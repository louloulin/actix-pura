# HTTP API Example

A RESTful API service backed by distributed Actix-Pura actors.

## Features

* RESTful HTTP endpoints
* Actor-based backend
* Distributed state management
* Health monitoring

## Running the Example

```bash
cargo run
```

Multiple nodes:

```bash
cargo run -- --multi --nodes 3
```

## Architecture

This example integrates actix-web with Actix-Pura to provide a RESTful API backed by a distributed actor system. The main components are:

1. **HTTP Server** - Provides REST endpoints
2. **API Actors** - Handle API requests
3. **Storage Actors** - Manage distributed state
