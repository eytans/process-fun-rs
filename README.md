# process-fun-rs

A Rust library for easily running functions in separate processes with minimal boilerplate.

Docs are AI generated, thanks Cline! (Please do open issue if you find any mistakes)

## Overview

`process-fun-rs` provides a simple macro-based approach to execute Rust functions in separate processes. It handles all the complexity of process spawning, argument serialization, and result communication, allowing you to focus on your business logic.

## Features

- Simple `#[process]` attribute macro for marking functions to create an additional version that runs in separate processes
- Automatic serialization/deserialization of function arguments and return values
- Type-safe process communication
- Error handling with custom error types
- Debug mode for troubleshooting process execution

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
process-fun = "0.1.0"
```

Basic example:

```rust
use process_fun::process;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct Point {
    x: i32,
    y: i32,
}

#[process]
pub fn add_points(p1: Point, p2: Point) -> Point {
    Point {
        x: p1.x + p2.x,
        y: p1.y + p2.y,
    }
}

fn main() {
    // Initialize the process-fun runtime
    process_fun::init_process_fun!();
    
    let p1 = Point { x: 1, y: 2 };
    let p2 = Point { x: 3, y: 4 };
    
    // This will execute in a separate process
    let result = add_points_process(p1, p2).unwrap();
    assert_eq!(result.x, 4);
    assert_eq!(result.y, 6);
}
```

## How It Works

1. The `#[process]` attribute macro generates a wrapper function with `_process` suffix
2. When called, the wrapper function:
   - Serializes the arguments to JSON
   - Spawns a new process with the current executable
   - Passes a unique function hash and serialized arguments
   - Deserializes and returns the result

## Crate Structure

- `process-fun`: Main crate providing the public API
- `process-fun-core`: Core functionality and types
- `process-fun-macro`: Implementation of the `#[process]` attribute macro

## Requirements

- Functions marked with `#[process]` must:
  - Be public (`pub`)
  - Have arguments and return types that implement `Serialize` and `Deserialize`
  - Not take `self` parameters

## License

This project is licensed under the Apache 2 License - see the [LICENSE](LICENSE) file for details.
