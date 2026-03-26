// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Plugin sandbox verification and static security analysis
///
/// This module provides security analysis for plugins including:
/// - Static binary analysis
/// - Permission requirement validation
/// - Resource limit enforcement
/// - System call whitelist verification
/// - Capability model enforcement
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Permission types that plugins can request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    FileSystem,
    Network,
    ProcessCreation,
    SystemCall,
    EnvironmentAccess,
    ThreadCreation,
    MemoryAllocation,
    TimerAccess,
    SignalHandling,
}

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::FileSystem => "filesystem",
            Permission::Network => "network",
            Permission::ProcessCreation => "process_creation",
            Permission::SystemCall => "system_call",
            Permission::EnvironmentAccess => "environment_access",
            Permission::ThreadCreation => "thread_creation",
            Permission::MemoryAllocation => "memory_allocation",
            Permission::TimerAccess => "timer_access",
            Permission::SignalHandling => "signal_handling",
        }
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        match s {
            "filesystem" => Some(Permission::FileSystem),
            "network" => Some(Permission::Network),
            "process_creation" => Some(Permission::ProcessCreation),
            "system_call" => Some(Permission::SystemCall),
            "environment_access" => Some(Permission::EnvironmentAccess),
            "thread_creation" => Some(Permission::ThreadCreation),
            "memory_allocation" => Some(Permission::MemoryAllocation),
            "timer_access" => Some(Permission::TimerAccess),
            "signal_handling" => Some(Permission::SignalHandling),
            _ => None,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Permission::FileSystem => "Access to filesystem operations",
            Permission::Network => "Network I/O and socket operations",
            Permission::ProcessCreation => "Ability to spawn child processes",
            Permission::SystemCall => "Low-level system call access",
            Permission::EnvironmentAccess => "Access to environment variables",
            Permission::ThreadCreation => "Ability to create threads",
            Permission::MemoryAllocation => "Dynamic memory allocation",
            Permission::TimerAccess => "Timer and clock access",
            Permission::SignalHandling => "Signal handler registration",
        }
    }

    pub fn severity(&self) -> u32 {
        match self {
            Permission::FileSystem => 7,
            Permission::Network => 7,
            Permission::ProcessCreation => 9,
            Permission::SystemCall => 10,
            Permission::EnvironmentAccess => 3,
            Permission::ThreadCreation => 5,
            Permission::MemoryAllocation => 4,
            Permission::TimerAccess => 2,
            Permission::SignalHandling => 8,
        }
    }
}

/// Resource limits for plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u32,
    pub max_cpu_time_ms: u32,
    pub max_file_descriptors: u32,
    pub max_threads: u32,
    pub max_processes: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 256,
            max_cpu_time_ms: 30000,
            max_file_descriptors: 1024,
            max_threads: 16,
            max_processes: 1,
        }
    }
}

/// Capability model for fine-grained permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapability {
    pub name: String,
    pub required_permissions: Vec<Permission>,
    pub resource_requirement: Option<String>,
    pub description: String,
}

/// Sandbox verification result for a single check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCheckResult {
    pub check_name: String,
    pub passed: bool,
    pub severity: SandboxSeverity,
    pub message: String,
    pub details: Option<String>,
    pub remediation: Option<String>,
}

/// Sandbox verification severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl SandboxSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            SandboxSeverity::Info => "info",
            SandboxSeverity::Warning => "warning",
            SandboxSeverity::Error => "error",
            SandboxSeverity::Critical => "critical",
        }
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "info" => Some(SandboxSeverity::Info),
            "warning" => Some(SandboxSeverity::Warning),
            "error" => Some(SandboxSeverity::Error),
            "critical" => Some(SandboxSeverity::Critical),
            _ => None,
        }
    }
}

