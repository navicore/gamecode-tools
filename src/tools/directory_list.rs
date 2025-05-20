//! Directory list tool implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};

use crate::{Error, Result};
use super::Tool;

/// Directory list tool
#[derive(Clone, Copy)]
pub struct DirectoryList;

/// Parameters for the directory list tool
#[derive(Debug, Deserialize)]
pub struct Params {
    /// Path to the directory to list
    pub path: String,
    
    /// Optional glob pattern to filter results
    #[serde(default)]
    pub pattern: Option<String>,
    
    /// Whether to include hidden files (those starting with a dot)
    #[serde(default)]
    pub include_hidden: bool,
    
    /// Whether to list directories only
    #[serde(default)]
    pub directories_only: bool,
    
    /// Whether to list files only
    #[serde(default)]
    pub files_only: bool,
}

/// File or directory entry information
#[derive(Debug, Serialize)]
pub struct Entry {
    /// Name of the file or directory
    pub name: String,
    
    /// Full path to the file or directory
    pub path: String,
    
    /// Whether this is a directory
    pub is_directory: bool,
    
    /// File size in bytes (0 for directories)
    pub size: u64,
    
    /// Last modification time as ISO 8601 string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
}

/// Output of the directory list tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// List of entries in the directory
    pub entries: Vec<Entry>,
    
    /// Total count of entries
    pub count: usize,
}

#[async_trait]
impl Tool for DirectoryList {
    type Params = Params;
    type Output = Output;
    
    fn name(&self) -> &str {
        "directory_list"
    }
    
    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        let path = PathBuf::from(&params.path);
        
        // Check if the path exists and is a directory
        let metadata = fs::metadata(&path).await?;
        
        if !metadata.is_dir() {
            return Err(Error::InvalidParam(format!(
                "Path '{}' is not a directory", params.path
            )));
        }
        
        // Read directory entries
        let mut entries = Vec::new();
        let mut dir = fs::read_dir(&path).await?;
        
        while let Some(entry) = dir.next_entry().await? {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy().to_string();
            
            // Skip hidden files if include_hidden is false
            if !params.include_hidden && file_name_str.starts_with('.') {
                continue;
            }
            
            // Get file metadata
            let metadata = match entry.metadata().await {
                Ok(meta) => meta,
                Err(_) => continue, // Skip if we can't get metadata
            };
            
            let is_directory = metadata.is_dir();
            
            // Skip based on directories_only or files_only flags
            if (params.directories_only && !is_directory) || 
               (params.files_only && is_directory) {
                continue;
            }
            
            // Apply pattern filtering if provided
            if let Some(pattern) = &params.pattern {
                // Simple glob-like pattern matching
                if !matches_pattern(&file_name_str, pattern) {
                    continue;
                }
            }
            
            // Get modification time
            let modified = match metadata.modified() {
                Ok(time) => {
                    // Convert SystemTime to timestamp and then to DateTime
                    match time.duration_since(std::time::UNIX_EPOCH) {
                        Ok(duration) => {
                            let secs = duration.as_secs() as i64;
                            let nsecs = duration.subsec_nanos();
                            
                            if let Some(dt) = DateTime::<Utc>::from_timestamp(secs, nsecs) {
                                Some(dt.to_rfc3339())
                            } else {
                                None
                            }
                        },
                        Err(_) => None,
                    }
                },
                Err(_) => None,
            };
            
            entries.push(Entry {
                name: file_name_str,
                path: entry.path().to_string_lossy().to_string(),
                is_directory,
                size: if is_directory { 0 } else { metadata.len() },
                modified,
            });
        }
        
        // Count total entries
        let count = entries.len();
        
        Ok(Output { entries, count })
    }
}

