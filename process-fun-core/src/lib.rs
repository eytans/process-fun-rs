use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::path::PathBuf;
use thiserror::Error;

pub use once_cell;

/// A function that can be executed in a separate process
#[derive(Debug)]
pub struct ProcessFunction {
    pub name: &'static str,
    pub hash: &'static once_cell::sync::Lazy<String>,
    pub handler: fn(String) -> Option<String>,
}

inventory::collect!(ProcessFunction);

/// Generate a unique hash for a function based on its name, arguments, and return type.
/// This is used to identify functions across process boundaries.
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


pub type FunId = PathBuf;

#[derive(Error, Debug)]
pub enum ProcessFunError {
    #[error("Multiple #[process] attributes found for function '{fun}'")]
    MultipleTags {
        fun: FunId,
    },
    #[error("No paths provided")]
    NoPaths,
    #[error("Paths '{path1}' and '{path2}' overlap")]
    PathsOverlap {
        path1: FunId,
        path2: FunId,
    },
    #[error("Expected #[process] attribute only on function with implementation but found '{item_text}'")]
    BadItemType {
        item_text: String
    },
    #[error("Failed to read or write file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to parse Rust file: {0}")]
    ParseError(#[from] syn::Error),
    #[error("Process communication error: {0}")]
    ProcessError(String),
    #[error("Failed to serialize or deserialize JSON: {0}")]
    JsonError(#[from] serde_json::Error),
}