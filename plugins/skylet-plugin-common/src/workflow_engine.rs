// Workflow execution engine for skylet-plugin-common v0.3.0
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub steps: Vec<WorkflowStep>,
    pub metadata: WorkflowMetadata,
}

/// Workflow step definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub step_type: StepType,
    pub input_mapping: HashMap<String, String>,
    pub output_mapping: HashMap<String, String>,
    pub parameters: serde_json::Value,
    pub conditions: Option<Vec<StepCondition>>,
    pub error_handling: ErrorHandling,
    pub timeout_seconds: Option<u64>,
    pub retry_policy: Option<RetryPolicy>,
}

/// Type of workflow step
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepType {
    #[serde(rename = "task")]
    Task { 
        executor_id: String,
        command: String,
        parameters: serde_json::Value,
    },
    #[serde(rename = "condition")]
    Condition {
        condition: String,
        true_path: String,
        false_path: String,
    },
    #[serde(rename = "parallel")]
    Parallel { 
        steps: Vec<WorkflowStep>,
        join_type: JoinType,
    },
    #[serde(rename = "sequential")]
    Sequential {
        steps: Vec<WorkflowStep>,
    },
    #[serde(rename = "subworkflow")]
    SubWorkflow {
        workflow_id: String,
        input_mapping: HashMap<String, String>,
    },
}

/// Parallel execution join type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JoinType {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "first")]
    First,
    #[serde(rename = "first_success")]
    FirstSuccess,
}

/// Step execution conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepCondition {
    pub expression: String,
    pub context_path: String,
}

/// Error handling strategy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorHandling {
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "retry")]
    Retry,
    #[serde(rename = "fail")]
    Fail,
    #[serde(rename = "custom")]
    Custom { strategy: String },
}

/// Retry policy for steps
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_type: BackoffType,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

/// Backoff type for retries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BackoffType {
    #[serde(rename = "fixed")]
    Fixed,
    #[serde(rename = "exponential")]
    Exponential { multiplier: f64 },
    #[serde(rename = "linear")]
    Linear,
}

/// Workflow metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub timeout_seconds: Option<u64>,
    pub max_concurrency: Option<u32>,
    pub priority: WorkflowPriority,
}

/// Workflow execution priority
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowPriority {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "critical")]
    Critical,
}

/// Workflow execution context
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    pub execution_id: String,
    pub workflow_id: String,
    pub variables: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    pub step_results: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    pub metadata: ExecutionMetadata,
}

/// Execution metadata
#[derive(Debug, Clone)]
pub struct ExecutionMetadata {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub triggered_by: Option<String>,
    pub environment: HashMap<String, String>,
    pub parent_execution_id: Option<String>,
    pub correlation_id: Option<String>,
}

/// Workflow input/output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInput {
    pub data: serde_json::Value,
    pub files: Option<Vec<WorkflowFile>>,
    pub context: Option<HashMap<String, String>>,
}

/// File attachment for workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowFile {
    pub id: String,
    pub name: String,
    pub content: Vec<u8>,
    pub content_type: String,
    pub path: String,
}

/// Workflow execution output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowOutput {
    pub data: serde_json::Value,
    pub status: ExecutionStatus,
    pub duration_ms: u64,
    pub logs: Vec<WorkflowLog>,
    pub artifacts: Option<Vec<WorkflowFile>>,
    pub errors: Vec<WorkflowExecutionError>,
}

/// Execution status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "timeout")]
    Timeout,
}

/// Workflow log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowLog {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogLevel,
    pub step_id: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Log level
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogLevel {
    #[serde(rename = "debug")]
    Debug,
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "fatal")]
    Fatal,
}

/// Workflow execution error (data structure for error info during workflow execution)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionError {
    pub step_id: String,
    pub error_code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub recoverable: bool,
}

/// Workflow executor trait
#[async_trait]
pub trait WorkflowExecutor: Send + Sync {
    /// Execute a single workflow step
    async fn execute_step(&self, 
        step: &WorkflowStep, 
        context: &WorkflowContext
    ) -> Result<StepResult, WorkflowError>;

