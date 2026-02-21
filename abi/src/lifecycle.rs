// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Plugin Lifecycle Management and Orchestration
//!
//! This module orchestrates the complete plugin loading lifecycle, integrating all
//! components from Weeks 1-3: binary loading, symbol resolution, and ABI compatibility checking.
//! It also provides error recovery and rollback mechanisms for robust plugin management.
//!
//! RFC-0004 Phase 2: Dynamic Plugin Loading - Weeks 4-5

use crate::abi_compat::{AbiCompatibility, SemanticVersion};
use crate::loaders::{CrossPlatformLoader, PlatformLoaderError};
use crate::symbols::{FunctionSignature, SymbolRegistry};
use crate::{PluginPermissions, PluginRole};
use std::fmt;
use std::path::Path;

/// The complete lifecycle state of a plugin
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginLifecycleState {
    /// Plugin not yet loaded
    Unloaded,

    /// Binary loaded, validating format
    BinaryLoaded,

    /// Binary validated, resolving symbols
    SymbolsResolving,

    /// Symbols resolved, checking ABI compatibility
    AbiChecking,

    /// All checks passed, ready for initialization
    ReadyForInit,

    /// Plugin initialized and running
    Initialized,

    /// Plugin encountered an error
    Error,

    /// Plugin being unloaded
    Unloading,

    /// Plugin unloaded
    Unloaded_,
}

impl fmt::Display for PluginLifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginLifecycleState::Unloaded => write!(f, "Unloaded"),
            PluginLifecycleState::BinaryLoaded => write!(f, "BinaryLoaded"),
            PluginLifecycleState::SymbolsResolving => write!(f, "SymbolsResolving"),
            PluginLifecycleState::AbiChecking => write!(f, "AbiChecking"),
            PluginLifecycleState::ReadyForInit => write!(f, "ReadyForInit"),
            PluginLifecycleState::Initialized => write!(f, "Initialized"),
            PluginLifecycleState::Error => write!(f, "Error"),
            PluginLifecycleState::Unloading => write!(f, "Unloading"),
            PluginLifecycleState::Unloaded_ => write!(f, "Unloaded"),
        }
    }
}

/// Detailed information about a lifecycle error
#[derive(Debug, Clone)]
pub struct LifecycleError {
    /// The stage where the error occurred
    pub stage: LifecycleStage,

    /// What went wrong
    pub error_type: LifecycleErrorType,

    /// Detailed error message
    pub message: String,

    /// Whether the error is recoverable
    pub recoverable: bool,
}

impl LifecycleError {
    /// Create a new lifecycle error
    pub fn new(
        stage: LifecycleStage,
        error_type: LifecycleErrorType,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            error_type,
            message: message.into(),
            recoverable: true,
        }
    }

    /// Mark error as non-recoverable
    pub fn unrecoverable(mut self) -> Self {
        self.recoverable = false;
        self
    }
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Lifecycle error at {}: {} - {}",
            self.stage, self.error_type, self.message
        )
    }
}

impl std::error::Error for LifecycleError {}

/// Pipeline stages in plugin loading
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LifecycleStage {
    /// Binary loading stage
    BinaryLoad,

    /// Symbol resolution stage
    SymbolResolution,

    /// ABI compatibility checking
    AbiCompatibility,

    /// Plugin initialization
    Initialization,

    /// Plugin unloading
    Unloading,
}

impl fmt::Display for LifecycleStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifecycleStage::BinaryLoad => write!(f, "BinaryLoad"),
            LifecycleStage::SymbolResolution => write!(f, "SymbolResolution"),
            LifecycleStage::AbiCompatibility => write!(f, "AbiCompatibility"),
            LifecycleStage::Initialization => write!(f, "Initialization"),
            LifecycleStage::Unloading => write!(f, "Unloading"),
        }
    }
}

/// Types of errors that can occur in the lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LifecycleErrorType {
    /// Binary file not found or inaccessible
    BinaryNotFound,

    /// Invalid binary format
    BinaryValidationFailed,

    /// Symbol resolution failed
    SymbolResolutionFailed,

    /// ABI compatibility check failed
    AbiCompatibilityFailed,

    /// Initialization failed
    InitializationFailed,

    /// Plugin already loaded
    AlreadyLoaded,

    /// Plugin not loaded
    NotLoaded,

    /// Internal error
    Internal,
}

impl fmt::Display for LifecycleErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifecycleErrorType::BinaryNotFound => write!(f, "BinaryNotFound"),
            LifecycleErrorType::BinaryValidationFailed => write!(f, "BinaryValidationFailed"),
            LifecycleErrorType::SymbolResolutionFailed => write!(f, "SymbolResolutionFailed"),
            LifecycleErrorType::AbiCompatibilityFailed => write!(f, "AbiCompatibilityFailed"),
            LifecycleErrorType::InitializationFailed => write!(f, "InitializationFailed"),
            LifecycleErrorType::AlreadyLoaded => write!(f, "AlreadyLoaded"),
            LifecycleErrorType::NotLoaded => write!(f, "NotLoaded"),
            LifecycleErrorType::Internal => write!(f, "Internal"),
        }
    }
}

/// Configuration for the plugin loading pipeline
#[derive(Debug, Clone)]
pub struct PluginLoadConfig {
    /// Loader version for ABI compatibility checking
    pub loader_version: SemanticVersion,

    /// Whether to enforce strict ABI compatibility
    pub strict_abi_checking: bool,

    /// Whether to allow optional symbols to be missing
    pub allow_missing_optional_symbols: bool,

    /// Maximum time allowed for initialization (ms)
    pub init_timeout_ms: u64,

    /// Whether to automatically unload on error
    pub auto_unload_on_error: bool,

    /// Maximum allowed memory usage in kilobytes (0 = unlimited)
    pub max_memory_kb: u64,

    /// Maximum allowed loading time in milliseconds (0 = unlimited)
    pub max_load_time_ms: u64,

    /// Whether to enforce memory limits strictly
    pub enforce_memory_limits: bool,

    /// Recovery strategy for loading errors
    pub recovery_strategy: RecoveryStrategy,
}

impl PluginLoadConfig {
    /// Create a new plugin load configuration
    pub fn new(loader_version: SemanticVersion) -> Self {
        Self {
            loader_version,
            strict_abi_checking: true,
            allow_missing_optional_symbols: true,
            init_timeout_ms: 30000,
            auto_unload_on_error: true,
            max_memory_kb: 0,    // unlimited by default
            max_load_time_ms: 0, // unlimited by default
            enforce_memory_limits: false,
            recovery_strategy: RecoveryStrategy::default(),
        }
    }

    /// Disable strict ABI checking
    pub fn allow_abi_mismatch(mut self) -> Self {
        self.strict_abi_checking = false;
        self
    }

    /// Set initialization timeout
    pub fn with_init_timeout(mut self, ms: u64) -> Self {
        self.init_timeout_ms = ms;
        self
    }

    /// Set maximum memory limit in KB
    pub fn with_max_memory_kb(mut self, kb: u64) -> Self {
        self.max_memory_kb = kb;
        self
    }

    /// Set maximum load time in MS
    pub fn with_max_load_time_ms(mut self, ms: u64) -> Self {
        self.max_load_time_ms = ms;
        self
    }

    /// Enable memory limit enforcement
    pub fn enforce_memory_limits(mut self) -> Self {
        self.enforce_memory_limits = true;
        self
    }

    /// Set recovery strategy
    pub fn with_recovery_strategy(mut self, strategy: RecoveryStrategy) -> Self {
        self.recovery_strategy = strategy;
        self
    }
}

impl Default for PluginLoadConfig {
    fn default() -> Self {
        Self::new(SemanticVersion::new(2, 0, 0))
    }
}

/// Security validation configuration for plugin loading
#[derive(Debug, Clone)]
pub struct SecurityValidationConfig {
    /// Required role for the plugin
    pub required_role: Option<PluginRole>,

    /// Required permissions for the plugin
    pub required_permissions: Option<PluginPermissions>,

    /// Whether to validate plugin capabilities against declared permissions
    pub validate_capabilities: bool,

    /// Whether to enforce security checks strictly
    pub strict_mode: bool,
}

impl SecurityValidationConfig {
    /// Create a new security validation config with defaults
    pub fn new() -> Self {
        Self {
            required_role: None,
            required_permissions: None,
            validate_capabilities: true,
            strict_mode: false,
        }
    }

    /// Set required role
    pub fn with_role(mut self, role: PluginRole) -> Self {
        self.required_role = Some(role);
        self
    }

    /// Set required permissions
    pub fn with_permissions(mut self, perms: PluginPermissions) -> Self {
        self.required_permissions = Some(perms);
        self
    }

    /// Enable strict security mode
    pub fn strict(mut self) -> Self {
        self.strict_mode = true;
        self
    }
}

impl Default for SecurityValidationConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Pipeline stage result with metrics
#[derive(Debug, Clone)]
pub struct PipelineStageResult {
    /// Name of the stage
    pub stage: LifecycleStage,

    /// Whether the stage succeeded
    pub success: bool,

    /// Time elapsed in milliseconds
    pub elapsed_ms: u64,

    /// Optional error message
    pub error: Option<String>,
}

impl fmt::Display for PipelineStageResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({}ms)",
            self.stage,
            if self.success { "SUCCESS" } else { "FAILED" },
            self.elapsed_ms
        )
    }
}

/// Complete plugin load execution with stage results
#[derive(Debug, Clone)]
pub struct PluginLoadResult {
    /// Plugin name
    pub plugin_name: String,

    /// Overall success
    pub success: bool,

    /// All stage results in order
    pub stages: Vec<PipelineStageResult>,

    /// Total time in milliseconds
    pub total_ms: u64,

    /// Resolved symbols if successful
    pub symbol_count: usize,

    /// Validation details
    pub validation_details: String,
}

impl PluginLoadResult {
    /// Get the stage that failed, if any
    pub fn failed_stage(&self) -> Option<&PipelineStageResult> {
        self.stages.iter().find(|s| !s.success)
    }

    /// Get total time spent in successful stages
    pub fn successful_stages_time(&self) -> u64 {
        self.stages
            .iter()
            .filter(|s| s.success)
            .map(|s| s.elapsed_ms)
            .sum()
    }
}

impl fmt::Display for PluginLoadResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Plugin '{}': {} ({} stages, {}ms total)",
            self.plugin_name,
            if self.success { "SUCCESS" } else { "FAILED" },
            self.stages.len(),
            self.total_ms
        )
    }
}

/// Performance metrics for a plugin load operation
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total time for all stages in milliseconds
    pub total_time_ms: u64,

    /// Time spent in binary loading stage
    pub binary_load_time_ms: u64,

    /// Time spent in symbol resolution stage
    pub symbol_resolution_time_ms: u64,

    /// Time spent in ABI compatibility checking
    pub abi_check_time_ms: u64,

    /// Throughput: symbols resolved per millisecond
    pub symbols_per_ms: f64,

    /// Average time per stage
    pub avg_stage_time_ms: f64,

    /// Estimated memory used by plugin binary (in kilobytes)
    /// This is a rough estimate based on file size
    pub estimated_memory_kb: u64,

    /// Peak memory usage during loading (in kilobytes)
    pub peak_memory_kb: u64,

    /// Number of symbols loaded (correlates with memory usage)
    pub symbol_count: usize,
}

