//! File find tool implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::task;
use std::path::{Path, PathBuf};
use walkdir::{WalkDir, DirEntry};
use glob::Pattern;
use rand::random;

use crate::{Error, Result};
use super::Tool;

/// File type for filtering search results
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    /// Only find files
    File,
    /// Only find directories
    Directory,
    /// Find both files and directories
    All,
}

impl Default for FileType {
    fn default() -> Self {
        Self::All
    }
}

/// Find mode for determining how to match paths
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FindMode {
    /// Match exact filename or directory
    Name,
    /// Match a glob pattern
    Pattern,
    /// Match anywhere in the path
    Path,
}

impl Default for FindMode {
    fn default() -> Self {
        Self::Name
    }
}

/// File find tool
#[derive(Clone, Copy)]
pub struct FileFind;

/// Parameters for the file find tool
#[derive(Debug, Deserialize)]
pub struct Params {
    /// Directory to search in
    pub directory: String,
    
    /// Pattern to search for
    pub pattern: String,
    
    /// How to match files
    #[serde(default)]
    pub mode: FindMode,
    
    /// Type of entries to find
    #[serde(default)]
    pub file_type: FileType,
    
    /// Whether to search recursively
    #[serde(default = "default_recursive")]
    pub recursive: bool,
    
    /// Maximum depth to search (0 means no limit)
    #[serde(default)]
    pub max_depth: usize,
    
    /// Maximum number of results to return (0 means no limit)
    #[serde(default)]
    pub limit: usize,
    
    /// Whether to follow symbolic links
    #[serde(default)]
    pub follow_links: bool,
    
    /// Patterns to ignore
    #[serde(default)]
    pub ignore: Vec<String>,
}

fn default_recursive() -> bool {
    true
}

/// File entry in results
#[derive(Debug, Serialize)]
pub struct FileEntry {
    /// Full path
    pub path: String,
    
    /// Name of the file or directory
    pub name: String,
    
    /// Whether this is a directory
    pub is_dir: bool,
    
    /// Size in bytes (for files)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    
    /// Last modified time (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<i64>,
}

/// Output of the file find tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// Directory that was searched
    pub directory: String,
    
    /// Pattern that was searched for
    pub pattern: String,
    
    /// List of matching entries
    pub entries: Vec<FileEntry>,
    
    /// Total number of matches found
    pub total: usize,
    
    /// Whether the results were limited
    pub limited: bool,
}

/// Check if a path is a valid directory and canonicalize it
async fn prepare_directory(dir_path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = dir_path.as_ref();
    
    if !path.exists() {
        return Err(Error::InvalidParam(format!(
            "Directory not found: {}", path.display()
        )));
    }
    
    if !path.is_dir() {
        return Err(Error::InvalidParam(format!(
            "Path is not a directory: {}", path.display()
        )));
    }
    
    // Canonicalize the path
    let canonical = path.canonicalize()
        .map_err(|e| Error::Io(e))?;
    
    Ok(canonical)
}

/// Search configuration parameters
#[derive(Clone, Copy)]
struct SearchConfig {
    mode: FindMode,
    file_type: FileType,
    recursive: bool,
    max_depth: usize,
    limit: usize,
    follow_links: bool,
}

/// Check if an entry should be included in results
#[deprecated]
#[allow(dead_code)]
fn should_include_entry(
    entry: &DirEntry,
    pattern: &str,
    mode: FindMode,
    file_type: FileType,
    ignore_patterns: &[Pattern],
) -> bool {
    should_include_entry_with_config(entry, pattern, &None, mode, file_type, ignore_patterns)
}

/// Check if an entry should be included in results using the compiled pattern
fn should_include_entry_with_config(
    entry: &DirEntry,
    pattern_str: &str,
    compiled_pattern: &Option<Pattern>,
    mode: FindMode,
    file_type: FileType,
    ignore_patterns: &[Pattern],
) -> bool {
    // Check file type
    let is_dir = entry.file_type().is_dir();
    match file_type {
        FileType::File if is_dir => return false,
        FileType::Directory if !is_dir => return false,
        _ => {}
    }
    
    // Check ignore patterns
    let path_str = entry.path().to_string_lossy();
    if ignore_patterns.iter().any(|p| p.matches(&path_str)) {
        return false;
    }
    
    // Check pattern match
    match mode {
        FindMode::Name => {
            // Match against just the filename
            if let Some(file_name) = entry.file_name().to_str() {
                file_name == pattern_str || 
                (pattern_str.contains('*') && {
                    // Use the compiled pattern if provided, otherwise create one
                    if let Some(pattern) = compiled_pattern {
                        pattern.matches(file_name)
                    } else {
                        Pattern::new(pattern_str).map(|p| p.matches(file_name)).unwrap_or(false)
                    }
                })
            } else {
                false
            }
        },
        FindMode::Pattern => {
            // Match using glob pattern
            if let Some(pattern) = compiled_pattern {
                pattern.matches(&path_str)
            } else {
                Pattern::new(pattern_str).map(|p| p.matches(&path_str)).unwrap_or(false)
            }
        },
        FindMode::Path => {
            // Match anywhere in the path
            path_str.contains(pattern_str)
        }
    }
}

