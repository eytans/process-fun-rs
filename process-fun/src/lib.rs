//! # process-fun
//! 
//! A library for easily running Rust functions in separate processes with minimal boilerplate.
//! 
//! ## Overview
//! 
//! This crate provides a simple macro-based approach to execute Rust functions in separate processes.
//! The `#[process]` attribute macro creates an additional version of your function that runs in a
//! separate process, while keeping the original function unchanged. This allows you to choose between
//! in-process and out-of-process execution as needed.
//! 
//! ## Process Execution Model
//! 
//! When a function marked with `#[process]` is called through its `_process` variant:
//! 
//! 1. The arguments are serialized to JSON
//! 2. A new process is forked from the current process
//! 3. The child process:
//!    - Deserializes the arguments
//!    - Executes the original function
//!    - Serializes and returns the result
//! 4. The parent process deserializes and returns the result
//! 
//! This execution model ensures complete isolation between the parent and child processes,
//! making it suitable for running potentially risky or resource-intensive operations.
//! 
//! ## Usage
//! 
//! ```rust
//! use process_fun::process;
//! use serde::{Serialize, Deserialize};
//! 
//! #[derive(Serialize, Deserialize, Debug)]
//! struct Point {
//!     x: i32,
//!     y: i32,
//! }
//! 
//! #[process]
//! pub fn add_points(p1: Point, p2: Point) -> Point {
//!     Point {
//!         x: p1.x + p2.x,
//!         y: p1.y + p2.y,
//!     }
//! }
//! 
//! fn main() {
//!     // Initialize the process-fun runtime
//!     process_fun::init_process_fun!();
//!     
//!     let p1 = Point { x: 1, y: 2 };
//!     let p2 = Point { x: 3, y: 4 };
//!     
//!     // Use original function (in-process)
//!     let result1 = add_points(p1.clone(), p2.clone());
//!     
//!     // Use process version (out-of-process)
//!     let result2 = add_points_process(p1, p2).unwrap();
//!     
//!     assert_eq!(result1.x, result2.x);
//!     assert_eq!(result1.y, result2.y);
//! }
//! ```

use serde::{Serialize, Deserialize};

pub use process_fun_macro::process;

/// Initialize the process-fun runtime. This should be called at the start of your main function.
/// 
/// This macro sets up the necessary runtime environment for process-fun to work. It:
/// 1. Checks command-line arguments for special process hash arguments
/// 2. If a hash is present, it looks up and executes the corresponding function
/// 3. If no hash is present, it continues normal execution
/// 
/// # Example
/// 
/// ```rust
/// fn main() {
///     process_fun::init_process_fun!();
///     // Your normal application code here
/// }
/// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    #[process]
    pub fn add_points(p1: Point, p2: Point) -> Point {
        Point {
            x: p1.x + p2.x,
            y: p1.y + p2.y,
        }
    }

    #[test]
    fn test_process_function() {
        let p1 = Point { x: 1, y: 2 };
        let p2 = Point { x: 3, y: 4 };

        let result = add_points_process(p1, p2).unwrap();
        assert_eq!(result.x, 4);
        assert_eq!(result.y, 6);
    }
}
