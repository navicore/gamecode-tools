//! File move tool implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::path::PathBuf;
use tokio::fs;

use super::Tool;
use crate::{Error, Result};

/// File move tool
#[derive(Clone, Copy)]
pub struct FileMove;

/// Parameters for the file move tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct Params {
    /// Source path
    pub source: String,

    /// Destination path
    pub destination: String,

    /// Whether to overwrite the destination if it exists
    #[serde(default)]
    pub overwrite: bool,

    /// Whether to create parent directories of the destination if they don't exist
    #[serde(default)]
    pub create_dirs: bool,
}

/// Output of the file move tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// Source path
    pub source: String,

    /// Destination path
    pub destination: String,

    /// Whether the destination was overwritten
    pub overwritten: bool,
}

#[async_trait]
impl Tool for FileMove {
    type Params = Params;
    type Output = Output;

    fn name(&self) -> &str {
        "file_move"
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        let source = PathBuf::from(&params.source);
        let destination = PathBuf::from(&params.destination);

        // Check if the source exists
        if !source.exists() {
            return Err(Error::InvalidParam(format!(
                "Source not found: {}",
                params.source
            )));
        }

        // Create parent directories if requested
        if params.create_dirs {
            if let Some(parent) = destination.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    fs::create_dir_all(parent).await?;
                }
            }
        } else if let Some(parent) = destination.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(Error::InvalidParam(format!(
                    "Destination parent directory does not exist: {}",
                    parent.display()
                )));
            }
        }

        // Check if the destination exists
        let dest_exists = destination.exists();

        if dest_exists && !params.overwrite {
            return Err(Error::InvalidParam(format!(
                "Destination already exists: {}",
                params.destination
            )));
        }

        // For overwrite operations, we need to remove the destination first
        // because rename can fail on some platforms when destination exists
        if dest_exists && params.overwrite {
            fs::remove_file(&destination).await?;
        }

        // Perform the move operation
        fs::rename(&source, &destination).await?;

        Ok(Output {
            source: params.source,
            destination: params.destination,
            overwritten: dest_exists,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    /// Helper function to create a test file with content
    async fn create_test_file(path: &PathBuf, content: &str) -> std::io::Result<()> {
        let mut file = File::create(path).await?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        Ok(())
    }

    /// Helper function to get a temporary test directory
    fn get_test_dir() -> PathBuf {
        let tmp_dir = std::env::temp_dir();
        tmp_dir.join(format!(
            "move_test_{}",
            chrono::Utc::now().timestamp_millis()
        ))
    }

    /// Helper function to clean up test directories
    async fn cleanup(path: &Path) {
        if path.exists() {
            // Try multiple times in case of file locking issues
            for _ in 0..3 {
                if let Ok(()) = fs::remove_dir_all(path).await {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
    }

    #[tokio::test]
    async fn test_file_move_simple() -> Result<()> {
        // Create a unique test directory
        let timestamp = chrono::Utc::now().timestamp_millis();
        let test_dir = std::env::temp_dir().join(format!("simple_move_test_{}", timestamp));

        // Ensure the directory is clean
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }

        // Create the directory
        fs::create_dir_all(&test_dir).await?;

        // Create source file with unique names to avoid conflicts
        let source_file = test_dir.join(format!("source_{}.txt", timestamp));
        let dest_file = test_dir.join(format!("destination_{}.txt", timestamp));

        // Create the source file
        create_test_file(&source_file, "Test content").await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify setup
        assert!(
            source_file.exists(),
            "Source file doesn't exist before move"
        );
        assert!(!dest_file.exists(), "Destination file exists before move");

        let tool = FileMove;

        // Move the file
        let params = Params {
            source: source_file.to_string_lossy().to_string(),
            destination: dest_file.to_string_lossy().to_string(),
            overwrite: false,
            create_dirs: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.source, source_file.to_string_lossy().to_string());
        assert_eq!(result.destination, dest_file.to_string_lossy().to_string());
        assert!(!result.overwritten);

        // Verify source doesn't exist and destination does
        assert!(!source_file.exists());
        assert!(dest_file.exists());

        // Verify content was moved
        let content = fs::read_to_string(&dest_file).await?;
        assert_eq!(content, "Test content");

        // Clean up
        cleanup(&test_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_file_move_overwrite() -> Result<()> {
        let test_dir = get_test_dir();

        // Clean up any existing test directory
        cleanup(&test_dir).await;

        // Wait a moment to ensure cleanup is complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Make sure the directory doesn't exist
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).await?;
        }

        // Create test directory
        fs::create_dir_all(&test_dir).await?;

        // Create source and destination files
        let source_file = test_dir.join("source.txt");
        let dest_file = test_dir.join("destination.txt");

        create_test_file(&source_file, "Source content").await?;
        create_test_file(&dest_file, "Destination content").await?;

        // Wait for filesystem to complete writes
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify files exist before trying to move
        assert!(
            source_file.exists(),
            "Source file doesn't exist before move"
        );
        assert!(
            dest_file.exists(),
            "Destination file doesn't exist before move"
        );

        let tool = FileMove;

        // Try to move without overwrite
        let params = Params {
            source: source_file.to_string_lossy().to_string(),
            destination: dest_file.to_string_lossy().to_string(),
            overwrite: false,
            create_dirs: false,
        };

        let result = tool.execute(params).await;

        // Should fail because destination exists
        assert!(result.is_err());

        // Create a new source file since the previous attempt might have removed or corrupted it
        create_test_file(&source_file, "Source content").await?;

        // Wait for filesystem to complete write
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify source exists again
        assert!(
            source_file.exists(),
            "Source file doesn't exist before second move attempt"
        );

        // Try to move with overwrite
        let params = Params {
            source: source_file.to_string_lossy().to_string(),
            destination: dest_file.to_string_lossy().to_string(),
            overwrite: true,
            create_dirs: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.source, source_file.to_string_lossy().to_string());
        assert_eq!(result.destination, dest_file.to_string_lossy().to_string());
        assert!(result.overwritten);

        // Verify content was overwritten
        let content = fs::read_to_string(&dest_file).await?;
        assert_eq!(content, "Source content");

        // Clean up
        cleanup(&test_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn test_file_move_create_dirs() -> Result<()> {
        // Create a unique test directory
        let timestamp = chrono::Utc::now().timestamp_millis();
        let test_dir = std::env::temp_dir().join(format!("dirs_move_test_{}", timestamp));

        // Ensure the directory is clean
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }

        // Create source file with unique names
        let source_file = test_dir.join(format!("source_{}.txt", timestamp));
        let dest_dir = test_dir.join(format!("nested_{}/deeply/directory", timestamp));
        let dest_file = dest_dir.join(format!("destination_{}.txt", timestamp));

        // Create test directory and source
        fs::create_dir_all(&test_dir).await?;
        create_test_file(&source_file, "Test content").await?;

        // Wait for filesystem
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify source exists
        assert!(
            source_file.exists(),
            "Source file doesn't exist before move"
        );

        let tool = FileMove;

        // Move the file with create_dirs
        let params = Params {
            source: source_file.to_string_lossy().to_string(),
            destination: dest_file.to_string_lossy().to_string(),
            overwrite: false,
            create_dirs: true,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.source, source_file.to_string_lossy().to_string());
        assert_eq!(result.destination, dest_file.to_string_lossy().to_string());
        assert!(!result.overwritten);

        // Verify directory was created and file was moved
        assert!(dest_dir.exists());
        assert!(dest_file.exists());

        // Clean up
        std::fs::remove_dir_all(&test_dir).ok();

        Ok(())
    }

    #[tokio::test]
    async fn test_file_move_no_source() -> Result<()> {
        let test_dir = get_test_dir();
        let source_file = test_dir.join("nonexistent.txt");
        let dest_file = test_dir.join("destination.txt");

        // Create test directory
        fs::create_dir_all(&test_dir).await?;

        let tool = FileMove;

        // Try to move a nonexistent file
        let params = Params {
            source: source_file.to_string_lossy().to_string(),
            destination: dest_file.to_string_lossy().to_string(),
            overwrite: false,
            create_dirs: false,
        };

        let result = tool.execute(params).await;

        // Should fail because source doesn't exist
        assert!(result.is_err());

        // Clean up
        cleanup(&test_dir).await;

        Ok(())
    }
}