    /// Validate that a step can be executed
    async fn validate_step(&self, 
        step: &WorkflowStep, 
        context: &WorkflowContext
    ) -> Result<(), WorkflowError>;

    /// Get executor capabilities
    fn capabilities(&self) -> ExecutorCapabilities;

    /// Get executor metadata
    fn metadata(&self) -> ExecutorMetadata;
}

/// Result of step execution
#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub status: StepStatus,
    pub output: serde_json::Value,
    pub duration_ms: u64,
    pub logs: Vec<WorkflowLog>,
    pub artifacts: Vec<WorkflowFile>,
}

/// Step execution status
#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    Success,
    Failure,
    Skipped,
}

/// Executor capabilities
#[derive(Debug, Clone)]
pub struct ExecutorCapabilities {
    pub supported_step_types: Vec<String>,
    pub max_parallel_steps: Option<u32>,
    pub supported_file_types: Vec<String>,
    pub requires_network: bool,
    pub max_file_size_mb: Option<u64>,
}

/// Executor metadata
#[derive(Debug, Clone)]
pub struct ExecutorMetadata {
    pub executor_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// Workflow execution engine
pub struct WorkflowEngine {
    workflows: Arc<RwLock<HashMap<String, WorkflowDefinition>>>,
    executors: Arc<RwLock<HashMap<String, Box<dyn WorkflowExecutor>>>>,
    state_store: Box<dyn WorkflowStateStore>,
    execution_queue: Arc<RwLock<Vec<WorkflowExecution>>>,
}

/// Active workflow execution
#[derive(Debug)]
pub struct WorkflowExecution {
    pub id: String,
    pub workflow_id: String,
    pub status: ExecutionStatus,
    pub context: WorkflowContext,
    pub current_step: Option<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Workflow state storage trait
#[async_trait]
pub trait WorkflowStateStore: Send + Sync {
    /// Save workflow state
    async fn save_state(&self, 
        execution_id: &str, 
        state: &WorkflowContext
    ) -> Result<(), WorkflowError>;

    /// Load workflow state
    async fn load_state(&self, 
        execution_id: &str
    ) -> Result<Option<WorkflowContext>, WorkflowError>;

    /// Save execution results
    async fn save_results(&self, 
        execution_id: &str, 
        results: &WorkflowOutput
    ) -> Result<(), WorkflowError>;

    /// Load execution results
    async fn load_results(&self, 
        execution_id: &str
    ) -> Result<Option<WorkflowOutput>, WorkflowError>;

    /// Delete workflow state
    async fn delete_state(&self, 
        execution_id: &str
    ) -> Result<(), WorkflowError>;

    /// List active executions
    async fn list_executions(&self, 
        workflow_id: &str
    ) -> Result<Vec<String>, WorkflowError>;
}

impl WorkflowEngine {
    /// Create a new workflow engine
    pub fn new<S: WorkflowStateStore + 'static>(state_store: S) -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
            state_store: Box::new(state_store),
            execution_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a workflow definition
    pub async fn register_workflow(&self, workflow: WorkflowDefinition) -> Result<(), WorkflowError> {
        let mut workflows = self.workflows.write().await;
        workflows.insert(workflow.id.clone(), workflow);
        Ok(())
    }

    /// Register a workflow executor
    pub async fn register_executor<E: WorkflowExecutor + 'static>(&self, executor: E) -> Result<(), WorkflowError> {
        let mut executors = self.executors.write().await;
        let metadata = executor.metadata();
        executors.insert(metadata.executor_id.clone(), Box::new(executor));
        Ok(())
    }

