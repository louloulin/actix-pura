# Actor Registry Cache Implementation

## Overview

We've implemented a high-performance lookup cache for the distributed actor registry in the Actix cluster system. This cache significantly improves the performance of actor lookups, which is a critical operation in any distributed actor system.

## Key Features

1. **Configurable Cache**
   - TTL (Time-To-Live): Configure how long entries remain valid in the cache
   - Maximum Size: Limit cache growth to control memory usage
   - Enable/Disable: Flexibility to turn caching on or off

2. **Cache Management**
   - Automatic periodic cleaning to remove expired entries
   - LRU-like eviction when maximum size is reached
   - Automatic invalidation when actors are registered or deregistered

3. **Performance Metrics**
   - Cache hit/miss statistics
   - Cache size tracking
   - Performance monitoring

## Configuration

The cache is configured through the `CacheConfig` struct:

```rust
// Default configuration
let cache_config = CacheConfig::new()
    .ttl(60)         // 60 seconds TTL
    .max_size(1000)  // Maximum 1000 entries
    .enabled(true);  // Cache enabled

// Custom configuration
let custom_cache = CacheConfig::new()
    .ttl(5)          // Short 5 seconds TTL 
    .max_size(20)    // Small maximum size
    .enabled(true);
```

## Performance Benefits

Our tests demonstrate significant performance improvements:

| Test Scenario | Cold Lookup | Warm Lookup | Improvement |
|---------------|-------------|-------------|-------------|
| 10 local actors | 156.917µs | 21.875µs | 7.2x faster |

The cache provides the greatest benefit for frequently accessed actors, which is a common pattern in most actor-based applications.

## Implementation Details

1. **Cache Data Structure**
   - HashMap with actor path as key
   - Value contains the actor reference and timestamp for TTL

2. **Cache Operations**
   - Lookup: Check cache first, fallback to regular lookup
   - Registration: Invalidate cache entries
   - Deregistration: Remove from cache

3. **Thread Safety**
   - Uses RwLock for concurrent access
   - Read-biased for optimal lookup performance

4. **Integration with Cluster System**
   - Cache configuration is part of ClusterConfig
   - Automatically applied when creating the actor registry

## Example Usage

```rust
// In cluster configuration
let config = ClusterConfig::new()
    .architecture(Architecture::Decentralized)
    .node_role(NodeRole::Peer)
    .cache_config(
        CacheConfig::new()
            .ttl(30)          // 30 seconds TTL
            .max_size(500)    // Maximum 500 entries
    )
    .build()
    .expect("Failed to create config");

// The cache is automatically configured in the created actor registry
let mut system = ClusterSystem::new("test-node", config);

// Check cache statistics
let registry = system.registry();
let (cache_size, cache_hits, cache_misses) = registry.cache_stats();
println!("Cache size: {}, Hits: {}, Misses: {}", cache_size, cache_hits, cache_misses);

// Clear cache if needed
registry.clear_cache();
```

## Future Improvements

1. **Advanced Eviction Policies**
   - Implement proper LRU, LFU or ARC algorithms
   - Priority-based caching for critical actors

2. **Distributed Cache Consistency**
   - Cache invalidation messages between nodes
   - Consistency protocols for multi-node deployments

3. **Adaptive TTL**
   - Dynamic TTL based on access patterns
   - Different TTLs for different actor types 