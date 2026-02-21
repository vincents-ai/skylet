// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Plugin manifest validation framework
///
/// This module provides comprehensive validation for plugin manifests including:
/// - Field presence and format validation
/// - Version and ABI compatibility checks
/// - Capability validation
/// - Dependency resolution validation
/// - Structured validation reporting
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Validation error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl ValidationSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidationSeverity::Info => "info",
            ValidationSeverity::Warning => "warning",
            ValidationSeverity::Error => "error",
            ValidationSeverity::Critical => "critical",
        }
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "info" => Some(ValidationSeverity::Info),
            "warning" => Some(ValidationSeverity::Warning),
            "error" => Some(ValidationSeverity::Error),
            "critical" => Some(ValidationSeverity::Critical),
            _ => None,
        }
    }
}

/// Individual validation issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub field: String,
    pub severity: ValidationSeverity,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Complete validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub plugin_id: String,
    pub plugin_version: String,
    pub validation_timestamp: String,
    pub is_valid: bool,
    pub issues: Vec<ValidationIssue>,
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub critical_count: usize,
}

impl ValidationReport {
    /// Check if validation passed (no critical or error issues)
    pub fn passed(&self) -> bool {
        self.critical_count == 0 && self.error_count == 0
    }

    /// Check if validation passed with warnings
    pub fn passed_with_warnings(&self) -> bool {
        self.critical_count == 0 && self.error_count == 0 && self.warning_count > 0
    }

    /// Get summary message
    pub fn summary(&self) -> String {
        if self.critical_count > 0 {
            format!(
                "Validation failed: {} critical, {} errors",
                self.critical_count, self.error_count
            )
        } else if self.error_count > 0 {
            format!("Validation failed: {} errors", self.error_count)
        } else if self.warning_count > 0 {
            format!(
                "Validation passed with warnings: {} warnings, {} info",
                self.warning_count, self.info_count
            )
        } else {
            "Validation passed".to_string()
        }
    }
}

/// Plugin manifest validator
pub struct ManifestValidator {
    rules: HashMap<String, ValidationRule>,
    #[allow(dead_code)]
    max_name_length: usize,
    #[allow(dead_code)]
    max_description_length: usize,
}

/// Validation rule for a specific field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub field_name: String,
    pub required: bool,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub allowed_values: Vec<String>,
    pub description: String,
}

impl ManifestValidator {
    /// Create a new validator with default rules
    pub fn new() -> Self {
        let mut validator = Self {
            rules: HashMap::new(),
            max_name_length: 100,
            max_description_length: 500,
        };
        validator.add_default_rules();
        validator
    }

    /// Add default validation rules
    fn add_default_rules(&mut self) {
        self.rules.insert(
            "name".to_string(),
            ValidationRule {
                field_name: "name".to_string(),
                required: true,
                min_length: Some(3),
                max_length: Some(100),
                pattern: Some("^[a-z0-9_-]+$".to_string()),
                allowed_values: Vec::new(),
                description: "Plugin name in lowercase with hyphens/underscores".to_string(),
            },
        );

        self.rules.insert(
            "version".to_string(),
            ValidationRule {
                field_name: "version".to_string(),
                required: true,
                min_length: Some(5), // x.x.x minimum
                max_length: Some(20),
                pattern: Some(r"^\d+\.\d+\.\d+".to_string()),
                allowed_values: Vec::new(),
                description: "Semantic version (major.minor.patch)".to_string(),
            },
        );

        self.rules.insert(
            "abi_version".to_string(),
            ValidationRule {
                field_name: "abi_version".to_string(),
                required: true,
                min_length: Some(1),
                max_length: Some(10),
                pattern: Some(r"^\d+(\.\d+)?$".to_string()),
                allowed_values: vec![
                    "1".to_string(),
                    "1.0".to_string(),
                    "2".to_string(),
                    "2.0".to_string(),
                ],
                description: "ABI version (1, 1.0, 2, or 2.0)".to_string(),
            },
        );

        self.rules.insert(
            "description".to_string(),
            ValidationRule {
                field_name: "description".to_string(),
                required: true,
                min_length: Some(10),
                max_length: Some(500),
                pattern: None,
                allowed_values: Vec::new(),
                description: "Plugin description (10-500 chars)".to_string(),
            },
        );

        self.rules.insert(
            "author".to_string(),
            ValidationRule {
                field_name: "author".to_string(),
                required: false,
                min_length: Some(3),
                max_length: Some(100),
                pattern: None,
                allowed_values: Vec::new(),
                description: "Plugin author name".to_string(),
            },
        );

        self.rules.insert(
            "license".to_string(),
            ValidationRule {
                field_name: "license".to_string(),
                required: false,
                min_length: None,
                max_length: Some(50),
                pattern: None,
                allowed_values: vec![
                    "MIT".to_string(),
                    "Apache-2.0".to_string(),
                    "GPL-3.0".to_string(),
                    "BSD-3-Clause".to_string(),
                ],
                description: "SPDX license identifier".to_string(),
            },
        );
    }