    /// Execute a workflow
    pub async fn execute_workflow(&self, 
        workflow_id: &str, 
        input: WorkflowInput
    ) -> Result<String, WorkflowError> {
        let execution_id = Uuid::new_v4().to_string();
        
        // Load workflow definition
        let workflow = {
            let workflows = self.workflows.read().await;
            workflows.get(workflow_id)
                .cloned()
                .ok_or_else(|| WorkflowError::workflow_not_found(workflow_id))?
        };

        // Create execution context
        let context = WorkflowContext {
            execution_id: execution_id.clone(),
            workflow_id: workflow_id.to_string(),
            variables: Arc::new(RwLock::new(HashMap::new())),
            step_results: Arc::new(RwLock::new(HashMap::new())),
            metadata: ExecutionMetadata {
                started_at: chrono::Utc::now(),
                triggered_by: None,
                environment: HashMap::new(),
                parent_execution_id: None,
                correlation_id: None,
            },
        };

        // Initialize context with input
        {
            let mut variables = context.variables.write().await;
            variables.insert("input".to_string(), input.data.clone());
            if let Some(files) = &input.files {
                variables.insert("files".to_string(), serde_json::to_value(files).unwrap_or(serde_json::Value::Null));
            }
            if let Some(ctx) = &input.context {
                for (key, value) in ctx {
                    variables.insert(format!("context.{}", key), serde_json::Value::String(value.clone()));
                }
            }
        }

        // Create execution record
        let execution = WorkflowExecution {
            id: execution_id.clone(),
            workflow_id: workflow_id.to_string(),
            status: ExecutionStatus::Running,
            context: context.clone(),
            current_step: None,
            started_at: chrono::Utc::now(),
            completed_at: None,
        };

        // Save initial state
        self.state_store.save_state(&execution_id, &context).await?;
        
        // Add to execution queue
        {
            let mut queue = self.execution_queue.write().await;
            queue.push(execution);
        }

        // Start execution (simplified - real implementation would use proper async task spawning)
        self.execute_workflow_steps(workflow_id, &workflow.steps, &context).await?;

        // Generate final output
        let output = WorkflowOutput {
            data: serde_json::json!({
                "status": "completed",
                "execution_id": execution_id
            }),
            status: ExecutionStatus::Completed,
            duration_ms: 1000, // Would calculate actual duration
            logs: vec![],
            artifacts: None,
            errors: vec![],
        };

        // Save results
        self.state_store.save_results(&execution_id, &output).await?;

        Ok(execution_id)
    }

    /// Execute workflow steps
    async fn execute_workflow_steps(&self, 
        workflow_id: &str,
        steps: &[WorkflowStep],
        context: &WorkflowContext
    ) -> Result<(), WorkflowError> {
        for step in steps {
            self.execute_step_internal(workflow_id, step, context).await?;
        }
        Ok(())
    }

    /// Execute a single step internally
    async fn execute_step_internal(&self, 
        workflow_id: &str,
        step: &WorkflowStep,
        context: &WorkflowContext
    ) -> Result<(), WorkflowError> {
        // Get appropriate executor
        let executors = self.executors.read().await;
        let executor = match &step.step_type {
            StepType::Task { executor_id, .. } => {
                executors.get(executor_id)
                    .ok_or_else(|| WorkflowError::executor_not_found(executor_id))?
            }
            // Handle other step types (condition, parallel, sequential, subworkflow)
            _ => {
                return Err(WorkflowError::unsupported_step_type(format!("{:?}", step.step_type)));
            }
        };

        // Validate step
        executor.validate_step(step, context).await?;

        // Execute step
        let step_result = executor.execute_step(step, context).await?;

        // Store step result
        {
            let mut step_results = context.step_results.write().await;
            step_results.insert(step.id.clone(), serde_json::Value::Object({
                let mut map = serde_json::Map::new();
                map.insert("status".to_string(), serde_json::Value::String(format!("{:?}", step_result.status)));
                map.insert("output".to_string(), step_result.output.clone());
                map
            }));
        }

        Ok(())
    }

    /// Get execution status
    pub async fn get_execution_status(&self, 
        execution_id: &str
    ) -> Result<Option<ExecutionStatus>, WorkflowError> {
        self.state_store.load_results(execution_id).await
            .map(|results| results.map(|r| r.status))
    }

