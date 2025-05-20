use gamecode_tools::create_bedrock_dispatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a dispatcher with the AWS Bedrock transformer
    let dispatcher = create_bedrock_dispatcher();
    
    // Standard JSONRPC request (parameters will be passed through to the tool as-is)
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
    
    // Bedrock-style wrapped request with fully wrapped parameters
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
    
    // Process both requests
    println!("STANDARD REQUEST:");
    let response1 = dispatcher.dispatch(standard_request).await?;
    println!("{}", response1);
    
    println!("\nBEDROCK-STYLE REQUEST:");
    let response2 = dispatcher.dispatch(bedrock_request).await?;
    println!("{}", response2);
    
    Ok(())
}