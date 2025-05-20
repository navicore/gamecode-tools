//! Response and parameter transformation support for different API formats.
//!
//! This module provides a way to transform JSONRPC parameters and results
//! to and from different API-specific formats.

use serde::{de::DeserializeOwned, Serialize};

use crate::Result;

/// Trait for transforming JSONRPC parameters and results
pub trait ResponseTransformer: Send + Sync {
    /// Transform a result before it's returned in a JSONRPC response
    fn transform_result(&self, result: serde_json::Value) -> Result<serde_json::Value>;
    
    /// Transform JSONRPC parameters received from a request
    fn transform_params(&self, params: serde_json::Value) -> Result<serde_json::Value>;
}

/// Helper functions to serialize/deserialize with type checking
pub fn serialize<T: Serialize>(value: T) -> Result<serde_json::Value> {
    Ok(serde_json::to_value(value)?)
}

pub fn deserialize<T: DeserializeOwned>(value: serde_json::Value) -> Result<T> {
    Ok(serde_json::from_value(value)?)
}

/// Standard JSONRPC transformer (no transformation)
#[derive(Clone, Default)]
pub struct StandardTransformer;

impl ResponseTransformer for StandardTransformer {
    fn transform_result(&self, result: serde_json::Value) -> Result<serde_json::Value> {
        Ok(result)
    }
    
    fn transform_params(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        Ok(params)
    }
}

/// AWS Bedrock transformer
///
/// Transforms all leaf values to format: `{"type": "text", "text": value}`
/// And extracts unwrapped parameters from Bedrock format
#[derive(Clone, Default)]
pub struct BedrockTransformer;

impl BedrockTransformer {
    /// Recursively transform all values in a JSON structure to Bedrock format
    fn transform_json_value(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            // For objects, recursively transform all values
            serde_json::Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k.clone(), self.transform_json_value(v));
                }
                serde_json::Value::Object(new_map)
            },
            // For arrays, recursively transform all elements
            serde_json::Value::Array(arr) => {
                let new_arr = arr.iter()
                    .map(|v| self.transform_json_value(v))
                    .collect();
                serde_json::Value::Array(new_arr)
            },
            // For leaf values (string, number, bool, null), wrap in the Bedrock format
            _ => {
                // Don't wrap if already wrapped
                if let serde_json::Value::Object(map) = value {
                    if map.contains_key("type") && map.contains_key("text") {
                        return value.clone();
                    }
                }
                
                serde_json::json!({
                    "type": "text",
                    "text": value
                })
            }
        }
    }
    
    /// Recursively unwrap any Bedrock format values in a JSON structure
    fn unwrap_json_value(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            // Check if this is a wrapped value
            serde_json::Value::Object(map) => {
                if let (Some(serde_json::Value::String(typ)), Some(content)) = (map.get("type"), map.get("text")) {
                    if typ == "text" {
                        return self.unwrap_json_value(content);
                    }
                }
                
                // Regular object, process recursively
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k.clone(), self.unwrap_json_value(v));
                }
                serde_json::Value::Object(new_map)
            },
            // For arrays, recursively unwrap all elements
            serde_json::Value::Array(arr) => {
                let new_arr = arr.iter()
                    .map(|v| self.unwrap_json_value(v))
                    .collect();
                serde_json::Value::Array(new_arr)
            },
            // For other values, return as is
            _ => value.clone(),
        }
    }
}

impl ResponseTransformer for BedrockTransformer {
    fn transform_result(&self, result: serde_json::Value) -> Result<serde_json::Value> {
        Ok(self.transform_json_value(&result))
    }
    
    fn transform_params(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        Ok(self.unwrap_json_value(&params))
    }
}

/// Create a standard transformer with no transformations
pub fn standard_transformer() -> StandardTransformer {
    StandardTransformer::default()
}

/// Create a transformer for AWS Bedrock format
pub fn bedrock_transformer() -> BedrockTransformer {
    BedrockTransformer::default()
}