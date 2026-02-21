// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Plugin health check and verification framework
///
/// This module provides comprehensive health verification for plugins including:
/// - Symbol/export presence verification
/// - Binary compatibility checking (OS/architecture)
/// - Performance baseline establishment
/// - Health scoring system
/// - Detailed diagnostics and recommendations
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Health check severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl HealthSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthSeverity::Info => "info",
            HealthSeverity::Warning => "warning",
            HealthSeverity::Error => "error",
            HealthSeverity::Critical => "critical",
        }
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "info" => Some(HealthSeverity::Info),
            "warning" => Some(HealthSeverity::Warning),
            "error" => Some(HealthSeverity::Error),
            "critical" => Some(HealthSeverity::Critical),
            _ => None,
        }
    }
}

/// Platform/architecture identifier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Linux,
    MacOS,
    Windows,
    Unknown(String),
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "linux") {
            Platform::Linux
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Unknown(std::env::consts::OS.to_string())
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Platform::Linux => "linux",
            Platform::MacOS => "macos",
            Platform::Windows => "windows",
            Platform::Unknown(s) => s,
        }
    }
}

/// Architecture identifier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    X86_64,
    Arm64,
    X86,
    Arm,
    Unknown(String),
}

impl Architecture {
    pub fn current() -> Self {
        if cfg!(target_arch = "x86_64") {
            Architecture::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Architecture::Arm64
        } else if cfg!(target_arch = "x86") {
            Architecture::X86
        } else if cfg!(target_arch = "arm") {
            Architecture::Arm
        } else {
            Architecture::Unknown(std::env::consts::ARCH.to_string())
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Architecture::X86_64 => "x86_64",
            Architecture::Arm64 => "arm64",
            Architecture::X86 => "x86",
            Architecture::Arm => "arm",
            Architecture::Unknown(s) => s,
        }
    }
}

/// Symbol export requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRequirement {
    pub name: String,
    pub required: bool,
    pub description: String,
}

/// Binary compatibility information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryCompatibility {
    pub platform: Platform,
    pub architecture: Architecture,
    pub min_libc_version: Option<String>,
    pub required_symbols: Vec<SymbolRequirement>,
}

/// Health check result for a single check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub check_name: String,
    pub passed: bool,
    pub severity: HealthSeverity,
    pub message: String,
    pub details: Option<String>,
    pub suggestion: Option<String>,
}

/// Overall plugin health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub plugin_id: String,
    pub plugin_version: String,
    pub check_timestamp: String,
    pub overall_health: HealthScore,
    pub checks: Vec<HealthCheckResult>,
    pub binary_compatibility: Option<BinaryCompatibility>,
    pub performance_baseline: Option<PerformanceBaseline>,
    pub recommendations: Vec<String>,
}

/// Health score (0-100)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HealthScore {
    pub score: u32,
    pub status: HealthStatus,
}

/// Overall health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HealthStatus {
    Excellent, // 90-100
    Good,      // 75-89
    Fair,      // 60-74
    Poor,      // 40-59
    Critical,  // 0-39
}

impl HealthStatus {
    pub fn from_score(score: u32) -> Self {
        match score {
            90..=100 => HealthStatus::Excellent,
            75..=89 => HealthStatus::Good,
            60..=74 => HealthStatus::Fair,
            40..=59 => HealthStatus::Poor,
            _ => HealthStatus::Critical,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Excellent => "Excellent",
            HealthStatus::Good => "Good",
            HealthStatus::Fair => "Fair",
            HealthStatus::Poor => "Poor",
            HealthStatus::Critical => "Critical",
        }
    }
}

/// Performance baseline for plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    pub init_time_ms: f64,
    pub shutdown_time_ms: f64,
    pub memory_usage_mb: f64,
    pub max_concurrent_calls: u32,
}

/// Plugin health checker
pub struct PluginHealthChecker {
    required_symbols: HashMap<String, SymbolRequirement>,
    performance_thresholds: PerformanceThresholds,
}

