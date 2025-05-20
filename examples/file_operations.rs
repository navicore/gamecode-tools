//! Example demonstrating file read and write operations

use gamecode_tools::{create_default_dispatcher, jsonrpc};
use serde_json::{json, Value};
use tokio::fs;
use base64::{engine::general_purpose, Engine as _};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable logging
    tracing_subscriber::fmt::init();
    
    // Create a default JSONRPC dispatcher
    let dispatcher = create_default_dispatcher();
    
    // Demo file paths
    let text_file = "example_text.txt";
    let binary_file = "example_binary.bin";
    
    // 1. Write a text file
    println!("Writing text file...");
    let text_content = "This is an example text file content.\nIt has multiple lines.\nCreated by the file_write tool.";
    
    let write_text_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "file_write",
        "params": {
            "path": text_file,
            "content": text_content,
            "content_type": "text",
            "create_dirs": false
        }
    });
    
    let text_result = dispatch_and_print_result(&dispatcher, write_text_request).await?;
    if text_result.get("result").is_some() {
        println!("Text file size: {} bytes\n", text_result["result"]["size"]);
    } else {
        println!("Failed to write text file: {}\n", text_result["error"]["message"]);
        return Ok(());
    }
    
    // 2. Read back the text file
    println!("Reading text file...");
    let read_text_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "file_read",
        "params": {
            "path": text_file,
            "content_type": "text",
            "line_numbers": true
        }
    });
    
    let read_text_result = dispatch_and_print_result(&dispatcher, read_text_request).await?;
    if read_text_result.get("result").is_some() {
        println!("Text file content:\n{}\n", read_text_result["result"]["content"]);
    } else {
        println!("Failed to read text file: {}\n", read_text_result["error"]["message"]);
        return Ok(());
    }
    
    // 3. Write a binary file
    println!("Writing binary file...");
    let binary_data = vec![0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    let base64_content = general_purpose::STANDARD.encode(&binary_data);
    
    let write_binary_request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "file_write",
        "params": {
            "path": binary_file,
            "content": base64_content,
            "content_type": "binary",
            "create_dirs": false
        }
    });
    
    let binary_result = dispatch_and_print_result(&dispatcher, write_binary_request).await?;
    if binary_result.get("result").is_none() {
        println!("Failed to write binary file: {}\n", binary_result["error"]["message"]);
        return Ok(());
    }
    println!("Binary file size: {} bytes\n", binary_result["result"]["size"]);
    
    // 4. Read back the binary file
    println!("Reading binary file...");
    let read_binary_request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "file_read",
        "params": {
            "path": binary_file,
            "content_type": "binary"
        }
    });
    
    let read_binary_result = dispatch_and_print_result(&dispatcher, read_binary_request).await?;
    if read_binary_result.get("result").is_none() {
        println!("Failed to read binary file: {}\n", read_binary_result["error"]["message"]);
        return Ok(());
    }
    
    let read_base64 = read_binary_result["result"]["content"].as_str().unwrap();
    println!("Binary file content (base64): {}", read_base64);
    
    // Decode and print the binary data
    let decoded = general_purpose::STANDARD.decode(read_base64)?;
    println!("Decoded binary data: {:?}\n", decoded);
    
    // 5. Clean up example files
    println!("Cleaning up example files...");
    fs::remove_file(text_file).await?;
    fs::remove_file(binary_file).await?;
    println!("Clean up complete.");
    
    Ok(())
}

/// Helper function to dispatch a request and print the result
async fn dispatch_and_print_result(
    dispatcher: &jsonrpc::Dispatcher,
    request: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    println!("Request: {}", request);
    
    let request_str = request.to_string();
    let result = dispatcher.dispatch(&request_str).await?;
    let result_value: Value = serde_json::from_str(&result)?;
    
    println!("Response: {}", result_value);
    
    Ok(result_value)
}