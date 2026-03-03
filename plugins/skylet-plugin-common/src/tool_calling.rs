// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Tool calling framework for skylet-plugin-common v0.3.0
use crate::llm_provider::{ToolDefinition, ToolParameters, ToolProperty};
use crate::PluginCommonError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tool calling trait - interfaces that can be called by LLMs
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get tool name
    fn name(&self) -> &str;

    /// Get tool description
    fn description(&self) -> &str;

    /// Get tool parameters schema
    fn parameters(&self) -> ToolParameters;

    /// Execute the tool with given arguments
    async fn call(&self, args: serde_json::Value) -> Result<ToolResult, ToolError>;

    /// Validate tool arguments before execution
    async fn validate_args(&self, args: &serde_json::Value) -> Result<(), ToolError>;

    /// Get tool metadata
    fn metadata(&self) -> ToolMetadata;

    /// Check if tool is available
    async fn is_available(&self) -> bool {
        true
    }

    /// Get tool capabilities
    fn capabilities(&self) -> ToolCapabilities;
}

/// Tool metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub category: ToolCategory,
    pub tags: Vec<String>,
    pub license: String,
    pub repository: Option<String>,
    pub homepage: Option<String>,
}

/// Tool category
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolCategory {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "network")]
    Network,
    #[serde(rename = "data")]
    Data,
    #[serde(rename = "utility")]
    Utility,
    #[serde(rename = "communication")]
    Communication,
    #[serde(rename = "development")]
    Development,
    #[serde(rename = "security")]
    Security,
    #[serde(rename = "ai")]
    AI,
    #[serde(rename = "analytics")]
    Analytics,
}

/// Tool capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapabilities {
    pub supports_streaming: bool,
    pub supports_file_upload: bool,
    pub supports_file_download: bool,
    pub max_payload_size_mb: Option<u64>,
    pub requires_authentication: bool,
    pub supported_formats: Vec<String>,
    pub rate_limits: Option<ToolRateLimits>,
}

/// Tool rate limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRateLimits {
    pub max_calls_per_minute: Option<u32>,
    pub max_calls_per_hour: Option<u32>,
    pub max_calls_per_day: Option<u32>,
    pub cooldown_seconds: Option<u64>,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub files: Option<Vec<ToolFile>>,
    pub logs: Vec<String>,
    pub execution_time_ms: u64,
    pub metadata: Option<serde_json::Value>,
}

/// Tool file output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFile {
    pub id: String,
    pub name: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub url: Option<String>,
    pub content: Option<Vec<u8>>,
    pub path: Option<String>,
}

/// Tool registry for managing available tools
pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Box<dyn Tool>>>>,
    categories: Arc<RwLock<HashMap<ToolCategory, Vec<String>>>>,
    rate_limiters: Arc<RwLock<HashMap<String, ToolRateLimiter>>>,
}

/// Tool rate limiter
pub struct ToolRateLimiter {
    pub limits: ToolRateLimits,
    pub call_history: Arc<RwLock<Vec<ToolCall>>>,
}