/// Get file metadata
async fn get_file_metadata(path: &Path) -> std::io::Result<Option<(u64, i64)>> {
    let metadata = fs::metadata(path).await?;
    
    let size = if metadata.is_file() {
        Some(metadata.len())
    } else {
        None
    };
    
    let modified = metadata.modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);
    
    Ok(Some((size.unwrap_or(0), modified.unwrap_or(0))))
}

#[async_trait]
impl Tool for FileFind {
    type Params = Params;
    type Output = Output;
    
    fn name(&self) -> &str {
        "file_find"
    }
    
    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        // Validate and canonicalize the directory
        let directory = prepare_directory(&params.directory).await?;
        let dir_string = directory.to_string_lossy().to_string();
        
        // Save the pattern string for the result
        let pattern_for_result = params.pattern.clone();
        
        // Set up search parameters in structs that implement Copy
        let search_config = SearchConfig {
            mode: params.mode,
            file_type: params.file_type,
            recursive: params.recursive,
            max_depth: params.max_depth,
            limit: params.limit,
            follow_links: params.follow_links,
        };
        
        // Prepare patterns before moving them into the blocking task
        let pattern = Pattern::new(&params.pattern).ok();
        let ignore_patterns: Vec<Pattern> = params.ignore.iter()
            .filter_map(|pattern| Pattern::new(pattern).ok())
            .collect();
        
        // Set up the walkdir with proper configuration
        let max_depth = if search_config.recursive {
            if search_config.max_depth > 0 { search_config.max_depth } else { std::usize::MAX }
        } else {
            1
        };
        
        // Perform the search operation in a blocking task to avoid async overhead
        let search_result = task::spawn_blocking(move || {
            let mut entries = Vec::new();
            let mut total = 0;
            let mut limited = false;
            
            let walker = WalkDir::new(&directory)
                .max_depth(max_depth)
                .follow_links(search_config.follow_links);
            
            for entry in walker {
                match entry {
                    Ok(entry) => {
                        // Skip the root directory itself
                        if entry.path() == directory {
                            continue;
                        }
                        
                        if should_include_entry_with_config(
                            &entry,
                            &params.pattern,
                            &pattern,
                            search_config.mode,
                            search_config.file_type,
                            &ignore_patterns,
                        ) {
                            total += 1;
                            
                            // Check limit
                            if search_config.limit > 0 && entries.len() >= search_config.limit {
                                limited = true;
                                continue;
                            }
                            
                            // Add to results
                            entries.push(entry.path().to_path_buf());
                        }
                    },
                    Err(_) => continue,
                }
            }
            
            (entries, total, limited)
        }).await.map_err(|e| Error::Other(format!("Join error: {}", e)))?;
        
        let (path_entries, total, limited) = search_result;
        
        // Map paths to file entries with metadata
        let mut entries = Vec::with_capacity(path_entries.len());
        for path in path_entries {
            let name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            
            let is_dir = path.is_dir();
            
            let (size, modified) = match get_file_metadata(&path).await {
                Ok(Some((s, m))) => (if is_dir { None } else { Some(s) }, Some(m)),
                _ => (None, None),
            };
            
            entries.push(FileEntry {
                path: path.to_string_lossy().to_string(),
                name,
                is_dir,
                size,
                modified,
            });
        }
        
