//! Directory make tool implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;
use std::path::PathBuf;

use crate::{Error, Result};
use super::Tool;

/// Directory make tool
#[derive(Clone, Copy)]
pub struct DirectoryMake;

/// Parameters for the directory make tool
#[derive(Debug, Deserialize)]
pub struct Params {
    /// Path of the directory to create
    pub path: String,
    
    /// Whether to create parent directories if they don't exist
    #[serde(default)]
    pub parents: bool,
    
    /// Don't throw an error if the directory already exists
    #[serde(default)]
    pub exist_ok: bool,
}

/// Output of the directory make tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// Path of the created directory
    pub path: String,
    
    /// Whether the directory was created (true) or already existed (false)
    pub created: bool,
}

#[async_trait]
impl Tool for DirectoryMake {
    type Params = Params;
    type Output = Output;
    
    fn name(&self) -> &str {
        "directory_make"
    }
    
    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        let path = PathBuf::from(&params.path);
        
        // Check if the directory already exists
        let already_exists = path.exists();
        
        if already_exists {
            if !path.is_dir() {
                return Err(Error::InvalidParam(format!(
                    "Path exists but is not a directory: {}", params.path
                )));
            }
            
            if !params.exist_ok {
                return Err(Error::InvalidParam(format!(
                    "Directory already exists: {}", params.path
                )));
            }
            
            // Directory exists and exist_ok is true
            return Ok(Output {
                path: params.path,
                created: false,
            });
        }
        
        // Create the directory
        let result = if params.parents {
            fs::create_dir_all(&path).await
        } else {
            fs::create_dir(&path).await
        };
        
        // Handle creation errors
        match result {
            Ok(_) => Ok(Output {
                path: params.path,
                created: true,
            }),
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    Err(Error::InvalidParam(format!(
                        "Parent directory does not exist: {}", 
                        path.parent().unwrap_or(&path).display()
                    )))
                },
                std::io::ErrorKind::PermissionDenied => {
                    Err(Error::PermissionDenied(format!(
                        "Permission denied: {}", params.path
                    )))
                },
                _ => Err(Error::Io(e)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;
    use std::path::Path;
    
    /// Helper function to create a temporary test directory path
    fn get_test_dir() -> PathBuf {
        let tmp_dir = std::env::temp_dir();
        let dir_name = format!("mkdir_test_{}", chrono::Utc::now().timestamp_millis());
        let path = tmp_dir.join(dir_name);
        
        // Make sure this path doesn't exist already
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        
        path
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
    async fn test_directory_make_simple() -> Result<()> {
        // Create a unique directory path
        let timestamp = chrono::Utc::now().timestamp_millis();
        let test_dir = std::env::temp_dir().join(format!("simple_mkdir_test_{}", timestamp));
        
        // Make sure it doesn't exist
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        
        // Double-check
        assert!(!test_dir.exists(), "Test directory still exists after cleanup");
        
        let tool = DirectoryMake;
        
        // Test creating a simple directory
        let path_str = test_dir.to_string_lossy().to_string();
        let params = Params {
            path: path_str.clone(),
            parents: false,
            exist_ok: false,
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.path, path_str);
        assert!(result.created);
        assert!(test_dir.exists());
        
        // Clean up
        std::fs::remove_dir_all(&test_dir).ok();
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_directory_make_nested() -> Result<()> {
        let test_dir = get_test_dir();
        let tool = DirectoryMake;
        
        // Clean up any existing test directory
        cleanup(&test_dir).await;
        
        // Wait a moment to ensure cleanup is complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Make sure the directory doesn't exist
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).await?;
        }
        
        // Create base directory first
        fs::create_dir(&test_dir).await?;
        
        // Create a nested directory path
        let nested_dir = test_dir.join("nested").join("deeply").join("directory");
        let nested_path = nested_dir.to_string_lossy().to_string();
        
        // Test creating a nested directory with parents
        let params = Params {
            path: nested_path.clone(),
            parents: true,
            exist_ok: false,
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.path, nested_path);
        assert!(result.created);
        assert!(nested_dir.exists());
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_directory_make_no_parents() -> Result<()> {
        let test_dir = get_test_dir();
        let nested_dir = test_dir.join("nonexistent/directory");
        let tool = DirectoryMake;
        
        // Clean up any existing test directory
        cleanup(&test_dir).await;
        
        // Test creating a nested directory without parents
        let params = Params {
            path: nested_dir.to_string_lossy().to_string(),
            parents: false,
            exist_ok: false,
        };
        
        let result = tool.execute(params).await;
        
        // Should fail because parent doesn't exist
        assert!(result.is_err());
        
        // Clean up
        cleanup(&test_dir).await;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_directory_make_existing() -> Result<()> {
        // Create a unique test directory with timestamp
        let timestamp = chrono::Utc::now().timestamp_millis();
        let test_dir = std::env::temp_dir().join(format!("mkdir_exist_test_{}", timestamp));
        let tool = DirectoryMake;
        
        // Clean up any existing directory
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        
        // Create the directory first
        fs::create_dir_all(&test_dir).await?;
        
        // Verify the directory exists
        assert!(test_dir.exists(), "Test directory not created");
        assert!(test_dir.is_dir(), "Test path is not a directory");
        
        // Wait for filesystem operations to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Try to create it again with exist_ok=false
        let params = Params {
            path: test_dir.to_string_lossy().to_string(),
            parents: false,
            exist_ok: false,
        };
        
        let result = tool.execute(params).await;
        
        // Should fail because directory already exists
        assert!(result.is_err(), "Creating an existing directory should fail with exist_ok=false");
        
        // Try to create it again with exist_ok=true
        let params = Params {
            path: test_dir.to_string_lossy().to_string(),
            parents: false,
            exist_ok: true,
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.path, test_dir.to_string_lossy().to_string());
        assert!(!result.created); // It wasn't newly created
        
        // Clean up directly without relying on the common cleanup function
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir).ok();
        }
        
        Ok(())
    }
}