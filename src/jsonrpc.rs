//! JSONRPC protocol structures and handling.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::Arc;

use crate::Error;
use crate::Result;
use crate::transform::{FormatTransformer, serialize, deserialize};

/// JSONRPC request structure
#[derive(Debug, Deserialize, Serialize)]
pub struct Request<P> {
    /// JSONRPC version (should be "2.0")
    pub jsonrpc: String,
    /// Method name
    pub method: String,
    /// Method parameters
    pub params: P,
    /// Request ID
    pub id: serde_json::Value,
}

/// Untyped JSONRPC request with JSON Value parameters
#[derive(Debug, Deserialize, Serialize)]
pub struct RawRequest {
    /// JSONRPC version (should be "2.0")
    pub jsonrpc: String,
    /// Method name
    pub method: String,
    /// Method parameters as raw JSON
    pub params: serde_json::Value,
    /// Request ID
    pub id: serde_json::Value,
}

/// JSONRPC success response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct SuccessResponse<T> {
    /// JSONRPC version (should be "2.0")
    pub jsonrpc: String,
    /// Response result
    pub result: T,
    /// Request ID (same as in the request)
    pub id: serde_json::Value,
}

/// JSONRPC error response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    /// JSONRPC version (should be "2.0")
    pub jsonrpc: String,
    /// Error details
    pub error: RpcError,
    /// Request ID (same as in the request)
    pub id: serde_json::Value,
}

/// JSONRPC error object
#[derive(Debug, Deserialize, Serialize)]
pub struct RpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSONRPC response (either success or error)
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Response<T> {
    /// Success response
    Success(SuccessResponse<T>),
    /// Error response
    Error(ErrorResponse),
}

/// Create a JSONRPC success response
pub fn success<T>(result: T, id: serde_json::Value) -> Response<T> {
    Response::Success(SuccessResponse {
        jsonrpc: "2.0".to_string(),
        result,
        id,
    })
}

/// Create a JSONRPC error response
pub fn error<T>(error: Error, id: serde_json::Value) -> Response<T> {
    let (code, message) = match &error {
        Error::Io(err) => (-32000, format!("I/O error: {}", err)),
        Error::Json(err) => (-32700, format!("Parse error: {}", err)),
        Error::InvalidParam(msg) => (-32602, format!("Invalid params: {}", msg)),
        Error::PermissionDenied(msg) => (-32001, format!("Permission denied: {}", msg)),
        Error::Other(msg) => (-32603, msg.clone()),
    };

    Response::Error(ErrorResponse {
        jsonrpc: "2.0".to_string(),
        error: RpcError {
            code,
            message,
            data: None,
        },
        id,
    })
}

/// Create a method not found error response
pub fn method_not_found<T>(id: serde_json::Value) -> Response<T> {
    Response::Error(ErrorResponse {
        jsonrpc: "2.0".to_string(),
        error: RpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        },
        id,
    })
}

/// Create an invalid request error response
pub fn invalid_request<T>(message: &str, id: serde_json::Value) -> Response<T> {
    Response::Error(ErrorResponse {
        jsonrpc: "2.0".to_string(),
        error: RpcError {
            code: -32600,
            message: format!("Invalid request: {}", message),
            data: None,
        },
        id,
    })
}

/// Tool handler function signature
pub type HandlerFn = Box<dyn Fn(serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value>> + Send>> + Send + Sync>;

/// JSONRPC request dispatcher
pub struct Dispatcher {
    /// Method handlers
    handlers: HashMap<String, HandlerFn>,
    /// Format transformer
    transformer: Arc<FormatTransformer>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    /// Create a new empty dispatcher with the standard transformer
    pub fn new() -> Self {
        Self::with_transformer(Arc::new(FormatTransformer::standard()))
    }
    
    /// Create a new empty dispatcher with a custom transformer
    pub fn with_transformer(transformer: Arc<FormatTransformer>) -> Self {
        Self {
            handlers: HashMap::new(),
            transformer,
        }
    }
    
    /// Get the current transformer
    pub fn transformer(&self) -> &FormatTransformer {
        &self.transformer
    }
    
    /// Register a method handler
    pub fn register<F, Fut, P, O>(&mut self, method: &str, handler: F)
    where
        F: Fn(P) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<O>> + Send + 'static,
        P: DeserializeOwned + Send + Sync + 'static,
        O: Serialize + Send + 'static,
    {
        let method_name = method.to_string();
        let transformer = self.transformer.clone();
        
        let handler_fn: HandlerFn = Box::new(move |params: serde_json::Value| {
            let handler_clone = handler.clone();
            let transformer_clone = transformer.clone();
            
            Box::pin(async move {
                // Transform parameters using the transformer
                let transformed_params = transformer_clone.transform_params(params)?;
                
                // Deserialize to the specific parameter type
                let typed_params: P = deserialize(transformed_params)?;
                
                // Execute the handler
                let result = handler_clone(typed_params).await?;
                
                // Serialize the result
                let json_result = serialize(result)?;
                
                // Transform result using the transformer
                transformer_clone.transform_result(json_result)
            })
        });
        
        self.handlers.insert(method_name, handler_fn);
    }
    
    /// Dispatch a JSONRPC request
    pub async fn dispatch(&self, request_str: &str) -> Result<String> {
        let raw_request: RawRequest = serde_json::from_str(request_str)?;
        
        let response = if raw_request.jsonrpc != "2.0" {
            let resp = invalid_request::<serde_json::Value>("Invalid JSONRPC version", raw_request.id);
            serde_json::to_string(&resp)?
        } else {
            match self.handlers.get(&raw_request.method) {
                Some(handler) => {
                    match handler(raw_request.params.clone()).await {
                        Ok(result) => {
                            let resp = success(result, raw_request.id);
                            serde_json::to_string(&resp)?
                        },
                        Err(e) => {
                            let resp = error::<serde_json::Value>(e, raw_request.id);
                            serde_json::to_string(&resp)?
                        }
                    }
                },
                None => {
                    let resp = method_not_found::<serde_json::Value>(raw_request.id);
                    serde_json::to_string(&resp)?
                }
            }
        };
        
        Ok(response)
    }
}