//! File diff tool implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;
use std::path::{Path, PathBuf};
use similar::{ChangeTag, TextDiff};

use crate::{Error, Result};
use super::Tool;

/// File diff tool
#[derive(Clone, Copy)]
pub struct FileDiff;

/// Type of diff
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffType {
    /// Unified diff format (most common)
    Unified,
    /// Side-by-side diff format 
    SideBySide,
    /// Line-by-line diff
    Line,
    /// Word-by-word diff
    Word,
    /// Character-by-character diff
    Character,
}

impl Default for DiffType {
    fn default() -> Self {
        Self::Unified
    }
}

/// Parameters for the file diff tool
#[derive(Debug, Deserialize)]
pub struct Params {
    /// Path to the first file
    pub file1: String,
    
    /// Path to the second file
    pub file2: String,
    
    /// Type of diff to generate
    #[serde(default)]
    pub diff_type: DiffType,
    
    /// Context lines to include around changes
    #[serde(default = "default_context")]
    pub context_lines: usize,
    
    /// Ignore whitespace changes
    #[serde(default)]
    pub ignore_whitespace: bool,
    
    /// Ignore case changes
    #[serde(default)]
    pub ignore_case: bool,
}

fn default_context() -> usize {
    3
}

/// A line in the diff
#[derive(Debug, Serialize)]
pub struct DiffLine {
    /// Line number in file1 (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line1: Option<usize>,
    
    /// Line number in file2 (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<usize>,
    
    /// Type of change
    pub change_type: String,
    
    /// Line content
    pub content: String,
}

/// A hunk of the diff (group of changes)
#[derive(Debug, Serialize)]
pub struct DiffHunk {
    /// Start line in file1
    pub start1: usize,
    
    /// End line in file1
    pub end1: usize,
    
    /// Start line in file2
    pub start2: usize,
    
    /// End line in file2
    pub end2: usize,
    
    /// Lines in this hunk
    pub lines: Vec<DiffLine>,
}

/// Output of the file diff tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// Path to the first file
    pub file1: String,
    
    /// Path to the second file
    pub file2: String,
    
    /// Type of diff
    pub diff_type: DiffType,
    
    /// Whether the files are identical
    pub identical: bool,
    
    /// Hunks of changes
    pub hunks: Vec<DiffHunk>,
    
    /// Text representation of the diff
    pub diff_text: String,
}

/// Check if a path exists and is a file
async fn check_file_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    
    if !path.exists() {
        return Err(Error::InvalidParam(format!(
            "File not found: {}", path.display()
        )));
    }
    
    if !path.is_file() {
        return Err(Error::InvalidParam(format!(
            "Path is not a file: {}", path.display()
        )));
    }
    
    Ok(path.to_path_buf())
}

/// Generate a unified diff
fn generate_unified_diff(
    content1: &str,
    content2: &str,
    file1: &str,
    file2: &str,
    _context_lines: usize,
    ignore_whitespace: bool,
    ignore_case: bool,
) -> (Vec<DiffHunk>, String) {
    // Preprocess content if needed
    let processed1 = preprocess_content(content1, ignore_whitespace, ignore_case);
    let processed2 = preprocess_content(content2, ignore_whitespace, ignore_case);
    
    // Build the diff using the new API
    let diff = TextDiff::from_lines(&processed1, &processed2);
    
    // Create hunks
    let mut hunks = Vec::new();
    let mut line1 = 1;
    let mut line2 = 1;
    
    // Build unified diff text
    let mut diff_text = String::new();
    diff_text.push_str(&format!("--- {}\n", file1));
    diff_text.push_str(&format!("+++ {}\n", file2));
    
    // Process diff operations
    for op in diff.ops() {
        let start1 = line1;
        let start2 = line2;
        
        let mut hunk = DiffHunk {
            start1,
            end1: start1,
            start2,
            end2: start2,
            lines: Vec::new(),
        };
        
        let mut hunk_text = String::new();
        
        for change in diff.iter_changes(op) {
            match change.tag() {
                ChangeTag::Equal => {
                    hunk.lines.push(DiffLine {
                        line1: Some(line1),
                        line2: Some(line2),
                        change_type: "equal".to_string(),
                        content: change.value().to_string(),
                    });
                    
                    hunk_text.push_str(&format!(" {}", change.value()));
                    line1 += 1;
                    line2 += 1;
                    hunk.end1 = line1 - 1;
                    hunk.end2 = line2 - 1;
                },
                ChangeTag::Delete => {
                    hunk.lines.push(DiffLine {
                        line1: Some(line1),
                        line2: None,
                        change_type: "delete".to_string(),
                        content: change.value().to_string(),
                    });
                    
                    hunk_text.push_str(&format!("-{}", change.value()));
                    line1 += 1;
                    hunk.end1 = line1 - 1;
                },
                ChangeTag::Insert => {
                    hunk.lines.push(DiffLine {
                        line1: None,
                        line2: Some(line2),
                        change_type: "insert".to_string(),
                        content: change.value().to_string(),
                    });
                    
                    hunk_text.push_str(&format!("+{}", change.value()));
                    line2 += 1;
                    hunk.end2 = line2 - 1;
                },
            }
        }
        
        // Format hunk header
        let hunk_header = format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.start1, hunk.end1 - hunk.start1 + 1,
            hunk.start2, hunk.end2 - hunk.start2 + 1
        );
        
        diff_text.push_str(&hunk_header);
        diff_text.push_str(&hunk_text);
        
        hunks.push(hunk);
    }
    
    (hunks, diff_text)
}

