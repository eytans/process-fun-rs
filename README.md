# process-fun

A Rust library for easily running functions in separate processes with seamless serialization.

## Features

- Simple `#[process]` attribute macro to mark functions for process execution
- Automatic serialization of arguments and return values
- Type-safe process communication
- Async support out of the box

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
process-fun = "0.1"
```

### Basic Example

```rust
use process_fun::process;
use serde::{Serialize, Deserialize};

// Mark your data types with Serialize and Deserialize
#[derive(Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

// Mark the function you want to run in a separate process
#[process]
fn calculate_distance(p1: Point, p2: Point) -> f64 {
    let dx = (p2.x - p1.x) as f64;
    let dy = (p2.y - p1.y) as f64;
    (dx * dx + dy * dy).sqrt()
}

#[tokio::main]
async fn main() {
    // Initialize process-fun
    process_fun::init_process_fun!();

    // Create some test points
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { x: 3, y: 4 };

    // Call the async version of the function
    let distance = calculate_distance_async(p1, p2).await.unwrap();
    println!("Distance: {}", distance); // Output: Distance: 5.0
}
```

### How it Works

1. The `#[process]` attribute generates an async version of your function with `_async` suffix
2. When you call the async version, it:
   - Serializes the arguments
   - Spawns a new process with the current executable
   - Passes the function identifier and serialized arguments
   - Deserializes the result

### Important Notes

- All types used in process functions must implement `Serialize` and `Deserialize`
- The `init_process_fun!()` macro must be called at the start of your main function
- Functions marked with `#[process]` cannot use references or complex types that don't implement `Serialize`/`Deserialize`

## Advanced Usage

### Error Handling

The async versions of process functions return `Result<T, std::io::Error>`:

```rust
#[process]
fn might_fail(x: i32) -> Result<i32, String> {
    if x < 0 {
        Err("negative numbers not allowed".to_string())
    } else {
        Ok(x * 2)
    }
}

async fn example() {
    match might_fail_async(-1).await {
        Ok(result) => println!("Success: {}", result),
        Err(e) => println!("Failed: {}", e),
    }
}
```

### Custom Types

Make sure your custom types implement the necessary traits:

```rust
#[derive(Serialize, Deserialize)]
struct ComplexData {
    values: Vec<f64>,
    metadata: HashMap<String, String>,
}

#[process]
fn process_data(data: ComplexData) -> ComplexData {
    // Process the data in a separate process
    // ...
}
```

## License

MIT License
