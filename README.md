# gamecode-tools

A Rust library providing JSONRPC-compatible tool functions for MCP (Model Context Protocol) applications.

## Features

- Async tool implementations with JSONRPC-compatible inputs and outputs
- Type-safe API using Rust's strong type system
- Tools for filesystem operations and search
- Flexible format transformers for different API conventions (standard JSONRPC, AWS Bedrock, etc.)
- Designed for macOS and Linux environments
- Proper logging with configurable log levels
- Thread-safe with proper mutex handling in async code

## Currently Implemented Tools

- `directory_list`: List directory contents with filtering options
- `directory_make`: Create directories
- `file_read`: Read file contents
- `file_write`: Write content to files
- `file_patch`: Apply patches to files
- `file_move`: Move or rename files
- `file_find`: Find files matching criteria
- `file_grep`: Search file contents
- `file_diff`: Compare files and generate diffs
- `shell`: Execute commands with security considerations

## Installation

Add to your Cargo.toml:

```toml
[dependencies]
gamecode-tools = "0.1.0"
```

## Usage

### Basic Example

```rust
use gamecode_tools::create_default_dispatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a dispatcher with all tools registered
    let dispatcher = create_default_dispatcher();
    
    // Process a JSONRPC request
    let request = r#"{
        "jsonrpc": "2.0",
        "method": "directory_list",
        "params": {
            "path": ".",
            "include_hidden": false
        },
        "id": 1
    }"#;
    
    let response = dispatcher.dispatch(request).await?;
    println!("{}", response);
    
    Ok(())
}
```

### Format Options

The library supports different input and output formats through explicit configuration:

```rust
use gamecode_tools::{
    // Ready-to-use dispatcher factories:
    create_default_dispatcher,        // Standard format for both input and output
    create_bedrock_dispatcher,        // AWS Bedrock format for both input and output
    create_standard_to_bedrock_dispatcher,  // Standard input, Bedrock output
    create_bedrock_to_standard_dispatcher,  // Bedrock input, Standard output
    
    // Or create custom configurations:
    FormatConfig, FormatTransformer, InputFormat, OutputFormat,
    create_dispatcher_with_transformer
};

// Create a custom format configuration
let config = FormatConfig::new(InputFormat::Standard, OutputFormat::Bedrock);
let transformer = FormatTransformer::new(config);
let dispatcher = create_dispatcher_with_transformer(transformer);

// Now all responses will be in Bedrock format, but it accepts standard input
```

The library supports these format options:

1. **Standard format**: Regular JSONRPC with direct values
   ```json
   {
     "jsonrpc": "2.0",
     "method": "directory_list",
     "params": {
       "path": "src",
       "include_hidden": false
     },
     "id": 1
   }
   ```

2. **AWS Bedrock format**: All values are recursively wrapped in `{"type": "text", "text": value}`
   ```json
   {
     "jsonrpc": "2.0",
     "method": "directory_list",
     "params": {
       "type": "text",
       "text": {
         "path": {"type": "text", "text": "src"},
         "include_hidden": {"type": "text", "text": false}
       }
     },
     "id": 1
   }
   ```

These formats can be applied independently to inputs and outputs, giving you full control over how your JSONRPC interface behaves.

### Direct Tool Usage

You can also use the tools directly without JSONRPC:

```rust
use gamecode_tools::tools::directory_list::{DirectoryList, Params};
use gamecode_tools::tools::Tool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tool = DirectoryList;
    
    let params = Params {
        path: ".".to_string(),
        pattern: Some("*.rs".to_string()),
        include_hidden: false,
        directories_only: false,
        files_only: true,
    };
    
    let result = tool.execute(params).await?;
    println!("Found {} Rust files", result.count);
    
    for entry in result.entries {
        println!("{}", entry.path);
    }
    
    Ok(())
}
```

## License

MIT

## Logging

The library uses the standard `log` crate for logging. This allows applications to choose their preferred logging implementation. 

```rust
use gamecode_tools::logging;
use log::{debug, error, info, trace, warn, LevelFilter};

// Using env_logger as an example
fn main() {
    // Initialize logging with env_logger
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Debug)
        .init();
        
    // Now you can use log macros
    info!("Application started");
    
    // Logs from the gamecode-tools will be visible based on your log level
}
```

You can control log levels using environment variables when using env_logger:

```bash
# Show only info and above
RUST_LOG=info cargo run

# Show debug logs for the gamecode-tools crate only
RUST_LOG=gamecode_tools=debug cargo run

# Show all debug logs
RUST_LOG=debug cargo run
```

## Contributing

Contributions are welcome! Please feel free to submit pull requests.