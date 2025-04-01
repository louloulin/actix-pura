# Chat System Example
A real-time distributed chat application built with Actix-Pura.

## Features

* Multi-room chat with user management
* Distributed architecture
* Real-time messaging
* System notifications

## Running the Example

Start a single node:

```bash
cargo run
```

Start multiple nodes for testing:

```bash
cargo run -- --multi --nodes 3
```

## Architecture

This example consists of two main actor types:

1. **ChatServiceActor** - Manages rooms and message distribution
2. **UserActor** - Represents connected users

When running in multi-node mode, the system demonstrates how actors can communicate across node boundaries.
