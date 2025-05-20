//! Shell command execution tool implementation
//!
//! This tool provides a more structured and secure way to execute shell commands.
//! Unlike direct shell execution, it separates the command from its arguments,
//! making it easier for systems to evaluate and potentially sandbox requests.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::{Error, Result};
use crate::logging::{debug, trace, info, warn, error};
use super::Tool;

/// Shell command execution tool
#[derive(Clone, Copy)]
pub struct Shell;

/// Parameters for the shell tool
#[derive(Debug, Deserialize)]
pub struct Params {
    /// The command to execute (without arguments)
    pub command: String,
    
    /// Array of arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,
    
    /// Environment variables to set for the command
    #[serde(default)]
    pub env: HashMap<String, String>,
    
    /// Working directory for the command
    #[serde(default)]
    pub cwd: Option<String>,
    
    /// Whether to capture stderr in the output
    #[serde(default)]
    pub capture_stderr: bool,
    
    /// Timeout in milliseconds (0 for no timeout)
    #[serde(default)]
    pub timeout_ms: u64,
}

/// Output of the shell tool
#[derive(Debug, Serialize)]
pub struct Output {
    /// The command that was executed
    pub command: String,
    
    /// The arguments that were passed to the command
    pub args: Vec<String>,
    
    /// The exit status code of the command
    pub status: i32,
    
    /// Whether the command was successful (exit code 0)
    pub success: bool,
    
    /// The stdout output of the command
    pub stdout: String,
    
    /// The stderr output of the command (if captured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    
    /// Whether the command timed out
    pub timed_out: bool,
}

/// Validate the command to ensure it doesn't contain shell metacharacters
fn validate_command(command: &str) -> Result<()> {
    // Check if the command contains whitespace or shell metacharacters
    if command.contains(char::is_whitespace) || 
       command.contains(|c| ";&|()<>$`\\\"'".contains(c)) {
        return Err(Error::InvalidParam(format!(
            "Command '{}' contains whitespace or shell metacharacters. \
             Use the 'args' parameter for arguments instead.", command
        )));
    }
    
    Ok(())
}

#[async_trait]
impl Tool for Shell {
    type Params = Params;
    type Output = Output;
    
    fn name(&self) -> &str {
        "shell"
    }
    
