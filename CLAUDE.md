# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`gamecode-tools` is a Rust library crate that provides JSONRPC-compatible tool functions for MCP (Model Context Protocol) applications. The tools are designed to work on macOS command line and are invoked via a Rust API asynchronously with Tokio.

## Core Architecture

The library is designed around these key components:

1. **Tool Trait**: The `Tool` trait defines the interface for all tools. Each tool implementation provides:
   - `name()`: Returns the tool name
   - `execute()`: Runs the tool with provided parameters
   - Each tool defines its own `Params` and `Output` types

2. **JSONRPC Dispatcher**: Handles JSONRPC protocol details, routing requests to the appropriate tool:
   - Parses incoming JSONRPC requests
   - Maps method names to registered tool handlers
   - Handles errors according to JSONRPC spec
   - Returns JSONRPC-formatted responses

3. **Response Transformers**: Format adapters for different API conventions:
   - `StandardTransformer`: Standard JSONRPC format
   - `BedrockTransformer`: AWS Bedrock format with `{"type": "text", "text": content}` wrappers
   - Custom transformers can be implemented by users

4. **Error Handling**: Uses standard Rust `Result<T, Error>` with custom `Error` enum:
   - No dependencies on `anyhow` or `thiserror`
   - Uses standard trait implementations

## Tool Implementations

Currently implemented tools:
- `directory_list`: Lists directory contents with filtering capabilities

Planned tools:
- `file_read`: Read file contents 
- `file_write`: Write content to a file
- `patch`: Apply patches to files
- `grep`: Search file contents
- `find`: Find files matching criteria
- `mkdir`: Create directories

## Development Commands

```bash
# Build the library
cargo build

# Run tests
cargo test

# Run a specific integration test
cargo test --test test_name

# Run a specific unit test
cargo test module::function_name

# Check formatting
cargo fmt --check

# Format code
cargo fmt

# Run linter
cargo clippy

# Generate documentation
cargo doc --open

# Run examples
cargo run --example directory_list
cargo run --example bedrock_transform

# Publish to crates.io (when ready)
cargo publish
```

## Testing Philosophy

- Unit tests for each tool in the same file as the implementation
- Test directories are created and cleaned up during tests
- Pattern matching and other utilities have dedicated tests
- Error handling is tested with appropriate cases

## Implementation Notes

When adding new tools:

1. Create a new file for the tool in `src/tools/`
2. Implement the `Tool` trait
3. Register the tool in `create_default_dispatcher()` in lib.rs
4. Add tests for the tool implementation
5. Add an example showing how to use the tool