//! # process-fun-core
//! 
//! Core functionality for the process-fun library. This crate provides the fundamental types
//! and functions needed to support out-of-process function execution.
//! 
//! This crate is not meant to be used directly - instead, use the `process-fun` crate
//! which provides a more ergonomic API.

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::path::PathBuf;
use thiserror::Error;
use std::io::prelude::*;
use interprocess::unnamed_pipe::{pipe, Sender, Recver};

#[cfg(unix)]
use {
    std::os::unix::io::{AsRawFd, FromRawFd, RawFd},
    nix::unistd::dup,
};

#[cfg(windows)]
use {
    std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle},
    windows_sys::Win32::Foundation::{HANDLE, DUPLICATE_HANDLE_OPTIONS, INVALID_HANDLE_VALUE},
    windows_sys::Win32::System::Threading::{GetCurrentProcess, DuplicateHandle},
};

pub use once_cell;

/// Environment variable for function hash
pub const ENV_FUNCTION_HASH: &str = "PROCESS_FUN_HASH";

/// Environment variable for argument pipe file descriptor
pub const ENV_ARG_FD: &str = "PROCESS_FUN_ARG_FD";

/// Environment variable for result pipe file descriptor
pub const ENV_RESULT_FD: &str = "PROCESS_FUN_RESULT_FD";

/// Create a pair of pipes for bidirectional communication
pub fn create_pipes() -> Result<(Recver, Sender, Recver, Sender), ProcessFunError> {
    // Create original pipes
    let (arg_write, arg_read) = pipe()
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to create arg pipe: {}", e)))?;
    
    let (result_write, result_read) = pipe()
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to create result pipe: {}", e)))?;

    // Create inheritable duplicates for the child process
    let arg_read = duplicate_for_child(arg_read)?;
    let result_write = duplicate_for_child(result_write)?;
    
    Ok((arg_read, arg_write, result_read, result_write))
}

#[cfg(unix)]
fn duplicate_for_child<T: AsRawFd + FromRawFd>(pipe: T) -> Result<T, ProcessFunError> {
    let fd = pipe.as_raw_fd();
    let new_fd = dup(fd)
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to dup handle: {}", e)))?;
    Ok(unsafe { T::from_raw_fd(new_fd) })
}

#[cfg(windows)]
fn duplicate_for_child<T: AsRawHandle + FromRawHandle>(pipe: T) -> Result<T, ProcessFunError> {
    let handle = pipe.as_raw_handle() as HANDLE;
    let current_process = unsafe { GetCurrentProcess() };
    let mut new_handle = 0;
    
    let success = unsafe {
        DuplicateHandle(
            current_process,
            handle,
            current_process,
            &mut new_handle,
            0,
            1, // bInheritHandle = TRUE
            DUPLICATE_HANDLE_OPTIONS(2), // DUPLICATE_SAME_ACCESS
        )
    };
    
    if success == 0 || new_handle == 0 {
        return Err(ProcessFunError::ProcessError("Failed to duplicate handle".into()));
    }
    
    Ok(unsafe { T::from_raw_handle(new_handle as RawHandle) })
}

/// Write data to a pipe and close it
pub fn write_to_pipe(mut fd: Sender, data: &[u8]) -> Result<(), ProcessFunError> {
    fd.write_all(data)
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to write to pipe: {}", e)))?;
    // Explicitly drop the sender to close the write end
    fd.flush()?;
    Ok(())
}

/// Read data from a pipe
pub fn read_from_pipe(fd: &mut Recver) -> Result<Vec<u8>, ProcessFunError> {
    let mut buffer = vec![];
    let _bytes_read = fd.read_to_end(&mut buffer)
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to read from pipe: {}", e)))?;
    Ok(buffer)
}

/// Platform-specific handle passing
#[cfg(unix)]
pub fn get_pipe_handle(pipe: &impl AsRawFd) -> String {
    pipe.as_raw_fd().to_string()
}

#[cfg(windows)]
pub fn get_pipe_handle(pipe: &impl AsRawHandle) -> String {
    let handle = pipe.as_raw_handle();
    // Convert to u64 to ensure consistent string representation
    (handle.0 as u64).to_string()
}

/// Platform-specific handle reconstruction
#[cfg(unix)]
pub unsafe fn recver_from_handle(handle: i32) -> Recver {
    Recver::from_raw_fd(handle)
}

#[cfg(windows)]
pub unsafe fn recver_from_handle(handle: u64) -> Recver {
    // Convert back from u64 to isize for Windows handle
    Recver::from_raw_handle(handle as isize)
}

#[cfg(unix)]
pub unsafe fn sender_from_handle(handle: i32) -> Sender {
    Sender::from_raw_fd(handle)
}

