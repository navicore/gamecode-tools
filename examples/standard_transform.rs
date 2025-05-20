use gamecode_tools::create_default_dispatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a dispatcher with the standard transformer
    let dispatcher = create_default_dispatcher();
    
    // Standard JSONRPC request
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
    
    // Process the request
    let response = dispatcher.dispatch(request).await?;
    println!("{}", response);
    
    Ok(())
}