//! File grep tool implementation

use async_trait::async_trait;
use glob::Pattern;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::task;
use walkdir::{DirEntry, WalkDir};

use super::Tool;
use crate::{Error, Result};

/// File grep tool
#[derive(Clone, Copy)]
pub struct FileGrep;

/// Parameters for the file grep tool
#[derive(Debug, Deserialize)]
pub struct Params {
    /// Directory to search in
    pub directory: String,

    /// Pattern to search for in file contents
    pub pattern: String,

    /// Whether the pattern is a regular expression
    #[serde(default)]
    pub regex: bool,

    /// Whether to match case insensitively
    #[serde(default)]
    pub case_insensitive: bool,

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

    /// Glob pattern to include files
    #[serde(default)]
    pub include: Option<String>,

    /// Glob patterns to exclude files
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Whether to include line numbers in the output
    #[serde(default)]
    pub line_numbers: bool,

    /// Number of context lines to include before the match
    #[serde(default)]
    pub before_context: usize,

    /// Number of context lines to include after the match
    #[serde(default)]
    pub after_context: usize,

    /// Whether to only return file names, not content
    #[serde(default)]
    pub file_names_only: bool,
}

fn default_recursive() -> bool {
    true
}

/// Match in a file
#[derive(Debug, Serialize)]
pub struct Match {
    /// Line number (1-based)
    pub line_number: usize,

    /// The matched line content
    pub line: String,

    /// Context lines before the match
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub before_context: Vec<String>,

    /// Context lines after the match
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub after_context: Vec<String>,
}

/// File with matches
#[derive(Debug, Serialize)]
pub struct FileMatch {
    /// Path to the file
    pub path: String,

    /// Size of the file in bytes
    pub size: u64,

    /// List of matches in the file
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<Match>,
}

/// Output of the file grep tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// Directory that was searched
    pub directory: String,

    /// Pattern that was searched for
    pub pattern: String,

    /// List of files with matches
    pub files: Vec<FileMatch>,

    /// Total number of files searched
    pub files_searched: usize,

    /// Total number of files with matches
    pub files_matched: usize,

    /// Total number of matches found
    pub total_matches: usize,

    /// Whether the results were limited
    pub limited: bool,
}

