# File Sync Example

A distributed file synchronization service built with Actix-Pura.

## Features

* File system monitoring
* Peer-to-peer synchronization
* Change detection
* Conflict resolution

## Running the Example

```bash
cargo run
```

Multiple nodes:

```bash
cargo run -- --multi --nodes 3
```

## Architecture

This example demonstrates how Actix-Pura can be used to build a distributed file synchronization system. The main components are:

1. **FileWatcherActor** - Monitors the local filesystem for changes
2. **SyncManagerActor** - Coordinates synchronization between nodes
3. **FileTransferActor** - Handles file transfers between nodes