/// Simple pattern matching function
fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        
        if parts.is_empty() {
            return true;
        }
        
        if pattern.starts_with('*') {
            if pattern.ends_with('*') {
                // *text*
                if parts.len() > 1 {
                    return name.contains(parts[1]);
                }
            } else {
                // *text
                return name.ends_with(parts[1]);
            }
        } else if pattern.ends_with('*') {
            // text*
            return name.starts_with(parts[0]);
        } else {
            // text*text
            if !name.starts_with(parts[0]) {
                return false;
            }
            
            if parts.len() > 1 && !name.ends_with(parts[parts.len() - 1]) {
                return false;
            }
            
            return true;
        }
    }
    
    // Exact match
    name == pattern
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs::create_dir_all;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;
    use std::io;
    
    async fn setup_test_dir() -> io::Result<PathBuf> {
        let test_dir = PathBuf::from("./test_dir");
        let _ = fs::remove_dir_all(&test_dir).await; // Ignore errors
        create_dir_all(&test_dir).await?;
        
        // Create some files
        let mut file1 = File::create(test_dir.join("file1.txt")).await?;
        file1.write_all(b"test content").await?;
        
        let mut file2 = File::create(test_dir.join("file2.dat")).await?;
        file2.write_all(b"binary content").await?;
        
        let mut hidden = File::create(test_dir.join(".hidden")).await?;
        hidden.write_all(b"hidden content").await?;
        
        // Create a subdirectory
        create_dir_all(test_dir.join("subdir")).await?;
        
        Ok(test_dir)
    }
    
    #[tokio::test]
    async fn test_directory_list() -> Result<()> {
        let test_dir = setup_test_dir().await.unwrap();
        
        let tool = DirectoryList;
        
        // Test basic listing
        let params = Params {
            path: test_dir.to_string_lossy().to_string(),
            pattern: None,
            include_hidden: false,
            directories_only: false,
            files_only: false,
        };
        
        let result = tool.execute(params).await?;
        
        // Should have 3 entries (2 files + 1 directory, excluding hidden)
        assert_eq!(result.count, 3);
        
        // Test with pattern
        let params = Params {
            path: test_dir.to_string_lossy().to_string(),
            pattern: Some("*.txt".to_string()),
            include_hidden: false,
            directories_only: false,
            files_only: false,
        };
        
        let result = tool.execute(params).await?;
        
        // Should have 1 entry (file1.txt)
        assert_eq!(result.count, 1);
        assert_eq!(result.entries[0].name, "file1.txt");
        
        // Test directories only
        let params = Params {
            path: test_dir.to_string_lossy().to_string(),
            pattern: None,
            include_hidden: false,
            directories_only: true,
            files_only: false,
        };
        
        let result = tool.execute(params).await?;
        
        // Should have 1 entry (subdir)
        assert_eq!(result.count, 1);
        assert!(result.entries[0].is_directory);
        
        // Test including hidden files
        let params = Params {
            path: test_dir.to_string_lossy().to_string(),
            pattern: None,
            include_hidden: true,
            directories_only: false,
            files_only: false,
        };
        
        let result = tool.execute(params).await?;
        
        // Should have 4 entries (3 visible + 1 hidden)
        assert_eq!(result.count, 4);
        
        // Clean up
        let _ = fs::remove_dir_all(&test_dir).await;
        
        Ok(())
    }
    
    #[test]
    fn test_pattern_matching() {
        // Exact match
        assert!(matches_pattern("file.txt", "file.txt"));
        assert!(!matches_pattern("file.txt", "file.dat"));
        
        // Wildcard at start
        assert!(matches_pattern("hello.txt", "*.txt"));
        assert!(!matches_pattern("hello.dat", "*.txt"));
        
        // Wildcard at end
        assert!(matches_pattern("prefix_something", "prefix_*"));
        assert!(!matches_pattern("something_else", "prefix_*"));
        
        // Wildcard in middle
        assert!(matches_pattern("prefix_suffix", "prefix_*suffix"));
        assert!(!matches_pattern("prefix_wrong", "prefix_*suffix"));
        
        // Wildcard at both ends
        assert!(matches_pattern("contains_text_inside", "*text*"));
        assert!(!matches_pattern("does_not_match", "*text*"));
    }
}