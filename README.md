# gamecode-tools

A Rust library providing JSONRPC-compatible tool functions for MCP (Model Context Protocol) applications.

## Features

- Async tool implementations with JSONRPC-compatible inputs and outputs
- Type-safe API using Rust's strong type system
- Tools for filesystem operations and search
- Format transformers for different API conventions (standard JSONRPC, AWS Bedrock, etc.)
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

### Using Different API Format Transformers

The library supports transforming JSONRPC parameters and results to match different API formats:

```rust
use gamecode_tools::create_bedrock_dispatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a dispatcher with AWS Bedrock transformer
    let dispatcher = create_bedrock_dispatcher();
    
    // This transforms the result to {"type": "text", "text": {...result...}}
    // It also accepts params in either standard format or wrapped format
    let request = r#"{
        "jsonrpc": "2.0",
        "method": "directory_list",
        "params": {
            "path": "src",
            "include_hidden": false
        },
        "id": 1
    }"#;
    
    let response = dispatcher.dispatch(request).await?;
    println!("{}", response);
    
    Ok(())
}
```

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

## Custom Transformers

You can create custom transformers by implementing the `ResponseTransformer` trait:

```rust
use std::sync::Arc;
use gamecode_tools::transform::ResponseTransformer;
use gamecode_tools::create_dispatcher_with_transformer;

struct MyCustomTransformer;

impl ResponseTransformer for MyCustomTransformer {
    // Implement the required methods
    // ...
}

let transformer = Arc::new(MyCustomTransformer);
let dispatcher = create_dispatcher_with_transformer(transformer);
```

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit pull requests.