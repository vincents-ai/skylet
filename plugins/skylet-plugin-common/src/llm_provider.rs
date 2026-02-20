// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// LLM Provider abstraction for skylet-plugin-common v0.3.0
use crate::PluginCommonError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub name: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a tool response message
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            name: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// Create an assistant message with tool calls
    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            name: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }
}

/// Message role in a conversation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: ToolParameters,
}

/// Tool parameters schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameters {
    #[serde(rename = "type")]
    pub param_type: String,
    pub properties: HashMap<String, ToolProperty>,
    pub required: Vec<String>,
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
}

impl ToolParameters {
    /// Create tool parameters from JSON schema
    pub fn json_schema(schema: &str) -> Result<Self, PluginCommonError> {
        serde_json::from_str(schema).map_err(|e| {
            PluginCommonError::SerializationFailed(format!(
                "Failed to parse tool parameters: {}",
                e
            ))
        })
    }

    /// Create simple tool parameters
    pub fn simple(
        param_type: &str,
        properties: HashMap<String, ToolProperty>,
        required: Vec<String>,
    ) -> Self {
        Self {
            param_type: param_type.to_string(),
            properties,
            required,
            schema: Some("http://json-schema.org/draft-07/schema#".to_string()),
        }
    }
}

/// Tool property definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProperty {
    #[serde(rename = "type")]
    pub property_type: String,
    pub description: Option<String>,
    pub enum_values: Option<Vec<String>>,
    pub default: Option<serde_json::Value>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}

impl ToolProperty {
    /// Create a string property
    pub fn string(description: Option<&str>) -> Self {
        Self {
            property_type: "string".to_string(),
            description: description.map(|s| s.to_string()),
            enum_values: None,
            default: None,
            minimum: None,
            maximum: None,
        }
    }

    /// Create an integer property
    pub fn integer(description: Option<&str>) -> Self {
        Self {
            property_type: "integer".to_string(),
            description: description.map(|s| s.to_string()),
            enum_values: None,
            default: None,
            minimum: None,
            maximum: None,
        }
    }

    /// Create a boolean property
    pub fn boolean(description: Option<&str>) -> Self {
        Self {
            property_type: "boolean".to_string(),
            description: description.map(|s| s.to_string()),
            enum_values: None,
            default: None,
            minimum: None,
            maximum: None,
        }
    }

    /// Create an enum property
    pub fn enum_property(values: Vec<String>, description: Option<&str>) -> Self {
        Self {
            property_type: "string".to_string(),
            description: description.map(|s| s.to_string()),
            enum_values: Some(values),
            default: None,
            minimum: None,
            maximum: None,
        }
    }

    /// Set default value
    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = Some(default);
        self
    }

    /// Set minimum value (for numbers)
    pub fn with_minimum(mut self, minimum: f64) -> Self {
        self.minimum = Some(minimum);
        self
    }

    /// Set maximum value (for numbers)
    pub fn with_maximum(mut self, maximum: f64) -> Self {
        self.maximum = Some(maximum);
        self
    }
}

/// Tool call made by the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call within a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

impl FunctionCall {
    /// Create a new function call
    pub fn new(name: &str, arguments: &serde_json::Value) -> Result<Self, PluginCommonError> {
        Ok(Self {
            name: name.to_string(),
            arguments: serde_json::to_string(arguments).map_err(|e| {
                PluginCommonError::SerializationFailed(format!(
                    "Failed to serialize arguments: {}",
                    e
                ))
            })?,
        })
    }

    /// Parse arguments to a specific type
    pub fn parse_arguments<T: for<'de> Deserialize<'de>>(&self) -> Result<T, PluginCommonError> {
        serde_json::from_str(&self.arguments).map_err(|e| {
            PluginCommonError::SerializationFailed(format!("Failed to parse arguments: {}", e))
        })
    }
}

/// Completion request to LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub tool_choice: Option<ToolChoice>,
    pub stream: Option<bool>,
    pub user: Option<String>,
}

impl CompletionRequest {
    /// Create a new completion request
    pub fn new(model: &str, messages: Vec<Message>) -> Self {
        Self {
            model: model.to_string(),
            messages,
            max_tokens: None,
            temperature: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            tools: None,
            tool_choice: None,
            stream: None,
            user: None,
        }
    }

    /// Set max tokens
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Add tools to the request
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool choice
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Enable streaming
    pub fn stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }
}

