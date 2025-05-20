//! File read tool implementation

use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

use super::Tool;
use crate::{Error, Result};

/// Content type for file reading
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    /// Return file as text (UTF-8)
    Text,
    /// Return file as base64 encoded binary
    Binary,
    /// Auto-detect based on file extension
    Auto,
}

impl Default for ContentType {
    fn default() -> Self {
        Self::Auto
    }
}

/// File read tool
#[derive(Clone, Copy)]
pub struct FileRead;

/// Parameters for the file read tool
#[derive(Debug, Deserialize)]
pub struct Params {
    /// Path of the file to read
    pub path: String,

    /// How to interpret the file content
    #[serde(default)]
    pub content_type: ContentType,

    /// Optional line offset to start reading from (only applies to text)
    #[serde(default)]
    pub offset: Option<usize>,

    /// Optional limit on the number of lines to read (only applies to text)
    #[serde(default)]
    pub limit: Option<usize>,

    /// Whether to include line numbers in the output (only applies to text)
    #[serde(default)]
    pub line_numbers: bool,
}

/// Output of the file read tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// Content of the file (text or base64 encoded)
    pub content: String,

    /// Size of the file in bytes
    pub size: u64,

    /// MIME type of the file
    pub mime_type: String,

    /// Type of content returned (text or binary)
    pub content_type: ContentType,

    /// Total number of lines in the file (if text and line numbers were requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<usize>,
}

/// Guess the MIME type from a file extension
fn guess_mime_type(path: &PathBuf) -> String {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match extension.to_lowercase().as_str() {
        // Text formats
        "txt" | "md" | "rs" | "js" | "ts" | "json" | "yml" | "yaml" | "toml" | "html" | "css" => {
            "text/plain".to_string()
        }

        // Image formats
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "png" => "image/png".to_string(),
        "gif" => "image/gif".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "webp" => "image/webp".to_string(),

        // Binary formats
        "pdf" => "application/pdf".to_string(),
        "zip" => "application/zip".to_string(),
        "gz" => "application/gzip".to_string(),
        "tar" => "application/x-tar".to_string(),
        "exe" => "application/octet-stream".to_string(),
        "dll" => "application/octet-stream".to_string(),

        // Default
        _ => "application/octet-stream".to_string(),
    }
}

/// Determine if a file should be read as text based on MIME type
fn is_text_mime_type(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
        || mime_type == "application/json"
        || mime_type == "application/xml"
        || mime_type == "application/javascript"
}

#[async_trait]
impl Tool for FileRead {
    type Params = Params;
    type Output = Output;

