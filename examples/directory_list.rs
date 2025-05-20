use gamecode_tools::jsonrpc::Dispatcher;
use gamecode_tools::tools::directory_list::{DirectoryList, Params};
use gamecode_tools::tools::Tool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize a dispatcher
    let mut dispatcher = Dispatcher::new();
    
    // Create the directory list tool
    let dir_list_tool = DirectoryList;
    
    // Register the tool with the dispatcher
    dispatcher.register(
        "directory_list",
        move |params: Params| async move {
            dir_list_tool.execute(params).await
        },
    );
    
    // Example JSONRPC request
    let request = r#"{
        "jsonrpc": "2.0",
        "method": "directory_list",
        "params": {
            "path": ".",
            "include_hidden": false
        },
        "id": 1
    }"#;
    
    // Dispatch the request
    let response = dispatcher.dispatch(request).await?;
    
    // Print the response
    println!("{}", response);
    
    Ok(())
}