    /// Cancel execution
    pub async fn cancel_execution(&self, 
        execution_id: &str
    ) -> Result<(), WorkflowError> {
        // Update execution status to cancelled
        // This is a simplified implementation
        Ok(())
    }

    /// List available workflows
    pub async fn list_workflows(&self) -> Vec<String> {
        let workflows = self.workflows.read().await;
        workflows.keys().cloned().collect()
    }

    /// List available executors
    pub async fn list_executors(&self) -> Vec<ExecutorMetadata> {
        let executors = self.executors.read().await;
        executors.values()
            .map(|executor| executor.metadata())
            .collect()
    }

    /// Get workflow definition
    pub async fn get_workflow(&self, workflow_id: &str) -> Option<WorkflowDefinition> {
        let workflows = self.workflows.read().await;
        workflows.get(workflow_id).cloned()
    }
}

/// Workflow error types
#[derive(thiserror::Error, Debug)]
pub enum WorkflowError {
    #[error("Workflow not found: {0}")]
    WorkflowNotFound(String),

    #[error("Executor not found: {0}")]
    ExecutorNotFound(String),

    #[error("Step execution failed: {0}")]
    StepExecutionFailed(String),

    #[error("Unsupported step type: {0}")]
    UnsupportedStepType(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("State store error: {0}")]
    StateStoreError(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Concurrency limit exceeded: {0}")]
    ConcurrencyLimitExceeded(String),
}

impl WorkflowError {
    pub fn workflow_not_found(id: &str) -> Self {
        Self::WorkflowNotFound(id.to_string())
    }

    pub fn executor_not_found(id: &str) -> Self {
        Self::ExecutorNotFound(id.to_string())
    }

    pub fn step_execution_failed(msg: impl Into<String>) -> Self {
        Self::StepExecutionFailed(msg.into())
    }

    pub fn unsupported_step_type(step_type: impl Into<String>) -> Self {
        Self::UnsupportedStepType(step_type.into())
    }

    pub fn validation_failed(msg: impl Into<String>) -> Self {
        Self::ValidationFailed(msg.into())
    }

    pub fn state_store_error(msg: impl Into<String>) -> Self {
        Self::StateStoreError(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    pub fn concurrency_limit_exceeded(msg: impl Into<String>) -> Self {
        Self::ConcurrencyLimitExceeded(msg.into())
    }
}

/// In-memory workflow state store
pub struct InMemoryWorkflowStateStore {
    states: Arc<RwLock<HashMap<String, WorkflowContext>>>,
    results: Arc<RwLock<HashMap<String, WorkflowOutput>>>,
}

impl InMemoryWorkflowStateStore {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl WorkflowStateStore for InMemoryWorkflowStateStore {
    async fn save_state(&self, 
        execution_id: &str, 
        state: &WorkflowContext
    ) -> Result<(), WorkflowError> {
        let mut states = self.states.write().await;
        states.insert(execution_id.to_string(), state.clone());
        Ok(())
    }

    async fn load_state(&self, 
        execution_id: &str
    ) -> Result<Option<WorkflowContext>, WorkflowError> {
        let states = self.states.read().await;
        Ok(states.get(execution_id).cloned())
    }

    async fn save_results(&self, 
        execution_id: &str, 
        results: &WorkflowOutput
    ) -> Result<(), WorkflowError> {
        let mut results_store = self.results.write().await;
        results_store.insert(execution_id.to_string(), results.clone());
        Ok(())
    }

    async fn load_results(&self, 
        execution_id: &str
    ) -> Result<Option<WorkflowOutput>, WorkflowError> {
        let results_store = self.results.read().await;
        Ok(results_store.get(execution_id).cloned())
    }

    async fn delete_state(&self, 
        execution_id: &str
    ) -> Result<(), WorkflowError> {
        let mut states = self.states.write().await;
        states.remove(execution_id);
        
        let mut results_store = self.results.write().await;
        results_store.remove(execution_id);
        
        Ok(())
    }

    async fn list_executions(&self, 
        _workflow_id: &str
    ) -> Result<Vec<String>, WorkflowError> {
        let states = self.states.read().await;
        Ok(states.keys().cloned().collect())
    }
}

/// Convenience functions
pub fn create_workflow_engine<S: WorkflowStateStore + 'static>(state_store: S) -> WorkflowEngine {
    WorkflowEngine::new(state_store)
}

pub fn create_in_memory_state_store() -> InMemoryWorkflowStateStore {
    InMemoryWorkflowStateStore::new()
}

pub fn create_workflow_definition(
    id: &str,
    name: &str,
    description: &str,
    steps: Vec<WorkflowStep>,
) -> WorkflowDefinition {
    WorkflowDefinition {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        version: "1.0.0".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "data": { "type": "any" }
            }
        }),
        output_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "result": { "type": "any" }
            }
        }),
        steps,
        metadata: WorkflowMetadata {
            author: None,
            tags: vec![],
            timeout_seconds: None,
            max_concurrency: None,
            priority: WorkflowPriority::Medium,
        },
    }
}