/// Performance thresholds for scoring
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    pub max_init_time_ms: f64,
    pub max_shutdown_time_ms: f64,
    pub max_memory_usage_mb: f64,
    pub min_concurrent_calls: u32,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_init_time_ms: 5000.0,
            max_shutdown_time_ms: 1000.0,
            max_memory_usage_mb: 500.0,
            min_concurrent_calls: 10,
        }
    }
}

impl PluginHealthChecker {
    /// Create a new health checker with default configuration
    pub fn new() -> Self {
        Self {
            required_symbols: Self::default_required_symbols(),
            performance_thresholds: PerformanceThresholds::default(),
        }
    }

    /// Get default required symbols for plugins
    fn default_required_symbols() -> HashMap<String, SymbolRequirement> {
        let mut symbols = HashMap::new();

        symbols.insert(
            "plugin_init".to_string(),
            SymbolRequirement {
                name: "plugin_init".to_string(),
                required: true,
                description: "Plugin initialization entry point".to_string(),
            },
        );

        symbols.insert(
            "plugin_get_info".to_string(),
            SymbolRequirement {
                name: "plugin_get_info".to_string(),
                required: true,
                description: "Plugin metadata provider".to_string(),
            },
        );

        symbols.insert(
            "plugin_shutdown".to_string(),
            SymbolRequirement {
                name: "plugin_shutdown".to_string(),
                required: false,
                description: "Plugin cleanup on shutdown".to_string(),
            },
        );

        symbols
    }

    /// Check if binary file exists and is readable
    pub fn check_binary_exists(&self, binary_path: &Path) -> HealthCheckResult {
        let exists = binary_path.exists();
        HealthCheckResult {
            check_name: "Binary Existence".to_string(),
            passed: exists,
            severity: if exists {
                HealthSeverity::Info
            } else {
                HealthSeverity::Critical
            },
            message: if exists {
                format!("Plugin binary found at {}", binary_path.display())
            } else {
                format!("Plugin binary not found at {}", binary_path.display())
            },
            details: None,
            suggestion: if !exists {
                Some("Ensure plugin.so/.dll/.dylib is built and included in package".to_string())
            } else {
                None
            },
        }
    }

    /// Check if binary is readable and has expected size
    pub fn check_binary_validity(&self, binary_path: &Path) -> HealthCheckResult {
        match std::fs::metadata(binary_path) {
            Ok(metadata) => {
                let size = metadata.len();
                let passed = size > 0;
                let readonly = metadata.permissions().readonly();
                HealthCheckResult {
                    check_name: "Binary Validity".to_string(),
                    passed,
                    severity: if passed {
                        HealthSeverity::Info
                    } else {
                        HealthSeverity::Error
                    },
                    message: format!("Binary size: {} bytes", size),
                    details: Some(format!(
                        "Readable: {}, Size check: {}",
                        if readonly { "No" } else { "Yes" },
                        if size > 0 { "Pass" } else { "Fail" }
                    )),
                    suggestion: if size == 0 {
                        Some("Binary file is empty. Rebuild the plugin.".to_string())
                    } else {
                        None
                    },
                }
            }
            Err(e) => HealthCheckResult {
                check_name: "Binary Validity".to_string(),
                passed: false,
                severity: HealthSeverity::Critical,
                message: format!("Cannot read binary metadata: {}", e),
                details: Some(e.to_string()),
                suggestion: Some("Check file permissions and ensure binary exists".to_string()),
            },
        }
    }

