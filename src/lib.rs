//! JSONRPC-compatible tool functions for MCP applications.
//!
//! This library provides a collection of async tool functions with JSONRPC
//! compatible inputs and outputs for filesystem and search operations.

use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

pub mod jsonrpc;
pub mod logging;
pub mod schema;
pub mod tools;
pub mod transform;

// Re-export key types
pub use transform::{FormatConfig, FormatTransformer, InputFormat, OutputFormat};
pub use schema::{ToolSchema, ToolSchemaRegistry, BedrockToolSpec, generate_tool_schema, to_bedrock_tool_spec};

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

/// Factory function to create a dispatcher with schema registry
pub fn create_dispatcher_with_schema_registry(
    transformer: transform::FormatTransformer,
) -> (jsonrpc::Dispatcher, schema::ToolSchemaRegistry) {
    let mut registry = schema::ToolSchemaRegistry::new();
    let dispatcher = create_dispatcher_with_transformer_and_registry(transformer, &mut registry);
    (dispatcher, registry)
}

/// Factory function to create a Bedrock dispatcher with schema registry  
pub fn create_bedrock_dispatcher_with_schemas() -> (jsonrpc::Dispatcher, schema::ToolSchemaRegistry) {
    create_dispatcher_with_schema_registry(transform::bedrock_to_standard_transformer())
}

/// Factory function to create a dispatcher with a custom transformer and schema registry
fn create_dispatcher_with_transformer_and_registry(
    transformer: transform::FormatTransformer,
    registry: &mut schema::ToolSchemaRegistry,
) -> jsonrpc::Dispatcher {
    use tools::Tool;

    let mut dispatcher = jsonrpc::Dispatcher::with_transformer(Arc::new(transformer));

    // Register directory_list tool
    let dir_list_tool = tools::directory_list::DirectoryList;
    registry.register::<tools::directory_list::Params>("directory_list", "List contents of a directory");
    dispatcher.register(
        "directory_list",
        move |params: tools::directory_list::Params| async move {
            dir_list_tool.execute(params).await
        },
    );

    // Register file_read tool
    let file_read_tool = tools::file_read::FileRead;
    registry.register::<tools::file_read::Params>("file_read", "Read a file from the filesystem");
    dispatcher.register(
        "file_read",
        move |params: tools::file_read::Params| async move { file_read_tool.execute(params).await },
    );

    // Register file_write tool
    let file_write_tool = tools::file_write::FileWrite;
    registry.register::<tools::file_write::Params>("file_write", "Write content to a file");
    dispatcher.register(
        "file_write",
        move |params: tools::file_write::Params| async move {
            file_write_tool.execute(params).await
        },
    );

    // Register file_patch tool
    let file_patch_tool = tools::file_patch::FilePatch;
    registry.register::<tools::file_patch::Params>("file_patch", "Apply a patch to a file");
    dispatcher.register(
        "file_patch",
        move |params: tools::file_patch::Params| async move {
            file_patch_tool.execute(params).await
        },
    );

    // Register directory_make tool
    let dir_make_tool = tools::directory_make::DirectoryMake;
    registry.register::<tools::directory_make::Params>("directory_make", "Create a directory");
    dispatcher.register(
        "directory_make",
        move |params: tools::directory_make::Params| async move {
            dir_make_tool.execute(params).await
        },
    );

    // Register file_move tool
    let file_move_tool = tools::file_move::FileMove;
    registry.register::<tools::file_move::Params>("file_move", "Move or rename a file");
    dispatcher.register(
        "file_move",
        move |params: tools::file_move::Params| async move { file_move_tool.execute(params).await },
    );

    // Register file_find tool
    let file_find_tool = tools::file_find::FileFind;
    registry.register::<tools::file_find::Params>("file_find", "Find files matching a pattern");
    dispatcher.register(
        "file_find",
        move |params: tools::file_find::Params| async move { file_find_tool.execute(params).await },
    );

    // Register file_grep tool
    let file_grep_tool = tools::file_grep::FileGrep;
    registry.register::<tools::file_grep::Params>("file_grep", "Search file contents for a pattern");
    dispatcher.register(
        "file_grep",
        move |params: tools::file_grep::Params| async move { file_grep_tool.execute(params).await },
    );

    // Register file_diff tool
    let file_diff_tool = tools::file_diff::FileDiff;
    registry.register::<tools::file_diff::Params>("file_diff", "Compare two files");
    dispatcher.register(
        "file_diff",
        move |params: tools::file_diff::Params| async move { file_diff_tool.execute(params).await },
    );

    // Register shell tool
    let shell_tool = tools::shell::Shell;
    registry.register::<tools::shell::Params>("shell", "Execute a shell command");
    dispatcher.register("shell", move |params: tools::shell::Params| async move {
        shell_tool.execute(params).await
    });

    dispatcher
}

/// Factory function to create a dispatcher with a custom transformer
pub fn create_dispatcher_with_transformer(
    transformer: transform::FormatTransformer,
) -> jsonrpc::Dispatcher {
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
        move |params: tools::file_read::Params| async move { file_read_tool.execute(params).await },
    );

    // Register file_write tool
    let file_write_tool = tools::file_write::FileWrite;
    dispatcher.register(
        "file_write",
        move |params: tools::file_write::Params| async move {
            file_write_tool.execute(params).await
        },
    );

    // Register file_patch tool
    let file_patch_tool = tools::file_patch::FilePatch;
    dispatcher.register(
        "file_patch",
        move |params: tools::file_patch::Params| async move {
            file_patch_tool.execute(params).await
        },
    );

    // Register directory_make tool
    let dir_make_tool = tools::directory_make::DirectoryMake;
    dispatcher.register(
        "directory_make",
        move |params: tools::directory_make::Params| async move {
            dir_make_tool.execute(params).await
        },
    );

    // Register file_move tool
    let file_move_tool = tools::file_move::FileMove;
    dispatcher.register(
        "file_move",
        move |params: tools::file_move::Params| async move { file_move_tool.execute(params).await },
    );

    // Register file_find tool
    let file_find_tool = tools::file_find::FileFind;
    dispatcher.register(
        "file_find",
        move |params: tools::file_find::Params| async move { file_find_tool.execute(params).await },
    );

    // Register file_grep tool
    let file_grep_tool = tools::file_grep::FileGrep;
    dispatcher.register(
        "file_grep",
        move |params: tools::file_grep::Params| async move { file_grep_tool.execute(params).await },
    );

    // Register file_diff tool
    let file_diff_tool = tools::file_diff::FileDiff;
    dispatcher.register(
        "file_diff",
        move |params: tools::file_diff::Params| async move { file_diff_tool.execute(params).await },
    );

    // Register shell tool
    let shell_tool = tools::shell::Shell;
    dispatcher.register("shell", move |params: tools::shell::Params| async move {
        shell_tool.execute(params).await
    });

    dispatcher
}