/// Check if a path is a valid directory and canonicalize it
async fn prepare_directory(dir_path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = dir_path.as_ref();

    if !path.exists() {
        return Err(Error::InvalidParam(format!(
            "Directory not found: {}",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(Error::InvalidParam(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }

    // Canonicalize the path
    let canonical = path.canonicalize().map_err(Error::Io)?;

    Ok(canonical)
}

/// Check if a file should be included in the search
fn should_include_file(
    entry: &DirEntry,
    include_pattern: &Option<Pattern>,
    exclude_patterns: &[Pattern],
) -> bool {
    // Check if it's a file
    if !entry.file_type().is_file() {
        return false;
    }

    // Get path as string
    let path_str = entry.path().to_string_lossy();

    // Check exclude patterns
    if exclude_patterns.iter().any(|p| p.matches(&path_str)) {
        return false;
    }

    // Check include pattern if specified
    if let Some(include) = include_pattern {
        if !include.matches(&path_str) {
            return false;
        }
    }

    true
}

/// Search a file for the pattern
async fn search_file(
    path: &Path,
    pattern: &str,
    regex: bool,
    case_insensitive: bool,
    before_context: usize,
    after_context: usize,
    file_names_only: bool,
) -> std::io::Result<Option<FileMatch>> {
    // Get file metadata
    let metadata = fs::metadata(path).await?;
    let size = metadata.len();

    // If only file names are needed, we can check for matches more efficiently
    if file_names_only {
        let content = fs::read_to_string(path).await?;

        // Check if there's a match without line-by-line processing
        let has_match = if regex {
            let regex_flags = if case_insensitive { "(?i)" } else { "" };
            let pattern = format!("{}{}", regex_flags, pattern);
            match Regex::new(&pattern) {
                Ok(re) => re.is_match(&content),
                Err(e) => {
                    // Log error for debugging but don't fail the whole operation
                    log::warn!("Invalid regex pattern '{}': {}", pattern, e);
                    false
                }
            }
        } else if case_insensitive {
            content.to_lowercase().contains(&pattern.to_lowercase())
        } else {
            content.contains(pattern)
        };

        if has_match {
            return Ok(Some(FileMatch {
                path: path.to_string_lossy().to_string(),
                size,
                matches: vec![],
            }));
        } else {
            return Ok(None);
        }
    }

    // Read the file content
    let content = fs::read_to_string(path).await?;
    let lines: Vec<&str> = content.lines().collect();

    // Prepare regex if needed
    let re = if regex {
        let regex_flags = if case_insensitive { "(?i)" } else { "" };
        let pattern = format!("{}{}", regex_flags, pattern);
        match Regex::new(&pattern) {
            Ok(re) => Some(re),
            Err(e) => {
                // Log error for debugging but don't fail the whole operation
                log::warn!("Invalid regex pattern '{}': {}", pattern, e);
                return Ok(None);
            }
        }
    } else {
        None
    };

    let mut matches = Vec::new();

    // Search for matches line by line
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1; // 1-based line number

        let is_match = if let Some(re) = &re {
            re.is_match(line)
        } else if case_insensitive {
            line.to_lowercase().contains(&pattern.to_lowercase())
        } else {
            line.contains(pattern)
        };

        if is_match {
            // Add context lines
            let before = if before_context > 0 {
                let start = i.saturating_sub(before_context);
                lines[start..i]
                    .iter()
                    .map(|&l| format!("{}:{}", start + 1, l))
                    .collect()
            } else {
                Vec::new()
            };

            let after = if after_context > 0 && i + 1 < lines.len() {
                let end = std::cmp::min(i + 1 + after_context, lines.len());
                lines[i + 1..end]
                    .iter()
                    .enumerate()
                    .map(|(idx, &l)| format!("{}:{}", i + 2 + idx, l))
                    .collect()
            } else {
                Vec::new()
            };

            matches.push(Match {
                line_number: line_num,
                line: line.to_string(),
                before_context: before,
                after_context: after,
            });
        }
    }

    if matches.is_empty() {
        Ok(None)
    } else {
        Ok(Some(FileMatch {
            path: path.to_string_lossy().to_string(),
            size,
            matches,
        }))
    }
}

#[async_trait]
impl Tool for FileGrep {
    type Params = Params;
    type Output = Output;

    fn name(&self) -> &str {
        "file_grep"
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        // Validate and canonicalize the directory
        let directory = prepare_directory(&params.directory).await?;
        let dir_string = directory.to_string_lossy().to_string();

        // Clone or extract parameters we'll need in the blocking task
        let pattern = params.pattern.clone();
        let regex = params.regex;
        let case_insensitive = params.case_insensitive;
        let recursive = params.recursive;
        let max_depth_param = params.max_depth;
        let limit = params.limit;
        let follow_links = params.follow_links;
        let before_context = params.before_context;
        let after_context = params.after_context;
        let file_names_only = params.file_names_only;

        // Prepare include pattern
        let include_pattern = params
            .include
            .as_ref()
            .and_then(|pattern| Pattern::new(pattern).ok());

        // Prepare exclude patterns
        let exclude_patterns: Vec<Pattern> = params
            .exclude
            .iter()
            .filter_map(|pattern| Pattern::new(pattern).ok())
            .collect();

        // Set up the walkdir with proper configuration
        let max_depth = if recursive {
            if max_depth_param > 0 {
                max_depth_param
            } else {
                usize::MAX
            }
        } else {
            1
        };

        // Get all file paths to search in a blocking task
        let file_paths = task::spawn_blocking(move || {
            let mut paths = Vec::new();

            let walker = WalkDir::new(&directory)
                .max_depth(max_depth)
                .follow_links(follow_links);

            walker.into_iter().for_each(|entry| {
                if let Ok(entry) = entry {
                    if should_include_file(&entry, &include_pattern, &exclude_patterns) {
                        paths.push(entry.path().to_path_buf());
                    }
                }
            });

            paths
        })
        .await
        .map_err(|e| Error::Other(format!("Join error: {}", e)))?;

        let files_to_search = file_paths.len();

        // Search files in parallel using a work pool
        let mut files = Vec::new();
        let mut files_matched = 0;
        let mut total_matches = 0;
        let mut limited = false;

        for path in file_paths {
            if limit > 0 && files.len() >= limit {
                limited = true;
                break;
            }

            match search_file(
                &path,
                &pattern,
                regex,
                case_insensitive,
                before_context,
                after_context,
                file_names_only,
            )
            .await
            {
                Ok(Some(file_match)) => {
                    files_matched += 1;
                    total_matches += file_match.matches.len();
                    files.push(file_match);
                }
                Ok(None) => {}
                Err(_) => continue,
            }
        }

        // Sort by path for consistency
        files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(Output {
            directory: dir_string,
            pattern,
            files,
            files_searched: files_to_search,
            files_matched,
            total_matches,
            limited,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::debug;
    use std::sync::Mutex;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;
    
    // Global mutex to ensure only one test uses the temp directory at a time
    // This prevents race conditions during test execution
    static TEMP_DIR_MUTEX: Mutex<()> = Mutex::new(());

    /// Helper function to get a temporary test directory
    fn get_test_dir() -> PathBuf {
        let tmp_dir = std::env::temp_dir();
        tmp_dir.join(format!(
            "grep_test_{}",
            chrono::Utc::now().timestamp_millis()
        ))
    }

    /// Helper function to clean up test directories
    async fn cleanup(path: &Path) {
        if path.exists() {
            let _ = fs::remove_dir_all(path).await;
        }
    }

    /// Helper function to create a test file with content
    async fn create_test_file(path: &PathBuf, content: &str) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = File::create(path).await?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    /// Helper function to set up a test directory structure with content
    async fn setup_test_directory() -> Result<PathBuf> {
        let test_dir = get_test_dir();

        // Create directories
        fs::create_dir_all(test_dir.join("dir1")).await?;
        fs::create_dir_all(test_dir.join("dir2/subdir")).await?;

        // Create files with content containing the word "find"
        create_test_file(
            &test_dir.join("file1.txt"),
            "This file contains the word find right here.",
        )
        .await?;

        create_test_file(
            &test_dir.join("file2.log"),
            "You will not find anything interesting here.",
        )
        .await?;

        create_test_file(&test_dir.join("dir1/file3.txt"), "No matches in this file.").await?;

        create_test_file(
            &test_dir.join("dir2/file4.log"),
            "FIND is here in uppercase.",
        )
        .await?;

        create_test_file(
            &test_dir.join("dir2/subdir/file5.txt"),
            "This file has multiple find occurrences.\nYou can find them easily.",
        )
        .await?;

        Ok(test_dir)
    }

    #[tokio::test]
    async fn test_grep_basic() -> Result<()> {
        let test_dir = setup_test_directory().await?;
        let tool = FileGrep;

        // Basic grep for "find"
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "find".to_string(),
            regex: false,
            case_insensitive: false,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            include: None,
            exclude: vec![],
            line_numbers: true,
            before_context: 0,
            after_context: 0,
            file_names_only: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.files.len(), 3); // 3 files contain "find" (exact case)
        assert_eq!(result.files_matched, 3);
        assert_eq!(result.total_matches, 4); // 4 occurrences in total

        // Cleanup
        cleanup(&test_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() -> Result<()> {
        let test_dir = setup_test_directory().await?;
        let tool = FileGrep;

        // Case insensitive grep for "find"
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "find".to_string(),
            regex: false,
            case_insensitive: true,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            include: None,
            exclude: vec![],
            line_numbers: true,
            before_context: 0,
            after_context: 0,
            file_names_only: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.files.len(), 4); // Now includes the file with "FIND"
        assert_eq!(result.files_matched, 4);
        assert_eq!(result.total_matches, 5); // 5 occurrences in total

        // Cleanup
        cleanup(&test_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_grep_regex() -> Result<()> {
        // Acquire the mutex to ensure this test has exclusive access to the temp directory
        let _lock = TEMP_DIR_MUTEX.lock().expect("Failed to acquire mutex for test");
        
        // Create a custom test directory with specific content for this test
        let test_dir = get_test_dir();
        debug!("Using test directory: {}", test_dir.display());

        // Make sure we start with a clean directory
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir).map_err(|e| {
                Error::Other(format!("Failed to remove existing test directory: {}", e))
            })?;
        }

        // Create directories - explicitly check the result
        fs::create_dir_all(&test_dir).await.map_err(|e| {
            Error::Other(format!("Failed to create test directory: {}", e))
        })?;

        // Create a file with specific content for regex testing - ensure it has "find" for our test
        let test_file_path = test_dir.join("regex_test_file.txt");
        let test_content = "This file contains the word find that will match our regex.";

        create_test_file(&test_file_path, test_content).await.map_err(|e| {
            Error::Other(format!("Failed to create test file: {}", e))
        })?;
        
        // Give the filesystem time to sync
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify the file was created successfully
        assert!(test_file_path.exists(), "Test file was not created");

        let content = fs::read_to_string(&test_file_path).await?;
        assert!(!content.is_empty(), "Test file is empty");
        debug!("Test file content: {}", content);

        let tool = FileGrep;

        // Use a simpler, more explicit regex that should reliably match "find"
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "f\\w+d".to_string(), // This will match "find" on all platforms
            regex: true,
            case_insensitive: false,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            include: None,
            exclude: vec![],
            line_numbers: true,
            before_context: 0,
            after_context: 0,
            file_names_only: false,
        };

        // Execute the search
        let result = tool.execute(params).await?;

        // Debug output to help diagnose failures
        debug!("Found {} files with matches", result.files.len());
        debug!("Total matches: {}", result.total_matches);

        for file in &result.files {
            debug!("File match: {}", file.path);
            for m in &file.matches {
                debug!("  Line {}: {}", m.line_number, m.line);
            }
        }

        // Verify that we found at least one match
        assert!(
            !result.files.is_empty(),
            "Should find files with regex pattern"
        );
        assert!(
            result.total_matches > 0,
            "Should find matches with regex pattern"
        );

        // Compile the regex once before the loop
        let pattern_regex = match Regex::new("f\\w+d") {
            Ok(re) => re,
            Err(e) => {
                return Err(Error::Other(format!("Failed to compile test regex: {}", e)));
            }
        };

        // Verify matches actually match our regex
        for file in &result.files {
            for match_item in &file.matches {
                let line = &match_item.line;
                assert!(
                    pattern_regex.is_match(line),
                    "Line should match regex 'f\\w+d': {}",
                    line
                );
            }
        }

        // Cleanup - with proper error handling
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).await.map_err(|e| {
                debug!("Non-fatal error during cleanup: {}", e);
                Error::Other(format!("Failed to clean up test directory: {}", e))
            })?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_grep_with_include() -> Result<()> {
        // Create a custom test directory with specific content for this test
        let test_dir = get_test_dir();

        // Clean up any existing test directory
        if test_dir.exists() {
            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // Create directories
        fs::create_dir_all(&test_dir).await?;

        // Create files with different extensions - 2 txt files with "find" and 1 log file with "find"
        let txt_files_with_find = [
            ("file1.txt", "This is a text file with find in it."),
            ("file2.txt", "Another text file with find."),
        ];

        let log_file_with_find = ("file3.log", "This log file also has find.");

        // Create all the files
        for (name, content) in &txt_files_with_find {
            create_test_file(&test_dir.join(name), content).await?;
        }
        create_test_file(&test_dir.join(log_file_with_find.0), log_file_with_find.1).await?;

        // Verify files exist
        for (name, _) in &txt_files_with_find {
            let path = test_dir.join(name);
            assert!(path.exists(), "Test file {} wasn't created", name);
        }
        assert!(
            test_dir.join(log_file_with_find.0).exists(),
            "Log file {} wasn't created",
            log_file_with_find.0
        );

        let tool = FileGrep;

        // Grep for "find" but only in .txt files
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "find".to_string(),
            regex: false,
            case_insensitive: false,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            include: Some("*.txt".to_string()),
            exclude: vec![],
            line_numbers: true,
            before_context: 0,
            after_context: 0,
            file_names_only: false,
        };

        let result = tool.execute(params).await?;

        // Debug output to help troubleshoot across platforms
        debug!("Files matched: {}", result.files.len());
        for file in &result.files {
            debug!("Matched file: {}", file.path);
        }

        // Check that all our txt files with "find" are matched
        assert_eq!(result.files.len(), txt_files_with_find.len());

        // All files should end with .txt
        assert!(result.files.iter().all(|f| f.path.ends_with(".txt")));

        // All matched files should be one of our txt test files with "find"
        for file in &result.files {
            let file_name = Path::new(&file.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            assert!(
                txt_files_with_find
                    .iter()
                    .any(|(name, _)| file_name == *name),
                "Unexpected file matched: {}",
                file_name
            );
        }

        // Cleanup
        cleanup(&test_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_grep_with_context() -> Result<()> {
        // Create a unique temp directory for this test
        let timestamp = chrono::Utc::now().timestamp_millis();
        let test_dir = std::env::temp_dir().join(format!("grep_context_test_{}", timestamp));

        // Create the test directory
        fs::create_dir_all(&test_dir).await?;

        let tool = FileGrep;

        // Create a custom file with multi-line content for testing context
        let test_file = test_dir.join("context_test.txt");

        // Create the test file with content
        create_test_file(
            &test_file,
            "Line 1\nLine 2\nHere is find in line 3\nLine 4\nLine 5\nAnother find here in line 6\nLine 7\n"
        ).await?;

        // Verify the file exists and read back content to confirm
        let content = fs::read_to_string(&test_file).await?;
        assert!(!content.is_empty(), "Test file is empty");
        debug!("Created test file at: {}", test_file.display());
        debug!("Content: {}", content);

        // Grep for "find" with context
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "find".to_string(),
            regex: false,
            case_insensitive: false,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            include: None, // Allow all files to be searched
            exclude: vec![],
            line_numbers: true,
            before_context: 1,
            after_context: 1,
            file_names_only: false,
        };

        let result = tool.execute(params).await?;

        // Debug
        debug!("Found {} files with matches", result.files.len());
        for file in &result.files {
            debug!("File: {} has {} matches", file.path, file.matches.len());
        }

        // Find our test file among the results
        let context_file = result
            .files
            .iter()
            .find(|f| f.path.contains("context_test.txt"))
            .expect("Test file with matches not found");

        // There should be 2 matches
        assert_eq!(context_file.matches.len(), 2, "Expected 2 matches in file");

        // Both matches should have before context
        assert!(
            !context_file.matches[0].before_context.is_empty(),
            "First match should have before context"
        );
        assert!(
            !context_file.matches[1].before_context.is_empty(),
            "Second match should have before context"
        );

        // Both matches should have after context
        assert!(
            !context_file.matches[0].after_context.is_empty(),
            "First match should have after context"
        );
        assert!(
            !context_file.matches[1].after_context.is_empty(),
            "Second match should have after context"
        );

        // Manual cleanup since we created our own directory
        if test_dir.exists() {
            let _ = fs::remove_dir_all(&test_dir).await;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_grep_files_only() -> Result<()> {
        // Acquire the mutex to ensure this test has exclusive access to the temp directory
        let _lock = TEMP_DIR_MUTEX.lock().expect("Failed to acquire mutex for test");
        
        // Create a custom test directory with specific content for this test
        let test_dir = get_test_dir();
        debug!("Using test directory: {}", test_dir.display());

        // Make sure we start with a clean directory
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir).map_err(|e| {
                Error::Other(format!("Failed to remove existing test directory: {}", e))
            })?;
        }

        // Create directories - explicitly check the result
        fs::create_dir_all(&test_dir).await.map_err(|e| {
            Error::Other(format!("Failed to create test directory: {}", e))
        })?;

        // Create exactly 3 files with "find" and 1 without
        let files_with_find = [
            ("file1.txt", "This file contains the word find."),
            ("file2.txt", "Another file with find in it."),
            ("file3.txt", "Third file with find."),
        ];

        let file_without_find = ("file4.txt", "This file has no matches.");

        // Create all the files with error handling
        for (name, content) in &files_with_find {
            create_test_file(&test_dir.join(name), content).await.map_err(|e| {
                Error::Other(format!("Failed to create test file {}: {}", name, e))
            })?;
        }
        
        create_test_file(&test_dir.join(file_without_find.0), file_without_find.1).await.map_err(|e| {
            Error::Other(format!("Failed to create test file {}: {}", file_without_find.0, e))
        })?;
        
        // Give the filesystem time to sync
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify files exist
        for (name, _) in &files_with_find {
            let path = test_dir.join(name);
            assert!(path.exists(), "Test file {} wasn't created", name);
        }

        let tool = FileGrep;

        // Grep for "find" but only report file names
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "find".to_string(),
            regex: false,
            case_insensitive: false,
            recursive: true,
            max_depth: 0,
            limit: 0,
            follow_links: false,
            include: None,
            exclude: vec![],
            line_numbers: true,
            before_context: 0,
            after_context: 0,
            file_names_only: true,
        };

        let result = tool.execute(params).await?;

        // Debug output to help troubleshoot across platforms
        debug!("Files matched: {}", result.files.len());
        for file in &result.files {
            debug!("Matched file: {}", file.path);
        }

        // Check for exactly the number of files we created with "find"
        assert_eq!(result.files.len(), files_with_find.len());

        // All result files should have empty matches array
        assert!(result.files.iter().all(|f| f.matches.is_empty()));

        // All matched files should be one of our test files with "find"
        for file in &result.files {
            let file_name = Path::new(&file.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            assert!(
                files_with_find.iter().any(|(name, _)| file_name == *name),
                "Unexpected file matched: {}",
                file_name
            );
        }

        // Cleanup - with proper error handling
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).await.map_err(|e| {
                debug!("Non-fatal error during cleanup: {}", e);
                Error::Other(format!("Failed to clean up test directory: {}", e))
            })?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_grep_with_limit() -> Result<()> {
        let test_dir = setup_test_directory().await?;
        let tool = FileGrep;

        // Grep for "find" but limit to 1 file
        let params = Params {
            directory: test_dir.to_string_lossy().to_string(),
            pattern: "find".to_string(),
            regex: false,
            case_insensitive: false,
            recursive: true,
            max_depth: 0,
            limit: 1,
            follow_links: false,
            include: None,
            exclude: vec![],
            line_numbers: true,
            before_context: 0,
            after_context: 0,
            file_names_only: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.files.len(), 1);
        assert!(result.limited);

        // Cleanup
        cleanup(&test_dir).await;

        Ok(())
    }
}