/// Tool choice strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolChoice {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "required")]
    Required,
    #[serde(rename = "function")]
    Function { name: String },
}

/// Completion response from LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<CompletionChoice>,
    pub usage: TokenUsage,
    pub system_fingerprint: Option<String>,
}

/// Individual completion choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: FinishReason,
    pub logprobs: Option<serde_json::Value>,
}

/// Why the completion finished
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FinishReason {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "tool_calls")]
    ToolCalls,
    #[serde(rename = "content_filter")]
    ContentFilter,
    #[serde(rename = "function_call")]
    FunctionCall,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming completion chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    pub usage: Option<TokenUsage>,
}

/// Individual streaming choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: ChunkDelta,
    pub finish_reason: Option<FinishReason>,
}

/// Delta content in streaming chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkDelta {
    pub role: Option<MessageRole>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
    pub capabilities: ModelCapabilities,
}

/// Model capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub text_generation: bool,
    pub function_calling: bool,
    pub streaming: bool,
    pub vision: bool,
    pub max_tokens: Option<u32>,
    pub max_context_length: Option<u32>,
    pub supported_languages: Vec<String>,
}

/// Streaming chunk receiver
pub type BoxStream<T> = Box<dyn futures::Stream<Item = Result<T, LLMError>> + Send + Unpin>;

/// LLM Provider trait
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Generate a completion response
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LLMError>;

    /// Generate a streaming completion response
    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<BoxStream<CompletionChunk>, LLMError>;

    /// List available models
    async fn list_models(&self) -> Result<Vec<ModelInfo>, LLMError>;

    /// Get information about a specific model
    async fn get_model_info(&self, model_id: &str) -> Result<ModelInfo, LLMError>;

    /// Check if a model is available
    async fn is_model_available(&self, model_id: &str) -> bool {
        self.list_models()
            .await
            .map(|models| models.iter().any(|m| m.id == model_id))
            .unwrap_or(false)
    }

    /// Estimate tokens for a message (provider-specific implementation)
    async fn estimate_tokens(&self, messages: &[Message]) -> Result<u32, LLMError>;

    /// Get provider name
    fn provider_name(&self) -> &'static str;

    /// Get provider capabilities
    fn capabilities(&self) -> LLProviderCapabilities;
}

/// Provider capabilities
#[derive(Debug, Clone)]
pub struct LLProviderCapabilities {
    pub streaming: bool,
    pub function_calling: bool,
    pub image_input: bool,
    pub json_mode: bool,
    pub system_messages: bool,
    pub tool_calls: bool,
    pub max_tokens_limit: Option<u32>,
}

/// LLM error types
#[derive(thiserror::Error, Debug)]
pub enum LLMError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Content filter: {0}")]
    ContentFilter(String),
}

impl LLMError {
    pub fn api_error(msg: impl Into<String>) -> Self {
        Self::ApiError(msg.into())
    }

    pub fn authentication(msg: impl Into<String>) -> Self {
        Self::Authentication(msg.into())
    }

    pub fn rate_limit(msg: impl Into<String>) -> Self {
        Self::RateLimit(msg.into())
    }

    pub fn model_not_found(model: &str) -> Self {
        Self::ModelNotFound(model.to_string())
    }

    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::InvalidRequest(msg.into())
    }

    pub fn network(msg: impl Into<String>) -> Self {
        Self::Network(msg.into())
    }

    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }

    pub fn provider(msg: impl Into<String>) -> Self {
        Self::Provider(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    pub fn content_filter(msg: impl Into<String>) -> Self {
        Self::ContentFilter(msg.into())
    }
}

/// Registry for multiple LLM providers
pub struct LLMRegistry {
    providers: RwLock<HashMap<String, Arc<dyn LLMProvider>>>,
    default_provider: RwLock<Option<String>>,
}

impl LLMRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            default_provider: RwLock::new(None),
        }
    }

    /// Register a provider
    pub async fn register_provider<P: LLMProvider + 'static>(&self, name: &str, provider: P) {
        let mut providers = self.providers.write().await;
        providers.insert(name.to_string(), Arc::new(provider));
    }

    /// Get a provider by name
    pub async fn get_provider(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        let providers = self.providers.read().await;
        // Arc is cheaply cloneable, so we can return a clone of the Arc
        providers.get(name).map(|p| Arc::clone(p))
    }

    /// Set default provider
    pub async fn set_default_provider(&self, name: &str) -> Result<(), LLMError> {
        let providers = self.providers.read().await;
        if providers.contains_key(name) {
            let mut default = self.default_provider.write().await;
            *default = Some(name.to_string());
            Ok(())
        } else {
            Err(LLMError::provider(format!("Provider '{}' not found", name)))
        }
    }

    /// Get default provider
    pub async fn get_default_provider(&self) -> Result<Arc<dyn LLMProvider>, LLMError> {
        let default_name = self.default_provider.read().await;
        if let Some(name) = default_name.as_ref() {
            self.get_provider(name).await.ok_or_else(|| {
                LLMError::provider(format!("Default provider '{}' unavailable", name))
            })
        } else {
            Err(LLMError::provider("No default provider set".to_string()))
        }
    }

    /// List all registered providers
    pub async fn list_providers(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
}

