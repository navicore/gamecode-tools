//! JSON Schema generation for tools
//!
//! This module provides functionality to generate JSON schemas for tool parameters
//! and generate tool specifications for various platforms (AWS Bedrock, OpenAI, etc.)

use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Tool schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for parameters
    pub parameters_schema: Value,
}

/// AWS Bedrock tool specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockToolSpec {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema in Bedrock format
    pub input_schema: BedrockInputSchema,
}

/// Bedrock input schema format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockInputSchema {
    /// JSON schema object
    pub json: Value,
}

/// Generate tool schema from a type that implements JsonSchema
pub fn generate_tool_schema<T: JsonSchema>(name: &str, description: &str) -> ToolSchema {
    let schema = schema_for!(T);
    ToolSchema {
        name: name.to_string(),
        description: description.to_string(),
        parameters_schema: serde_json::to_value(&schema).unwrap_or(Value::Null),
    }
}

/// Convert a tool schema to AWS Bedrock format
pub fn to_bedrock_tool_spec(schema: &ToolSchema) -> BedrockToolSpec {
    BedrockToolSpec {
        name: schema.name.clone(),
        description: schema.description.clone(),
        input_schema: BedrockInputSchema {
            json: schema.parameters_schema.clone(),
        },
    }
}

/// Convert a tool schema to OpenAI function format
pub fn to_openai_function(schema: &ToolSchema) -> Value {
    serde_json::json!({
        "name": schema.name,
        "description": schema.description,
        "parameters": schema.parameters_schema
    })
}

/// Registry for all tool schemas
#[derive(Debug, Default)]
pub struct ToolSchemaRegistry {
    schemas: HashMap<String, ToolSchema>,
}

impl ToolSchemaRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool schema
    pub fn register<T: JsonSchema>(&mut self, name: &str, description: &str) {
        let schema = generate_tool_schema::<T>(name, description);
        self.schemas.insert(name.to_string(), schema);
    }

    /// Get a tool schema by name
    pub fn get(&self, name: &str) -> Option<&ToolSchema> {
        self.schemas.get(name)
    }

    /// Get all tool schemas
    pub fn get_all(&self) -> &HashMap<String, ToolSchema> {
        &self.schemas
    }

    /// Get all tool names
    pub fn get_tool_names(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }

    /// Convert all schemas to Bedrock format
    pub fn to_bedrock_specs(&self) -> Vec<BedrockToolSpec> {
        self.schemas.values().map(to_bedrock_tool_spec).collect()
    }

    /// Convert all schemas to OpenAI format
    pub fn to_openai_functions(&self) -> Vec<Value> {
        self.schemas.values().map(to_openai_function).collect()
    }

    /// Get schemas as JSON
    pub fn to_json(&self) -> Value {
        serde_json::to_value(&self.schemas).unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[allow(dead_code)]
    #[derive(JsonSchema, Deserialize)]
    struct TestParams {
        path: String,
        #[serde(default)]
        recursive: bool,
    }

    #[test]
    fn test_generate_tool_schema() {
        let schema = generate_tool_schema::<TestParams>("test_tool", "Test description");

        assert_eq!(schema.name, "test_tool");
        assert_eq!(schema.description, "Test description");
        assert!(schema.parameters_schema.is_object());
    }

    #[test]
    fn test_tool_registry() {
        let mut registry = ToolSchemaRegistry::new();
        registry.register::<TestParams>("test_tool", "Test description");

        assert!(registry.get("test_tool").is_some());
        assert_eq!(registry.get_tool_names().len(), 1);

        let bedrock_specs = registry.to_bedrock_specs();
        assert_eq!(bedrock_specs.len(), 1);
        assert_eq!(bedrock_specs[0].name, "test_tool");
    }
}
