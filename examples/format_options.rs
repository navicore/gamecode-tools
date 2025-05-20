use gamecode_tools::{
    create_default_dispatcher,
    create_bedrock_dispatcher,
    create_standard_to_bedrock_dispatcher,
    create_bedrock_to_standard_dispatcher,
    FormatConfig, 
    FormatTransformer, 
    InputFormat, 
    OutputFormat
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Sample JSONRPC request in standard format
    let standard_request = r#"{
        "jsonrpc": "2.0",
        "method": "directory_list",
        "params": {
            "path": "src",
            "include_hidden": false,
            "pattern": "*.rs"
        },
        "id": 1
    }"#;
    
    // Bedrock-style request with fully wrapped parameters
    // In real Bedrock format, all scalar values are wrapped
    let bedrock_request = r#"{
        "jsonrpc": "2.0",
        "method": "directory_list",
        "params": {
            "type": "text",
            "text": {
                "path": {"type": "text", "text": "src"},
                "include_hidden": {"type": "text", "text": false},
                "pattern": {"type": "text", "text": "*.rs"}
            }
        },
        "id": 2
    }"#;
    
    // 1. Standard format (standard input -> standard output)
    println!("=== STANDARD FORMAT (standard input -> standard output) ===");
    let dispatcher = create_default_dispatcher();
    let response = dispatcher.dispatch(standard_request).await?;
    println!("{}", response);
    println!();
    
    // 2. Bedrock format (Bedrock input -> Bedrock output)
    println!("=== BEDROCK FORMAT (Bedrock input -> Bedrock output) ===");
    let dispatcher = create_bedrock_dispatcher();
    let response = dispatcher.dispatch(bedrock_request).await?;
    println!("{}", response);
    println!();
    
    // 3. Standard input -> Bedrock output
    println!("=== MIXED FORMAT (Standard input -> Bedrock output) ===");
    let dispatcher = create_standard_to_bedrock_dispatcher();
    let response = dispatcher.dispatch(standard_request).await?;
    println!("{}", response);
    println!();
    
    // 4. Bedrock input -> Standard output
    println!("=== MIXED FORMAT (Bedrock input -> Standard output) ===");
    let dispatcher = create_bedrock_to_standard_dispatcher();
    let response = dispatcher.dispatch(bedrock_request).await?;
    println!("{}", response);
    println!();
    
    // 5. Custom format configuration
    println!("=== CUSTOM FORMAT CONFIGURATION ===");
    let config = FormatConfig::new(InputFormat::Bedrock, OutputFormat::Bedrock);
    let transformer = FormatTransformer::new(config);
    
    // Use the create_dispatcher_with_transformer function from the library
    let dispatcher = gamecode_tools::create_dispatcher_with_transformer(transformer);
    let response = dispatcher.dispatch(bedrock_request).await?;
    println!("{}", response);
    
    Ok(())
}