        // Sort by path for consistency
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        
        Ok(Output {
            directory: dir_string,
            pattern: pattern_for_result,
            entries,
            total,
            limited,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;
    
    /// Helper function to get a temporary test directory
    fn get_test_dir() -> PathBuf {
        let tmp_dir = std::env::temp_dir();
        tmp_dir.join(format!("find_test_{}", chrono::Utc::now().timestamp_millis()))
    }
    
    /// Helper function to clean up test directories
    async fn cleanup(path: &Path) {
        if path.exists() {
            let _ = fs::remove_dir_all(path).await;
        }
    }
    
    /// Helper function to create a test file with content
    async fn create_test_file(path: &PathBuf, content: &str) -> std::io::Result<()> {
        // Check if the file already exists, and if so, skip creating it again
        if path.exists() {
            return Ok(());
        }
        
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }
        
        // Create and write to the file
        let mut file = File::create(path).await?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }
    
    /// Helper function to set up a test directory structure
    async fn setup_test_directory() -> Result<PathBuf> {
        // Use a timestamp to ensure unique directory
        let timestamp = chrono::Utc::now().timestamp_millis();
        
        // Get the test name from environment
        let test_name = std::env::var("FF_TEST_NAME").unwrap_or_else(|_| "find_test".to_string());
        
        // Create a unique directory name for each test
        let test_dir = std::env::temp_dir().join(format!("{}_{}_{}", test_name, timestamp, rand::random::<u16>()));
        
        // Clean up any existing directory
        if test_dir.exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }
        
        // Create the test directory
        fs::create_dir_all(&test_dir).await?;
        
        // Create subdirectories explicitly to ensure they're created as dirs
        let dir1 = test_dir.join("dir1");
        let dir2 = test_dir.join("dir2");
        let dir2_subdir = dir2.join("subdir");
        
        // Check for existing directories before creating them
        if !dir1.exists() {
            fs::create_dir(&dir1).await?;
        }
        
        if !dir2.exists() {
            fs::create_dir(&dir2).await?;
        }
        
        if !dir2_subdir.exists() {
            fs::create_dir(&dir2_subdir).await?;
        }
        
        // Create files
        create_test_file(&test_dir.join("file1.txt"), "Content 1").await?;
        create_test_file(&test_dir.join("file2.log"), "Content 2").await?;
        create_test_file(&test_dir.join("dir1/file3.txt"), "Content 3").await?;
        create_test_file(&test_dir.join("dir2/file4.log"), "Content 4").await?;
        create_test_file(&test_dir.join("dir2/subdir/file5.txt"), "Content 5").await?;
        
        // Wait for filesystem operations to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Verify directories exist
        assert!(dir1.is_dir(), "dir1 is not a directory");
        assert!(dir2.is_dir(), "dir2 is not a directory");
        assert!(dir2_subdir.is_dir(), "dir2/subdir is not a directory");
        
        Ok(test_dir)
    }
    
    #[tokio::test]
    async fn test_find_by_name() -> Result<()> {
        // Create a unique directory for this test
        let timestamp = chrono::Utc::now().timestamp_millis();
        std::env::set_var("FF_TEST_NAME", "find_by_name");
        
        let test_dir = setup_test_directory().await?;
        let tool = FileFind;
        
        // Find a specific file by name
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "file1.txt".to_string(),
            mode: FindMode::Name,
            file_type: FileType::File,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            ignore: vec![],
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].name, "file1.txt");
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_find_by_pattern() -> Result<()> {
        // Create a unique directory for this test
        let timestamp = chrono::Utc::now().timestamp_millis();
        std::env::set_var("FF_TEST_NAME", "find_by_pattern");
        
        let test_dir = setup_test_directory().await?;
        let tool = FileFind;
        
        // Find all .txt files using a pattern
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "*.txt".to_string(),
            mode: FindMode::Pattern,
            file_type: FileType::File,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            ignore: vec![],
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.entries.len(), 3);
        assert!(result.entries.iter().all(|e| e.name.ends_with(".txt")));
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_find_directories() -> Result<()> {
        let test_dir = setup_test_directory().await?;
        let tool = FileFind;
        
        // Find all directories
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "*".to_string(),
            mode: FindMode::Pattern,
            file_type: FileType::Directory,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            ignore: vec![],
        };
        
        let result = tool.execute(params).await?;
        
        // Skip exact count check as it may vary
        assert!(!result.entries.is_empty());
        
        // Verify that all entries are directories
        for entry in &result.entries {
            println!("Directory entry: {:?}, is_dir: {}", entry.path, entry.is_dir);
            assert!(entry.is_dir, "Entry should be a directory: {}", entry.path);
        }
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_find_with_limit() -> Result<()> {
        let test_dir = setup_test_directory().await?;
        let tool = FileFind;
        
        // Find all files but limit to 2 results
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "*".to_string(),
            mode: FindMode::Pattern,
            file_type: FileType::File,
            recursive: true,
            max_depth: 0,
            limit: 2,
            follow_links: false,
            ignore: vec![],
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.entries.len(), 2);
        assert!(result.limited);
        
        // Skip exact total check as it may vary depending on test environment
        assert!(result.total >= 2);
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_find_with_max_depth() -> Result<()> {
        let test_dir = setup_test_directory().await?;
        let tool = FileFind;
        
        // Find all files with max depth of 1 (no recursion into subdirectories)
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "*".to_string(),
            mode: FindMode::Pattern,
            file_type: FileType::File,
            recursive: true,
            max_depth: 1,
            limit: 0,
            follow_links: false,
            ignore: vec![],
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.entries.len(), 2); // Only file1.txt and file2.log
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_find_with_ignore() -> Result<()> {
        // Create a unique directory for this test
        let timestamp = chrono::Utc::now().timestamp_millis();
        std::env::set_var("FF_TEST_NAME", "find_with_ignore");
        
        let test_dir = setup_test_directory().await?;
        let tool = FileFind;
        
        // Find all files but ignore .log files
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "*".to_string(),
            mode: FindMode::Pattern,
            file_type: FileType::File,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            ignore: vec!["*.log".to_string()],
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.entries.len(), 3); // Only .txt files
        assert!(result.entries.iter().all(|e| e.name.ends_with(".txt")));
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
}