pub fn create_task_step(
    id: &str,
    name: &str,
    executor_id: &str,
    command: &str,
) -> WorkflowStep {
    WorkflowStep {
        id: id.to_string(),
        name: name.to_string(),
        step_type: StepType::Task {
            executor_id: executor_id.to_string(),
            command: command.to_string(),
            parameters: serde_json::json!({}),
        },
        input_mapping: HashMap::new(),
        output_mapping: HashMap::new(),
        parameters: serde_json::json!({}),
        conditions: None,
        error_handling: ErrorHandling::Fail,
        timeout_seconds: None,
        retry_policy: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_registration() {
        let engine = create_workflow_engine(create_in_memory_state_store());
        
        let workflow = create_workflow_definition(
            "test-workflow",
            "Test Workflow",
            "A test workflow",
            vec![create_task_step("step1", "Step 1", "test-executor", "echo hello")],
        );
        
        let result = engine.register_workflow(workflow).await;
        assert!(result.is_ok());
        
        let workflows = engine.list_workflows().await;
        assert!(workflows.contains(&"test-workflow".to_string()));
    }

     #[tokio::test]
     async fn test_in_memory_state_store() {
        let store = create_in_memory_state_store();
        
        let context = WorkflowContext {
            execution_id: "test-execution".to_string(),
            workflow_id: "test-workflow".to_string(),
            variables: Arc::new(RwLock::new(HashMap::new())),
            step_results: Arc::new(RwLock::new(HashMap::new())),
            metadata: ExecutionMetadata {
                started_at: chrono::Utc::now(),
                triggered_by: Some("test".to_string()),
                environment: HashMap::new(),
                parent_execution_id: None,
                correlation_id: Some("test-correlation".to_string()),
            },
        };

        // Test saving and loading
        let result = store.save_state("test-execution", &context).await;
        assert!(result.is_ok());
        
        let loaded_context = store.load_state("test-execution").await;
         assert!(loaded_context.is_ok());
         
         let loaded = loaded_context.unwrap().unwrap();
         assert_eq!(loaded.execution_id, context.execution_id);
         assert_eq!(loaded.workflow_id, context.workflow_id);
    }

    #[test]
    fn test_workflow_step_creation() {
        let step = create_task_step("step1", "Test Step", "test-executor", "echo hello");
        
        assert_eq!(step.id, "step1");
        assert_eq!(step.name, "Test Step");
        
        if let StepType::Task { executor_id, command, .. } = step.step_type {
            assert_eq!(executor_id, "test-executor");
            assert_eq!(command, "echo hello");
        } else {
            panic!("Expected Task step type");
        }
    }

    #[test]
    fn test_workflow_error_creation() {
        let error = WorkflowError::workflow_not_found("test-workflow");
        assert!(error.to_string().contains("Workflow not found"));
    }
}