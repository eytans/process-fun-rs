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
//! 1. A new process is forked from the current process
//! 2. A ProcessWrapper is returned which allows:
//!    - Waiting for completion with optional timeout
//!    - Automatic process cleanup on timeout or drop
//!    - Safe result deserialization
//!
//! This execution model ensures complete isolation between the parent and child processes,
//! making it suitable for running potentially risky or resource-intensive operations.
//!
//! ## Usage
//!
//! ```rust
//! use process_fun::process;
//! use serde::{Serialize, Deserialize};
//! use std::time::Duration;
//!
//! #[derive(Serialize, Deserialize, Debug, Clone)]
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
//!     let p1 = Point { x: 1, y: 2 };
//!     let p2 = Point { x: 3, y: 4 };
//!     
//!     // Use original function (in-process)
//!     let result1 = add_points(p1.clone(), p2.clone());
//!     
//!     // Use process version with timeout (out-of-process)
//!     let mut process = add_points_process(p1, p2).unwrap();
//!     let result2 = process.timeout(Duration::from_secs(5)).unwrap();
//!     
//!     assert_eq!(result1.x, result2.x);
//!     assert_eq!(result1.y, result2.y);
//! }
//! ```
//!
//! ## Timeout Example
//!
//! ```rust
//! use process_fun::process;
//! use std::time::Duration;
//! use std::thread;
//!
//! #[process]
//! fn long_task() -> i32 {
//!     thread::sleep(Duration::from_secs(10));
//!     42
//! }
//!
//! fn main() {
//!     let mut process = long_task_process().unwrap();
//!     
//!     // Process will be killed if it exceeds timeout
//!     match process.timeout(Duration::from_secs(1)) {
//!         Ok(result) => println!("Task completed: {}", result),
//!         Err(e) => println!("Task timed out: {}", e)
//!     }
//! }
//! ```

#[allow(unused)]
use serde::{Deserialize, Serialize};

pub use process_fun_macro::process;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Counter {
        value: i32,
    }

    impl Counter {
        pub fn new(initial: i32) -> Self {
            Self { value: initial }
        }

        #[process]
        pub fn get_value(&self) -> i32 {
            self.value
        }
    }

    #[test]
    fn test_self_methods() {
        let counter = Counter::new(5);

        // Test immutable reference
        let result = counter.get_value_process().unwrap().wait().unwrap();
        assert_eq!(result, 5);
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

        let result = add_points_process(p1, p2).unwrap().wait().unwrap();
        assert_eq!(result.x, 4);
        assert_eq!(result.y, 6);
    }

    #[process]
    fn panicking_function() -> i32 {
        panic!("This function panics!");
    }

    #[test]
    fn test_process_panic() {
        let result = panicking_function_process().unwrap().wait();
        assert!(result.is_err(), "Expected error due to panic");
    }

    #[process]
    fn slow_but_within_timeout() -> i32 {
        thread::sleep(Duration::from_millis(500));
        42
    }

    #[test]
    fn test_timeout_success() {
        let mut process = slow_but_within_timeout_process().unwrap();
        let result = process.timeout(Duration::from_secs(1));
        assert!(result.is_ok(), "{}", result.unwrap_err().to_string());
        assert_eq!(result.unwrap(), 42);
    }

    #[process]
    fn write_file_slow() -> bool {
        // Try to write to a file after sleeping
        thread::sleep(Duration::from_secs(2));
        fs::write("test_timeout.txt", "This should not be written").unwrap();
        true
    }

    #[test]
    fn test_timeout_kill() {
        // Clean up any existing file
        let _ = fs::remove_file("test_timeout.txt");

        let mut process = write_file_slow_process().unwrap();
        let result = process.timeout(Duration::from_millis(500));

        // Should timeout
        assert!(result.is_err());

        // Give a small grace period for the filesystem
        thread::sleep(Duration::from_secs(2));

        // File should not exist since process was killed
        assert!(
            !std::path::Path::new("test_timeout.txt").exists(),
            "Process wasn't killed in time - file was created"
        );

        // Clean up
        let _ = fs::remove_file("test_timeout.txt");
    }

    #[process]
    fn long_calculation(iterations: u64) -> u64 {
        let mut sum: u64 = 0;
        for i in 0..iterations {
            sum = sum.wrapping_add(i);
            if i % 1000 == 0 {
                // Small sleep to make it actually take some time
                thread::sleep(Duration::from_micros(1));
            }
        }
        sum
    }

    #[test]
    fn test_long_calculation() {
        let iterations = 1_000_000;
        let mut process = long_calculation_process(iterations).unwrap();
        let start_time = std::time::Instant::now();
        let result = process.timeout(Duration::from_secs(5));
        let elapsed = start_time.elapsed();
        assert!(
            result.is_ok(),
            "Long calculation should complete within timeout"
        );
        assert!(
            elapsed < Duration::from_secs(3),
            "Long calculation should complete within timeout and return early"
        );
        // Verify the result matches in-process calculation
        let expected = long_calculation(iterations);
        assert_eq!(result.unwrap(), expected);
    }
}