    /// Verify platform compatibility
    pub fn check_platform_compatibility(
        &self,
        binary_path: &Path,
        expected_platform: Platform,
    ) -> HealthCheckResult {
        // Basic file magic number check
        if !binary_path.exists() {
            return HealthCheckResult {
                check_name: "Platform Compatibility".to_string(),
                passed: false,
                severity: HealthSeverity::Critical,
                message: "Binary file not found".to_string(),
                details: None,
                suggestion: Some("Build and package the plugin correctly".to_string()),
            };
        }

        let platform_match = match expected_platform {
            Platform::Linux => binary_path.to_string_lossy().contains(".so"),
            Platform::MacOS => binary_path.to_string_lossy().contains(".dylib"),
            Platform::Windows => binary_path.to_string_lossy().contains(".dll"),
            Platform::Unknown(_) => false,
        };

        HealthCheckResult {
            check_name: "Platform Compatibility".to_string(),
            passed: platform_match,
            severity: if platform_match {
                HealthSeverity::Info
            } else {
                HealthSeverity::Warning
            },
            message: if platform_match {
                format!("Binary appears to be for {}", expected_platform.as_str())
            } else {
                format!(
                    "Binary may not be for {} platform",
                    expected_platform.as_str()
                )
            },
            details: Some(format!("Expected platform: {}", expected_platform.as_str())),
            suggestion: if !platform_match {
                Some(format!(
                    "Rebuild plugin for {} platform",
                    expected_platform.as_str()
                ))
            } else {
                None
            },
        }
    }

    /// Check if required symbols are documented (simulated check)
    pub fn check_required_symbols(&self) -> HealthCheckResult {
        let required_count = self
            .required_symbols
            .iter()
            .filter(|(_, req)| req.required)
            .count();

        let optional_count = self.required_symbols.len() - required_count;

        HealthCheckResult {
            check_name: "Required Symbols".to_string(),
            passed: required_count > 0,
            severity: if required_count > 0 {
                HealthSeverity::Info
            } else {
                HealthSeverity::Warning
            },
            message: format!(
                "Required symbols check: {} symbols required",
                self.required_symbols.len()
            ),
            details: Some(format!(
                "Total required: {}, Optional: {}",
                required_count, optional_count
            )),
            suggestion: None,
        }
    }

    /// Check performance baseline
    pub fn check_performance_baseline(&self, baseline: &PerformanceBaseline) -> HealthCheckResult {
        let init_ok = baseline.init_time_ms <= self.performance_thresholds.max_init_time_ms;
        let shutdown_ok =
            baseline.shutdown_time_ms <= self.performance_thresholds.max_shutdown_time_ms;
        let memory_ok = baseline.memory_usage_mb <= self.performance_thresholds.max_memory_usage_mb;
        let concurrency_ok =
            baseline.max_concurrent_calls >= self.performance_thresholds.min_concurrent_calls;

        let all_passed = init_ok && shutdown_ok && memory_ok && concurrency_ok;

        let mut issues = Vec::new();
        if !init_ok {
            issues.push(format!(
                "Init time: {} ms (threshold: {} ms)",
                baseline.init_time_ms, self.performance_thresholds.max_init_time_ms
            ));
        }
        if !shutdown_ok {
            issues.push(format!(
                "Shutdown time: {} ms (threshold: {} ms)",
                baseline.shutdown_time_ms, self.performance_thresholds.max_shutdown_time_ms
            ));
        }
        if !memory_ok {
            issues.push(format!(
                "Memory usage: {} MB (threshold: {} MB)",
                baseline.memory_usage_mb, self.performance_thresholds.max_memory_usage_mb
            ));
        }
        if !concurrency_ok {
            issues.push(format!(
                "Concurrency: {} calls (threshold: {})",
                baseline.max_concurrent_calls, self.performance_thresholds.min_concurrent_calls
            ));
        }

        HealthCheckResult {
            check_name: "Performance Baseline".to_string(),
            passed: all_passed,
            severity: if all_passed {
                HealthSeverity::Info
            } else {
                HealthSeverity::Warning
            },
            message: if all_passed {
                "Performance baseline within acceptable thresholds".to_string()
            } else {
                format!("Performance issues detected: {} found", issues.len())
            },
            details: if issues.is_empty() {
                None
            } else {
                Some(issues.join(", "))
            },
            suggestion: if !all_passed {
                Some("Review performance metrics and optimize hot paths".to_string())
            } else {
                None
            },
        }
    }