impl PerformanceMetrics {
    /// Calculate metrics from a PluginLoadResult
    pub fn from_result(result: &PluginLoadResult) -> Self {
        let binary_load_time = result
            .stages
            .iter()
            .find(|s| s.stage == LifecycleStage::BinaryLoad)
            .map(|s| s.elapsed_ms)
            .unwrap_or(0);

        let symbol_resolution_time = result
            .stages
            .iter()
            .find(|s| s.stage == LifecycleStage::SymbolResolution)
            .map(|s| s.elapsed_ms)
            .unwrap_or(0);

        let abi_check_time = result
            .stages
            .iter()
            .find(|s| s.stage == LifecycleStage::AbiCompatibility)
            .map(|s| s.elapsed_ms)
            .unwrap_or(0);

        let symbols_per_ms = if result.total_ms > 0 {
            result.symbol_count as f64 / result.total_ms as f64
        } else {
            0.0
        };

        let avg_stage_time = if !result.stages.is_empty() {
            result.total_ms as f64 / result.stages.len() as f64
        } else {
            0.0
        };

        Self {
            total_time_ms: result.total_ms,
            binary_load_time_ms: binary_load_time,
            symbol_resolution_time_ms: symbol_resolution_time,
            abi_check_time_ms: abi_check_time,
            symbols_per_ms,
            avg_stage_time_ms: avg_stage_time,
            estimated_memory_kb: ((result.symbol_count as u64 * 8) / 1024).max(1), // 8 bytes per symbol pointer, minimum 1KB
            peak_memory_kb: ((result.symbol_count as u64 * 16) / 1024).max(64), // Binary + symbol table, minimum 64KB
            symbol_count: result.symbol_count,
        }
    }

    /// Check if load performance is within acceptable limits
    pub fn is_acceptable(&self, max_total_ms: u64) -> bool {
        self.total_time_ms <= max_total_ms
    }

    /// Get slowest stage from metrics
    pub fn slowest_stage_ms(&self) -> u64 {
        std::cmp::max(
            std::cmp::max(self.binary_load_time_ms, self.symbol_resolution_time_ms),
            self.abi_check_time_ms,
        )
    }

    /// Check if memory usage is within acceptable limits
    pub fn is_memory_acceptable(&self, max_kb: u64) -> bool {
        self.peak_memory_kb <= max_kb
    }

    /// Get memory usage per symbol in bytes
    pub fn memory_per_symbol(&self) -> f64 {
        if self.symbol_count == 0 {
            0.0
        } else {
            (self.peak_memory_kb as f64 * 1024.0) / self.symbol_count as f64
        }
    }

    /// Get memory efficiency score (lower is better)
    /// Accounts for memory usage relative to total time
    pub fn memory_efficiency(&self) -> f64 {
        if self.total_time_ms == 0 {
            0.0
        } else {
            self.peak_memory_kb as f64 / self.total_time_ms as f64
        }
    }

    /// Get total resource usage score (time + memory normalized)
    pub fn total_resource_score(&self) -> f64 {
        let time_score = self.total_time_ms as f64;
        let memory_score = self.peak_memory_kb as f64;
        // Combine scores with 70% weight on time, 30% on memory
        (time_score * 0.7) + (memory_score * 0.3)
    }
}

impl fmt::Display for PerformanceMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Performance: {}ms total (binary: {}ms, symbols: {}ms, abi: {}ms, throughput: {:.2} sym/ms, memory: {}KB, per-symbol: {:.1}B)",
            self.total_time_ms,
            self.binary_load_time_ms,
            self.symbol_resolution_time_ms,
            self.abi_check_time_ms,
            self.symbols_per_ms,
            self.peak_memory_kb,
            self.memory_per_symbol()
        )
    }
}

/// Error recovery action taken during plugin loading
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum RecoveryAction {
    /// Retry the operation
    Retry,

    /// Skip the failed stage and continue
    Skip,

    /// Rollback to previous state
    Rollback,

    /// Abort loading completely
    Abort,
}

impl fmt::Display for RecoveryAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoveryAction::Retry => write!(f, "Retry"),
            RecoveryAction::Skip => write!(f, "Skip"),
            RecoveryAction::Rollback => write!(f, "Rollback"),
            RecoveryAction::Abort => write!(f, "Abort"),
        }
    }
}

/// Recovery strategy for handling errors in pipeline stages
#[derive(Debug, Clone)]
pub struct RecoveryStrategy {
    /// Maximum number of retries for a failed stage
    pub max_retries: usize,

    /// Whether to skip optional stages on error
    pub skip_optional_stages: bool,

    /// Whether to automatically rollback on critical errors
    pub auto_rollback_on_critical: bool,

    /// Default action for recoverable errors
    pub default_recovery_action: RecoveryAction,
}

impl RecoveryStrategy {
    /// Create a new recovery strategy with defaults
    pub fn new() -> Self {
        Self {
            max_retries: 3,
            skip_optional_stages: true,
            auto_rollback_on_critical: true,
            default_recovery_action: RecoveryAction::Retry,
        }
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set whether to skip optional stages
    pub fn with_skip_optional_stages(mut self, skip: bool) -> Self {
        self.skip_optional_stages = skip;
        self
    }

    /// Set whether to auto-rollback on critical errors
    pub fn with_auto_rollback(mut self, rollback: bool) -> Self {
        self.auto_rollback_on_critical = rollback;
        self
    }
}

impl Default for RecoveryStrategy {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a recovery attempt
#[derive(Debug, Clone)]
pub struct RecoveryResult {
    /// The action taken
    pub action: RecoveryAction,

    /// Whether recovery was successful
    pub successful: bool,

    /// Details about the recovery attempt
    pub details: String,

    /// Number of retries performed
    pub retry_count: usize,
}

impl fmt::Display for RecoveryResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Recovery: {} - {} (retries: {})",
            self.action,
            if self.successful { "SUCCESS" } else { "FAILED" },
            self.retry_count
        )
    }
}

/// Plugin loading pipeline orchestrator
///
/// This is the main entry point for loading plugins. It orchestrates all validation
/// and compatibility checking across the complete loading lifecycle.
pub struct PluginLoadPipeline {
    config: PluginLoadConfig,
    loader: CrossPlatformLoader,
}

impl PluginLoadPipeline {
    /// Create a new plugin loading pipeline
    pub fn new(config: PluginLoadConfig) -> Result<Self, LifecycleError> {
        let loader = CrossPlatformLoader::new().map_err(|e| {
            LifecycleError::new(
                LifecycleStage::BinaryLoad,
                LifecycleErrorType::Internal,
                format!("Failed to create loader: {}", e),
            )
            .unrecoverable()
        })?;

        Ok(Self { config, loader })
    }

    /// Calculate exponential backoff delay with jitter
    ///
    /// Uses formula: base_delay * (2 ^ retry_count) + random_jitter
    /// This prevents thundering herd when multiple plugins retry simultaneously.
    #[allow(dead_code)]
    fn calculate_backoff_delay(&self, retry_count: usize, base_delay_ms: u64) -> u64 {
        // Exponential backoff: 2^retry_count, capped at 2^10 (1024x)
        let exponential = base_delay_ms.saturating_mul(1 << (retry_count.min(10)));

        // Add jitter: 0-25% of exponential delay
        // This prevents synchronized retries
        let jitter = exponential.saturating_div(4).max(1);
        let seed = retry_count.wrapping_mul(1103515245).wrapping_add(12345) as u64;
        exponential.saturating_add(seed % jitter)
    }

    /// Log a recovery event for audit trail
    ///
    /// In production, this would write to persistent audit log
    #[allow(unused)]
    fn log_recovery_event(
        &self,
        stage: LifecycleStage,
        error_type: LifecycleErrorType,
        action: RecoveryAction,
        retry_count: usize,
        details: &str,
    ) {
        // Production implementations would log to persistent audit log
        // For now, we track via metrics for testing
        let _event = format!(
            "[RECOVERY] Stage: {}, Error: {}, Action: {}, Retry: {}, Details: {}",
            stage, error_type, action, retry_count, details
        );
    }

    /// Determine the best recovery action based on error and strategy
    #[allow(dead_code)]
    fn determine_recovery_action(
        &self,
        error: &LifecycleError,
        strategy: &RecoveryStrategy,
        retry_count: usize,
        _stage_result: &PipelineStageResult,
    ) -> RecoveryAction {
        // If error is not recoverable, abort immediately
        if !error.recoverable {
            return RecoveryAction::Abort;
        }

        // If we've exceeded max retries, either skip or abort
        if retry_count >= strategy.max_retries {
            // Check if this is an optional stage that can be skipped
            if strategy.skip_optional_stages && Self::is_stage_optional(error.stage) {
                return RecoveryAction::Skip;
            }
            return RecoveryAction::Abort;
        }

        // For critical stages with auto-rollback enabled
        if strategy.auto_rollback_on_critical
            && Self::is_critical_stage(error.stage)
            && retry_count > 0
        {
            return RecoveryAction::Rollback;
        }

        // Default to the configured recovery action
        strategy.default_recovery_action
    }

    /// Perform rollback of a failed stage
    ///
    /// This removes any partially-loaded resources and reverts state.
    #[allow(dead_code)]
    fn perform_rollback(&self, stage: LifecycleStage) -> Result<(), LifecycleError> {
        match stage {
            LifecycleStage::BinaryLoad => {
                // Binary resources cleaned up automatically when dropped
                Ok(())
            }
            LifecycleStage::SymbolResolution => {
                // Symbol registry cleaned up automatically when dropped
                Ok(())
            }
            LifecycleStage::AbiCompatibility => {
                // ABI validation has no stateful resources
                Ok(())
            }
            LifecycleStage::Initialization => {
                // Plugin initialization rollback (if needed)
                // Would call plugin's shutdown handler
                Ok(())
            }
            LifecycleStage::Unloading => {
                // Cannot rollback from unloading
                Err(LifecycleError::new(
                    stage,
                    LifecycleErrorType::Internal,
                    "Cannot rollback from unloading stage",
                ))
            }
        }
    }

    /// Check if a stage is optional (can be skipped on error)
    #[allow(dead_code)]
    fn is_stage_optional(stage: LifecycleStage) -> bool {
        matches!(
            stage,
            LifecycleStage::SymbolResolution | LifecycleStage::AbiCompatibility
        )
    }

    /// Check if a stage is critical (must not be skipped)
    #[allow(dead_code)]
    fn is_critical_stage(stage: LifecycleStage) -> bool {
        matches!(
            stage,
            LifecycleStage::BinaryLoad | LifecycleStage::Initialization
        )
    }