#[cfg(windows)]
pub unsafe fn sender_from_handle(handle: u64) -> Sender {
    // Convert back from u64 to isize for Windows handle
    Sender::from_raw_handle(handle as isize)
}

/// Check if we're running as a child process
pub fn is_child_process() -> bool {
    std::env::var(ENV_FUNCTION_HASH).is_ok() &&
    std::env::var(ENV_ARG_FD).is_ok() &&
    std::env::var(ENV_RESULT_FD).is_ok()
}

/// Get function hash and pipe file descriptors from environment if running as child process
pub fn get_child_process_info() -> Option<(String, Recver, Sender)> {
    let hash = std::env::var(ENV_FUNCTION_HASH).ok()?;
    
    #[cfg(unix)]
    let arg_fd: RawFd = std::env::var(ENV_ARG_FD).ok()?.parse().ok()?;
    #[cfg(unix)]
    let result_fd: RawFd = std::env::var(ENV_RESULT_FD).ok()?.parse().ok()?;
    
    #[cfg(windows)]
    let arg_fd: u64 = std::env::var(ENV_ARG_FD).ok()?.parse().ok()?;
    #[cfg(windows)]
    let result_fd: u64 = std::env::var(ENV_RESULT_FD).ok()?.parse().ok()?;
    
    unsafe {
        Some((
            hash,
            recver_from_handle(arg_fd),
            sender_from_handle(result_fd)
        ))
    }
}

/// Represents a function that can be executed in a separate process.
/// 
/// This struct is used internally by the process-fun runtime to track and execute
/// functions marked with the `#[process]` attribute. Each function gets two versions:
/// 
/// 1. The original function that runs in the current process
/// 2. A `_process` suffixed version that runs in a separate process
/// 
/// # Fields
/// 
/// * `name` - The name of the original function
/// * `hash` - A unique hash identifying the function, derived from its name, arguments, and return type
/// * `handler` - A function pointer that handles the actual execution in the child process,
///   including deserialization of arguments and serialization of the result
#[derive(Debug)]
pub struct ProcessFunction {
    pub name: &'static str,
    pub hash: &'static once_cell::sync::Lazy<String>,
    pub handler: fn(String) -> Option<String>,
}

inventory::collect!(ProcessFunction);

/// Generate a unique hash for a function based on its name, arguments, and return type.
/// 
/// This function is used internally by the process-fun runtime to generate unique identifiers
/// for functions that can be executed across process boundaries. The hash is used to match
/// function calls between parent and child processes.
/// 
/// # Arguments
/// 
/// * `fn_name` - The name of the function
/// * `fn_args` - A string representation of the function's arguments
/// * `fn_output` - A string representation of the function's return type
/// 
/// # Returns
/// 
/// A string containing the hexadecimal representation of the hash
/// 
/// # Implementation Details
/// 
/// The hash is generated by combining:
/// - The function name
/// - The types of all arguments
/// - The return type
/// 
/// This ensures that functions with the same name but different signatures get different hashes.
pub fn generate_unique_hash(fn_name: &str, fn_args: &str, fn_output: &str) -> String {
    let mut hasher = DefaultHasher::new();
    
    // Hash function name
    fn_name.hash(&mut hasher);
    
    // Hash argument types
    fn_args.hash(&mut hasher);
    
    // Hash return type
    fn_output.hash(&mut hasher);
    
    format!("{:x}", hasher.finish())
}

/// Type alias for function identifiers, represented as filesystem paths
pub type FunId = PathBuf;

/// Errors that can occur during process-fun operations
#[derive(Error, Debug)]
pub enum ProcessFunError {
    /// Multiple #[process] attributes were found on a single function.
    /// Only one #[process] attribute is allowed per function.
    #[error("Multiple #[process] attributes found for function '{fun}'")]
    MultipleTags {
        fun: FunId,
    },

    /// No paths were provided where required
    #[error("No paths provided")]
    NoPaths,

    /// Two paths in the configuration overlap
    #[error("Paths '{path1}' and '{path2}' overlap")]
    PathsOverlap {
        path1: FunId,
        path2: FunId,
    },

    /// The #[process] attribute was used on an invalid item type.
    /// It can only be used on function definitions.
    #[error("Expected #[process] attribute only on function with implementation but found '{item_text}'")]
    BadItemType {
        item_text: String
    },

    /// An I/O error occurred during process execution or file operations
    #[error("Failed to read or write file: {0}")]
    IoError(#[from] std::io::Error),

    /// Failed to parse Rust source code
    #[error("Failed to parse Rust file: {0}")]
    ParseError(#[from] syn::Error),

    /// Error during process communication between parent and child processes
    #[error("Process communication error: {0}")]
    ProcessError(String),

    /// JSON serialization/deserialization error for function arguments or results
    #[error("Failed to serialize or deserialize JSON: {0}")]
    JsonError(#[from] serde_json::Error),
}
