//! Tools module with all tool implementations.

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use crate::Result;

/// Base trait for all tool implementations
#[async_trait]
pub trait Tool {
    /// The parameter type for the tool
    type Params: DeserializeOwned + Send + Sync;

    /// The result type for the tool
    type Output: Serialize + Send;

    /// The name of the tool
    fn name(&self) -> &str;

    /// Execute the tool with the given parameters
    async fn execute(&self, params: Self::Params) -> Result<Self::Output>;
}

pub mod directory_list;
pub mod directory_make;
pub mod file_diff;
pub mod file_find;
pub mod file_grep;
pub mod file_move;
pub mod file_patch;
pub mod file_read;
pub mod file_write;
pub mod shell;
