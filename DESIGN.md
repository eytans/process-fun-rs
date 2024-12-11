# Technical Design Document

## Architecture Overview

process-fun-rs is designed as a multi-crate workspace to separate concerns and provide a clean API:

```
process-fun-rs/
├── process-fun/       # Main crate with public API
├── process-fun-core/  # Core types and functionality
└── process-fun-macro/ # Procedural macro implementation
```

## Core Concepts

### Function Transformation

The `#[process]` attribute macro transforms a function in two ways:
1. Keeps the original function unchanged for in-process calls
2. Creates a new `_process` suffixed function for out-of-process execution

Example:
```rust
#[process]
pub fn add(x: i32, y: i32) -> i32 {
    x + y
}

// Generates:
// 1. Original function (unchanged)
pub fn add(x: i32, y: i32) -> i32 {
    x + y
}

// 2. Process version
pub fn add_process(x: i32, y: i32) -> Result<i32, ProcessFunError> {
    // Process execution logic
}
```

### Process Communication

Communication between parent and child processes uses:
- JSON serialization for arguments and results
- Command-line arguments for function identification
- Standard output/error for result transmission

Flow:
1. Parent process:
   - Serializes function arguments to JSON
   - Generates unique function hash
   - Spawns child process with hash and JSON
2. Child process:
   - Receives hash and JSON via command-line
   - Looks up function by hash
   - Deserializes arguments
   - Executes function
   - Serializes and returns result
3. Parent process:
   - Deserializes result
   - Returns to caller

### Function Registration

Functions are registered at compile time using the `inventory` crate:
1. Each `#[process]` function gets a unique hash based on:
   - Function name
   - Argument types
   - Return type
2. The macro generates a static registration that maps:
   - Hash → Function handler
3. At runtime, the child process uses this registry to:
   - Look up the correct function by hash
   - Execute it with the provided arguments

## Error Handling

Comprehensive error handling covers:
- Process execution failures
- Serialization/deserialization errors
- Function lookup failures
- Runtime errors

All errors are wrapped in `ProcessFunError` enum for consistent error handling.

## Design Decisions

### Why JSON for Serialization?
- Human-readable for debugging
- Well-supported in Rust ecosystem
- Flexible schema evolution
- Good performance for most use cases

### Why Command-line Arguments?
- Simple and reliable IPC mechanism
- Works across all platforms
- Easy to debug and trace
- No need for complex socket/pipe handling

### Why Function Hashes?
- Unique identification across processes
- Compile-time generation prevents runtime errors
- Includes type information for safety
- Efficient lookup in child process

### Why Keep Original Function?
- Allows choice between in-process and out-of-process
- No overhead when isolation not needed
- Easy to switch between modes
- Better testing capabilities

## Future Considerations

Potential improvements and extensions:

1. Performance Optimizations
   - Binary serialization format option
   - Shared memory for large data
   - Process pooling

2. Feature Extensions
   - Async function support
   - Custom serialization formats
   - Process resource limits
   - Function timeouts

3. Developer Experience
   - Better error messages
   - More debugging tools
   - Configuration options
   - Process monitoring