/// Tool call record for rate limiting
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tool_name: String,
    pub args_hash: u64,
    pub success: bool,
    pub execution_time_ms: u64,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            categories: Arc::new(RwLock::new(HashMap::new())),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a tool
    pub async fn register_tool<T: Tool + 'static>(&self, tool: T) -> Result<(), ToolError> {
        let tool_name = tool.name().to_string();
        let metadata = tool.metadata().clone();
        let capabilities = tool.capabilities().clone();
        let tool_box = Box::new(tool);
        
        let mut tools = self.tools.write().await;
        tools.insert(tool_name.clone(), tool_box);

        // Update category mapping
        let mut categories = self.categories.write().await;
        categories.entry(metadata.category.clone())
            .or_insert_with(Vec::new)
            .push(tool_name.clone());

        // Setup rate limiter if tool has limits
        if let Some(limits) = &capabilities.rate_limits {
            let rate_limiter = ToolRateLimiter {
                limits: limits.clone(),
                call_history: Arc::new(RwLock::new(Vec::new())),
            };
            
            let mut rate_limiters = self.rate_limiters.write().await;
            rate_limiters.insert(tool_name.clone(), rate_limiter);
        }

        Ok(())
    }

    /// Unregister a tool
    pub async fn unregister_tool(&self, tool_name: &str) -> Result<(), ToolError> {
        let mut tools = self.tools.write().await;
        tools.remove(tool_name);

        let mut rate_limiters = self.rate_limiters.write().await;
        rate_limiters.remove(tool_name);

        Ok(())
    }

    /// Get a tool by name
    pub async fn get_tool(&self, tool_name: &str) -> Option<Box<dyn Tool>> {
        let tools = self.tools.read().await;
        // Note: This is simplified - in real implementation would use Arc cloning
        None
    }

    /// List all registered tools
    pub async fn list_tools(&self) -> Vec<ToolInfo> {
        let tools = self.tools.read().await;
        tools.iter().map(|(name, tool)| ToolInfo {
            name: name.clone(),
            description: tool.description().to_string(),
            parameters: tool.parameters(),
            capabilities: tool.capabilities(),
            metadata: tool.metadata(),
        }).collect()
    }

    /// List tools by category
    pub async fn list_tools_by_category(&self, category: ToolCategory) -> Vec<ToolInfo> {
        let tools = self.tools.read().await;
        let categories = self.categories.read().await;
        
        if let Some(tool_names) = categories.get(&category) {
            tool_names.iter()
                .filter_map(|name| {
                    tools.get(name).map(|tool| {
                        ToolInfo {
                            name: name.clone(),
                            description: tool.description().to_string(),
                            parameters: tool.parameters(),
                            capabilities: tool.capabilities(),
                            metadata: tool.metadata(),
                        }
                    })
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Execute a tool with arguments
    pub async fn execute_tool(&self, 
        tool_name: &str, 
        args: serde_json::Value
    ) -> Result<ToolResult, ToolError> {
        // Check rate limits
        if let Err(e) = self.check_rate_limits(tool_name).await {
            return Err(e);
        }

        // Get tool and validate arguments
        let tools = self.tools.read().await;
        let tool = tools.get(tool_name)
            .ok_or_else(|| ToolError::tool_not_found(tool_name.to_string()))?;

        tool.validate_args(&args).await?;

        // Execute the tool
        let result = tool.call(args.clone()).await?;

        // Record call for rate limiting
        self.record_tool_call(tool_name, &args, &result).await;

        Ok(result)
    }

    /// Check if tool execution is allowed by rate limits
    async fn check_rate_limits(&self, tool_name: &str) -> Result<(), ToolError> {
        let rate_limiters = self.rate_limiters.read().await;
        if let Some(limiter) = rate_limiters.get(tool_name) {
            // Check if rate limits would be exceeded
            let call_history = limiter.call_history.read().await;
            let now = chrono::Utc::now();
            
            // Check minute limit
            if let Some(minute_limit) = limiter.limits.max_calls_per_minute {
                let recent_calls: u32 = call_history.iter()
                    .filter(|call| {
                        let minutes_passed = (now - call.timestamp).num_minutes();
                        minutes_passed < 1
                    })
                    .count() as u32;
                
                if recent_calls >= minute_limit {
                    return Err(ToolError::rate_limit_exceeded(format!(
                        "Minute limit of {} exceeded", minute_limit
                    )));
                }
            }

            // Check hour limit
            if let Some(hour_limit) = limiter.limits.max_calls_per_hour {
                let recent_calls: u32 = call_history.iter()
                    .filter(|call| {
                        let hours_passed = (now - call.timestamp).num_hours();
                        hours_passed < 1
                    })
                    .count() as u32;
                
                if recent_calls >= hour_limit {
                    return Err(ToolError::rate_limit_exceeded(format!(
                        "Hour limit of {} exceeded", hour_limit
                    )));
                }
            }

            // Check day limit
            if let Some(day_limit) = limiter.limits.max_calls_per_day {
                let recent_calls: u32 = call_history.iter()
                    .filter(|call| {
                        let days_passed = (now - call.timestamp).num_days();
                        days_passed < 1
                    })
                    .count() as u32;
                
                if recent_calls >= day_limit {
                    return Err(ToolError::rate_limit_exceeded(format!(
                        "Day limit of {} exceeded", day_limit
                    )));
                }
            }
        }

        Ok(())
    }

    /// Record a tool call for rate limiting
    async fn record_tool_call(&self, 
        tool_name: &str, 
        args: &serde_json::Value, 
        result: &ToolResult
    ) {
        let rate_limiters = self.rate_limiters.read().await;
        if let Some(limiter) = rate_limiters.get(tool_name) {
            let mut call_history = limiter.call_history.write().await;
            
            let call_record = ToolCall {
                timestamp: chrono::Utc::now(),
                tool_name: tool_name.to_string(),
                args_hash: self.hash_args(args),
                success: result.success,
                execution_time_ms: result.execution_time_ms,
            };
            
            call_history.push(call_record);
            
            // Clean up old calls (older than 24 hours)
            let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
            call_history.retain(|call| call.timestamp > cutoff);
        }
    }

    /// Hash arguments for rate limiting tracking
    fn hash_args(&self, args: &serde_json::Value) -> u64 {
        // Simple hash - in production would use a better hashing algorithm
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        args.to_string().hash(&mut hasher);
        hasher.finish()
    }

    /// Get tool execution statistics
    pub async fn get_tool_stats(&self, tool_name: &str) -> Option<ToolStats> {
        let rate_limiters = self.rate_limiters.read().await;
        if let Some(limiter) = rate_limiters.get(tool_name) {
            let call_history = limiter.call_history.read().await;
            let now = chrono::Utc::now();
            
            let last_24h_calls = call_history.iter()
                .filter(|call| {
                    let hours_passed = (now - call.timestamp).num_hours();
                    hours_passed <= 24
                })
                .count();

            let successful_calls = call_history.iter()
                .filter(|call| {
                    let hours_passed = (now - call.timestamp).num_hours();
                    hours_passed <= 24 && call.success
                })
                .count();

            let avg_execution_time = if last_24h_calls > 0 {
                let total_time: u64 = call_history.iter()
                    .filter(|call| {
                        let hours_passed = (now - call.timestamp).num_hours();
                        hours_passed <= 24
                    })
                    .map(|call| call.execution_time_ms)
                    .sum();
                
                total_time / last_24h_calls as u64
            } else {
                0
            };

            Some(ToolStats {
                total_calls_last_24h: last_24h_calls as u32,
                successful_calls_last_24h: successful_calls as u32,
                average_execution_time_ms: avg_execution_time,
                last_called: call_history.last().map(|call| call.timestamp),
            })
        } else {
            None
        }
    }

    /// Search for tools by name or description
    pub async fn search_tools(&self, query: &str) -> Vec<ToolInfo> {
        let tools = self.tools.read().await;
        let query_lower = query.to_lowercase();
        
        tools.iter()
            .filter(|(_, tool)| {
                tool.name().to_lowercase().contains(&query_lower) ||
                tool.description().to_lowercase().contains(&query_lower) ||
                tool.metadata().tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .map(|(name, tool)| ToolInfo {
                name: name.clone(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
                capabilities: tool.capabilities(),
                metadata: tool.metadata(),
            })
            .collect()
    }

    /// Get tools by capability
    pub async fn get_tools_by_capability(&self, capability: &str) -> Vec<ToolInfo> {
        let tools = self.tools.read().await;
        
        tools.iter()
            .filter(|(_, tool)| {
                match capability {
                    "streaming" => tool.capabilities().supports_streaming,
                    "file_upload" => tool.capabilities().supports_file_upload,
                    "file_download" => tool.capabilities().supports_file_download,
                    _ => false,
                }
            })
            .map(|(name, tool)| ToolInfo {
                name: name.clone(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
                capabilities: tool.capabilities(),
                metadata: tool.metadata(),
            })
            .collect()
    }
}

/// Tool information for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: ToolParameters,
    pub capabilities: ToolCapabilities,
    pub metadata: ToolMetadata,
}

/// Tool execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStats {
    pub total_calls_last_24h: u32,
    pub successful_calls_last_24h: u32,
    pub average_execution_time_ms: u64,
    pub last_called: Option<chrono::DateTime<chrono::Utc>>,
}

/// Tool error types
#[derive(thiserror::Error, Debug)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Tool unavailable: {0}")]
    Unavailable(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ToolError {
    pub fn tool_not_found(name: impl Into<String>) -> Self {
        Self::ToolNotFound(name.into())
    }

    pub fn execution_failed(msg: impl Into<String>) -> Self {
        Self::ExecutionFailed(msg.into())
    }

    pub fn invalid_arguments(msg: impl Into<String>) -> Self {
        Self::InvalidArguments(msg.into())
    }

    pub fn validation_failed(msg: impl Into<String>) -> Self {
        Self::ValidationFailed(msg.into())
    }

    pub fn rate_limit_exceeded(msg: impl Into<String>) -> Self {
        Self::RateLimitExceeded(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    pub fn permission_denied(msg: impl Into<String>) -> Self {
        Self::PermissionDenied(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Convenience functions for common tool patterns
pub fn create_tool_registry() -> ToolRegistry {
    ToolRegistry::new()
}

pub fn create_tool_metadata(
    name: &str,
    version: &str,
    author: &str,
    description: &str,
    category: ToolCategory,
) -> ToolMetadata {
    ToolMetadata {
        name: name.to_string(),
        version: version.to_string(),
        author: author.to_string(),
        description: description.to_string(),
        category,
        tags: vec![],
        license: "MIT".to_string(),
        repository: None,
        homepage: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTool {
        name: String,
    description: String,
    parameters: ToolParameters,
    capabilities: ToolCapabilities,
    metadata: ToolMetadata,
    call_count: Arc<RwLock<u32>>,
    should_fail: bool,
    rate_limits: Option<ToolRateLimits>,
    }

    impl TestTool {
        fn new(name: &str, description: &str, should_fail: bool) -> Self {
            Self {
                name: name.to_string(),
                description: description.to_string(),
                parameters: ToolParameters::simple(
                    "object",
                    HashMap::from([
                        ("input".to_string(), 
                            ToolProperty::string(Some("Input to process"))
                        )
                    ]),
                    vec!["input".to_string()],
                ),
                capabilities: ToolCapabilities {
                    supports_streaming: false,
                    supports_file_upload: false,
                    supports_file_download: false,
                    max_payload_size_mb: Some(10),
                    requires_authentication: false,
                    supported_formats: vec!["text".to_string()],
                    rate_limits: None,
                },
                metadata: create_tool_metadata(name, "1.0", "test", description, ToolCategory::Utility),
                call_count: Arc::new(RwLock::new(0)),
                should_fail,
                rate_limits: None,
            }
        }
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn parameters(&self) -> ToolParameters {
            self.parameters.clone()
        }

        async fn call(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
            let count = {
                let mut count = self.call_count.write().await;
                *count += 1;
                *count
            };

            if self.should_fail {
                return Err(ToolError::execution_failed("Test tool failure"));
            }

            Ok(ToolResult {
                success: true,
                data: Some(serde_json::json!({
                    "processed_input": args,
                    "call_count": count
                })),
                files: None,
                logs: vec![format!("Processed call #{}", count)],
                execution_time_ms: 100,
                metadata: None,
            })
        }

        async fn validate_args(&self, args: &serde_json::Value) -> Result<(), ToolError> {
            if let serde_json::Value::Object(map) = args {
                if map.contains_key("input") {
                    Ok(())
                } else {
                    Err(ToolError::invalid_arguments("Missing 'input' parameter"))
                }
            } else {
                Err(ToolError::invalid_arguments("Expected object argument"))
            }
        }

        fn metadata(&self) -> ToolMetadata {
            self.metadata.clone()
        }

        async fn is_available(&self) -> bool {
            !self.should_fail
        }

        fn capabilities(&self) -> ToolCapabilities {
            let mut caps = self.capabilities.clone();
            caps.rate_limits = self.rate_limits.clone();
            caps
        }
    }

    #[tokio::test]
    async fn test_tool_registration() {
        let registry = create_tool_registry();
        let tool = TestTool::new("test-tool", "A test tool", false);
        
        let result = registry.register_tool(tool).await;
        assert!(result.is_ok());
        
        let tools = registry.list_tools().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "test-tool");
    }

    #[tokio::test]
    async fn test_tool_execution() {
         let registry = create_tool_registry();
         let tool = TestTool::new("test-tool", "A test tool", false);
         
         let _ = registry.register_tool(tool).await;
        
        let args = serde_json::json!({
            "input": "test data"
        });
        
        let result = registry.execute_tool("test-tool", args).await;
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.data.is_some());
    }

    #[tokio::test]
    async fn test_tool_search() {
        let registry = create_tool_registry();
         let tool = TestTool::new("search-tool", "Search tool", false);
         let other_tool = TestTool::new("math-tool", "Math tool", false);
         
         let _ = registry.register_tool(tool).await;
         let _ = registry.register_tool(other_tool).await;
        
        // Search by description
        let search_results = registry.search_tools("search").await;
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name, "search-tool");
        
        // Search by name
        let name_results = registry.search_tools("math").await;
        assert_eq!(name_results.len(), 1);
        assert_eq!(name_results[0].name, "math-tool");
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let registry = create_tool_registry();
        let mut tool = TestTool::new("limited-tool", "Limited tool", false);
        
        // Add rate limits
        tool.rate_limits = Some(ToolRateLimits {
            max_calls_per_minute: Some(2),
            max_calls_per_hour: Some(10),
            max_calls_per_day: Some(100),
            cooldown_seconds: Some(5),
         });
         
         let _ = registry.register_tool(tool).await;
        
        let args = serde_json::json!({"input": "test"});
        
        // First call should succeed
        let result1 = registry.execute_tool("limited-tool", args.clone()).await;
        assert!(result1.is_ok());
        
        // Second call should succeed
        let result2 = registry.execute_tool("limited-tool", args.clone()).await;
        assert!(result2.is_ok());
        
        // Third call should fail due to minute limit
        let result3 = registry.execute_tool("limited-tool", args).await;
        assert!(result3.is_err());
        
        if let Err(ToolError::RateLimitExceeded(_)) = result3 {
            // This is expected
        } else {
            panic!("Expected rate limit error");
        }
    }
}