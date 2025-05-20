//! JSONRPC-compatible tool functions for MCP applications.
//!
//! This library provides a collection of async tool functions with JSONRPC
//! compatible inputs and outputs for filesystem and search operations.

use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

pub mod tools;
pub mod jsonrpc;
pub mod transform;

// Re-export key types
pub use transform::{FormatConfig, FormatTransformer, InputFormat, OutputFormat};

/// Custom error type for the library
#[derive(Debug)]
pub enum Error {
    /// Input/output error
    Io(std::io::Error),
    /// JSON serialization/deserialization error
    Json(serde_json::Error),
    /// Invalid parameter error
    InvalidParam(String),
    /// Operation not permitted error
    PermissionDenied(String),
    /// General error
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::Json(err) => write!(f, "JSON error: {}", err),
            Error::InvalidParam(msg) => write!(f, "Invalid parameter: {}", msg),
            Error::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Error::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::Json(err) => Some(err),
            Error::InvalidParam(_) => None,
            Error::PermissionDenied(_) => None,
            Error::Other(_) => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

/// Type alias for library results
pub type Result<T> = std::result::Result<T, Error>;

/// Factory function to create a dispatcher with all available tools registered
pub fn create_default_dispatcher() -> jsonrpc::Dispatcher {
    create_dispatcher_with_transformer(transform::standard_transformer())
}

/// Factory function to create a dispatcher with all available tools registered using the AWS Bedrock transformer
pub fn create_bedrock_dispatcher() -> jsonrpc::Dispatcher {
    create_dispatcher_with_transformer(transform::bedrock_transformer())
}

/// Factory function for a dispatcher that accepts standard input but produces Bedrock output
pub fn create_standard_to_bedrock_dispatcher() -> jsonrpc::Dispatcher {
    create_dispatcher_with_transformer(transform::standard_to_bedrock_transformer())
}

/// Factory function for a dispatcher that accepts Bedrock input but produces standard output
pub fn create_bedrock_to_standard_dispatcher() -> jsonrpc::Dispatcher {
    create_dispatcher_with_transformer(transform::bedrock_to_standard_transformer())
}

/// Factory function to create a dispatcher with a custom transformer
pub fn create_dispatcher_with_transformer(transformer: transform::FormatTransformer) -> jsonrpc::Dispatcher {
    use tools::Tool;
    
    let mut dispatcher = jsonrpc::Dispatcher::with_transformer(Arc::new(transformer));
    
    // Register directory_list tool
    let dir_list_tool = tools::directory_list::DirectoryList;
    dispatcher.register(
        "directory_list",
        move |params: tools::directory_list::Params| async move {
            dir_list_tool.execute(params).await
        },
    );
    
    // Register file_read tool
    let file_read_tool = tools::file_read::FileRead;
    dispatcher.register(
        "file_read",
        move |params: tools::file_read::Params| async move {
            file_read_tool.execute(params).await
        },
    );
    
    // Register file_write tool
    let file_write_tool = tools::file_write::FileWrite;
    dispatcher.register(
        "file_write",
        move |params: tools::file_write::Params| async move {
            file_write_tool.execute(params).await
        },
    );
    
    dispatcher
}