//! Example demonstrating extended file operations

use gamecode_tools::{create_default_dispatcher, jsonrpc};
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable logging
    tracing_subscriber::fmt::init();

    // Create a default JSONRPC dispatcher
    let dispatcher = create_default_dispatcher();

    // Demo paths
    let base_dir = "extended_examples";
    let search_content = "This is a searchable content with PATTERN to find.";

    // 1. Create a directory structure
    println!("Creating directory structure...");
    let mkdir_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "directory_make",
        "params": {
            "path": format!("{}/nested/structure", base_dir),
            "parents": true,
            "exist_ok": true
        }
    });

    let _ = dispatch_and_print_result(&dispatcher, mkdir_request).await?;

    // 2. Create some files for search testing
    println!("\nCreating test files...");
    for i in 1..=5 {
        let file_path = format!("{}/nested/file{}.txt", base_dir, i);
        let content = format!("{} File number {}", search_content, i);

        let write_request = json!({
            "jsonrpc": "2.0",
            "id": 10 + i,
            "method": "file_write",
            "params": {
                "path": file_path,
                "content": content,
                "content_type": "text",
                "create_dirs": false
            }
        });

        dispatch_and_print_result(&dispatcher, write_request).await?;
    }

    // 3. Create a file to move later
    println!("\nCreating a file to move...");
    let source_file = format!("{}/source_file.txt", base_dir);
    let write_request = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "file_write",
        "params": {
            "path": source_file,
            "content": "This file will be moved to a new location.",
            "content_type": "text",
            "create_dirs": false
        }
    });

    dispatch_and_print_result(&dispatcher, write_request).await?;

    // 4. Find files using file_find
    println!("\nFinding all text files...");
    let find_request = json!({
        "jsonrpc": "2.0",
        "id": 30,
        "method": "file_find",
        "params": {
            "directory": base_dir,
            "pattern": "*.txt",
            "mode": "pattern",
            "file_type": "file",
            "recursive": true,
            "max_depth": 0,
            "limit": 0,
            "follow_links": false,
            "ignore": []
        }
    });

    let find_result = dispatch_and_print_result(&dispatcher, find_request).await?;
    println!(
        "Found {} files",
        find_result["result"]["entries"].as_array().unwrap().len()
    );

    // 5. Grep for a pattern
    println!("\nSearching for 'PATTERN' in files...");
    let grep_request = json!({
        "jsonrpc": "2.0",
        "id": 40,
        "method": "file_grep",
        "params": {
            "directory": base_dir,
            "pattern": "PATTERN",
            "regex": false,
            "case_insensitive": false,
            "recursive": true,
            "max_depth": 0,
            "limit": 0,
            "follow_links": false,
            "include": "*.txt",
            "exclude": [],
            "line_numbers": true,
            "file_names_only": false,
            "before_context": 0,
            "after_context": 0
        }
    });

    let grep_result = dispatch_and_print_result(&dispatcher, grep_request).await?;
    println!(
        "Found {} files containing the pattern",
        grep_result["result"]["files_matched"]
    );

    // 6. Move the source file to a new location
    println!("\nMoving file...");
    let dest_file = format!("{}/nested/structure/moved_file.txt", base_dir);
    let move_request = json!({
        "jsonrpc": "2.0",
        "id": 50,
        "method": "file_move",
        "params": {
            "source": source_file,
            "destination": dest_file,
            "overwrite": false,
            "create_dirs": false
        }
    });

    let move_result = dispatch_and_print_result(&dispatcher, move_request).await?;
    println!("File moved: {}", move_result["result"]["overwritten"]);

    // 7. Verify the move with directory listing
    println!("\nListing files in destination directory...");
    let list_request = json!({
        "jsonrpc": "2.0",
        "id": 60,
        "method": "directory_list",
        "params": {
            "path": format!("{}/nested/structure", base_dir),
            "pattern": "*",
            "recursive": false
        }
    });

    let list_result = dispatch_and_print_result(&dispatcher, list_request).await?;
    println!(
        "Files in destination directory: {}",
        list_result["result"]["entries"].as_array().unwrap().len()
    );

    // 8. Clean up
    println!("\nCleaning up...");
    if Path::new(base_dir).exists() {
        fs::remove_dir_all(base_dir).await?;
        println!("Removed example directory and all contents");
    }

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