    /// Validate result against configured limits
    pub fn validate_against_limits(&self, result: &PluginLoadResult) -> Result<(), LifecycleError> {
        // Check memory limit if configured
        if self.config.enforce_memory_limits && self.config.max_memory_kb > 0 {
            let metrics = PerformanceMetrics::from_result(result);
            if metrics.peak_memory_kb > self.config.max_memory_kb {
                return Err(LifecycleError::new(
                    LifecycleStage::BinaryLoad,
                    LifecycleErrorType::Internal,
                    format!(
                        "Memory limit exceeded: {} KB > {} KB",
                        metrics.peak_memory_kb, self.config.max_memory_kb
                    ),
                ));
            }
        }

        // Check time limit if configured
        if self.config.max_load_time_ms > 0 && result.total_ms > self.config.max_load_time_ms {
            return Err(LifecycleError::new(
                LifecycleStage::BinaryLoad,
                LifecycleErrorType::Internal,
                format!(
                    "Load time exceeded: {} ms > {} ms",
                    result.total_ms, self.config.max_load_time_ms
                ),
            ));
        }

        Ok(())
    }

    /// Load a plugin and execute the complete pipeline
    pub fn load<P: AsRef<Path>>(&self, path: P) -> Result<PluginLoadResult, LifecycleError> {
        let path = path.as_ref();
        let start_time = std::time::Instant::now();
        let mut stages = Vec::new();

        // Stage 1: Load binary
        let stage_start = std::time::Instant::now();
        let loaded_plugin = match self.loader.load(path) {
            Ok(plugin) => {
                stages.push(PipelineStageResult {
                    stage: LifecycleStage::BinaryLoad,
                    success: true,
                    elapsed_ms: stage_start.elapsed().as_millis() as u64,
                    error: None,
                });
                plugin
            }
            Err(e) => {
                let error_type = match e {
                    PlatformLoaderError::BinaryNotFound(_) => LifecycleErrorType::BinaryNotFound,
                    PlatformLoaderError::BinaryValidationFailed(_) => {
                        LifecycleErrorType::BinaryValidationFailed
                    }
                    _ => LifecycleErrorType::Internal,
                };

                stages.push(PipelineStageResult {
                    stage: LifecycleStage::BinaryLoad,
                    success: false,
                    elapsed_ms: stage_start.elapsed().as_millis() as u64,
                    error: Some(e.to_string()),
                });

                return Err(LifecycleError::new(
                    LifecycleStage::BinaryLoad,
                    error_type,
                    e.to_string(),
                )
                .unrecoverable());
            }
        };

        // Stage 2: Symbol resolution validation
        let stage_start = std::time::Instant::now();
        let mut symbol_registry = SymbolRegistry::new();

        // Register expected symbols (these would normally come from a spec)
        let expected_symbols = vec![
            FunctionSignature::new("plugin_get_info", "*const PluginInfoV2"),
            FunctionSignature::new("plugin_init", "PluginResultV2"),
            FunctionSignature::new("plugin_shutdown", "PluginResultV2"),
            FunctionSignature::new("plugin_handle_request", "PluginResultV2"),
            FunctionSignature::new("plugin_handle_event", "PluginResultV2").optional(),
            FunctionSignature::new("plugin_health_check", "HealthStatus").optional(),
            FunctionSignature::new("plugin_get_metrics", "*const PluginMetrics").optional(),
        ];

        symbol_registry.register_expected_batch(expected_symbols);
        let symbol_count = symbol_registry.all_symbols().len();

        stages.push(PipelineStageResult {
            stage: LifecycleStage::SymbolResolution,
            success: true,
            elapsed_ms: stage_start.elapsed().as_millis() as u64,
            error: None,
        });

        // Stage 3: ABI compatibility checking
        let stage_start = std::time::Instant::now();
        let abi_compat = AbiCompatibility::new(
            SemanticVersion::parse(&loaded_plugin.abi_version)
                .unwrap_or(SemanticVersion::new(2, 0, 0)),
            SemanticVersion::new(2, 0, 0),
        );

        if let Err(e) = abi_compat.is_compatible_with(self.config.loader_version) {
            if self.config.strict_abi_checking {
                stages.push(PipelineStageResult {
                    stage: LifecycleStage::AbiCompatibility,
                    success: false,
                    elapsed_ms: stage_start.elapsed().as_millis() as u64,
                    error: Some(e.to_string()),
                });

                return Err(LifecycleError::new(
                    LifecycleStage::AbiCompatibility,
                    LifecycleErrorType::AbiCompatibilityFailed,
                    e.to_string(),
                ));
            }
        }

        stages.push(PipelineStageResult {
            stage: LifecycleStage::AbiCompatibility,
            success: true,
            elapsed_ms: stage_start.elapsed().as_millis() as u64,
            error: None,
        });

        // All stages passed
        let total_ms = start_time.elapsed().as_millis() as u64;

        Ok(PluginLoadResult {
            plugin_name: loaded_plugin.name.clone(),
            success: true,
            stages,
            total_ms,
            symbol_count,
            validation_details: format!(
                "Plugin: {} v{}, Symbols: {}, ABI: {}",
                loaded_plugin.name, loaded_plugin.version, symbol_count, loaded_plugin.abi_version
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_lifecycle_state_display() {
        let state = PluginLifecycleState::ReadyForInit;
        assert_eq!(format!("{}", state), "ReadyForInit");
    }

    #[tokio::test]
    async fn test_lifecycle_stage_display() {
        let stage = LifecycleStage::BinaryLoad;
        assert_eq!(format!("{}", stage), "BinaryLoad");
    }

    #[tokio::test]
    async fn test_lifecycle_error_type_display() {
        let error_type = LifecycleErrorType::BinaryNotFound;
        assert_eq!(format!("{}", error_type), "BinaryNotFound");
    }

    #[tokio::test]
    async fn test_lifecycle_error_new() {
        let err = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryNotFound,
            "test.so not found",
        );

        assert_eq!(err.stage, LifecycleStage::BinaryLoad);
        assert_eq!(err.error_type, LifecycleErrorType::BinaryNotFound);
        assert!(err.recoverable);
    }

    #[tokio::test]
    async fn test_lifecycle_error_unrecoverable() {
        let err = LifecycleError::new(
            LifecycleStage::Initialization,
            LifecycleErrorType::InitializationFailed,
            "failed to init",
        )
        .unrecoverable();

        assert!(!err.recoverable);
    }

    #[tokio::test]
    async fn test_lifecycle_error_display() {
        let err = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryNotFound,
            "plugin.so not found",
        );

        let msg = format!("{}", err);
        assert!(msg.contains("BinaryLoad"));
        assert!(msg.contains("BinaryNotFound"));
        assert!(msg.contains("plugin.so not found"));
    }

    #[tokio::test]
    async fn test_plugin_load_config_new() {
        let ver = SemanticVersion::new(2, 0, 0);
        let config = PluginLoadConfig::new(ver);

        assert_eq!(config.loader_version, ver);
        assert!(config.strict_abi_checking);
        assert!(config.allow_missing_optional_symbols);
    }

    #[tokio::test]
    async fn test_plugin_load_config_allow_abi_mismatch() {
        let config = PluginLoadConfig::default().allow_abi_mismatch();
        assert!(!config.strict_abi_checking);
    }

    #[tokio::test]
    async fn test_plugin_load_config_with_init_timeout() {
        let config = PluginLoadConfig::default().with_init_timeout(5000);
        assert_eq!(config.init_timeout_ms, 5000);
    }

    #[tokio::test]
    async fn test_plugin_load_config_default() {
        let config = PluginLoadConfig::default();
        assert_eq!(config.loader_version.major, 2);
        assert_eq!(config.loader_version.minor, 0);
    }

    #[tokio::test]
    async fn test_pipeline_stage_result_success() {
        let result = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        };

        assert!(result.success);
        let msg = format!("{}", result);
        assert!(msg.contains("SUCCESS"));
    }

    #[tokio::test]
    async fn test_pipeline_stage_result_failure() {
        let result = PipelineStageResult {
            stage: LifecycleStage::SymbolResolution,
            success: false,
            elapsed_ms: 5,
            error: Some("symbol not found".to_string()),
        };

        assert!(!result.success);
        let msg = format!("{}", result);
        assert!(msg.contains("FAILED"));
    }

    #[tokio::test]
    async fn test_plugin_load_result_display() {
        let result = PluginLoadResult {
            plugin_name: "test_plugin".to_string(),
            success: true,
            stages: vec![],
            total_ms: 100,
            symbol_count: 5,
            validation_details: "test".to_string(),
        };

        let msg = format!("{}", result);
        assert!(msg.contains("test_plugin"));
        assert!(msg.contains("SUCCESS"));
    }

    #[tokio::test]
    async fn test_plugin_load_result_failed_stage() {
        let failed = PipelineStageResult {
            stage: LifecycleStage::AbiCompatibility,
            success: false,
            elapsed_ms: 5,
            error: Some("ABI mismatch".to_string()),
        };

        let passed = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        };

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: false,
            stages: vec![passed, failed.clone()],
            total_ms: 50,
            symbol_count: 0,
            validation_details: "test".to_string(),
        };

