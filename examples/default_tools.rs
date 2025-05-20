use gamecode_tools::create_default_dispatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the default dispatcher with all tools registered
    let dispatcher = create_default_dispatcher();
    
    // Example JSONRPC request for directory_list
    let request = r#"{
        "jsonrpc": "2.0",
        "method": "directory_list",
        "params": {
            "path": "src",
            "include_hidden": false,
            "pattern": "*.rs"
        },
        "id": 1
    }"#;
    
    // Dispatch the request
    let response = dispatcher.dispatch(request).await?;
    
    // Print the response
    println!("{}", response);
    
    Ok(())
}