/// Preprocess content based on diff options
fn preprocess_content(content: &str, ignore_whitespace: bool, ignore_case: bool) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut processed_lines = Vec::new();
    
    for line in lines {
        let mut processed = line.to_string();
        
        if ignore_whitespace {
            // Replace multiple whitespaces with a single space
            processed = processed.split_whitespace().collect::<Vec<_>>().join(" ");
        }
        
        if ignore_case {
            processed = processed.to_lowercase();
        }
        
        processed_lines.push(processed);
    }
    
    processed_lines.join("\n")
}

#[async_trait]
impl Tool for FileDiff {
    type Params = Params;
    type Output = Output;
    
    fn name(&self) -> &str {
        "file_diff"
    }
    
    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        // Validate file paths
        let file1_path = check_file_path(&params.file1).await?;
        let file2_path = check_file_path(&params.file2).await?;
        
        // Read file contents
        let content1 = fs::read_to_string(&file1_path).await?;
        let content2 = fs::read_to_string(&file2_path).await?;
        
        // Check if files are identical (after preprocessing)
        let processed1 = preprocess_content(&content1, params.ignore_whitespace, params.ignore_case);
        let processed2 = preprocess_content(&content2, params.ignore_whitespace, params.ignore_case);
        
        // Compare processed content for equality
        let identical = processed1 == processed2;
        
        // Generate diff based on type
        let (hunks, diff_text) = match params.diff_type {
            DiffType::Unified => generate_unified_diff(
                &content1, 
                &content2, 
                &params.file1, 
                &params.file2,
                params.context_lines,
                params.ignore_whitespace,
                params.ignore_case,
            ),
            // For now, fallback to unified diff for other formats
            // In a real implementation, we would add specialized diff generators for each format
            _ => generate_unified_diff(
                &content1, 
                &content2, 
                &params.file1, 
                &params.file2,
                params.context_lines,
                params.ignore_whitespace,
                params.ignore_case,
            ),
        };
        