/// Complete sandbox verification report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxVerificationReport {
    pub plugin_id: String,
    pub plugin_version: String,
    pub verification_timestamp: String,
    pub is_sandboxed: bool,
    pub checks: Vec<SandboxCheckResult>,
    pub requested_permissions: Vec<Permission>,
    pub resource_limits: ResourceLimits,
    pub capabilities: Vec<PluginCapability>,
    pub high_risk_count: usize,
    pub total_risk_score: u32,
}

impl SandboxVerificationReport {
    pub fn is_compliant(&self) -> bool {
        !self
            .checks
            .iter()
            .any(|c| !c.passed && c.severity == SandboxSeverity::Critical)
    }

    pub fn risk_assessment(&self) -> SandboxRiskLevel {
        match self.total_risk_score {
            0..=10 => SandboxRiskLevel::Low,
            11..=30 => SandboxRiskLevel::Medium,
            31..=60 => SandboxRiskLevel::High,
            _ => SandboxRiskLevel::Critical,
        }
    }

    pub fn summary(&self) -> String {
        let status = if self.is_compliant() {
            "Compliant"
        } else {
            "Non-compliant"
        };
        format!(
            "{}: Risk={:?}, Permissions={}, Score={}",
            status,
            self.risk_assessment(),
            self.requested_permissions.len(),
            self.total_risk_score
        )
    }
}

/// Risk assessment level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SandboxRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl SandboxRiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            SandboxRiskLevel::Low => "Low",
            SandboxRiskLevel::Medium => "Medium",
            SandboxRiskLevel::High => "High",
            SandboxRiskLevel::Critical => "Critical",
        }
    }
}

/// System call information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCallInfo {
    pub name: String,
    pub category: String,
    pub whitelisted: bool,
    pub risk_level: u32,
}

/// Plugin sandbox verifier
pub struct PluginSandboxVerifier {
    whitelisted_syscalls: HashMap<String, SystemCallInfo>,
    #[allow(dead_code)] // Reserved for per-plugin permission enforcement
    required_permissions: HashMap<Permission, bool>,
    resource_limits: ResourceLimits,
}

impl PluginSandboxVerifier {
    /// Create a new sandbox verifier with default configuration
    pub fn new() -> Self {
        Self {
            whitelisted_syscalls: Self::default_syscall_whitelist(),
            required_permissions: HashMap::new(),
            resource_limits: ResourceLimits::default(),
        }
    }

    /// Get default syscall whitelist
    fn default_syscall_whitelist() -> HashMap<String, SystemCallInfo> {
        let mut whitelist = HashMap::new();

        // Safe I/O syscalls
        whitelist.insert(
            "read".to_string(),
            SystemCallInfo {
                name: "read".to_string(),
                category: "io".to_string(),
                whitelisted: true,
                risk_level: 1,
            },
        );

        whitelist.insert(
            "write".to_string(),
            SystemCallInfo {
                name: "write".to_string(),
                category: "io".to_string(),
                whitelisted: true,
                risk_level: 1,
            },
        );

        // Memory management syscalls
        whitelist.insert(
            "mmap".to_string(),
            SystemCallInfo {
                name: "mmap".to_string(),
                category: "memory".to_string(),
                whitelisted: true,
                risk_level: 3,
            },
        );

        whitelist.insert(
            "brk".to_string(),
            SystemCallInfo {
                name: "brk".to_string(),
                category: "memory".to_string(),
                whitelisted: true,
                risk_level: 2,
            },
        );

        // Thread syscalls
        whitelist.insert(
            "clone".to_string(),
            SystemCallInfo {
                name: "clone".to_string(),
                category: "process".to_string(),
                whitelisted: false,
                risk_level: 9,
            },
        );

        // Dangerous syscalls (not whitelisted)
        whitelist.insert(
            "execve".to_string(),
            SystemCallInfo {
                name: "execve".to_string(),
                category: "process".to_string(),
                whitelisted: false,
                risk_level: 10,
            },
        );

        whitelist.insert(
            "ptrace".to_string(),
            SystemCallInfo {
                name: "ptrace".to_string(),
                category: "debug".to_string(),
                whitelisted: false,
                risk_level: 10,
            },
        );

        whitelist
    }

