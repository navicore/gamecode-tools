//! File write tool implementation

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

use super::Tool;
use crate::{Error, Result};

/// Content type for file writing
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    /// Content is provided as text (UTF-8)
    Text,
    /// Content is provided as base64 encoded binary
    Binary,
}

impl Default for ContentType {
    fn default() -> Self {
        Self::Text
    }
}

/// File write tool
#[derive(Clone, Copy)]
pub struct FileWrite;

/// Parameters for the file write tool
#[derive(Debug, Deserialize)]
pub struct Params {
    /// Path of the file to write
    pub path: String,

    /// Content to write to the file
    pub content: String,

    /// How to interpret the provided content
    #[serde(default)]
    pub content_type: ContentType,

    /// Whether to create parent directories if they don't exist
    #[serde(default)]
    pub create_dirs: bool,
}

/// Output of the file write tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// Path of the written file
    pub path: String,

    /// Size of the written file in bytes
    pub size: u64,

    /// Type of content that was written
    pub content_type: ContentType,

    /// Whether the file was created (true) or modified (false)
    pub created: bool,
}

#[async_trait]
impl Tool for FileWrite {
    type Params = Params;
    type Output = Output;

    fn name(&self) -> &str {
        "file_write"
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        let path = PathBuf::from(&params.path);

        // Handle parent directories
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                if params.create_dirs {
                    fs::create_dir_all(parent).await?;
                } else {
                    return Err(Error::InvalidParam(format!(
                        "Parent directory does not exist: {}",
                        parent.display()
                    )));
                }
            }
        }

        // Check if the file already exists
        let created = !path.exists();

        // Write the content based on the content type
        match params.content_type {
            ContentType::Text => {
                // Write the content as text
                fs::write(&path, &params.content).await?;
            }
            ContentType::Binary => {
                // Decode the base64 content
                let binary_data = general_purpose::STANDARD
                    .decode(&params.content)
                    .map_err(|e| Error::InvalidParam(format!("Invalid base64 content: {}", e)))?;

                // Write the binary data
                fs::write(&path, binary_data).await?;
            }
        }

        // Get the file metadata
        let metadata = fs::metadata(&path).await?;
        let size = metadata.len();

        Ok(Output {
            path: params.path,
            size,
            content_type: params.content_type,
            created,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs::File;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn test_file_write_text() -> Result<()> {
        let test_file = "./test_write.txt";
        let test_content = "This is a test content.\nWith multiple lines.";

        let tool = FileWrite;

        // Test writing text content
        let params = Params {
            path: test_file.to_string(),
            content: test_content.to_string(),
            content_type: ContentType::Text,
            create_dirs: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.path, test_file);
        assert_eq!(result.size as usize, test_content.len());
        assert_eq!(result.content_type, ContentType::Text);
        assert!(result.created);

        // Verify the file was written correctly
        let mut file = File::open(test_file).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;

        assert_eq!(content, test_content);

        // Clean up
        tokio::fs::remove_file(test_file).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_file_write_binary() -> Result<()> {
        let test_file = "./test_write.bin";
        let binary_data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let base64_content = general_purpose::STANDARD.encode(binary_data);

        let tool = FileWrite;

        // Test writing binary content
        let params = Params {
            path: test_file.to_string(),
            content: base64_content,
            content_type: ContentType::Binary,
            create_dirs: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.path, test_file);
        assert_eq!(result.size as usize, binary_data.len());
        assert_eq!(result.content_type, ContentType::Binary);
        assert!(result.created);

        // Verify the file was written correctly
        let mut file = File::open(test_file).await?;
        let mut content = Vec::new();
        file.read_to_end(&mut content).await?;

        assert_eq!(content, binary_data);

        // Clean up
        tokio::fs::remove_file(test_file).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_file_write_create_dirs() -> Result<()> {
        let test_dir = "./test_dir_for_write";
        let test_file = format!("{}/nested/test_write.txt", test_dir);
        let test_content = "Content with created directories.";

        // Clean up any previous test directories first
        let dir_path = PathBuf::from(test_dir);
        if dir_path.exists() {
            tokio::fs::remove_dir_all(&dir_path).await.ok();
        }

        let tool = FileWrite;

        // Test creating directories when writing
        let params = Params {
            path: test_file.clone(),
            content: test_content.to_string(),
            content_type: ContentType::Text,
            create_dirs: true,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.path, test_file);
        assert!(result.created);

        // Verify the file and directories were created
        let path = PathBuf::from(&test_file);
        assert!(path.exists());

        // Clean up
        tokio::fs::remove_dir_all(test_dir).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_file_write_no_parent_dir() -> Result<()> {
        let test_file = "./nonexistent_dir/test_write.txt";
        let test_content = "This content won't be written.";

        let tool = FileWrite;

        // Test writing to a file with nonexistent parent directory without create_dirs
        let params = Params {
            path: test_file.to_string(),
            content: test_content.to_string(),
            content_type: ContentType::Text,
            create_dirs: false,
        };

        let result = tool.execute(params).await;

        assert!(result.is_err());

        if let Err(Error::InvalidParam(msg)) = result {
            assert!(msg.contains("Parent directory does not exist"));
        } else {
            panic!("Expected InvalidParam error");
        }

        Ok(())
    }
}
