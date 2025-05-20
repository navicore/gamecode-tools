//! Response and parameter transformation support for different API formats.
//!
//! This module provides a way to transform JSONRPC parameters and results
//! to and from different API-specific formats.

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::Result;

/// Input format for parameters
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFormat {
    /// Standard JSONRPC format
    Standard,
    /// AWS Bedrock format with type wrappers
    Bedrock,
}

/// Output format for results
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Standard JSONRPC format
    Standard,
    /// AWS Bedrock format with type wrappers
    Bedrock,
}

/// Format configuration for a dispatcher
#[derive(Clone, Copy, Debug)]
pub struct FormatConfig {
    /// Format for input parameters
    pub input_format: InputFormat,
    /// Format for output results
    pub output_format: OutputFormat,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            input_format: InputFormat::Standard,
            output_format: OutputFormat::Standard,
        }
    }
}

impl FormatConfig {
    /// Create a new format configuration
    pub fn new(input_format: InputFormat, output_format: OutputFormat) -> Self {
        Self {
            input_format,
            output_format,
        }
    }

    /// Create a standard format configuration
    pub fn standard() -> Self {
        Self {
            input_format: InputFormat::Standard,
            output_format: OutputFormat::Standard,
        }
    }

    /// Create a Bedrock format configuration
    pub fn bedrock() -> Self {
        Self {
            input_format: InputFormat::Bedrock,
            output_format: OutputFormat::Bedrock,
        }
    }

    /// Create a configuration that accepts standard input but produces Bedrock output
    pub fn standard_to_bedrock() -> Self {
        Self {
            input_format: InputFormat::Standard,
            output_format: OutputFormat::Bedrock,
        }
    }

    /// Create a configuration that accepts Bedrock input but produces standard output
    pub fn bedrock_to_standard() -> Self {
        Self {
            input_format: InputFormat::Bedrock,
            output_format: OutputFormat::Standard,
        }
    }
}

/// Format transformer for JSONRPC parameters and results
#[derive(Clone, Debug)]
pub struct FormatTransformer {
    /// The format configuration
    config: FormatConfig,
}

/// Transform a JSON value to Bedrock format
fn to_bedrock_format(value: &Value) -> Value {
    match value {
        // For objects, recursively transform all values
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k.clone(), to_bedrock_format(v));
            }
            Value::Object(new_map)
        }
        // For arrays, recursively transform all elements
        Value::Array(arr) => {
            let new_arr = arr.iter().map(to_bedrock_format).collect();
            Value::Array(new_arr)
        }
        // For leaf values (string, number, bool, null), wrap in the Bedrock format
        _ => {
            // Don't wrap if already wrapped
            if let Value::Object(map) = value {
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

fn from_bedrock_format(value: &Value) -> Value {
    match value {
        // Check if this is a wrapped value
        Value::Object(map) => {
            if let (Some(Value::String(typ)), Some(content)) = (map.get("type"), map.get("text")) {
                if typ == "text" {
                    return from_bedrock_format(content);
                }
            }

            // Regular object, process recursively
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k.clone(), from_bedrock_format(v));
            }
            Value::Object(new_map)
        }
        // For arrays, recursively unwrap all elements
        Value::Array(arr) => {
            let new_arr = arr.iter().map(from_bedrock_format).collect();
            Value::Array(new_arr)
        }
        // For other values, return as is
        _ => value.clone(),
    }
}

impl FormatTransformer {
    /// Create a new format transformer with the given configuration
    pub fn new(config: FormatConfig) -> Self {
        Self { config }
    }

    /// Create a standard format transformer
    pub fn standard() -> Self {
        Self::new(FormatConfig::standard())
    }

    /// Create a Bedrock format transformer
    pub fn bedrock() -> Self {
        Self::new(FormatConfig::bedrock())
    }

    /// Get the current format configuration
    pub fn config(&self) -> FormatConfig {
        self.config
    }

    /// Extract a value from Bedrock format
    /// Transform parameters based on the input format
    pub fn transform_params(&self, params: Value) -> Result<Value> {
        match self.config.input_format {
            InputFormat::Standard => Ok(params),
            InputFormat::Bedrock => Ok(from_bedrock_format(&params)),
        }
    }

    /// Transform result based on the output format
    pub fn transform_result(&self, result: Value) -> Result<Value> {
        match self.config.output_format {
            OutputFormat::Standard => Ok(result),
            OutputFormat::Bedrock => Ok(to_bedrock_format(&result)),
        }
    }
}

impl Default for FormatTransformer {
    fn default() -> Self {
        Self::standard()
    }
}

/// Helper functions to serialize/deserialize with type checking
pub fn serialize<T: Serialize>(value: T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

pub fn deserialize<T: DeserializeOwned>(value: Value) -> Result<T> {
    Ok(serde_json::from_value(value)?)
}

/// Create a standard format transformer
pub fn standard_transformer() -> FormatTransformer {
    FormatTransformer::standard()
}

/// Create a Bedrock format transformer
pub fn bedrock_transformer() -> FormatTransformer {
    FormatTransformer::bedrock()
}

/// Create a transformer that accepts standard input but produces Bedrock output
pub fn standard_to_bedrock_transformer() -> FormatTransformer {
    FormatTransformer::new(FormatConfig::standard_to_bedrock())
}

/// Create a transformer that accepts Bedrock input but produces standard output
pub fn bedrock_to_standard_transformer() -> FormatTransformer {
    FormatTransformer::new(FormatConfig::bedrock_to_standard())
}