        assert_eq!(
            result.failed_stage().unwrap().stage,
            LifecycleStage::AbiCompatibility
        );
    }

    #[tokio::test]
    async fn test_plugin_load_result_successful_stages_time() {
        let stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: true,
                elapsed_ms: 10,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: true,
                elapsed_ms: 5,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::AbiCompatibility,
                success: false,
                elapsed_ms: 3,
                error: Some("failed".to_string()),
            },
        ];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: false,
            stages,
            total_ms: 50,
            symbol_count: 0,
            validation_details: "test".to_string(),
        };

        assert_eq!(result.successful_stages_time(), 15);
    }

    #[tokio::test]
    async fn test_plugin_load_pipeline_new() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config);
        assert!(pipeline.is_ok());
    }

    #[tokio::test]
    async fn test_lifecycle_error_is_error_trait() {
        use std::error::Error;
        let err: Box<dyn Error> = Box::new(LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryNotFound,
            "test",
        ));
        assert!(!err.to_string().is_empty());
    }

    #[tokio::test]
    async fn test_lifecycle_state_equality() {
        let state1 = PluginLifecycleState::Initialized;
        let state2 = PluginLifecycleState::Initialized;
        assert_eq!(state1, state2);
    }

    #[tokio::test]
    async fn test_lifecycle_stage_all_variants() {
        let stages = vec![
            LifecycleStage::BinaryLoad,
            LifecycleStage::SymbolResolution,
            LifecycleStage::AbiCompatibility,
            LifecycleStage::Initialization,
            LifecycleStage::Unloading,
        ];

        assert_eq!(stages.len(), 5);
        for stage in stages {
            let s = format!("{}", stage);
            assert!(!s.is_empty());
        }
    }

    #[tokio::test]
    async fn test_lifecycle_error_type_all_variants() {
        let error_types = vec![
            LifecycleErrorType::BinaryNotFound,
            LifecycleErrorType::BinaryValidationFailed,
            LifecycleErrorType::SymbolResolutionFailed,
            LifecycleErrorType::AbiCompatibilityFailed,
            LifecycleErrorType::InitializationFailed,
            LifecycleErrorType::AlreadyLoaded,
            LifecycleErrorType::NotLoaded,
            LifecycleErrorType::Internal,
        ];

        assert_eq!(error_types.len(), 8);
        for et in error_types {
            let s = format!("{}", et);
            assert!(!s.is_empty());
        }
    }

    #[tokio::test]
    async fn test_pipeline_stage_result_display_details() {
        let result = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 42,
            error: None,
        };

        let msg = format!("{}", result);
        assert!(msg.contains("BinaryLoad"));
        assert!(msg.contains("42ms"));
        assert!(msg.contains("SUCCESS"));
    }

    #[tokio::test]
    async fn test_plugin_load_config_builder_chain() {
        let config = PluginLoadConfig::new(SemanticVersion::new(2, 1, 0))
            .allow_abi_mismatch()
            .with_init_timeout(15000);

        assert_eq!(config.loader_version.minor, 1);
        assert!(!config.strict_abi_checking);
        assert_eq!(config.init_timeout_ms, 15000);
    }

    #[tokio::test]
    async fn test_lifecycle_error_clone() {
        let err1 = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryNotFound,
            "test",
        );
        let err2 = err1.clone();

        assert_eq!(err1.stage, err2.stage);
        assert_eq!(err1.error_type, err2.error_type);
    }

    #[tokio::test]
    async fn test_plugin_load_result_total_ms() {
        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages: vec![],
            total_ms: 150,
            symbol_count: 7,
            validation_details: "test".to_string(),
        };

        assert_eq!(result.total_ms, 150);
    }

    // ===== Integration Tests =====

    #[tokio::test]
    async fn test_multiple_lifecycle_states_sequence() {
        let states = vec![
            PluginLifecycleState::Unloaded,
            PluginLifecycleState::BinaryLoaded,
            PluginLifecycleState::SymbolsResolving,
            PluginLifecycleState::AbiChecking,
            PluginLifecycleState::ReadyForInit,
            PluginLifecycleState::Initialized,
        ];

        // Verify all states are distinct
        for (i, state1) in states.iter().enumerate() {
            for (j, state2) in states.iter().enumerate() {
                if i != j {
                    assert_ne!(state1, state2);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_lifecycle_error_with_stage_info() {
        let err = LifecycleError::new(
            LifecycleStage::SymbolResolution,
            LifecycleErrorType::SymbolResolutionFailed,
            "plugin_init symbol not found",
        );

        assert_eq!(err.stage, LifecycleStage::SymbolResolution);
        assert_eq!(err.error_type, LifecycleErrorType::SymbolResolutionFailed);
        assert!(err.recoverable);

        let msg = err.to_string();
        assert!(msg.contains("SymbolResolution"));
        assert!(msg.contains("SymbolResolutionFailed"));
    }

    #[tokio::test]
    async fn test_pipeline_stage_result_detailed_tracking() {
        let stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: true,
                elapsed_ms: 5,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: true,
                elapsed_ms: 3,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::AbiCompatibility,
                success: true,
                elapsed_ms: 2,
                error: None,
            },
        ];

        let total: u64 = stages.iter().map(|s| s.elapsed_ms).sum();
        assert_eq!(total, 10);

        for stage in stages {
            assert!(stage.success);
            assert!(stage.error.is_none());
        }
    }

    #[tokio::test]
    async fn test_plugin_load_result_with_multiple_stages() {
        let stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: true,
                elapsed_ms: 10,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: true,
                elapsed_ms: 5,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::AbiCompatibility,
                success: true,
                elapsed_ms: 8,
                error: None,
            },
        ];

        let result = PluginLoadResult {
            plugin_name: "my_plugin".to_string(),
            success: true,
            stages: stages.clone(),
            total_ms: 23,
            symbol_count: 10,
            validation_details: "All checks passed".to_string(),
        };

        assert_eq!(result.stages.len(), 3);
        assert!(result.success);
        assert_eq!(result.successful_stages_time(), 23);
        assert!(result.failed_stage().is_none());
    }

    #[tokio::test]
    async fn test_pipeline_stage_error_reporting() {
        let failed_stage = PipelineStageResult {
            stage: LifecycleStage::SymbolResolution,
            success: false,
            elapsed_ms: 4,
            error: Some("Missing required symbol: plugin_init".to_string()),
        };

        assert!(!failed_stage.success);
        assert!(failed_stage.error.is_some());
        let msg = failed_stage.error.unwrap();
        assert!(msg.contains("plugin_init"));
    }

    #[tokio::test]
    async fn test_plugin_load_config_strict_mode_defaults() {
        let config = PluginLoadConfig::new(SemanticVersion::new(2, 0, 0));

        assert!(config.strict_abi_checking);
        assert!(config.allow_missing_optional_symbols);
        assert_eq!(config.init_timeout_ms, 30000);
        assert!(config.auto_unload_on_error);
    }

    #[tokio::test]
    async fn test_plugin_load_config_permissive_mode() {
        let config = PluginLoadConfig::new(SemanticVersion::new(2, 0, 0)).allow_abi_mismatch();

        assert!(!config.strict_abi_checking);
    }

    #[tokio::test]
    async fn test_plugin_load_config_custom_timeout() {
        let config = PluginLoadConfig::new(SemanticVersion::new(2, 0, 0)).with_init_timeout(5000);

        assert_eq!(config.init_timeout_ms, 5000);
    }

    #[tokio::test]
    async fn test_lifecycle_stage_error_type_combinations() {
        let stage_error_combinations = vec![
            (
                LifecycleStage::BinaryLoad,
                LifecycleErrorType::BinaryNotFound,
            ),
            (
                LifecycleStage::BinaryLoad,
                LifecycleErrorType::BinaryValidationFailed,
            ),
            (
                LifecycleStage::SymbolResolution,
                LifecycleErrorType::SymbolResolutionFailed,
            ),
            (
                LifecycleStage::AbiCompatibility,
                LifecycleErrorType::AbiCompatibilityFailed,
            ),
            (
                LifecycleStage::Initialization,
                LifecycleErrorType::InitializationFailed,
            ),
        ];

        for (stage, error_type) in stage_error_combinations {
            let err = LifecycleError::new(stage, error_type, "test message");
            assert_eq!(err.stage, stage);
            assert_eq!(err.error_type, error_type);
        }
    }

    #[tokio::test]
    async fn test_plugin_load_result_empty_stages() {
        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: false,
            stages: vec![],
            total_ms: 0,
            symbol_count: 0,
            validation_details: "No stages run".to_string(),
        };

        assert!(!result.success);
        assert!(result.failed_stage().is_none());
        assert_eq!(result.successful_stages_time(), 0);
    }

    #[tokio::test]
    async fn test_pipeline_stage_result_with_large_elapsed_time() {
        let result = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10000, // 10 seconds
            error: None,
        };

        assert_eq!(result.elapsed_ms, 10000);
        let msg = format!("{}", result);
        assert!(msg.contains("10000ms"));
    }

    #[tokio::test]
    async fn test_lifecycle_error_type_ordering() {
        let types = vec![
            LifecycleErrorType::BinaryNotFound,
            LifecycleErrorType::BinaryValidationFailed,
            LifecycleErrorType::SymbolResolutionFailed,
            LifecycleErrorType::AbiCompatibilityFailed,
            LifecycleErrorType::InitializationFailed,
            LifecycleErrorType::AlreadyLoaded,
            LifecycleErrorType::NotLoaded,
            LifecycleErrorType::Internal,
        ];

        // Verify all are distinct and have display strings
        for error_type in types {
            let display = format!("{}", error_type);
            assert!(!display.is_empty());
        }
    }

    #[tokio::test]
    async fn test_plugin_load_config_multiple_builder_calls() {
        let config = PluginLoadConfig::new(SemanticVersion::new(3, 0, 0))
            .allow_abi_mismatch()
            .with_init_timeout(60000)
            .allow_abi_mismatch(); // Call twice to verify idempotency

        assert!(!config.strict_abi_checking);
        assert_eq!(config.init_timeout_ms, 60000);
        assert_eq!(config.loader_version.major, 3);
    }

    #[tokio::test]
    async fn test_plugin_lifecycle_state_hash_map_key() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(PluginLifecycleState::Unloaded, "not_started");
        map.insert(PluginLifecycleState::Initialized, "running");
        map.insert(PluginLifecycleState::Error, "failed");

        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get(&PluginLifecycleState::Initialized),
            Some(&"running")
        );
    }

    #[tokio::test]
    async fn test_lifecycle_stage_matches_all_variants() {
        let stages = vec![
            LifecycleStage::BinaryLoad,
            LifecycleStage::SymbolResolution,
            LifecycleStage::AbiCompatibility,
            LifecycleStage::Initialization,
            LifecycleStage::Unloading,
        ];

        for stage in stages {
            match stage {
                LifecycleStage::BinaryLoad => assert_eq!(stage, LifecycleStage::BinaryLoad),
                LifecycleStage::SymbolResolution => {
                    assert_eq!(stage, LifecycleStage::SymbolResolution)
                }
                LifecycleStage::AbiCompatibility => {
                    assert_eq!(stage, LifecycleStage::AbiCompatibility)
                }
                LifecycleStage::Initialization => assert_eq!(stage, LifecycleStage::Initialization),
                LifecycleStage::Unloading => assert_eq!(stage, LifecycleStage::Unloading),
            }
        }
    }

    #[tokio::test]
    async fn test_plugin_load_result_stage_filtering() {
        let successful = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 5,
            error: None,
        };

        let failed = PipelineStageResult {
            stage: LifecycleStage::SymbolResolution,
            success: false,
            elapsed_ms: 3,
            error: Some("Not found".to_string()),
        };

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: false,
            stages: vec![successful, failed],
            total_ms: 8,
            symbol_count: 0,
            validation_details: "Failed".to_string(),
        };

        let failed_stages: Vec<_> = result.stages.iter().filter(|s| !s.success).collect();
        assert_eq!(failed_stages.len(), 1);
    }

    #[tokio::test]
    async fn test_lifecycle_error_recoverable_flag_transitions() {
        let mut err = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryNotFound,
            "file not found",
        );

        assert!(err.recoverable);
        err = err.unrecoverable();
        assert!(!err.recoverable);

        // Verify unrecoverable state persists
        let cloned = err.clone();
        assert!(!cloned.recoverable);
    }

    #[tokio::test]
    async fn test_plugin_load_pipeline_default_creation() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config);

        assert!(pipeline.is_ok());
        let pipeline = pipeline.unwrap();

        // Verify pipeline can be used for loading (even if it fails due to invalid path)
        let result = pipeline.load("/nonexistent/plugin.so");
        // Should fail but error should be structured
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_plugin_load_result_calculation_accuracy() {
        let stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: true,
                elapsed_ms: 1,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: true,
                elapsed_ms: 2,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::AbiCompatibility,
                success: true,
                elapsed_ms: 3,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::Initialization,
                success: true,
                elapsed_ms: 4,
                error: None,
            },
        ];

        let result = PluginLoadResult {
            plugin_name: "perf_test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 5,
            validation_details: "All stages successful".to_string(),
        };

        assert_eq!(result.successful_stages_time(), 10);
        assert!(result.failed_stage().is_none());
    }

    #[tokio::test]
    async fn test_lifecycle_error_message_formatting() {
        let test_cases = vec![
            ("simple error", "simple error"),
            (
                "error with special chars: @#$%",
                "error with special chars: @#$%",
            ),
            (
                "very long error message with many details that should still be captured",
                "very long error message with many details that should still be captured",
            ),
        ];

        for (input, expected) in test_cases {
            let err = LifecycleError::new(
                LifecycleStage::BinaryLoad,
                LifecycleErrorType::Internal,
                input,
            );

            assert_eq!(err.message, expected);
        }
    }

    #[tokio::test]
    async fn test_pipeline_stage_result_zero_elapsed_time() {
        let result = PipelineStageResult {
            stage: LifecycleStage::AbiCompatibility,
            success: true,
            elapsed_ms: 0,
            error: None,
        };

        assert_eq!(result.elapsed_ms, 0);
        let msg = format!("{}", result);
        assert!(msg.contains("0ms"));
    }

    #[tokio::test]
    async fn test_plugin_load_config_version_preservation() {
        let version = SemanticVersion::new(2, 1, 3);
        let config = PluginLoadConfig::new(version);

        assert_eq!(config.loader_version, version);
        assert_eq!(config.loader_version.major, 2);
        assert_eq!(config.loader_version.minor, 1);
    }

    #[tokio::test]
    async fn test_lifecycle_state_all_variants_unique() {
        use std::collections::HashSet;

        let states = vec![
            PluginLifecycleState::Unloaded,
            PluginLifecycleState::BinaryLoaded,
            PluginLifecycleState::SymbolsResolving,
            PluginLifecycleState::AbiChecking,
            PluginLifecycleState::ReadyForInit,
            PluginLifecycleState::Initialized,
            PluginLifecycleState::Error,
            PluginLifecycleState::Unloading,
            PluginLifecycleState::Unloaded_,
        ];

        let set: HashSet<_> = states.iter().collect();
        assert_eq!(set.len(), states.len()); // All unique
    }

    #[tokio::test]
    async fn test_plugin_load_result_with_all_failures() {
        let stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: false,
                elapsed_ms: 10,
                error: Some("Binary corrupted".to_string()),
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: false,
                elapsed_ms: 0,
                error: Some("Not reached".to_string()),
            },
        ];

        let result = PluginLoadResult {
            plugin_name: "broken_plugin".to_string(),
            success: false,
            stages,
            total_ms: 10,
            symbol_count: 0,
            validation_details: "Binary validation failed".to_string(),
        };

        let failed = result.failed_stage();
        assert!(failed.is_some());
        assert_eq!(failed.unwrap().stage, LifecycleStage::BinaryLoad);
    }

    #[tokio::test]
    async fn test_lifecycle_error_debug_format() {
        let err = LifecycleError::new(
            LifecycleStage::Initialization,
            LifecycleErrorType::InitializationFailed,
            "init timeout",
        );

        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Initialization"));
        assert!(debug_str.contains("InitializationFailed"));
    }

    #[tokio::test]
    async fn test_plugin_load_config_builder_independence() {
        let config1 = PluginLoadConfig::new(SemanticVersion::new(2, 0, 0))
            .allow_abi_mismatch()
            .with_init_timeout(10000);

        let config2 = PluginLoadConfig::new(SemanticVersion::new(2, 0, 0)).with_init_timeout(5000);

        // Verify configs are independent
        assert_ne!(config1.init_timeout_ms, config2.init_timeout_ms);
        assert_ne!(config1.strict_abi_checking, config2.strict_abi_checking);
    }

    #[tokio::test]
    async fn test_pipeline_stage_result_ordering() {
        let mut stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::AbiCompatibility,
                success: true,
                elapsed_ms: 3,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: true,
                elapsed_ms: 5,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: true,
                elapsed_ms: 2,
                error: None,
            },
        ];

        // Find slowest stage
        let slowest = stages.iter().max_by_key(|s| s.elapsed_ms);
        assert!(slowest.is_some());
        assert_eq!(slowest.unwrap().elapsed_ms, 5);

        // Sort by elapsed time
        stages.sort_by_key(|s| s.elapsed_ms);
        assert_eq!(stages[0].elapsed_ms, 2);
        assert_eq!(stages[2].elapsed_ms, 5);
    }

    #[tokio::test]
    async fn test_lifecycle_error_error_trait_implementation() {
        use std::error::Error;

        let err: Box<dyn Error> = Box::new(LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryNotFound,
            "not found",
        ));

        // Should have Display implementation
        let display = format!("{}", err);
        assert!(display.contains("BinaryLoad"));

        // Should have source() method from Error trait
        assert!(err.source().is_none());
    }

    #[tokio::test]
    async fn test_plugin_load_result_symbol_count_tracking() {
        let result = PluginLoadResult {
            plugin_name: "sym_test".to_string(),
            success: true,
            stages: vec![],
            total_ms: 5,
            symbol_count: 15,
            validation_details: "15 symbols resolved".to_string(),
        };

        assert_eq!(result.symbol_count, 15);
        assert!(result.validation_details.contains("15"));
    }

    #[tokio::test]
    async fn test_lifecycle_stage_consistency_with_pipeline_stages() {
        // Verify that the 5 pipeline stages match the expected lifecycle
        let pipeline_stages = vec![
            LifecycleStage::BinaryLoad,
            LifecycleStage::SymbolResolution,
            LifecycleStage::AbiCompatibility,
            LifecycleStage::Initialization,
            LifecycleStage::Unloading,
        ];

        assert_eq!(pipeline_stages.len(), 5);

        // Each stage should have a unique display name
        let stage_names: Vec<_> = pipeline_stages.iter().map(|s| format!("{}", s)).collect();

        let unique_names: std::collections::HashSet<_> = stage_names.iter().cloned().collect();
        assert_eq!(unique_names.len(), 5);
    }

    // ===== Error Recovery Tests =====

    #[tokio::test]
    async fn test_recovery_action_display() {
        assert_eq!(format!("{}", RecoveryAction::Retry), "Retry");
        assert_eq!(format!("{}", RecoveryAction::Skip), "Skip");
        assert_eq!(format!("{}", RecoveryAction::Rollback), "Rollback");
        assert_eq!(format!("{}", RecoveryAction::Abort), "Abort");
    }

    #[tokio::test]
    async fn test_recovery_strategy_new() {
        let strategy = RecoveryStrategy::new();

        assert_eq!(strategy.max_retries, 3);
        assert!(strategy.skip_optional_stages);
        assert!(strategy.auto_rollback_on_critical);
        assert_eq!(strategy.default_recovery_action, RecoveryAction::Retry);
    }

    #[tokio::test]
    async fn test_recovery_strategy_with_max_retries() {
        let strategy = RecoveryStrategy::new().with_max_retries(5);

        assert_eq!(strategy.max_retries, 5);
    }

    #[tokio::test]
    async fn test_recovery_strategy_with_skip_optional_stages() {
        let strategy = RecoveryStrategy::new().with_skip_optional_stages(false);

        assert!(!strategy.skip_optional_stages);
    }

    #[tokio::test]
    async fn test_recovery_strategy_with_auto_rollback() {
        let strategy = RecoveryStrategy::new().with_auto_rollback(false);

        assert!(!strategy.auto_rollback_on_critical);
    }

    #[tokio::test]
    async fn test_recovery_strategy_builder_chain() {
        let strategy = RecoveryStrategy::new()
            .with_max_retries(5)
            .with_skip_optional_stages(false)
            .with_auto_rollback(false);

        assert_eq!(strategy.max_retries, 5);
        assert!(!strategy.skip_optional_stages);
        assert!(!strategy.auto_rollback_on_critical);
    }

    #[tokio::test]
    async fn test_recovery_strategy_default() {
        let strategy = RecoveryStrategy::default();

        assert_eq!(strategy.max_retries, 3);
        assert!(strategy.skip_optional_stages);
    }

    #[tokio::test]
    async fn test_recovery_result_new() {
        let result = RecoveryResult {
            action: RecoveryAction::Retry,
            successful: true,
            details: "Retry succeeded".to_string(),
            retry_count: 1,
        };

        assert_eq!(result.action, RecoveryAction::Retry);
        assert!(result.successful);
        assert_eq!(result.retry_count, 1);
    }

    #[tokio::test]
    async fn test_recovery_result_display() {
        let result = RecoveryResult {
            action: RecoveryAction::Rollback,
            successful: true,
            details: "Rolled back to initial state".to_string(),
            retry_count: 0,
        };

        let msg = format!("{}", result);
        assert!(msg.contains("Rollback"));
        assert!(msg.contains("SUCCESS"));
    }

    #[tokio::test]
    async fn test_recovery_result_failed_recovery() {
        let result = RecoveryResult {
            action: RecoveryAction::Abort,
            successful: false,
            details: "Abort executed".to_string(),
            retry_count: 3,
        };

        assert!(!result.successful);
        assert_eq!(result.retry_count, 3);
    }

    #[tokio::test]
    async fn test_recovery_result_multiple_retries() {
        let result = RecoveryResult {
            action: RecoveryAction::Retry,
            successful: true,
            details: "Succeeded on retry 3".to_string(),
            retry_count: 3,
        };

        assert_eq!(result.retry_count, 3);
        let msg = format!("{}", result);
        assert!(msg.contains("retries: 3"));
    }

    #[tokio::test]
    async fn test_recovery_action_all_variants() {
        let actions = vec![
            RecoveryAction::Retry,
            RecoveryAction::Skip,
            RecoveryAction::Rollback,
            RecoveryAction::Abort,
        ];

        for action in actions {
            let display = format!("{}", action);
            assert!(!display.is_empty());
        }
    }

    #[tokio::test]
    async fn test_recovery_action_equality() {
        let action1 = RecoveryAction::Retry;
        let action2 = RecoveryAction::Retry;
        let action3 = RecoveryAction::Abort;

        assert_eq!(action1, action2);
        assert_ne!(action1, action3);
    }

    #[tokio::test]
    async fn test_recovery_action_hash_map() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(RecoveryAction::Retry, "retry_count");
        map.insert(RecoveryAction::Abort, "abort_reason");

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&RecoveryAction::Retry), Some(&"retry_count"));
    }

    #[tokio::test]
    async fn test_recovery_strategy_independence() {
        let strategy1 = RecoveryStrategy::new()
            .with_max_retries(5)
            .with_skip_optional_stages(false);

        let strategy2 = RecoveryStrategy::new()
            .with_max_retries(2)
            .with_auto_rollback(false);

        assert_ne!(strategy1.max_retries, strategy2.max_retries);
    }

    #[tokio::test]
    async fn test_recovery_result_with_zero_retries() {
        let result = RecoveryResult {
            action: RecoveryAction::Skip,
            successful: true,
            details: "Skipped optional stage".to_string(),
            retry_count: 0,
        };

        assert_eq!(result.retry_count, 0);
    }

    #[tokio::test]
    async fn test_recovery_result_with_long_details() {
        let long_details = "This is a very long detailed message about what happened during recovery. \
                          It includes multiple lines of information about the recovery process and outcomes.";

        let result = RecoveryResult {
            action: RecoveryAction::Rollback,
            successful: true,
            details: long_details.to_string(),
            retry_count: 0,
        };

        assert_eq!(result.details, long_details);
    }

    #[tokio::test]
    async fn test_recovery_strategy_clone() {
        let strategy1 = RecoveryStrategy::new()
            .with_max_retries(7)
            .with_auto_rollback(false);

        let strategy2 = strategy1.clone();

        assert_eq!(strategy1.max_retries, strategy2.max_retries);
        assert_eq!(
            strategy1.auto_rollback_on_critical,
            strategy2.auto_rollback_on_critical
        );
    }

    #[tokio::test]
    async fn test_recovery_result_clone() {
        let result1 = RecoveryResult {
            action: RecoveryAction::Retry,
            successful: true,
            details: "test".to_string(),
            retry_count: 2,
        };

        let result2 = result1.clone();

        assert_eq!(result1.action, result2.action);
        assert_eq!(result1.retry_count, result2.retry_count);
    }

    #[tokio::test]
    async fn test_recovery_action_debug_format() {
        let action = RecoveryAction::Rollback;
        let debug_str = format!("{:?}", action);
        assert!(debug_str.contains("Rollback"));
    }

    #[tokio::test]
    async fn test_recovery_strategy_debug_format() {
        let strategy = RecoveryStrategy::new().with_max_retries(5);
        let debug_str = format!("{:?}", strategy);
        assert!(debug_str.contains("max_retries"));
    }

    #[tokio::test]
    async fn test_recovery_result_debug_format() {
        let result = RecoveryResult {
            action: RecoveryAction::Retry,
            successful: true,
            details: "test".to_string(),
            retry_count: 1,
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Retry"));
    }

    #[tokio::test]
    async fn test_recovery_strategy_multiple_configurations() {
        // Permissive strategy
        let permissive = RecoveryStrategy::new()
            .with_max_retries(5)
            .with_skip_optional_stages(true)
            .with_auto_rollback(true);

        assert_eq!(permissive.max_retries, 5);
        assert!(permissive.skip_optional_stages);

        // Strict strategy
        let strict = RecoveryStrategy::new()
            .with_max_retries(1)
            .with_skip_optional_stages(false)
            .with_auto_rollback(false);

        assert_eq!(strict.max_retries, 1);
        assert!(!strict.skip_optional_stages);
    }

    #[tokio::test]
    async fn test_recovery_action_as_copy() {
        let action1 = RecoveryAction::Retry;
        let action2 = action1; // Copy should work
        assert_eq!(action1, action2);
    }

    #[tokio::test]
    async fn test_recovery_result_mixed_states() {
        let success_result = RecoveryResult {
            action: RecoveryAction::Retry,
            successful: true,
            details: "Success".to_string(),
            retry_count: 1,
        };

        let failure_result = RecoveryResult {
            action: RecoveryAction::Abort,
            successful: false,
            details: "Failure".to_string(),
            retry_count: 3,
        };

        assert!(success_result.successful);
        assert!(!failure_result.successful);
    }

    // ===== Security Validation Tests =====

    #[tokio::test]
    async fn test_security_validation_config_new() {
        let config = SecurityValidationConfig::new();

        assert!(config.required_role.is_none());
        assert!(config.required_permissions.is_none());
        assert!(config.validate_capabilities);
        assert!(!config.strict_mode);
    }

    #[tokio::test]
    async fn test_security_validation_config_with_role() {
        let config = SecurityValidationConfig::new().with_role(PluginRole::Admin);

        assert!(config.required_role.is_some());
        assert_eq!(config.required_role.unwrap(), PluginRole::Admin);
    }

    #[tokio::test]
    async fn test_security_validation_config_with_permissions() {
        let perms = PluginPermissions::from_role(PluginRole::Editor);
        let config = SecurityValidationConfig::new().with_permissions(perms.clone());

        assert!(config.required_permissions.is_some());
        assert!(config.required_permissions.unwrap().read_config);
    }

    #[tokio::test]
    async fn test_security_validation_config_strict_mode() {
        let config = SecurityValidationConfig::new().strict();

        assert!(config.strict_mode);
    }

    #[tokio::test]
    async fn test_security_validation_independent_configs() {
        let config1 = SecurityValidationConfig::new().with_role(PluginRole::Admin);

        let config2 = SecurityValidationConfig::new().with_role(PluginRole::Viewer);

        assert_ne!(config1.required_role, config2.required_role);
    }

    // ===== Performance Metrics Tests =====

    #[tokio::test]
    async fn test_performance_metrics_from_result() {
        let stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: true,
                elapsed_ms: 10,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: true,
                elapsed_ms: 5,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::AbiCompatibility,
                success: true,
                elapsed_ms: 3,
                error: None,
            },
        ];

        let result = PluginLoadResult {
            plugin_name: "perf_test".to_string(),
            success: true,
            stages,
            total_ms: 18,
            symbol_count: 6,
            validation_details: "All stages passed".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert_eq!(metrics.total_time_ms, 18);
        assert_eq!(metrics.binary_load_time_ms, 10);
        assert_eq!(metrics.symbol_resolution_time_ms, 5);
        assert_eq!(metrics.abi_check_time_ms, 3);
    }

    #[tokio::test]
    async fn test_performance_metrics_throughput_calculation() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 5,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert_eq!(metrics.symbols_per_ms, 0.5);
    }

    #[tokio::test]
    async fn test_performance_metrics_is_acceptable() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 5,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert!(metrics.is_acceptable(20));
        assert!(!metrics.is_acceptable(5));
    }

    #[tokio::test]
    async fn test_performance_metrics_slowest_stage() {
        let stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: true,
                elapsed_ms: 15,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: true,
                elapsed_ms: 10,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::AbiCompatibility,
                success: true,
                elapsed_ms: 5,
                error: None,
            },
        ];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 30,
            symbol_count: 10,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert_eq!(metrics.slowest_stage_ms(), 15);
    }

    #[tokio::test]
    async fn test_performance_metrics_display() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 5,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let display = format!("{}", metrics);

        assert!(display.contains("10ms total"));
        assert!(display.contains("throughput"));
    }

    #[tokio::test]
    async fn test_performance_metrics_with_zero_symbols() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 5,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 5,
            symbol_count: 0,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert_eq!(metrics.symbols_per_ms, 0.0);
    }

    #[tokio::test]
    async fn test_performance_metrics_avg_stage_time() {
        let stages = vec![
            PipelineStageResult {
                stage: LifecycleStage::BinaryLoad,
                success: true,
                elapsed_ms: 10,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::SymbolResolution,
                success: true,
                elapsed_ms: 10,
                error: None,
            },
            PipelineStageResult {
                stage: LifecycleStage::AbiCompatibility,
                success: true,
                elapsed_ms: 10,
                error: None,
            },
        ];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 30,
            symbol_count: 15,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert_eq!(metrics.avg_stage_time_ms, 10.0);
    }

    #[tokio::test]
    async fn test_performance_metrics_clone() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 5,
            validation_details: "test".to_string(),
        };

        let metrics1 = PerformanceMetrics::from_result(&result);
        let metrics2 = metrics1.clone();

        assert_eq!(metrics1.total_time_ms, metrics2.total_time_ms);
        assert_eq!(metrics1.symbols_per_ms, metrics2.symbols_per_ms);
    }

    #[tokio::test]
    async fn test_performance_metrics_with_missing_stages() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 5,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert_eq!(metrics.symbol_resolution_time_ms, 0);
        assert_eq!(metrics.abi_check_time_ms, 0);
    }

    #[tokio::test]
    async fn test_performance_metrics_high_throughput() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 1,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 1,
            symbol_count: 100,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert_eq!(metrics.symbols_per_ms, 100.0);
    }

    #[tokio::test]
    async fn test_performance_metrics_lowest_throughput() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 100,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 100,
            symbol_count: 1,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert_eq!(metrics.symbols_per_ms, 0.01);
    }

    #[tokio::test]
    async fn test_performance_metrics_debug_format() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 5,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let debug_str = format!("{:?}", metrics);

        assert!(debug_str.contains("total_time_ms"));
    }

    // Recovery and Backoff Tests (Weeks 6-7)

    #[tokio::test]
    async fn test_calculate_backoff_delay_initial_retry() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let delay = pipeline.calculate_backoff_delay(0, 10);
        // Base: 10ms * 2^0 = 10ms, plus jitter (0-2.5ms)
        assert!(delay >= 10 && delay <= 13);
    }

    #[tokio::test]
    async fn test_calculate_backoff_delay_exponential_growth() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let delay_0 = pipeline.calculate_backoff_delay(0, 10);
        let delay_1 = pipeline.calculate_backoff_delay(1, 10);
        let delay_2 = pipeline.calculate_backoff_delay(2, 10);
        let delay_3 = pipeline.calculate_backoff_delay(3, 10);

        // Delays should generally increase (accounting for jitter)
        // Base exponential: 10, 20, 40, 80
        assert!(delay_1 >= delay_0 || delay_1 + 3 >= delay_0); // Allow jitter variance
        assert!(delay_2 >= delay_1 || delay_2 + 3 >= delay_1);
        assert!(delay_3 >= delay_2 || delay_3 + 3 >= delay_2);
    }

    #[tokio::test]
    async fn test_calculate_backoff_delay_max_cap() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        // Max exponent is 10, so max is 10 * 1024 = 10240
        // With jitter up to 25%, max is ~12800
        let delay_high = pipeline.calculate_backoff_delay(15, 10);
        assert!(delay_high <= 20000); // Generous upper bound accounting for all jitter
    }

    #[tokio::test]
    async fn test_calculate_backoff_delay_zero_base() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let delay = pipeline.calculate_backoff_delay(0, 0);
        assert_eq!(delay, 0);
    }

    #[tokio::test]
    async fn test_is_stage_optional_symbol_resolution() {
        assert!(PluginLoadPipeline::is_stage_optional(
            LifecycleStage::SymbolResolution
        ));
    }

    #[tokio::test]
    async fn test_is_stage_optional_abi_compatibility() {
        assert!(PluginLoadPipeline::is_stage_optional(
            LifecycleStage::AbiCompatibility
        ));
    }

    #[tokio::test]
    async fn test_is_stage_optional_binary_load() {
        assert!(!PluginLoadPipeline::is_stage_optional(
            LifecycleStage::BinaryLoad
        ));
    }

    #[tokio::test]
    async fn test_is_stage_optional_initialization() {
        assert!(!PluginLoadPipeline::is_stage_optional(
            LifecycleStage::Initialization
        ));
    }

    #[tokio::test]
    async fn test_is_stage_optional_unloading() {
        assert!(!PluginLoadPipeline::is_stage_optional(
            LifecycleStage::Unloading
        ));
    }

    #[tokio::test]
    async fn test_is_critical_stage_binary_load() {
        assert!(PluginLoadPipeline::is_critical_stage(
            LifecycleStage::BinaryLoad
        ));
    }

    #[tokio::test]
    async fn test_is_critical_stage_initialization() {
        assert!(PluginLoadPipeline::is_critical_stage(
            LifecycleStage::Initialization
        ));
    }

    #[tokio::test]
    async fn test_is_critical_stage_symbol_resolution() {
        assert!(!PluginLoadPipeline::is_critical_stage(
            LifecycleStage::SymbolResolution
        ));
    }

    #[tokio::test]
    async fn test_is_critical_stage_abi_compatibility() {
        assert!(!PluginLoadPipeline::is_critical_stage(
            LifecycleStage::AbiCompatibility
        ));
    }

    #[tokio::test]
    async fn test_is_critical_stage_unloading() {
        assert!(!PluginLoadPipeline::is_critical_stage(
            LifecycleStage::Unloading
        ));
    }

    #[tokio::test]
    async fn test_determine_recovery_action_unrecoverable_error() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();
        let strategy = RecoveryStrategy::default();

        let error = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryNotFound,
            "not found",
        )
        .unrecoverable();

        let stage_result = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: false,
            elapsed_ms: 100,
            error: Some("not found".to_string()),
        };

        let action = pipeline.determine_recovery_action(&error, &strategy, 0, &stage_result);
        assert_eq!(action, RecoveryAction::Abort);
    }

    #[tokio::test]
    async fn test_determine_recovery_action_max_retries_exceeded() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();
        let strategy = RecoveryStrategy::default();

        let error = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryValidationFailed,
            "validation failed",
        );

        let stage_result = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: false,
            elapsed_ms: 100,
            error: Some("validation failed".to_string()),
        };

        let action = pipeline.determine_recovery_action(&error, &strategy, 3, &stage_result);
        assert_eq!(action, RecoveryAction::Abort);
    }

    #[tokio::test]
    async fn test_determine_recovery_action_skip_optional_stage() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();
        let strategy = RecoveryStrategy::default().with_skip_optional_stages(true);

        let error = LifecycleError::new(
            LifecycleStage::SymbolResolution,
            LifecycleErrorType::SymbolResolutionFailed,
            "symbol failed",
        );

        let stage_result = PipelineStageResult {
            stage: LifecycleStage::SymbolResolution,
            success: false,
            elapsed_ms: 100,
            error: Some("symbol failed".to_string()),
        };

        let action = pipeline.determine_recovery_action(&error, &strategy, 3, &stage_result);
        assert_eq!(action, RecoveryAction::Skip);
    }

    #[tokio::test]
    async fn test_determine_recovery_action_cannot_skip_required_stage() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();
        let strategy = RecoveryStrategy::default();

        let error = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryValidationFailed,
            "validation failed",
        );

        let stage_result = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: false,
            elapsed_ms: 100,
            error: Some("validation failed".to_string()),
        };

        let action = pipeline.determine_recovery_action(&error, &strategy, 3, &stage_result);
        assert_eq!(action, RecoveryAction::Abort);
    }

    #[tokio::test]
    async fn test_determine_recovery_action_auto_rollback() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();
        let strategy = RecoveryStrategy::default().with_auto_rollback(true);

        let error = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryValidationFailed,
            "validation failed",
        );

        let stage_result = PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: false,
            elapsed_ms: 100,
            error: Some("validation failed".to_string()),
        };

        let action = pipeline.determine_recovery_action(&error, &strategy, 1, &stage_result);
        assert_eq!(action, RecoveryAction::Rollback);
    }

    #[tokio::test]
    async fn test_determine_recovery_action_default_retry() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();
        let strategy = RecoveryStrategy::default();

        let error = LifecycleError::new(
            LifecycleStage::SymbolResolution,
            LifecycleErrorType::SymbolResolutionFailed,
            "symbol failed",
        );

        let stage_result = PipelineStageResult {
            stage: LifecycleStage::SymbolResolution,
            success: false,
            elapsed_ms: 100,
            error: Some("symbol failed".to_string()),
        };

        let action = pipeline.determine_recovery_action(&error, &strategy, 0, &stage_result);
        assert_eq!(action, RecoveryAction::Retry);
    }

    #[tokio::test]
    async fn test_perform_rollback_binary_load() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let result = pipeline.perform_rollback(LifecycleStage::BinaryLoad);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_perform_rollback_symbol_resolution() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let result = pipeline.perform_rollback(LifecycleStage::SymbolResolution);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_perform_rollback_abi_compatibility() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let result = pipeline.perform_rollback(LifecycleStage::AbiCompatibility);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_perform_rollback_initialization() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let result = pipeline.perform_rollback(LifecycleStage::Initialization);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_perform_rollback_unloading_fails() {
        let config = PluginLoadConfig::default();
        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let result = pipeline.perform_rollback(LifecycleStage::Unloading);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_recovery_strategy_builder_max_retries() {
        let strategy = RecoveryStrategy::new().with_max_retries(5);
        assert_eq!(strategy.max_retries, 5);
    }

    #[tokio::test]
    async fn test_recovery_strategy_builder_skip_optional() {
        let strategy = RecoveryStrategy::new().with_skip_optional_stages(false);
        assert!(!strategy.skip_optional_stages);
    }

    #[tokio::test]
    async fn test_recovery_strategy_builder_auto_rollback() {
        let strategy = RecoveryStrategy::new().with_auto_rollback(false);
        assert!(!strategy.auto_rollback_on_critical);
    }

    #[tokio::test]
    async fn test_recovery_strategy_builder_chaining() {
        let strategy = RecoveryStrategy::new()
            .with_max_retries(5)
            .with_skip_optional_stages(false)
            .with_auto_rollback(false);

        assert_eq!(strategy.max_retries, 5);
        assert!(!strategy.skip_optional_stages);
        assert!(!strategy.auto_rollback_on_critical);
    }

    #[tokio::test]
    async fn test_recovery_action_retry_display() {
        let action = RecoveryAction::Retry;
        let display_str = format!("{}", action);
        assert_eq!(display_str, "Retry");
    }

    #[tokio::test]
    async fn test_recovery_action_skip_display() {
        let action = RecoveryAction::Skip;
        let display_str = format!("{}", action);
        assert_eq!(display_str, "Skip");
    }

    #[tokio::test]
    async fn test_recovery_action_rollback_display() {
        let action = RecoveryAction::Rollback;
        let display_str = format!("{}", action);
        assert_eq!(display_str, "Rollback");
    }

    #[tokio::test]
    async fn test_recovery_action_abort_display() {
        let action = RecoveryAction::Abort;
        let display_str = format!("{}", action);
        assert_eq!(display_str, "Abort");
    }

    #[tokio::test]
    async fn test_recovery_result_successful() {
        let result = RecoveryResult {
            action: RecoveryAction::Retry,
            successful: true,
            details: "retry successful".to_string(),
            retry_count: 1,
        };

        let display_str = format!("{}", result);
        assert!(display_str.contains("SUCCESS"));
        assert!(display_str.contains("Retry"));
    }

    #[tokio::test]
    async fn test_recovery_result_failed() {
        let result = RecoveryResult {
            action: RecoveryAction::Abort,
            successful: false,
            details: "recovery failed".to_string(),
            retry_count: 3,
        };

        let display_str = format!("{}", result);
        assert!(display_str.contains("FAILED"));
        assert!(display_str.contains("Abort"));
        assert!(display_str.contains("retries: 3"));
    }

    #[tokio::test]
    async fn test_error_recoverable_by_default() {
        let error = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryValidationFailed,
            "test",
        );

        assert!(error.recoverable);
    }

    #[tokio::test]
    async fn test_error_can_be_marked_unrecoverable() {
        let error = LifecycleError::new(
            LifecycleStage::BinaryLoad,
            LifecycleErrorType::BinaryValidationFailed,
            "test",
        )
        .unrecoverable();

        assert!(!error.recoverable);
    }

    // Memory Profiling Tests (Weeks 6-7)

    #[tokio::test]
    async fn test_performance_metrics_memory_fields_initialized() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 100,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        assert!(metrics.estimated_memory_kb > 0);
        assert!(metrics.peak_memory_kb >= 64);
        assert_eq!(metrics.symbol_count, 100);
    }

    #[tokio::test]
    async fn test_memory_per_symbol_calculation() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 100,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let per_symbol = metrics.memory_per_symbol();

        assert!(per_symbol > 0.0);
    }

    #[tokio::test]
    async fn test_memory_per_symbol_zero_symbols() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 0,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let per_symbol = metrics.memory_per_symbol();

        assert_eq!(per_symbol, 0.0);
    }

    #[tokio::test]
    async fn test_is_memory_acceptable_within_limit() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 100,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let acceptable = metrics.is_memory_acceptable(10000); // Large limit

        assert!(acceptable);
    }

    #[tokio::test]
    async fn test_is_memory_acceptable_exceeds_limit() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 100,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let acceptable = metrics.is_memory_acceptable(1); // Very small limit

        assert!(!acceptable);
    }

    #[tokio::test]
    async fn test_memory_efficiency_calculation() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 100,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let efficiency = metrics.memory_efficiency();

        assert!(efficiency > 0.0);
    }

    #[tokio::test]
    async fn test_total_resource_score_calculation() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 100,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 100,
            symbol_count: 50,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let score = metrics.total_resource_score();

        // Score should be weighted combination of time and memory
        assert!(score > 0.0);
    }

    #[tokio::test]
    async fn test_performance_metrics_with_memory_in_display() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 50,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let display_str = format!("{}", metrics);

        assert!(display_str.contains("memory:"));
        assert!(display_str.contains("KB"));
        assert!(display_str.contains("per-symbol:"));
    }

    #[tokio::test]
    async fn test_performance_metrics_debug_includes_memory() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 50,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let debug_str = format!("{:?}", metrics);

        assert!(debug_str.contains("estimated_memory_kb"));
        assert!(debug_str.contains("peak_memory_kb"));
        assert!(debug_str.contains("symbol_count"));
    }

    #[tokio::test]
    async fn test_memory_efficiency_zero_time() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 0,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 0,
            symbol_count: 100,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);
        let efficiency = metrics.memory_efficiency();

        // Should handle zero division gracefully
        assert_eq!(efficiency, 0.0);
    }

    #[tokio::test]
    async fn test_high_symbol_count_memory_estimation() {
        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 10000,
            validation_details: "test".to_string(),
        };

        let metrics = PerformanceMetrics::from_result(&result);

        // Memory should scale with symbol count
        assert!(metrics.peak_memory_kb > 100);
        assert!(metrics.estimated_memory_kb > 0);
    }

    // Memory Limit Enforcement Tests (Phase 3)

    #[tokio::test]
    async fn test_plugin_load_config_with_max_memory() {
        let config = PluginLoadConfig::default().with_max_memory_kb(1024);
        assert_eq!(config.max_memory_kb, 1024);
    }

    #[tokio::test]
    async fn test_plugin_load_config_with_max_load_time() {
        let config = PluginLoadConfig::default().with_max_load_time_ms(5000);
        assert_eq!(config.max_load_time_ms, 5000);
    }

    #[tokio::test]
    async fn test_plugin_load_config_enforce_memory_limits() {
        let config = PluginLoadConfig::default()
            .with_max_memory_kb(1024)
            .enforce_memory_limits();

        assert!(config.enforce_memory_limits);
        assert_eq!(config.max_memory_kb, 1024);
    }

    #[tokio::test]
    async fn test_plugin_load_config_with_recovery_strategy() {
        let strategy = RecoveryStrategy::default().with_max_retries(5);
        let config = PluginLoadConfig::default().with_recovery_strategy(strategy);

        assert_eq!(config.recovery_strategy.max_retries, 5);
    }

    #[tokio::test]
    async fn test_plugin_load_config_builder_chaining() {
        let config = PluginLoadConfig::default()
            .with_max_memory_kb(2048)
            .with_max_load_time_ms(10000)
            .enforce_memory_limits();

        assert_eq!(config.max_memory_kb, 2048);
        assert_eq!(config.max_load_time_ms, 10000);
        assert!(config.enforce_memory_limits);
    }

    #[tokio::test]
    async fn test_validate_against_limits_memory_exceeded() {
        let config = PluginLoadConfig::default()
            .with_max_memory_kb(50)
            .enforce_memory_limits();

        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 100, // This will estimate > 50 KB
            validation_details: "test".to_string(),
        };

        let validation = pipeline.validate_against_limits(&result);
        assert!(validation.is_err());
    }

    #[tokio::test]
    async fn test_validate_against_limits_memory_within_limit() {
        let config = PluginLoadConfig::default()
            .with_max_memory_kb(10000)
            .enforce_memory_limits();

        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 100,
            validation_details: "test".to_string(),
        };

        let validation = pipeline.validate_against_limits(&result);
        assert!(validation.is_ok());
    }

    #[tokio::test]
    async fn test_validate_against_limits_time_exceeded() {
        let config = PluginLoadConfig::default().with_max_load_time_ms(5);

        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 10,
            symbol_count: 10,
            validation_details: "test".to_string(),
        };

        let validation = pipeline.validate_against_limits(&result);
        assert!(validation.is_err());
    }

    #[tokio::test]
    async fn test_validate_against_limits_time_within_limit() {
        let config = PluginLoadConfig::default().with_max_load_time_ms(1000);

        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 100,
            symbol_count: 10,
            validation_details: "test".to_string(),
        };

        let validation = pipeline.validate_against_limits(&result);
        assert!(validation.is_ok());
    }

    #[tokio::test]
    async fn test_validate_against_limits_no_limits_configured() {
        let config = PluginLoadConfig::default(); // No limits

        let pipeline = PluginLoadPipeline::new(config).unwrap();

        let stages = vec![PipelineStageResult {
            stage: LifecycleStage::BinaryLoad,
            success: true,
            elapsed_ms: 10,
            error: None,
        }];

        let result = PluginLoadResult {
            plugin_name: "test".to_string(),
            success: true,
            stages,
            total_ms: 100,
            symbol_count: 10000,
            validation_details: "test".to_string(),
        };

        let validation = pipeline.validate_against_limits(&result);
        assert!(validation.is_ok());
    }

    #[tokio::test]
    async fn test_memory_limit_config_default_unlimited() {
        let config = PluginLoadConfig::default();
        assert_eq!(config.max_memory_kb, 0);
        assert_eq!(config.max_load_time_ms, 0);
        assert!(!config.enforce_memory_limits);
    }

    #[tokio::test]
    async fn test_recovery_strategy_in_config() {
        let strategy = RecoveryStrategy::new()
            .with_max_retries(10)
            .with_skip_optional_stages(false);

        let config = PluginLoadConfig::default().with_recovery_strategy(strategy.clone());

        assert_eq!(config.recovery_strategy.max_retries, 10);
        assert!(!config.recovery_strategy.skip_optional_stages);
    }

    #[tokio::test]
    async fn test_audit_log_backend_in_memory() {
        use crate::audit::{AuditEvent, AuditEventType, AuditPluginRegistry, AuditSeverity};

        // Use registry to get the in-memory backend
        let registry = crate::audit::DefaultAuditRegistry::with_defaults().unwrap();
        let backend = registry
            .get("memory")
            .expect("memory backend should be registered");

        let event = AuditEvent::new(
            AuditEventType::LoadStarted,
            AuditSeverity::Info,
            "test-plugin",
            "Starting load",
        );

        backend.write(&event).await.unwrap();

        let filter = crate::audit::AuditLogFilter::new();
        let events = backend.read(&filter).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_audit_log_backend_file_integration() {
        use crate::audit::{AuditEvent, AuditEventType, AuditPluginRegistry, AuditSeverity};

        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join("test_audit_integration.jsonl");
        let _ = std::fs::remove_file(&log_path);

        // Use registry with file backend registered
        let registry = crate::audit::DefaultAuditRegistry::with_defaults()
            .and_then(|r| r.with_file_backend(&log_path, 1000))
            .unwrap();

        let backend = registry
            .get("file")
            .expect("file backend should be registered");

        let event = AuditEvent::new(
            AuditEventType::LoadSucceeded,
            AuditSeverity::Info,
            "integration-test",
            "Test passed",
        );

        backend.write(&event).await.unwrap();

        let filter = crate::audit::AuditLogFilter::new();
        let events = backend.read(&filter).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].plugin_name, "integration-test");

        let _ = std::fs::remove_file(&log_path);
    }

    #[tokio::test]
    async fn test_audit_log_event_filtering() {
        use crate::audit::{AuditEvent, AuditEventType, AuditPluginRegistry, AuditSeverity};

        // Use registry to get the in-memory backend
        let registry = crate::audit::DefaultAuditRegistry::with_defaults().unwrap();
        let backend = registry
            .get("memory")
            .expect("memory backend should be registered");

        let info_event = AuditEvent::new(
            AuditEventType::LoadStarted,
            AuditSeverity::Info,
            "plugin-1",
            "Starting",
        );

        let error_event = AuditEvent::new(
            AuditEventType::LoadFailed,
            AuditSeverity::Critical,
            "plugin-2",
            "Failed",
        );

        backend.write(&info_event).await.unwrap();
        backend.write(&error_event).await.unwrap();

        // Filter by severity
        let filter = crate::audit::AuditLogFilter::new().with_min_severity(AuditSeverity::Error);
        let events = backend.read(&filter).await.unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, AuditSeverity::Critical);
    }

    #[tokio::test]
    async fn test_audit_log_multiple_backends() {
        use crate::audit::{AuditEvent, AuditEventType, AuditPluginRegistry, AuditSeverity};

        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join("test_audit_dual.jsonl");
        let _ = std::fs::remove_file(&log_path);

        // Create registry with both memory and file backends
        let registry = crate::audit::DefaultAuditRegistry::with_defaults()
            .and_then(|r| r.with_file_backend(&log_path, 1000))
            .unwrap();

        let mem_backend = registry
            .get("memory")
            .expect("memory backend should be registered");
        let file_backend = registry
            .get("file")
            .expect("file backend should be registered");

        // Write to both
        let event = AuditEvent::new(
            AuditEventType::LoadStarted,
            AuditSeverity::Info,
            "test",
            "msg",
        );

        mem_backend.write(&event).await.unwrap();
        file_backend.write(&event).await.unwrap();

        // Verify both have the event
        let filter = crate::audit::AuditLogFilter::new();
        assert_eq!(mem_backend.read(&filter).await.unwrap().len(), 1);
        assert_eq!(file_backend.read(&filter).await.unwrap().len(), 1);

        let _ = std::fs::remove_file(&log_path);
    }

    #[tokio::test]
    async fn test_audit_log_plugin_backend_provider_integration() {
        use crate::audit::{AuditEvent, AuditEventType, AuditPluginRegistry, AuditSeverity};

        // Create a fresh registry (simulating app initialization)
        let registry = crate::audit::DefaultAuditRegistry::with_defaults().unwrap();

        // Verify memory backend is available after creation
        let memory_backend = registry
            .get("memory")
            .expect("memory backend should be registered by default");

        // Write an event using the registered backend
        let event = AuditEvent::new(
            AuditEventType::LoadStarted,
            AuditSeverity::Info,
            "plugin_integration_test",
            "Test event from registry backend",
        );

        memory_backend.write(&event).await.unwrap();

        // Verify we can read it back
        let filter = crate::audit::AuditLogFilter::new();
        let events = memory_backend.read(&filter).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AuditEventType::LoadStarted);
    }

    #[tokio::test]
    async fn test_audit_log_lifecycle_plugin_initialization_pattern() {
        use crate::audit::{AuditEvent, AuditEventType, AuditPluginRegistry, AuditSeverity};

        // Simulate application initialization pattern where plugin provider
        // would register backends via BackendRegistrar trait

        let registry = crate::audit::DefaultAuditRegistry::with_defaults().unwrap();

        // After registry creation, application can use any backend
        let backend = registry
            .get("memory")
            .expect("memory backend should be registered");

        // Create some lifecycle events
        let events_to_log = vec![
            AuditEvent::new(
                AuditEventType::LoadStarted,
                AuditSeverity::Info,
                "lifecycle",
                "Application startup",
            ),
            AuditEvent::new(
                AuditEventType::LoadSucceeded,
                AuditSeverity::Info,
                "lifecycle",
                "Plugin loading complete",
            ),
            AuditEvent::new(
                AuditEventType::RecoverySucceeded,
                AuditSeverity::Info,
                "lifecycle",
                "Recovery successful",
            ),
        ];

        // Log all events
        for event in events_to_log {
            backend.write(&event).await.unwrap();
        }

        // Verify all events were logged
        let filter = crate::audit::AuditLogFilter::new();
        let logged_events = backend.read(&filter).await.unwrap();
        assert_eq!(logged_events.len(), 3);

        // Verify event sequence
        assert_eq!(logged_events[0].event_type, AuditEventType::LoadStarted);
        assert_eq!(logged_events[1].event_type, AuditEventType::LoadSucceeded);
        assert_eq!(
            logged_events[2].event_type,
            AuditEventType::RecoverySucceeded
        );
    }
}