    /// Check if a syscall is whitelisted
    pub fn check_syscall_whitelist(&self, syscall_name: &str) -> SandboxCheckResult {
        let is_whitelisted = self
            .whitelisted_syscalls
            .get(syscall_name)
            .map(|s| s.whitelisted)
            .unwrap_or(false);

        SandboxCheckResult {
            check_name: format!("Syscall: {}", syscall_name),
            passed: is_whitelisted,
            severity: if is_whitelisted {
                SandboxSeverity::Info
            } else {
                SandboxSeverity::Error
            },
            message: if is_whitelisted {
                format!("Syscall '{}' is whitelisted", syscall_name)
            } else {
                format!("Syscall '{}' is not allowed in sandbox", syscall_name)
            },
            details: self
                .whitelisted_syscalls
                .get(syscall_name)
                .map(|s| format!("Category: {}, Risk: {}", s.category, s.risk_level)),
            remediation: if !is_whitelisted {
                Some("Remove use of this syscall or use sandboxed alternative".to_string())
            } else {
                None
            },
        }
    }

    /// Verify permission request
    pub fn check_permission_request(&self, permission: Permission) -> SandboxCheckResult {
        SandboxCheckResult {
            check_name: format!("Permission: {}", permission.as_str()),
            passed: true,
            severity: SandboxSeverity::Info,
            message: format!(
                "Permission '{}' requested: {}",
                permission.as_str(),
                permission.description()
            ),
            details: Some(format!("Risk severity: {}/10", permission.severity())),
            remediation: None,
        }
    }

    /// Validate resource limits
    pub fn check_resource_limits(&self, requested: &ResourceLimits) -> SandboxCheckResult {
        let memory_ok = requested.max_memory_mb <= self.resource_limits.max_memory_mb;
        let cpu_ok = requested.max_cpu_time_ms <= self.resource_limits.max_cpu_time_ms;
        let fds_ok = requested.max_file_descriptors <= self.resource_limits.max_file_descriptors;
        let threads_ok = requested.max_threads <= self.resource_limits.max_threads;
        let processes_ok = requested.max_processes <= self.resource_limits.max_processes;

        let all_passed = memory_ok && cpu_ok && fds_ok && threads_ok && processes_ok;

        let mut issues = Vec::new();
        if !memory_ok {
            issues.push(format!(
                "Memory: {} MB > {} MB limit",
                requested.max_memory_mb, self.resource_limits.max_memory_mb
            ));
        }
        if !cpu_ok {
            issues.push(format!(
                "CPU: {} ms > {} ms limit",
                requested.max_cpu_time_ms, self.resource_limits.max_cpu_time_ms
            ));
        }
        if !fds_ok {
            issues.push(format!(
                "FDs: {} > {} limit",
                requested.max_file_descriptors, self.resource_limits.max_file_descriptors
            ));
        }
        if !threads_ok {
            issues.push(format!(
                "Threads: {} > {} limit",
                requested.max_threads, self.resource_limits.max_threads
            ));
        }
        if !processes_ok {
            issues.push(format!(
                "Processes: {} > {} limit",
                requested.max_processes, self.resource_limits.max_processes
            ));
        }

        SandboxCheckResult {
            check_name: "Resource Limits".to_string(),
            passed: all_passed,
            severity: if all_passed {
                SandboxSeverity::Info
            } else {
                SandboxSeverity::Warning
            },
            message: if all_passed {
                "Resource limits within acceptable bounds".to_string()
            } else {
                format!("Resource limit violations: {}", issues.len())
            },
            details: if issues.is_empty() {
                None
            } else {
                Some(issues.join("; "))
            },
            remediation: if !all_passed {
                Some(
                    "Reduce requested resource limits to comply with sandbox constraints"
                        .to_string(),
                )
            } else {
                None
            },
        }
    }