        Ok(Output {
            file1: params.file1,
            file2: params.file2,
            diff_type: params.diff_type,
            identical,
            hunks,
            diff_text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;
    
    /// Helper function to create a temporary test directory
    fn get_test_dir() -> PathBuf {
        let tmp_dir = std::env::temp_dir();
        let timestamp = chrono::Utc::now().timestamp_millis();
        tmp_dir.join(format!("diff_test_{}", timestamp))
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
    
    #[tokio::test]
    async fn test_diff_identical_files() -> Result<()> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let test_dir = std::env::temp_dir().join(format!("diff_test_identical_{}", timestamp));
        
        // Ensure directory is clean
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        
        let file1 = test_dir.join("file1.txt");
        let file2 = test_dir.join("file2.txt");
        let content = "Line 1\nLine 2\nLine 3\n";
        
        // Create test files
        fs::create_dir_all(&test_dir).await?;
        create_test_file(&file1, content).await?;
        create_test_file(&file2, content).await?;
        
        // Verify files were created and have the right content
        let content1 = fs::read_to_string(&file1).await?;
        let content2 = fs::read_to_string(&file2).await?;
        
        assert_eq!(content1, content);
        assert_eq!(content2, content);
        
        let tool = FileDiff;
        
        // Compare identical files
        let params = Params {
            file1: file1.to_string_lossy().to_string(),
            file2: file2.to_string_lossy().to_string(),
            diff_type: DiffType::Unified,
            context_lines: 3,
            ignore_whitespace: false,
            ignore_case: false,
        };
        
        let result = tool.execute(params).await?;
        
        // Print the contents for debugging
        println!("File 1: '{}'", content1);
        println!("File 2: '{}'", content2);
        println!("Processed1: '{}'", preprocess_content(&content1, false, false));
        println!("Processed2: '{}'", preprocess_content(&content2, false, false));
        println!("Identical: {}", result.identical);
        
        // The files are definitely identical
        assert!(result.identical, "Files should be identical");
        
        // Clean up
        std::fs::remove_dir_all(&test_dir).ok();
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_diff_different_files() -> Result<()> {
        let test_dir = get_test_dir();
        let file1 = test_dir.join("file1.txt");
        let file2 = test_dir.join("file2.txt");
        
        // Create test files with differences
        fs::create_dir_all(&test_dir).await?;
        create_test_file(&file1, "Line 1\nLine 2\nLine 3\n").await?;
        create_test_file(&file2, "Line 1\nModified Line\nLine 3\n").await?;
        
        let tool = FileDiff;
        
        // Compare different files
        let params = Params {
            file1: file1.to_string_lossy().to_string(),
            file2: file2.to_string_lossy().to_string(),
            diff_type: DiffType::Unified,
            context_lines: 3,
            ignore_whitespace: false,
            ignore_case: false,
        };
        
        let result = tool.execute(params).await?;
        
        assert!(!result.identical);
        assert!(!result.hunks.is_empty());
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_diff_with_ignore_whitespace() -> Result<()> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let test_dir = std::env::temp_dir().join(format!("diff_test_whitespace_{}", timestamp));
        
        // Ensure directory is clean
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        
        let file1 = test_dir.join("file1.txt");
        let file2 = test_dir.join("file2.txt");
        
        // Create test files with whitespace differences
        fs::create_dir_all(&test_dir).await?;
        create_test_file(&file1, "Line 1\nLine 2\nLine 3\n").await?;
        create_test_file(&file2, "Line 1\nLine   2\nLine 3\n").await?;
        
        // Verify files were created with the right content
        let content1 = fs::read_to_string(&file1).await?;
        let content2 = fs::read_to_string(&file2).await?;
        
        assert_eq!(content1, "Line 1\nLine 2\nLine 3\n");
        assert_eq!(content2, "Line 1\nLine   2\nLine 3\n");
        assert_ne!(content1, content2, "Contents should be different");
        
        let tool = FileDiff;
        
        // Compare with ignore_whitespace = false
        let params = Params {
            file1: file1.to_string_lossy().to_string(),
            file2: file2.to_string_lossy().to_string(),
            diff_type: DiffType::Unified,
            context_lines: 3,
            ignore_whitespace: false,
            ignore_case: false,
        };
        
        let result = tool.execute(params).await?;
        
        // Print debugging info
        println!("With ignore_whitespace=false:");
        println!("File 1: '{}'", content1);
        println!("File 2: '{}'", content2);
        println!("Processed1: '{}'", preprocess_content(&content1, false, false));
        println!("Processed2: '{}'", preprocess_content(&content2, false, false));
        
        assert!(!result.identical, "Files should be different without ignore_whitespace");
        
        // Compare with ignore_whitespace = true
        let params = Params {
            file1: file1.to_string_lossy().to_string(),
            file2: file2.to_string_lossy().to_string(),
            diff_type: DiffType::Unified,
            context_lines: 3,
            ignore_whitespace: true,
            ignore_case: false,
        };
        
        let result = tool.execute(params).await?;
        
        // Print debugging info
        println!("With ignore_whitespace=true:");
        println!("Processed1: '{}'", preprocess_content(&content1, true, false));
        println!("Processed2: '{}'", preprocess_content(&content2, true, false));
        
        assert!(result.identical, "Files should be identical with ignore_whitespace");
        
        // Clean up
        std::fs::remove_dir_all(&test_dir).ok();
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_diff_with_ignore_case() -> Result<()> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let test_dir = std::env::temp_dir().join(format!("diff_test_case_{}", timestamp));
        
        // Ensure directory is clean
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        
        let file1 = test_dir.join("file1.txt");
        let file2 = test_dir.join("file2.txt");
        
        // Create test files with case differences
        fs::create_dir_all(&test_dir).await?;
        create_test_file(&file1, "Line 1\nLine 2\nLine 3\n").await?;
        create_test_file(&file2, "Line 1\nLINE 2\nLine 3\n").await?;
        
        // Verify files were created with the right content
        let content1 = fs::read_to_string(&file1).await?;
        let content2 = fs::read_to_string(&file2).await?;
        
        assert_eq!(content1, "Line 1\nLine 2\nLine 3\n");
        assert_eq!(content2, "Line 1\nLINE 2\nLine 3\n");
        assert_ne!(content1, content2, "Contents should be different");
        
        let tool = FileDiff;
        
        // Compare with ignore_case = false
        let params = Params {
            file1: file1.to_string_lossy().to_string(),
            file2: file2.to_string_lossy().to_string(),
            diff_type: DiffType::Unified,
            context_lines: 3,
            ignore_whitespace: false,
            ignore_case: false,
        };
        
        let result = tool.execute(params).await?;
        
        // Print debugging info
        println!("With ignore_case=false:");
        println!("File 1: '{}'", content1);
        println!("File 2: '{}'", content2);
        println!("Processed1: '{}'", preprocess_content(&content1, false, false));
        println!("Processed2: '{}'", preprocess_content(&content2, false, false));
        
        assert!(!result.identical, "Files should be different without ignore_case");
        
        // Compare with ignore_case = true
        let params = Params {
            file1: file1.to_string_lossy().to_string(),
            file2: file2.to_string_lossy().to_string(),
            diff_type: DiffType::Unified,
            context_lines: 3,
            ignore_whitespace: false,
            ignore_case: true,
        };
        
        let result = tool.execute(params).await?;
        
        // Print debugging info
        println!("With ignore_case=true:");
        println!("Processed1: '{}'", preprocess_content(&content1, false, true));
        println!("Processed2: '{}'", preprocess_content(&content2, false, true));
        
        assert!(result.identical, "Files should be identical with ignore_case");
        
        // Clean up
        std::fs::remove_dir_all(&test_dir).ok();
        
        Ok(())
    }
}