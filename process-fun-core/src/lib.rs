//! # process-fun-core
//!
//! Core functionality for the process-fun library. This crate provides the fundamental types
//! and functions needed to support out-of-process function execution.
//!
//! This crate is not meant to be used directly - instead, use the `process-fun` crate
//! which provides a more ergonomic API.

use interprocess::unnamed_pipe::{Recver, Sender};
use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::sys::signal::{self, Signal};
use nix::unistd::{fork, pipe2, ForkResult, Pid};
use serde::{Deserialize, Serialize};
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use std::{fmt, mem};
use thiserror::Error;

// Re-export specific items needed by generated code with clear namespacing
pub mod sys {
    pub use nix::sys::signal::{self, Signal};
    pub use nix::sys::wait::{waitpid, WaitStatus};
    pub use nix::unistd::{fork, getpid, ForkResult, Pid};
}

// Use a more efficient binary serialization format
pub mod ser {
    use bincode::{deserialize, serialize, Error};
    use serde::{Deserialize, Serialize};
    pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
        serialize(value)
    }

    pub fn from_slice<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, Error> {
        let val = deserialize(bytes)?;
        Ok(val)
    }
}

/// Wrapper for a process execution that allows awaiting or aborting the process
#[derive(Debug)]
pub struct ProcessWrapper<T> {
    child_pid: Pid,
    start_time: Option<SystemTime>,
    receiver: Option<Recver>,
    result: Arc<Mutex<Option<Vec<u8>>>>,
    _ghost: std::marker::PhantomData<T>,
}

impl<T> fmt::Display for ProcessWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Process(pid={})", self.child_pid)
    }
}

impl<T> ProcessWrapper<T>
where
    T: serde::de::DeserializeOwned,
{
    /// Create a new ProcessWrapper
    pub fn new(child_pid: Pid, receiver: Recver) -> Self {
        Self {
            child_pid,
            start_time: None,
            receiver: Some(receiver),
            result: Arc::new(Mutex::new(None)),
            _ghost: std::marker::PhantomData,
        }
    }

    /// Wait for the process to complete and return its result
    pub fn wait(&mut self) -> Result<T, ProcessFunError> {
        // Ensure we have the start time for process validation
        self.ensure_start_time()?;

        // Check if we already have a result
        if let Some(bytes) = self.result.lock().unwrap().take() {
            return ser::from_slice(&bytes).map_err(ProcessFunError::from);
        }

        // Read result from pipe
        let receiver = self.receiver.take().ok_or_else(|| {
            ProcessFunError::ProcessError("Process already completed".to_string())
        })?;

        let mut receiver = receiver;
        let result_bytes = read_from_pipe(&mut receiver)?;
        let result: T = ser::from_slice(&result_bytes)?;

        Ok(result)
    }

    /// Wait for the process to complete with a timeout
    pub fn timeout(&mut self, duration: Duration) -> Result<T, ProcessFunError> {
        // Ensure we have the start time for process validation
        self.ensure_start_time()?;

        // Take ownership of the receiver
        let receiver = self.receiver.take().ok_or_else(|| {
            ProcessFunError::ProcessError("Process already completed".to_string())
        })?;

        // Create a channel for the calculation thread to signal completion
        let (tx, rx) = mpsc::channel();

        // Spawn thread to read from pipe
        let result = self.result.clone();
        std::thread::spawn(move || {
            let mut receiver = receiver;
            if let Ok(bytes) = read_from_pipe(&mut receiver) {
                *result.lock().unwrap() = Some(bytes);
                let _ = tx.send(true); // Signal completion
            }
        });

        // Wait for either timeout or completion
        match rx.recv_timeout(duration) {
            Ok(_) => {
                // Process completed within timeout
                if let Some(bytes) = self.result.lock().unwrap().take() {
                    return ser::from_slice(&bytes).map_err(ProcessFunError::from);
                }
                // This shouldn't happen as we got a completion signal
                Err(ProcessFunError::ProcessError(
                    "Process result not found".to_string(),
                ))
            }
            Err(_) => {
                // Timeout occurred
                self.abort()?;
                Err(ProcessFunError::TimeoutError)
            }
        }
    }
}

#[inline]
pub fn stat_pid_start(pid: Pid) -> Result<SystemTime, ProcessFunError> {
    let proc_path = format!("/proc/{}/stat", pid.as_raw());
    nix::sys::stat::stat(proc_path.as_str())
        .map_err(|e| ProcessFunError::ProcessError(format!("Failed to stat process: {}", e)))
        .and_then(|stat| {
            SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(stat.st_ctime as u64))
                .ok_or_else(|| {
                    ProcessFunError::ProcessError(
                        "Failed to calculate process start time".to_string(),
                    )
                })
        })
}