impl Default for LLMRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for creating LLM requests
pub fn create_completion_request(model: &str, messages: Vec<Message>) -> CompletionRequest {
    CompletionRequest::new(model, messages)
}

pub fn create_system_message(content: &str) -> Message {
    Message::system(content)
}

pub fn create_user_message(content: &str) -> Message {
    Message::user(content)
}

pub fn create_assistant_message(content: &str) -> Message {
    Message::assistant(content)
}

pub fn create_tool_result(tool_call_id: &str, content: &str) -> Message {
    Message::tool_result(tool_call_id, content)
}

pub fn create_tool_definition(
    name: &str,
    description: &str,
    parameters: ToolParameters,
) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let user_msg = Message::user("Hello, world!");
        assert_eq!(user_msg.role, MessageRole::User);
        assert_eq!(user_msg.content, "Hello, world!");

        let system_msg = Message::system("You are a helpful assistant.");
        assert_eq!(system_msg.role, MessageRole::System);
        assert_eq!(system_msg.content, "You are a helpful assistant.");
    }

    #[test]
    fn test_tool_definition() {
        let parameters = ToolParameters::simple(
            "object",
            HashMap::from([
                (
                    "query".to_string(),
                    ToolProperty::string(Some("Search query")),
                ),
                (
                    "limit".to_string(),
                    ToolProperty::integer(Some("Result limit")),
                ),
            ]),
            vec!["query".to_string()],
        );

        let tool = create_tool_definition("search", "Search for information", parameters);

        assert_eq!(tool.name, "search");
        assert_eq!(tool.description, "Search for information");
        assert_eq!(tool.parameters.param_type, "object");
    }

    #[test]
    fn test_completion_request() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello!"),
        ];

        let request = create_completion_request("gpt-3.5-turbo", messages)
            .max_tokens(100)
            .temperature(0.7)
            .stream(true);

        assert_eq!(request.model, "gpt-3.5-turbo");
        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.stream, Some(true));
    }

    #[test]
    fn test_function_call() {
        let args = serde_json::json!({
            "city": "New York",
            "units": "metric"
        });

        let call = FunctionCall::new("get_weather", &args).unwrap();
        assert_eq!(call.name, "get_weather");

        let parsed: serde_json::Value = call.parse_arguments().unwrap();
        assert_eq!(parsed["city"], "New York");
        assert_eq!(parsed["units"], "metric");
    }

    #[test]
    fn test_tool_property() {
        let string_prop = ToolProperty::string(Some("A string input"));
        assert_eq!(string_prop.property_type, "string");
        assert_eq!(string_prop.description, Some("A string input".to_string()));

        let int_prop = ToolProperty::integer(None)
            .with_minimum(0.0)
            .with_maximum(100.0);
        assert_eq!(int_prop.property_type, "integer");
        assert_eq!(int_prop.minimum, Some(0.0));
        assert_eq!(int_prop.maximum, Some(100.0));

        let enum_prop = ToolProperty::enum_property(
            vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            Some("Priority level"),
        );
        assert_eq!(
            enum_prop.enum_values,
            Some(vec![
                "low".to_string(),
                "medium".to_string(),
                "high".to_string()
            ])
        );
    }

    #[tokio::test]
    async fn test_llm_registry() {
        let registry = LLMRegistry::new();

        // Test listing providers (should be empty)
        let providers = registry.list_providers().await;
        assert!(providers.is_empty());

        // Test getting default provider (should error)
        let result = registry.get_default_provider().await;
        assert!(result.is_err());
    }
}
