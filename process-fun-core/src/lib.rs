//! # process-fun-core
//!
//! Core functionality for the process-fun library. This crate provides the fundamental types
//! and functions needed to support out-of-process function execution.
//!
//! This crate is not meant to be used directly - instead, use the `process-fun` crate
//! which provides a more ergonomic API.

use interprocess::unnamed_pipe::{Recver, Sender};
use nix::fcntl::OFlag;
use nix::unistd::{fork, pipe2, ForkResult};
use std::io::prelude::*;
use std::path::PathBuf;
use thiserror::Error;

/// Create a pipe for communication between parent and child processes
pub fn create_pipes() -> Result<(Recver, Sender), ProcessFunError> {
    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Creating communication pipes");

    // Create pipe with O_CLOEXEC flag
    let (read_fd, write_fd) = pipe2(OFlag::O_CLOEXEC)
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to create pipe: {}", e)))?;

    // Convert raw file descriptors to Sender/Recver
    let recver = Recver::from(read_fd);
    let sender = Sender::from(write_fd);

    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Pipes created successfully");

    Ok((recver, sender))
}

/// Write data to a pipe and close it
pub fn write_to_pipe(mut fd: Sender, data: &[u8]) -> Result<(), ProcessFunError> {
    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Writing {} bytes to pipe", data.len());

    fd.write_all(data)
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to write to pipe: {}", e)))?;
    
    // Let the pipe be automatically flushed and closed when dropped
    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Successfully wrote data to pipe");

    Ok(())
}

/// Read data from a pipe
pub fn read_from_pipe(fd: &mut Recver) -> Result<Vec<u8>, ProcessFunError> {
    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Starting to read from pipe");

    let mut buffer = vec![];
    #[allow(unused_variables)]
    let bytes_read = fd
        .read_to_end(&mut buffer)
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to read from pipe: {}", e)))?;

    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Read {} bytes from pipe", bytes_read);

    Ok(buffer)
}

/// Fork the current process and return ForkResult
pub fn fork_process() -> Result<ForkResult, ProcessFunError> {
    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Forking process");

    let result = unsafe {
        fork().map_err(|e| ProcessFunError::ProcessError(format!("Failed to fork process: {}", e)))
    };

    #[cfg(feature = "debug")]
    if let Ok(fork_result) = &result {
        match fork_result {
            ForkResult::Parent { child } => {
                eprintln!(
                    "[process-fun-debug] Fork successful - parent process, child pid: {}",
                    child
                );
            }
            ForkResult::Child => {
                eprintln!("[process-fun-debug] Fork successful - child process");
            }
        }
    }

    result
}

/// Type alias for function identifiers, represented as filesystem paths
pub type FunId = PathBuf;

/// Errors that can occur during process-fun operations
#[derive(Error, Debug)]
pub enum ProcessFunError {
    /// Multiple #[process] attributes were found on a single function.
    /// Only one #[process] attribute is allowed per function.
    #[error("Multiple #[process] attributes found for function '{fun}'")]
    MultipleTags { fun: FunId },

    /// The #[process] attribute was used on an invalid item type.
    /// It can only be used on function definitions.
    #[error("Expected #[process] attribute only on function with implementation but found '{item_text}'")]
    BadItemType { item_text: String },

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