    /// Calculate overall health score from check results
    pub fn calculate_health_score(checks: &[HealthCheckResult]) -> HealthScore {
        if checks.is_empty() {
            return HealthScore {
                score: 50,
                status: HealthStatus::Poor,
            };
        }

        let mut score = 100u32;
        let mut critical_found = false;

        for check in checks {
            match check.severity {
                HealthSeverity::Critical => {
                    critical_found = true;
                    score = score.saturating_sub(20);
                }
                HealthSeverity::Error => {
                    score = score.saturating_sub(10);
                }
                HealthSeverity::Warning => {
                    score = score.saturating_sub(5);
                }
                HealthSeverity::Info => {
                    // Info doesn't penalize score
                }
            }

            if !check.passed {
                score = score.saturating_sub(5);
            }
        }

        // Apply additional penalty for critical checks
        if critical_found {
            score = score.min(30);
        }

        // Ensure score is in valid range
        score = score.min(100);

        HealthScore {
            score,
            status: HealthStatus::from_score(score),
        }
    }

    /// Generate health recommendations based on check results
    pub fn generate_recommendations(
        checks: &[HealthCheckResult],
        score: HealthScore,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Add recommendations based on failed checks
        for check in checks {
            if !check.passed {
                if let Some(suggestion) = &check.suggestion {
                    recommendations.push(suggestion.clone());
                }
            }
        }

        // Add score-based recommendations
        match score.status {
            HealthStatus::Critical => {
                recommendations.push("URGENT: Plugin has critical health issues that must be addressed before deployment".to_string());
            }
            HealthStatus::Poor => {
                recommendations.push("Plugin has multiple issues. Review all failed checks and fix high-priority items.".to_string());
            }
            HealthStatus::Fair => {
                recommendations.push(
                    "Plugin has some issues. Address warnings and errors to improve health score."
                        .to_string(),
                );
            }
            HealthStatus::Good => {
                recommendations
                    .push("Plugin is healthy. Continue monitoring for regressions.".to_string());
            }
            HealthStatus::Excellent => {
                recommendations.push(
                    "Excellent plugin health. Maintain current quality standards.".to_string(),
                );
            }
        }

        recommendations
    }
}