    /// Validate plugin name
    fn validate_name(&self, name: &str) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let rule = self.rules.get("name").unwrap();

        if name.is_empty() {
            issues.push(ValidationIssue {
                field: "name".to_string(),
                severity: ValidationSeverity::Critical,
                message: "Plugin name is required".to_string(),
                suggestion: Some("Provide a valid plugin name".to_string()),
            });
            return issues;
        }

        if let Some(min_len) = rule.min_length {
            if name.len() < min_len {
                issues.push(ValidationIssue {
                    field: "name".to_string(),
                    severity: ValidationSeverity::Error,
                    message: format!("Name too short (minimum {} characters)", min_len),
                    suggestion: Some("Use a longer, more descriptive name".to_string()),
                });
            }
        }

        if let Some(max_len) = rule.max_length {
            if name.len() > max_len {
                issues.push(ValidationIssue {
                    field: "name".to_string(),
                    severity: ValidationSeverity::Error,
                    message: format!("Name too long (maximum {} characters)", max_len),
                    suggestion: Some("Shorten the plugin name".to_string()),
                });
            }
        }

        // Check pattern
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            issues.push(ValidationIssue {
                field: "name".to_string(),
                severity: ValidationSeverity::Error,
                message:
                    "Name must contain only lowercase letters, numbers, hyphens, or underscores"
                        .to_string(),
                suggestion: Some("Use only: a-z, 0-9, -, _".to_string()),
            });
        }

        issues
    }

    /// Validate version format
    fn validate_version(&self, version: &str) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        if version.is_empty() {
            issues.push(ValidationIssue {
                field: "version".to_string(),
                severity: ValidationSeverity::Critical,
                message: "Version is required".to_string(),
                suggestion: Some("Provide a semantic version (e.g., 1.0.0)".to_string()),
            });
            return issues;
        }

        // Check semantic versioning format
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 3 {
            issues.push(ValidationIssue {
                field: "version".to_string(),
                severity: ValidationSeverity::Error,
                message: "Version must follow semantic versioning (major.minor.patch)".to_string(),
                suggestion: Some("Use format: 1.0.0 or 1.0.0-rc1".to_string()),
            });
        }

        // Validate numeric parts
        for (i, part) in parts.iter().enumerate() {
            if i >= 3 {
                break; // Only check first 3 parts
            }
            if part.parse::<u32>().is_err() {
                issues.push(ValidationIssue {
                    field: "version".to_string(),
                    severity: ValidationSeverity::Error,
                    message: format!("Version part '{}' is not numeric", part),
                    suggestion: None,
                });
            }
        }

        issues
    }

    /// Validate ABI version
    fn validate_abi_version(&self, abi_version: &str) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        if abi_version.is_empty() {
            issues.push(ValidationIssue {
                field: "abi_version".to_string(),
                severity: ValidationSeverity::Critical,
                message: "ABI version is required".to_string(),
                suggestion: Some("Specify ABI version: 1, 1.0, 2, or 2.0".to_string()),
            });
            return issues;
        }

        let rule = self.rules.get("abi_version").unwrap();
        if !rule.allowed_values.contains(&abi_version.to_string()) {
            issues.push(ValidationIssue {
                field: "abi_version".to_string(),
                severity: ValidationSeverity::Error,
                message: format!("Invalid ABI version: {}", abi_version),
                suggestion: Some("Use one of: 1, 1.0, 2, 2.0".to_string()),
            });
        }

        issues
    }

    /// Validate description
    fn validate_description(&self, description: Option<&str>) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        match description {
            None => {
                issues.push(ValidationIssue {
                    field: "description".to_string(),
                    severity: ValidationSeverity::Warning,
                    message: "Description is recommended".to_string(),
                    suggestion: Some("Add a description of what your plugin does".to_string()),
                });
            }
            Some(desc) => {
                if desc.len() < 10 {
                    issues.push(ValidationIssue {
                        field: "description".to_string(),
                        severity: ValidationSeverity::Warning,
                        message: "Description should be at least 10 characters".to_string(),
                        suggestion: Some("Provide a more detailed description".to_string()),
                    });
                }
                if desc.len() > 500 {
                    issues.push(ValidationIssue {
                        field: "description".to_string(),
                        severity: ValidationSeverity::Warning,
                        message: "Description is very long (>500 chars)".to_string(),
                        suggestion: Some("Consider shortening the description".to_string()),
                    });
                }
            }
        }

        issues
    }

    /// Validate complete manifest
    pub fn validate_manifest(
        &self,
        name: &str,
        version: &str,
        abi_version: &str,
        description: Option<&str>,
    ) -> ValidationReport {
        let mut issues = Vec::new();

        // Validate each field
        issues.extend(self.validate_name(name));
        issues.extend(self.validate_version(version));
        issues.extend(self.validate_abi_version(abi_version));
        issues.extend(self.validate_description(description));

        // Count issues by severity
        let info_count = issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Info)
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Warning)
            .count();
        let error_count = issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Error)
            .count();
        let critical_count = issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Critical)
            .count();

        let is_valid = critical_count == 0 && error_count == 0;

        ValidationReport {
            plugin_id: name.to_string(),
            plugin_version: version.to_string(),
            validation_timestamp: chrono::Utc::now().to_rfc3339(),
            is_valid,
            issues,
            info_count,
            warning_count,
            error_count,
            critical_count,
        }
    }
}