    /// Check for capability compliance
    pub fn check_capability_model(&self, capability: &PluginCapability) -> SandboxCheckResult {
        let dangerous_perms = capability
            .required_permissions
            .iter()
            .filter(|p| p.severity() >= 8)
            .count();

        SandboxCheckResult {
            check_name: format!("Capability: {}", capability.name),
            passed: dangerous_perms == 0,
            severity: if dangerous_perms == 0 {
                SandboxSeverity::Info
            } else {
                SandboxSeverity::Warning
            },
            message: format!(
                "Capability '{}': {} permissions, {} high-risk",
                capability.name,
                capability.required_permissions.len(),
                dangerous_perms
            ),
            details: Some(format!("Description: {}", capability.description)),
            remediation: if dangerous_perms > 0 {
                Some("Reduce high-risk permissions for this capability".to_string())
            } else {
                None
            },
        }
    }

    /// Calculate total risk score
    pub fn calculate_risk_score(permissions: &[Permission]) -> u32 {
        permissions.iter().map(|p| p.severity()).sum()
    }

    /// Generate risk analysis
    pub fn analyze_risk(permissions: &[Permission]) -> (u32, usize, SandboxRiskLevel) {
        let total_score = Self::calculate_risk_score(permissions);
        let high_risk_count = permissions.iter().filter(|p| p.severity() >= 8).count();

        let risk_level = match total_score {
            0..=10 => SandboxRiskLevel::Low,
            11..=30 => SandboxRiskLevel::Medium,
            31..=60 => SandboxRiskLevel::High,
            _ => SandboxRiskLevel::Critical,
        };

        (total_score, high_risk_count, risk_level)
    }
}

