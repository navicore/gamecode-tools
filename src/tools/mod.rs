//! Tools module with all tool implementations.

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

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