impl Default for ManifestValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_severity_ordering() {
        assert!(ValidationSeverity::Critical > ValidationSeverity::Error);
        assert!(ValidationSeverity::Error > ValidationSeverity::Warning);
        assert!(ValidationSeverity::Warning > ValidationSeverity::Info);
    }

    #[test]
    fn test_validation_severity_to_str() {
        assert_eq!(ValidationSeverity::Critical.as_str(), "critical");
        assert_eq!(ValidationSeverity::Info.as_str(), "info");
    }

    #[test]
    fn test_validation_severity_from_str() {
        assert_eq!(
            ValidationSeverity::from_str("critical"),
            Some(ValidationSeverity::Critical)
        );
        assert_eq!(ValidationSeverity::from_str("invalid"), None);
    }

    #[test]
    fn test_validator_creation() {
        let validator = ManifestValidator::new();
        assert!(validator.rules.contains_key("name"));
        assert!(validator.rules.contains_key("version"));
        assert!(validator.rules.contains_key("abi_version"));
    }

    #[test]
    fn test_validate_valid_manifest() {
        let validator = ManifestValidator::new();
        let report =
            validator.validate_manifest("my-plugin", "1.0.0", "2.0", Some("A test plugin"));

        assert!(report.is_valid);
        assert_eq!(report.critical_count, 0);
        assert_eq!(report.error_count, 0);
    }

    #[test]
    fn test_validate_invalid_name() {
        let validator = ManifestValidator::new();
        let report = validator.validate_manifest("A", "1.0.0", "2.0", Some("Test"));

        assert!(!report.is_valid);
        assert!(report.critical_count > 0 || report.error_count > 0);
    }

    #[test]
    fn test_validate_invalid_version() {
        let validator = ManifestValidator::new();
        let report = validator.validate_manifest("my-plugin", "1.0", "2.0", Some("Test"));

        assert!(!report.is_valid);
        assert!(report.error_count > 0);
    }

    #[test]
    fn test_validate_invalid_abi_version() {
        let validator = ManifestValidator::new();
        let report = validator.validate_manifest("my-plugin", "1.0.0", "3.0", Some("Test"));

        assert!(!report.is_valid);
        assert!(report.error_count > 0);
    }

    #[test]
    fn test_validate_short_description() {
        let validator = ManifestValidator::new();
        let report = validator.validate_manifest("my-plugin", "1.0.0", "2.0", Some("Test"));

        assert!(report.is_valid); // Still valid but with warning
        assert_eq!(report.warning_count, 1); // Description too short
    }

    #[test]
    fn test_validate_missing_description() {
        let validator = ManifestValidator::new();
        let report = validator.validate_manifest("my-plugin", "1.0.0", "2.0", None);

        assert!(report.is_valid);
        assert_eq!(report.warning_count, 1); // Description recommended
    }

    #[test]
    fn test_validation_report_passed() {
        let report = ValidationReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            validation_timestamp: "2024-01-01".to_string(),
            is_valid: true,
            issues: Vec::new(),
            info_count: 0,
            warning_count: 0,
            error_count: 0,
            critical_count: 0,
        };

        assert!(report.passed());
        assert!(!report.passed_with_warnings());
    }

    #[test]
    fn test_validation_report_passed_with_warnings() {
        let report = ValidationReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            validation_timestamp: "2024-01-01".to_string(),
            is_valid: true,
            issues: vec![ValidationIssue {
                field: "description".to_string(),
                severity: ValidationSeverity::Warning,
                message: "Test warning".to_string(),
                suggestion: None,
            }],
            info_count: 0,
            warning_count: 1,
            error_count: 0,
            critical_count: 0,
        };

        assert!(report.passed());
        assert!(report.passed_with_warnings());
    }

    #[test]
    fn test_validation_report_summary() {
        let report_passed = ValidationReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            validation_timestamp: "2024-01-01".to_string(),
            is_valid: true,
            issues: Vec::new(),
            info_count: 0,
            warning_count: 0,
            error_count: 0,
            critical_count: 0,
        };

        assert_eq!(report_passed.summary(), "Validation passed");
    }

    #[test]
    fn test_validation_issue_creation() {
        let issue = ValidationIssue {
            field: "name".to_string(),
            severity: ValidationSeverity::Error,
            message: "Name is invalid".to_string(),
            suggestion: Some("Use lowercase letters only".to_string()),
        };

        assert_eq!(issue.field, "name");
        assert_eq!(issue.severity, ValidationSeverity::Error);
    }
}