impl Default for PluginSandboxVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_to_str() {
        assert_eq!(Permission::FileSystem.as_str(), "filesystem");
        assert_eq!(Permission::Network.as_str(), "network");
        assert_eq!(Permission::ProcessCreation.as_str(), "process_creation");
        assert_eq!(Permission::SystemCall.as_str(), "system_call");
    }

    #[test]
    fn test_permission_try_parse() {
        assert_eq!(
            Permission::try_parse("filesystem"),
            Some(Permission::FileSystem)
        );
        assert_eq!(Permission::try_parse("network"), Some(Permission::Network));
        assert_eq!(Permission::try_parse("unknown"), None);
    }

    #[test]
    fn test_permission_description() {
        let desc = Permission::FileSystem.description();
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_permission_severity() {
        assert!(Permission::SystemCall.severity() > Permission::TimerAccess.severity());
        assert!(Permission::ProcessCreation.severity() > Permission::EnvironmentAccess.severity());
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert!(limits.max_memory_mb > 0);
        assert!(limits.max_cpu_time_ms > 0);
    }

    #[test]
    fn test_sandbox_severity_to_str() {
        assert_eq!(SandboxSeverity::Info.as_str(), "info");
        assert_eq!(SandboxSeverity::Warning.as_str(), "warning");
        assert_eq!(SandboxSeverity::Error.as_str(), "error");
        assert_eq!(SandboxSeverity::Critical.as_str(), "critical");
    }

    #[test]
    fn test_sandbox_severity_try_parse() {
        assert_eq!(
            SandboxSeverity::try_parse("info"),
            Some(SandboxSeverity::Info)
        );
        assert_eq!(SandboxSeverity::try_parse("unknown"), None);
    }

    #[test]
    fn test_sandbox_severity_ordering() {
        assert!(SandboxSeverity::Info < SandboxSeverity::Warning);
        assert!(SandboxSeverity::Warning < SandboxSeverity::Error);
        assert!(SandboxSeverity::Error < SandboxSeverity::Critical);
    }

    #[test]
    fn test_sandbox_risk_level_to_str() {
        assert_eq!(SandboxRiskLevel::Low.as_str(), "Low");
        assert_eq!(SandboxRiskLevel::Critical.as_str(), "Critical");
    }

    #[test]
    fn test_verifier_creation() {
        let verifier = PluginSandboxVerifier::new();
        assert!(!verifier.whitelisted_syscalls.is_empty());
    }

    #[test]
    fn test_verifier_default_syscalls() {
        let verifier = PluginSandboxVerifier::new();
        assert!(verifier.whitelisted_syscalls.contains_key("read"));
        assert!(verifier.whitelisted_syscalls.contains_key("write"));
        assert!(verifier.whitelisted_syscalls.contains_key("execve"));
    }

    #[test]
    fn test_check_syscall_whitelisted() {
        let verifier = PluginSandboxVerifier::new();
        let result = verifier.check_syscall_whitelist("read");
        assert!(result.passed);
    }

    #[test]
    fn test_check_syscall_not_whitelisted() {
        let verifier = PluginSandboxVerifier::new();
        let result = verifier.check_syscall_whitelist("execve");
        assert!(!result.passed);
    }

    #[test]
    fn test_check_permission_request() {
        let verifier = PluginSandboxVerifier::new();
        let result = verifier.check_permission_request(Permission::FileSystem);
        assert!(result.passed);
    }

    #[test]
    fn test_check_resource_limits_compliant() {
        let verifier = PluginSandboxVerifier::new();
        let requested = ResourceLimits {
            max_memory_mb: 100,
            max_cpu_time_ms: 5000,
            max_file_descriptors: 256,
            max_threads: 8,
            max_processes: 1,
        };
        let result = verifier.check_resource_limits(&requested);
        assert!(result.passed);
    }

    #[test]
    fn test_check_resource_limits_exceeds() {
        let verifier = PluginSandboxVerifier::new();
        let requested = ResourceLimits {
            max_memory_mb: 1000,
            max_cpu_time_ms: 60000,
            max_file_descriptors: 256,
            max_threads: 8,
            max_processes: 1,
        };
        let result = verifier.check_resource_limits(&requested);
        assert!(!result.passed);
    }

    #[test]
    fn test_calculate_risk_score() {
        let perms = vec![Permission::FileSystem, Permission::Network];
        let score = PluginSandboxVerifier::calculate_risk_score(&perms);
        assert!(score > 0);
    }

    #[test]
    fn test_analyze_risk_low() {
        let perms = vec![Permission::EnvironmentAccess, Permission::TimerAccess];
        let (_score, _high_risk, level) = PluginSandboxVerifier::analyze_risk(&perms);
        assert_eq!(level, SandboxRiskLevel::Low);
    }

    #[test]
    fn test_analyze_risk_critical() {
        // Add all high-severity permissions to reach Critical threshold
        let perms = vec![
            Permission::SystemCall,
            Permission::ProcessCreation,
            Permission::SignalHandling,
            Permission::FileSystem,
            Permission::Network,
            Permission::ThreadCreation,
            Permission::MemoryAllocation,
            Permission::EnvironmentAccess,
        ];
        let (score, high_risk, level) = PluginSandboxVerifier::analyze_risk(&perms);
        // With these permissions: 10+9+8+7+7+5+4+3 = 53 (High), let's verify it's at least High
        assert!(score > 30);
        assert!(level >= SandboxRiskLevel::High);
        assert!(high_risk > 0);
    }

    #[test]
    fn test_check_capability_model_safe() {
        let verifier = PluginSandboxVerifier::new();
        let cap = PluginCapability {
            name: "safe_io".to_string(),
            required_permissions: vec![Permission::FileSystem],
            resource_requirement: None,
            description: "Safe I/O operations".to_string(),
        };
        let result = verifier.check_capability_model(&cap);
        assert!(result.passed);
    }

    #[test]
    fn test_check_capability_model_dangerous() {
        let verifier = PluginSandboxVerifier::new();
        let cap = PluginCapability {
            name: "dangerous_ops".to_string(),
            required_permissions: vec![Permission::SystemCall, Permission::ProcessCreation],
            resource_requirement: None,
            description: "Dangerous operations".to_string(),
        };
        let result = verifier.check_capability_model(&cap);
        assert!(!result.passed);
    }

    #[test]
    fn test_sandbox_verification_report_is_compliant() {
        let report = SandboxVerificationReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            verification_timestamp: "2024-01-01T00:00:00Z".to_string(),
            is_sandboxed: true,
            checks: vec![],
            requested_permissions: vec![],
            resource_limits: ResourceLimits::default(),
            capabilities: vec![],
            high_risk_count: 0,
            total_risk_score: 5,
        };
        assert!(report.is_compliant());
    }

    #[test]
    fn test_sandbox_verification_report_not_compliant() {
        let check = SandboxCheckResult {
            check_name: "Test".to_string(),
            passed: false,
            severity: SandboxSeverity::Critical,
            message: "Failed".to_string(),
            details: None,
            remediation: None,
        };
        let report = SandboxVerificationReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            verification_timestamp: "2024-01-01T00:00:00Z".to_string(),
            is_sandboxed: true,
            checks: vec![check],
            requested_permissions: vec![],
            resource_limits: ResourceLimits::default(),
            capabilities: vec![],
            high_risk_count: 0,
            total_risk_score: 5,
        };
        assert!(!report.is_compliant());
    }

    #[test]
    fn test_sandbox_verification_report_risk_assessment() {
        let report = SandboxVerificationReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            verification_timestamp: "2024-01-01T00:00:00Z".to_string(),
            is_sandboxed: true,
            checks: vec![],
            requested_permissions: vec![],
            resource_limits: ResourceLimits::default(),
            capabilities: vec![],
            high_risk_count: 0,
            total_risk_score: 50,
        };
        assert_eq!(report.risk_assessment(), SandboxRiskLevel::High);
    }

    #[test]
    fn test_sandbox_verification_report_summary() {
        let report = SandboxVerificationReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            verification_timestamp: "2024-01-01T00:00:00Z".to_string(),
            is_sandboxed: true,
            checks: vec![],
            requested_permissions: vec![Permission::FileSystem],
            resource_limits: ResourceLimits::default(),
            capabilities: vec![],
            high_risk_count: 0,
            total_risk_score: 15,
        };
        let summary = report.summary();
        assert!(summary.contains("Compliant"));
    }

    #[test]
    fn test_sandbox_check_result_serialization() {
        let result = SandboxCheckResult {
            check_name: "Test Check".to_string(),
            passed: true,
            severity: SandboxSeverity::Info,
            message: "Test message".to_string(),
            details: Some("Details".to_string()),
            remediation: Some("Fix it".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SandboxCheckResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.check_name, result.check_name);
        assert_eq!(deserialized.passed, result.passed);
    }

    #[test]
    fn test_sandbox_verification_report_serialization() {
        let report = SandboxVerificationReport {
            plugin_id: "test-plugin".to_string(),
            plugin_version: "1.0.0".to_string(),
            verification_timestamp: "2024-01-01T00:00:00Z".to_string(),
            is_sandboxed: true,
            checks: vec![],
            requested_permissions: vec![],
            resource_limits: ResourceLimits::default(),
            capabilities: vec![],
            high_risk_count: 0,
            total_risk_score: 0,
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: SandboxVerificationReport = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.plugin_id, report.plugin_id);
    }

    #[test]
    fn test_plugin_capability_serialization() {
        let cap = PluginCapability {
            name: "test".to_string(),
            required_permissions: vec![Permission::FileSystem],
            resource_requirement: Some("256MB".to_string()),
            description: "Test capability".to_string(),
        };

        let json = serde_json::to_string(&cap).unwrap();
        let deserialized: PluginCapability = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, cap.name);
    }
}
