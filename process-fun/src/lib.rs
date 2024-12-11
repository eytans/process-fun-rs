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
//! 2. A new process is spawned using the current executable
//! 3. A unique hash identifying the function is passed to the child process
//! 4. The child process:
//!    - Deserializes the arguments
//!    - Executes the original function
//!    - Serializes and returns the result
//! 5. The parent process deserializes and returns the result
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

use interprocess::unnamed_pipe::{Recver, Sender};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub use process_fun_core::generate_unique_hash;
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
#[macro_export]
macro_rules! init_process_fun {
    () => {
        if let Ok(hash) = std::env::var(process_fun_core::ENV_FUNCTION_HASH) {
            #[cfg(feature = "debug")]
            eprintln!("[process-fun] Found function hash: {}", hash);

            let arg_handle = std::env::var(process_fun_core::ENV_ARG_FD)
                .ok()
                .and_then(|s| s.parse().ok());
            let result_handle = std::env::var(process_fun_core::ENV_RESULT_FD)
                .ok()
                .and_then(|s| s.parse().ok());

            #[cfg(feature = "debug")]
            eprintln!(
                "[process-fun] Handles - arg: {:?}, result: {:?}",
                arg_handle, result_handle
            );

            if let (Some(arg_handle), Some(result_handle)) = (arg_handle, result_handle) {
                // Create pipe handles using platform-specific reconstruction
                let mut arg_pipe = unsafe { process_fun_core::recver_from_handle(arg_handle) };
                let mut result_pipe =
                    unsafe { process_fun_core::sender_from_handle(result_handle) };

                // Find and execute the function with matching hash
                let mut found = false;
                for func in inventory::iter::<process_fun_core::ProcessFunction> {
                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun] Checking function with hash: {}", **func.hash);

                    if **func.hash == hash {
                        found = true;
                        #[cfg(feature = "debug")]
                        eprintln!("[process-fun] Found matching function, executing...");

                        // Read arguments from pipe
                        let mut args_buf = Vec::new();
                        if let Ok(_size) = arg_pipe.read_to_end(&mut args_buf) {
                            #[cfg(feature = "debug")]
                            eprintln!("[process-fun] Read {} bytes from pipe", _size);
                            let args_json = String::from_utf8_lossy(&args_buf);
                            if let Some(result) = (func.handler)(args_json.to_string()) {
                                // Write result to pipe
                                process_fun_core::write_to_pipe(result_pipe, result.as_bytes())
                                    .unwrap();
                                #[cfg(feature = "debug")]
                                eprintln!("[process-fun] Function executed successfully");
                                std::process::exit(0);
                            }
                        }
                        #[cfg(feature = "debug")]
                        eprintln!(
                            "[process-fun] Found function hash but did not succeed: {}",
                            hash
                        );

                        std::process::exit(1)
                    }
                }

                // If we got here and found is false, no matching function was found
                if !found {
                    eprintln!("Error: No function found matching hash {}", hash);
                    #[cfg(feature = "debug")]
                    eprintln!("[process-fun] Available function hashes:");
                    #[cfg(feature = "debug")]
                    for func in inventory::iter::<process_fun_core::ProcessFunction> {
                        eprintln!("[process-fun]   - {}", **func.hash);
                    }
                    std::process::exit(1);
                }
            }
        }
        #[cfg(feature = "debug")]
        eprintln!("[process-fun] No function hash found, continuing normal execution");
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    use process_fun_core::ProcessFunError;
    pub fn add_points(p1: Point, p2: Point) -> Point {
        Point {
            x: p1.x + p2.x,
            y: p1.y + p2.y,
        }
    }
    static ADD_POINTS_PROCESS_HASH: process_fun_core::once_cell::sync::Lazy<String> =
        process_fun_core::once_cell::sync::Lazy::new(|| {
            process_fun_core::generate_unique_hash(
                "add_points",
                &quote::quote!((p1, p2)).to_string(),
                &quote::quote!(Point).to_string(),
            )
        });
    #[allow(non_snake_case)]
    pub fn add_points_process(p1: Point, p2: Point) -> Result<Point, ProcessFunError> {
        use serde_json;
        use std::process::Command;
        let (arg_read, arg_write, mut result_read, result_write) =
            process_fun_core::create_pipes()?;
        let args_tuple = (p1, p2);
        let args_json = serde_json::to_string(&args_tuple)?;
        let current_exe = std::env::current_exe()?;
        #[cfg(feature = "debug")]
        {
            eprintln!("[process-fun-debug] Processing function: {}", "add_points");
            eprintln!(
                "[process-fun-debug] Generated hash: {}",
                *ADD_POINTS_PROCESS_HASH
            );
            eprintln!(
                "[process-fun-debug] Arguments tuple type: {}",
                stringify!((Point, Point))
            );
            eprintln!("[process-fun-debug] Serialized arguments: {}", args_json);
            eprintln!("[process-fun-debug] Current executable: {:?}", current_exe);
        }
        let hashp = process_fun_core::generate_unique_hash(
            "add_points",
            &quote::quote!((p1, p2)).to_string(),
            &quote::quote!(Point).to_string(),
        );
        #[cfg(feature = "debug")]
        eprintln!("[process-fun-debug] Generated hash for process: {}", hashp);
        process_fun_core::write_to_pipe(arg_write, args_json.as_bytes())?;
        let arg_handle = process_fun_core::get_pipe_handle(&arg_read);
        let result_handle = process_fun_core::get_pipe_handle(&result_write);
        let mut child = Command::new(current_exe);
        child
            .env(process_fun_core::ENV_FUNCTION_HASH, &hashp)
            .env(process_fun_core::ENV_ARG_FD, arg_handle)
            .env(process_fun_core::ENV_RESULT_FD, result_handle);
        #[cfg(feature = "test-debug")]
        {
            child.arg("--nocapture");
        }
        let mut child = child.spawn()?;
        drop(arg_read);
        drop(result_write);
        #[cfg(feature = "debug")]
        eprintln!("[process-fun-debug] Child process spawned. Waiting for completion...");
        let status = child.wait()?;
        if !status.success() {
            return Err(ProcessFunError::ProcessError(
                "Child process failed".to_string(),
            ));
        }
        #[cfg(feature = "debug")]
        eprintln!("[process-fun-debug] Child process completed. Reading from pipe...");
        let result_bytes = process_fun_core::read_from_pipe(&mut result_read)?;
        drop(result_read);
        let result: Point = serde_json::from_slice(&result_bytes)?;
        #[cfg(feature = "debug")]
        {
            eprintln!("[process-fun-debug] Deserialized result: {:?}", result);
        }
        Ok(result)
    }
    inventory::submit! {
        process_fun_core :: ProcessFunction
        {
            name : "add_points", hash : & ADD_POINTS_PROCESS_HASH, handler : |
            args_json : String | -> Option < String >
            {
                #[cfg(feature = "debug")]
                {
                    eprintln!
                    ("[process-fun-debug] Handler called for function: {}",
                    "add_points"); eprintln!
                    ("[process-fun-debug] Received args_json: {}", args_json);
                } let args : (Point, Point) = serde_json ::
                from_str(& args_json).ok() ? ; let (p1, p2) = args; let result =
                add_points(p1, p2); #[cfg(feature = "debug")]
                {
                    eprintln!
                    ("[process-fun-debug] Handler result: {:?}", result);
                } serde_json :: to_string(& result).ok()
            }
        }
    }

    #[test]
    fn test_process_function() {
        init_process_fun!();
        let p1 = Point { x: 1, y: 2 };
        let p2 = Point { x: 3, y: 4 };

        let result = add_points_process(p1, p2).unwrap();
        assert_eq!(result.x, 4);
        assert_eq!(result.y, 6);
    }
}
