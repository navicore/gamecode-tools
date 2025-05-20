# gamecode-tools

A Rust library providing JSONRPC-compatible tool functions for MCP (Model Context Protocol) applications.

## Features

- Async tool implementations with JSONRPC-compatible inputs and outputs
- Type-safe API using Rust's strong type system
- Tools for filesystem operations and search
- Flexible format transformers for different API conventions (standard JSONRPC, AWS Bedrock, etc.)
- macOS command line support

## Currently Implemented Tools

- `directory_list`: List directory contents with filtering options

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

## Contributing

Contributions are welcome! Please feel free to submit pull requests.