# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`gamecode-tools` is a Rust library crate that provides JSONRPC-compatible tool functions for MCP (Model Context Protocol) applications. The tools are designed to work on macOS and Linux environments and are invoked via a Rust API asynchronously with Tokio.

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

3. **Format Transformers**: Format adapters for different API conventions:
   - `FormatTransformer`: Configurable transformer for different input/output formats
   - Supports standard JSONRPC format and AWS Bedrock format
   - Allows independent control of input and output formats
   - Handles recursive transformation of nested JSON structures

4. **Error Handling**: Uses standard Rust `Result<T, Error>` with custom `Error` enum:
   - No dependencies on `anyhow` or `thiserror`
   - Uses standard trait implementations

5. **Logging**: Uses the standard `log` crate:
   - Allows applications to choose their preferred logging implementation
   - Provides debug, info, warn, error, and trace levels
   - All tools use proper logging for consistent output

## Tool Implementations

Currently implemented tools:
- `directory_list`: Lists directory contents with filtering capabilities
- `directory_make`: Create directories
- `file_read`: Read file contents
- `file_write`: Write content to files
- `file_patch`: Apply patches to files
- `file_move`: Move or rename files
- `file_find`: Find files matching criteria
- `file_grep`: Search file contents
- `file_diff`: Compare files and generate diffs
- `shell`: Execute commands with security considerations

## Format Options

The library supports these format transformers:

1. **Standard**: Regular JSONRPC with direct scalar values
2. **Bedrock**: AWS Bedrock format with `{"type": "text", "text": value}` wrappers for all scalar values
3. **Mixed formats**: Allows independent control of input and output formats

Format options can be specified in several ways:

1. Using factory functions:
   - `create_default_dispatcher()`: Standard format for both input and output
   - `create_bedrock_dispatcher()`: Bedrock format for both input and output
   - `create_standard_to_bedrock_dispatcher()`: Standard input, Bedrock output
   - `create_bedrock_to_standard_dispatcher()`: Bedrock input, Standard output

2. Using custom configuration:
   ```rust
   let config = FormatConfig::new(InputFormat::Standard, OutputFormat::Bedrock);
   let transformer = FormatTransformer::new(config);
   let dispatcher = create_dispatcher_with_transformer(transformer);
   ```

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
cargo run --example format_options
cargo run --example logging_example

# Run examples with logging enabled
RUST_LOG=debug cargo run --example directory_list

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

When working with different formats:
1. Remember that format transformers handle both input and output formats
2. The Bedrock transformer recursively wraps all leaf values
3. Format detection is explicit, not automatic

When using logging:
1. Import logging macros from the `log` crate instead of using `println!`
2. Use appropriate log levels:
   - `trace!`: Extremely detailed information, rarely needed
   - `debug!`: Debugging information useful during development
   - `info!`: General information about program execution
   - `warn!`: Potentially harmful situations that might need attention
   - `error!`: Error conditions that should be addressed
3. Test code should use `debug!` for diagnostic information
4. Avoid holding locks across `.await` points in async code