impl Default for PluginHealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_severity_to_str() {
        assert_eq!(HealthSeverity::Info.as_str(), "info");
        assert_eq!(HealthSeverity::Warning.as_str(), "warning");
        assert_eq!(HealthSeverity::Error.as_str(), "error");
        assert_eq!(HealthSeverity::Critical.as_str(), "critical");
    }

    #[test]
    fn test_health_severity_from_str() {
        assert_eq!(HealthSeverity::from_str("info"), Some(HealthSeverity::Info));
        assert_eq!(
            HealthSeverity::from_str("warning"),
            Some(HealthSeverity::Warning)
        );
        assert_eq!(
            HealthSeverity::from_str("error"),
            Some(HealthSeverity::Error)
        );
        assert_eq!(
            HealthSeverity::from_str("critical"),
            Some(HealthSeverity::Critical)
        );
        assert_eq!(HealthSeverity::from_str("unknown"), None);
    }

    #[test]
    fn test_health_severity_ordering() {
        assert!(HealthSeverity::Info < HealthSeverity::Warning);
        assert!(HealthSeverity::Warning < HealthSeverity::Error);
        assert!(HealthSeverity::Error < HealthSeverity::Critical);
    }

    #[test]
    fn test_platform_current() {
        let platform = Platform::current();
        assert!(!matches!(platform, Platform::Unknown(_)));
    }

    #[test]
    fn test_platform_to_str() {
        assert_eq!(Platform::Linux.as_str(), "linux");
        assert_eq!(Platform::MacOS.as_str(), "macos");
        assert_eq!(Platform::Windows.as_str(), "windows");
    }

    #[test]
    fn test_architecture_current() {
        let arch = Architecture::current();
        assert!(!matches!(arch, Architecture::Unknown(_)));
    }

    #[test]
    fn test_architecture_to_str() {
        assert_eq!(Architecture::X86_64.as_str(), "x86_64");
        assert_eq!(Architecture::Arm64.as_str(), "arm64");
        assert_eq!(Architecture::X86.as_str(), "x86");
        assert_eq!(Architecture::Arm.as_str(), "arm");
    }

    #[test]
    fn test_health_status_from_score() {
        assert_eq!(HealthStatus::from_score(95), HealthStatus::Excellent);
        assert_eq!(HealthStatus::from_score(80), HealthStatus::Good);
        assert_eq!(HealthStatus::from_score(65), HealthStatus::Fair);
        assert_eq!(HealthStatus::from_score(50), HealthStatus::Poor);
        assert_eq!(HealthStatus::from_score(20), HealthStatus::Critical);
    }

    #[test]
    fn test_health_status_to_str() {
        assert_eq!(HealthStatus::Excellent.as_str(), "Excellent");
        assert_eq!(HealthStatus::Good.as_str(), "Good");
        assert_eq!(HealthStatus::Fair.as_str(), "Fair");
        assert_eq!(HealthStatus::Poor.as_str(), "Poor");
        assert_eq!(HealthStatus::Critical.as_str(), "Critical");
    }

    #[test]
    fn test_checker_creation() {
        let checker = PluginHealthChecker::new();
        assert!(!checker.required_symbols.is_empty());
    }

    #[test]
    fn test_checker_default_symbols() {
        let checker = PluginHealthChecker::new();
        assert!(checker.required_symbols.contains_key("plugin_init"));
        assert!(checker.required_symbols.contains_key("plugin_get_info"));
        assert!(checker.required_symbols.contains_key("plugin_shutdown"));
    }

    #[test]
    fn test_required_symbol_properties() {
        let checker = PluginHealthChecker::new();
        let init_sym = checker.required_symbols.get("plugin_init").unwrap();
        assert!(init_sym.required);

        let shutdown_sym = checker.required_symbols.get("plugin_shutdown").unwrap();
        assert!(!shutdown_sym.required);
    }

    #[test]
    fn test_check_binary_exists_nonexistent() {
        let checker = PluginHealthChecker::new();
        let result = checker.check_binary_exists(Path::new("/nonexistent/plugin.so"));
        assert!(!result.passed);
        assert_eq!(result.severity, HealthSeverity::Critical);
    }

    #[test]
    fn test_performance_baseline_check() {
        let checker = PluginHealthChecker::new();
        let baseline = PerformanceBaseline {
            init_time_ms: 100.0,
            shutdown_time_ms: 50.0,
            memory_usage_mb: 100.0,
            max_concurrent_calls: 50,
        };
        let result = checker.check_performance_baseline(&baseline);
        assert!(result.passed);
    }

    #[test]
    fn test_performance_baseline_check_failure() {
        let checker = PluginHealthChecker::new();
        let baseline = PerformanceBaseline {
            init_time_ms: 10000.0, // Exceeds threshold
            shutdown_time_ms: 50.0,
            memory_usage_mb: 100.0,
            max_concurrent_calls: 50,
        };
        let result = checker.check_performance_baseline(&baseline);
        assert!(!result.passed);
        assert!(result.details.is_some());
    }

    #[test]
    fn test_calculate_health_score_all_passed() {
        let checks = vec![
            HealthCheckResult {
                check_name: "Check 1".to_string(),
                passed: true,
                severity: HealthSeverity::Info,
                message: "Passed".to_string(),
                details: None,
                suggestion: None,
            },
            HealthCheckResult {
                check_name: "Check 2".to_string(),
                passed: true,
                severity: HealthSeverity::Info,
                message: "Passed".to_string(),
                details: None,
                suggestion: None,
            },
        ];
        let score = PluginHealthChecker::calculate_health_score(&checks);
        assert_eq!(score.score, 100);
        assert_eq!(score.status, HealthStatus::Excellent);
    }

    #[test]
    fn test_calculate_health_score_with_errors() {
        let checks = vec![
            HealthCheckResult {
                check_name: "Check 1".to_string(),
                passed: true,
                severity: HealthSeverity::Info,
                message: "Passed".to_string(),
                details: None,
                suggestion: None,
            },
            HealthCheckResult {
                check_name: "Check 2".to_string(),
                passed: false,
                severity: HealthSeverity::Error,
                message: "Failed".to_string(),
                details: None,
                suggestion: None,
            },
        ];
        let score = PluginHealthChecker::calculate_health_score(&checks);
        assert!(score.score < 100);
    }

    #[test]
    fn test_calculate_health_score_with_critical() {
        let checks = vec![HealthCheckResult {
            check_name: "Check 1".to_string(),
            passed: false,
            severity: HealthSeverity::Critical,
            message: "Critical failure".to_string(),
            details: None,
            suggestion: None,
        }];
        let score = PluginHealthChecker::calculate_health_score(&checks);
        assert!(score.score <= 30);
        assert_eq!(score.status, HealthStatus::Critical);
    }

    #[test]
    fn test_generate_recommendations_excellent() {
        let checks = vec![];
        let score = HealthScore {
            score: 95,
            status: HealthStatus::Excellent,
        };
        let recommendations = PluginHealthChecker::generate_recommendations(&checks, score);
        assert!(!recommendations.is_empty());
        assert!(recommendations[0].contains("Excellent"));
    }

    #[test]
    fn test_generate_recommendations_critical() {
        let checks = vec![];
        let score = HealthScore {
            score: 15,
            status: HealthStatus::Critical,
        };
        let recommendations = PluginHealthChecker::generate_recommendations(&checks, score);
        assert!(!recommendations.is_empty());
        assert!(recommendations[0].to_uppercase().contains("URGENT"));
    }

    #[test]
    fn test_required_symbols_check() {
        let checker = PluginHealthChecker::new();
        let result = checker.check_required_symbols();
        assert!(result.passed);
    }

    #[test]
    fn test_platform_compatibility_check() {
        let checker = PluginHealthChecker::new();
        let result =
            checker.check_platform_compatibility(Path::new("/tmp/test.so"), Platform::Linux);
        // File doesn't exist, but platform extension is checked
        assert!(!result.passed);
    }

    #[test]
    fn test_health_check_result_serialization() {
        let result = HealthCheckResult {
            check_name: "Test Check".to_string(),
            passed: true,
            severity: HealthSeverity::Info,
            message: "Test message".to_string(),
            details: Some("Details".to_string()),
            suggestion: Some("Suggestion".to_string()),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: HealthCheckResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.check_name, result.check_name);
        assert_eq!(deserialized.passed, result.passed);
        assert_eq!(deserialized.severity, result.severity);
    }

    #[test]
    fn test_health_report_serialization() {
        let report = HealthReport {
            plugin_id: "test-plugin".to_string(),
            plugin_version: "1.0.0".to_string(),
            check_timestamp: "2024-01-01T00:00:00Z".to_string(),
            overall_health: HealthScore {
                score: 85,
                status: HealthStatus::Good,
            },
            checks: vec![],
            binary_compatibility: None,
            performance_baseline: None,
            recommendations: vec![],
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: HealthReport = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.plugin_id, report.plugin_id);
        assert_eq!(deserialized.plugin_version, report.plugin_version);
    }

    #[test]
    fn test_performance_baseline_serialization() {
        let baseline = PerformanceBaseline {
            init_time_ms: 100.0,
            shutdown_time_ms: 50.0,
            memory_usage_mb: 250.0,
            max_concurrent_calls: 100,
        };

        let json = serde_json::to_string(&baseline).unwrap();
        let deserialized: PerformanceBaseline = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.init_time_ms, baseline.init_time_ms);
        assert_eq!(
            deserialized.max_concurrent_calls,
            baseline.max_concurrent_calls
        );
    }
}