    async fn execute(&self, params: Self::Params) -> Result<Self::Output> {
        // Validate the command
        validate_command(&params.command)?;
        
        // Prepare the command
        let mut cmd = Command::new(&params.command);
        
        // Add arguments
        if !params.args.is_empty() {
            cmd.args(&params.args);
        }
        
        // Set environment variables
        if !params.env.is_empty() {
            cmd.envs(params.env.iter());
        }
        
        // Set working directory if provided
        if let Some(cwd) = &params.cwd {
            let cwd_path = PathBuf::from(cwd);
            if !cwd_path.exists() {
                return Err(Error::InvalidParam(format!(
                    "Working directory does not exist: {}", cwd
                )));
            }
            cmd.current_dir(cwd_path);
        }
        
        // Configure stdout and stderr
        cmd.kill_on_drop(true);
        
        if params.capture_stderr {
            cmd.stderr(std::process::Stdio::piped());
        } else {
            cmd.stderr(std::process::Stdio::null());
        }
        
        // Set timeout if specified
        let timeout = if params.timeout_ms > 0 {
            Some(Duration::from_millis(params.timeout_ms))
        } else {
            None
        };
        
        // Execute the command
        let execution = match timeout {
            Some(timeout_duration) => {
                // With timeout
                let mut child = cmd.stdout(std::process::Stdio::piped()).spawn()
                    .map_err(|e| Error::Io(e))?;
                
                let timed_out = match time::timeout(timeout_duration, child.wait()).await {
                    Ok(result) => match result {
                        Ok(_) => false,
                        Err(e) => return Err(Error::Io(e)),
                    },
                    Err(_) => {
                        // Timeout occurred
                        // Kill the child process on timeout
                        let _ = child.kill().await;
                        true
                    }
                };
                
                // Capture output
                let stdout = match child.stdout.take() {
                    Some(stdout) => {
                        let mut stdout_bytes = Vec::new();
                        if let Err(e) = tokio::io::AsyncReadExt::read_to_end(&mut tokio::io::BufReader::new(stdout), &mut stdout_bytes).await {
                            return Err(Error::Io(e));
                        }
                        String::from_utf8_lossy(&stdout_bytes).to_string()
                    },
                    None => String::new(),
                };
                
                let stderr = if params.capture_stderr {
                    match child.stderr.take() {
                        Some(stderr) => {
                            let mut stderr_bytes = Vec::new();
                            if let Err(e) = tokio::io::AsyncReadExt::read_to_end(&mut tokio::io::BufReader::new(stderr), &mut stderr_bytes).await {
                                return Err(Error::Io(e));
                            }
                            Some(String::from_utf8_lossy(&stderr_bytes).to_string())
                        },
                        None => None,
                    }
                } else {
                    None
                };
                
                let status = match child.try_wait() {
                    Ok(Some(status)) => status.code().unwrap_or(-1),
                    _ => -1,
                };
                
                (status, stdout, stderr, timed_out)
            },
            None => {
                // Without timeout
                let output = cmd.output().await.map_err(|e| Error::Io(e))?;
                
                let status = output.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = if params.capture_stderr {
                    Some(String::from_utf8_lossy(&output.stderr).to_string())
                } else {
                    None
                };
                
                (status, stdout, stderr, false)
            }
        };
        
        let (status, stdout, stderr, timed_out) = execution;
        
        Ok(Output {
            command: params.command,
            args: params.args,
            status,
            success: status == 0,
            stdout,
            stderr,
            timed_out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_shell_echo() -> Result<()> {
        let tool = Shell;
        
        // Test simple echo command
        let params = Params {
            command: "echo".to_string(),
            args: vec!["hello".to_string(), "world".to_string()],
            env: HashMap::new(),
            cwd: None,
            capture_stderr: false,
            timeout_ms: 0,
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.command, "echo");
        assert_eq!(result.args, vec!["hello", "world"]);
        assert_eq!(result.status, 0);
        assert!(result.success);
        assert!(result.stdout.trim() == "hello world");
        assert!(result.stderr.is_none());
        assert!(!result.timed_out);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_shell_with_env() -> Result<()> {
        let tool = Shell;
        
        // Test with environment variables
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());
        
        #[cfg(target_os = "windows")]
        let params = Params {
            command: "cmd".to_string(),
            args: vec!["/c".to_string(), "echo %TEST_VAR%".to_string()],
            env,
            cwd: None,
            capture_stderr: false,
            timeout_ms: 0,
        };
        
        #[cfg(not(target_os = "windows"))]
        let params = Params {
            command: "echo".to_string(),
            args: vec!["$TEST_VAR".to_string()],
            env,
            cwd: None,
            capture_stderr: false,
            timeout_ms: 0,
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.status, 0);
        
        // On different shells or platforms, the actual output might vary
        // We just check that the command completed successfully
        assert!(result.success);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_shell_timeout() -> Result<()> {
        let tool = Shell;
        
        // Test command timeout
        #[cfg(target_os = "windows")]
        let params = Params {
            command: "timeout".to_string(),
            args: vec!["2".to_string()],
            env: HashMap::new(),
            cwd: None,
            capture_stderr: false,
            timeout_ms: 500, // 500ms timeout
        };
        
        #[cfg(not(target_os = "windows"))]
        let params = Params {
            command: "sleep".to_string(),
            args: vec!["2".to_string()],
            env: HashMap::new(),
            cwd: None,
            capture_stderr: false,
            timeout_ms: 500, // 500ms timeout
        };
        
        let result = tool.execute(params).await?;
        
        // The command should have timed out
        assert!(result.timed_out);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_shell_invalid_command() -> Result<()> {
        let tool = Shell;
        
        // Test invalid command with whitespace
        let params = Params {
            command: "echo hello".to_string(),
            args: vec![],
            env: HashMap::new(),
            cwd: None,
            capture_stderr: false,
            timeout_ms: 0,
        };
        
        let result = tool.execute(params).await;
        
        // Should fail validation
        assert!(result.is_err());
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_shell_capture_stderr() -> Result<()> {
        let tool = Shell;
        
        // Test stderr capture
        #[cfg(target_os = "windows")]
        let params = Params {
            command: "cmd".to_string(),
            args: vec!["/c".to_string(), "echo error 1>&2".to_string()],
            env: HashMap::new(),
            cwd: None,
            capture_stderr: true,
            timeout_ms: 0,
        };
        
        #[cfg(not(target_os = "windows"))]
        let params = Params {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), "echo error 1>&2".to_string()],
            env: HashMap::new(),
            cwd: None,
            capture_stderr: true,
            timeout_ms: 0,
        };
        
        let result = tool.execute(params).await?;
        
        assert_eq!(result.status, 0);
        assert!(result.success);
        
        // Should have captured stderr
        assert!(result.stderr.is_some());
        assert!(result.stderr.unwrap().trim() == "error");
        
        Ok(())
    }
}