    fn name(&self) -> &str {
        "file_read"
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        let path = PathBuf::from(&params.path);

        // Check if the file exists
        if !path.exists() {
            return Err(Error::InvalidParam(format!(
                "File not found: {}",
                params.path
            )));
        }

        // Check if it's a file (not a directory)
        let metadata = fs::metadata(&path).await?;
        if !metadata.is_file() {
            return Err(Error::InvalidParam(format!(
                "Path is not a file: {}",
                params.path
            )));
        }

        // Get file size
        let size = metadata.len();

        // Determine MIME type
        let mime_type = guess_mime_type(&path);

        // Determine content type based on params and MIME type
        let effective_content_type = match params.content_type {
            ContentType::Text => ContentType::Text,
            ContentType::Binary => ContentType::Binary,
            ContentType::Auto => {
                if is_text_mime_type(&mime_type) {
                    ContentType::Text
                } else {
                    ContentType::Binary
                }
            }
        };

        match effective_content_type {
            ContentType::Text => {
                // Read the file as text
                let content = fs::read_to_string(&path).await.map_err(|e| Error::Io(e))?;

                // Process line numbers if requested
                let lines: Vec<&str> = content.lines().collect();
                let line_count = if params.line_numbers {
                    Some(lines.len())
                } else {
                    None
                };

                // Apply offset and limit if specified
                let processed_content = if params.offset.is_some() || params.limit.is_some() {
                    let offset = params.offset.unwrap_or(0);
                    let limit = params.limit.unwrap_or(lines.len().saturating_sub(offset));

                    if offset >= lines.len() {
                        "".to_string()
                    } else {
                        let end = (offset + limit).min(lines.len());

                        if params.line_numbers {
                            // Format with line numbers
                            lines[offset..end]
                                .iter()
                                .enumerate()
                                .map(|(i, line)| format!("{:>6}  {}", offset + i + 1, line))
                                .collect::<Vec<String>>()
                                .join("\n")
                        } else {
                            // Format without line numbers
                            lines[offset..end].join("\n")
                        }
                    }
                } else if params.line_numbers {
                    // Format the entire file with line numbers
                    lines
                        .iter()
                        .enumerate()
                        .map(|(i, line)| format!("{:>6}  {}", i + 1, line))
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    // Return the entire file as is
                    content
                };

                Ok(Output {
                    content: processed_content,
                    size,
                    mime_type,
                    content_type: ContentType::Text,
                    line_count,
                })
            }
            ContentType::Binary => {
                // Read the file as binary
                let content = fs::read(&path).await?;

                // Encode as base64
                let base64_content = general_purpose::STANDARD.encode(&content);

                Ok(Output {
                    content: base64_content,
                    size,
                    mime_type,
                    content_type: ContentType::Binary,
                    line_count: None,
                })
            }
            ContentType::Auto => {
                // This case should never happen since we've already converted Auto to Text or Binary
                // But we need to handle it for the compiler
                unreachable!("Auto content type should have been converted to Text or Binary")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    async fn create_test_text_file() -> io::Result<PathBuf> {
        let test_file = PathBuf::from("./test_file_read.txt");
        let mut file = File::create(&test_file).await?;

        // Write multiple lines to the file
        file.write_all(b"Line 1\nLine 2\nLine 3\nLine 4\nLine 5")
            .await?;
        file.flush().await?;

        Ok(test_file)
    }

    async fn create_test_binary_file() -> io::Result<PathBuf> {
        let test_file = PathBuf::from("./test_file_read.bin");
        let mut file = File::create(&test_file).await?;

        // Write some binary data
        file.write_all(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]).await?;
        file.flush().await?;

        Ok(test_file)
    }

    #[tokio::test]
    async fn test_file_read_text() -> Result<()> {
        let test_file = create_test_text_file().await.unwrap();

        let tool = FileRead;

        // Test reading the entire file as text
        let params = Params {
            path: test_file.to_string_lossy().to_string(),
            content_type: ContentType::Text,
            offset: None,
            limit: None,
            line_numbers: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.content, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5");
        assert_eq!(result.content_type, ContentType::Text);
        assert!(result.line_count.is_none());

        // Get the actual size from the file
        let metadata = tokio::fs::metadata(&test_file).await?;
        assert_eq!(result.size, metadata.len());

        // Test with line numbers
        let params = Params {
            path: test_file.to_string_lossy().to_string(),
            content_type: ContentType::Text,
            offset: None,
            limit: None,
            line_numbers: true,
        };

        let result = tool.execute(params).await?;

        assert_eq!(
            result.content,
            "     1  Line 1\n     2  Line 2\n     3  Line 3\n     4  Line 4\n     5  Line 5"
        );
        assert_eq!(result.line_count, Some(5));

        // Test with offset and limit
        let params = Params {
            path: test_file.to_string_lossy().to_string(),
            content_type: ContentType::Text,
            offset: Some(1),
            limit: Some(2),
            line_numbers: false,
        };

        let result = tool.execute(params).await?;

        assert_eq!(result.content, "Line 2\nLine 3");

        // Clean up
        tokio::fs::remove_file(test_file).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_file_read_binary() -> Result<()> {
        let test_file = create_test_binary_file().await.unwrap();

        let tool = FileRead;

        // Test reading the file as binary
        let params = Params {
            path: test_file.to_string_lossy().to_string(),
            content_type: ContentType::Binary,
            offset: None,
            limit: None,
            line_numbers: false,
        };

        let result = tool.execute(params).await?;

        // The binary content should be base64 encoded
        assert_eq!(result.content, "AAECAwQFBgcICQ==");
        assert_eq!(result.content_type, ContentType::Binary);
        assert!(result.line_count.is_none());

        // Get the actual size from the file
        let metadata = tokio::fs::metadata(&test_file).await?;
        assert_eq!(result.size, metadata.len());

        // Test auto-detection with binary file
        let params = Params {
            path: test_file.to_string_lossy().to_string(),
            content_type: ContentType::Auto,
            offset: None,
            limit: None,
            line_numbers: false,
        };

        let result = tool.execute(params).await?;

        // Should detect as binary
        assert_eq!(result.content_type, ContentType::Binary);

        // Clean up
        tokio::fs::remove_file(test_file).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_file_not_found() -> Result<()> {
        let tool = FileRead;

        let params = Params {
            path: "nonexistent_file.txt".to_string(),
            content_type: ContentType::Auto,
            offset: None,
            limit: None,
            line_numbers: false,
        };

        let result = tool.execute(params).await;

        assert!(result.is_err());

        if let Err(Error::InvalidParam(msg)) = result {
            assert!(msg.contains("File not found"));
        } else {
            panic!("Expected InvalidParam error");
        }

        Ok(())
    }
}