impl<T> ProcessWrapper<T> {
    /// Lazily read the start time from pipe if not already read
    #[inline]
    fn ensure_start_time(&mut self) -> Result<(), ProcessFunError> {
        if self.start_time.is_some() {
            return Ok(());
        }

        if let Some(receiver) = &mut self.receiver {
            let start_time = read_start_time_from_pipe(receiver)?;
            self.start_time = Some(start_time);
            Ok(())
        } else {
            Err(ProcessFunError::ProcessError(
                "Process already completed".to_string(),
            ))
        }
    }

    /// Check if the process is still the same one we created
    #[inline]
    fn is_same_process(&mut self) -> bool {
        if self.ensure_start_time().is_err() {
            return false;
        }
        // Ensure we have the start time for validation
        if let Some(start_time) = self.start_time {
            stat_pid_start(self.child_pid)
                .map(|stat| stat == start_time)
                .unwrap_or(false)
        } else {
            false
        }
    }

    #[inline]
    fn kill(&mut self) -> Result<(), Errno> {
        // Only kill if it's the same process we created
        if self.is_same_process() {
            match signal::kill(self.child_pid, Signal::SIGKILL) {
                Ok(()) => Ok(()),
                Err(Errno::ESRCH) => Ok(()), // Process already exited
                Err(e) => Err(e),
            }
        } else {
            Ok(()) // Different process with same PID, consider it "already killed"
        }
    }

    /// Abort the process
    pub fn abort(&mut self) -> Result<(), ProcessFunError> {
        // Take ownership of the receiver to ensure it's dropped
        let _ = self.receiver.take();

        self.kill().map_err(|e| {
            ProcessFunError::ProcessError(format!("Failed to send SIGKILL to process: {}", e))
        })?;
        Ok(())
    }
}

impl<T> Drop for ProcessWrapper<T> {
    fn drop(&mut self) {
        // Take ownership of the receiver to ensure it's dropped
        let _ = self.receiver.take();

        // Attempt to kill the process if it's still running
        let _ = self.kill();
    }
}

/// Create a pipe for communication between parent and child processes
#[inline]
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

const SYSTEM_TIME_SIZE: usize = mem::size_of::<SystemTime>();

#[inline]
fn system_time_to_bytes_unsafe(time: SystemTime) -> [u8; SYSTEM_TIME_SIZE] {
    unsafe { mem::transmute::<SystemTime, [u8; SYSTEM_TIME_SIZE]>(time) }
}

#[inline]
fn bytes_to_system_time_unsafe(bytes: [u8; SYSTEM_TIME_SIZE]) -> SystemTime {
    unsafe { mem::transmute::<[u8; SYSTEM_TIME_SIZE], SystemTime>(bytes) }
}

/// Write time to pipe
#[inline]
pub fn write_time(fd: &mut Sender, time: SystemTime) -> Result<(), ProcessFunError> {
    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Writing start time to pipe");

    let time_bytes = system_time_to_bytes_unsafe(time);
    fd.write_all(&time_bytes)?;

    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Successfully wrote start time to pipe");

    Ok(())
}

/// Write data to a pipe and close it
#[inline]
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

/// Read start time from pipe
#[inline]
pub fn read_start_time_from_pipe(fd: &mut Recver) -> Result<SystemTime, ProcessFunError> {
    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Reading start time from pipe");

    let mut buffer = [0u8; SYSTEM_TIME_SIZE];
    fd.read_exact(&mut buffer)?;
    let start_time: SystemTime = bytes_to_system_time_unsafe(buffer);

    #[cfg(feature = "debug")]
    eprintln!("[process-fun-debug] Read start time from pipe");

    Ok(start_time)
}

/// Read data from a pipe
#[inline]
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
#[inline]
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
#[derive(Error, Debug, Serialize, Deserialize)]
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
    IoError(String),

    /// Failed to parse Rust source code
    #[error("Failed to parse Rust file: {0}")]
    ParseError(String),

    /// Error during process communication between parent and child processes
    #[error("Process communication error: {0}")]
    ProcessError(String),

    /// serialization/deserialization error for function arguments or results
    #[error("Failed to serialize or deserialize: {0}")]
    SerError(String),

    /// Process execution timed out
    #[error("Process execution timed out")]
    TimeoutError,
}

impl From<bincode::Error> for ProcessFunError {
    fn from(err: bincode::Error) -> Self {
        ProcessFunError::SerError(err.to_string())
    }
}

impl From<std::io::Error> for ProcessFunError {
    fn from(err: std::io::Error) -> Self {
        ProcessFunError::IoError(err.to_string())
    }
}

impl From<syn::Error> for ProcessFunError {
    fn from(err: syn::Error) -> Self {
        ProcessFunError::ParseError(err.to_string())
    }
}