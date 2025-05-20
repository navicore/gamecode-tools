//! Example demonstrating file patch operations

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
    let text_file = "example_text_to_patch.txt";
    let binary_file = "example_binary_to_patch.bin";
    
    // 1. Create original text file
    println!("Creating original text file...");
    let original_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    
    let write_text_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "file_write",
        "params": {
            "path": text_file,
            "content": original_text,
            "content_type": "text",
            "create_dirs": false
        }
    });
    
    let _ = dispatch_and_print_result(&dispatcher, write_text_request).await?;
    
    // 2. Generate a unified diff for the text file
    println!("\nGenerating unified diff...");
    
    // Create a simple unified diff manually for the example
    let unified_diff = r#"--- example_text_to_patch.txt
+++ example_text_to_patch.txt
@@ -1,5 +1,6 @@
 Line 1
-Line 2
+Modified Line 2
 Line 3
 Line 4
+Added Line
 Line 5"#;
    
    println!("Unified diff:\n{}", unified_diff);
    
    // 3. Apply the patch to the text file
    println!("\nApplying text patch...");
    let patch_text_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "file_patch",
        "params": {
            "path": text_file,
            "patch": unified_diff,
            "patch_type": "unified",
            "create_backup": true
        }
    });
    
    let patch_result = dispatch_and_print_result(&dispatcher, patch_text_request).await?;
    
    // 4. Read the patched text file
    println!("\nReading patched text file...");
    let read_text_request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "file_read",
        "params": {
            "path": text_file,
            "content_type": "text",
            "line_numbers": true
        }
    });
    
    let read_result = dispatch_and_print_result(&dispatcher, read_text_request).await?;
    println!("Patched text content:\n{}", read_result["result"]["content"].as_str().unwrap());
    
    // 5. Create original binary file
    println!("\nCreating original binary file...");
    let original_binary = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let base64_content = general_purpose::STANDARD.encode(&original_binary);
    
    let write_binary_request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "file_write",
        "params": {
            "path": binary_file,
            "content": base64_content,
            "content_type": "binary",
            "create_dirs": false
        }
    });
    
    let _ = dispatch_and_print_result(&dispatcher, write_binary_request).await?;
    
    // 6. Generate a binary patch
    println!("\nGenerating binary patch...");
    // For binary patches, we use a simple format: "offset:base64data"
    let binary_patch = "2:Cg==\n5:UFQ="; // Change bytes at offset 2 and 5
    
    println!("Binary patch:\n{}", binary_patch);
    
    // 7. Apply the binary patch
    println!("\nApplying binary patch...");
    let patch_binary_request = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "file_patch",
        "params": {
            "path": binary_file,
            "patch": binary_patch,
            "patch_type": "binary",
            "create_backup": true
        }
    });
    
    let patch_binary_result = dispatch_and_print_result(&dispatcher, patch_binary_request).await?;
    
    // 8. Read the patched binary file
    println!("\nReading patched binary file...");
    let read_binary_request = json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "file_read",
        "params": {
            "path": binary_file,
            "content_type": "binary"
        }
    });
    
    let read_binary_result = dispatch_and_print_result(&dispatcher, read_binary_request).await?;
    let content = read_binary_result["result"]["content"].as_str().unwrap();
    let decoded = general_purpose::STANDARD.decode(content)?;
    println!("Patched binary content: {:?}", decoded);
    
    // Expected result: [0, 1, 10, 3, 4, 80, 84, 7, 8, 9]
    // We replaced byte at offset 2 with 10 (base64: Cg==)
    // We replaced bytes at offset 5 with 80, 84 (base64: UFQ=)
    
    // 9. Clean up files
    println!("\nCleaning up files...");
    fs::remove_file(text_file).await?;
    if let Some(backup_path) = patch_result["result"]["backup_path"].as_str() {
        fs::remove_file(backup_path).await?;
    }
    
    fs::remove_file(binary_file).await?;
    if let Some(backup_path) = patch_binary_result["result"]["backup_path"].as_str() {
        fs::remove_file(backup_path).await?;
    }
    
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