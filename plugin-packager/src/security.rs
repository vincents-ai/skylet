// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Plugin vulnerability scanning and security auditing
///
/// This module provides capabilities for:
/// - Scanning plugins for known vulnerabilities
/// - License compliance checking
/// - Supply chain security audit
/// - CVE detection and reporting
/// - Dependency vulnerability analysis
use serde::{Deserialize, Serialize};

/// Vulnerability severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VulnerabilitySeverity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl VulnerabilitySeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            VulnerabilitySeverity::Informational => "informational",
            VulnerabilitySeverity::Low => "low",
            VulnerabilitySeverity::Medium => "medium",
            VulnerabilitySeverity::High => "high",
            VulnerabilitySeverity::Critical => "critical",
        }
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "informational" => Some(VulnerabilitySeverity::Informational),
            "low" => Some(VulnerabilitySeverity::Low),
            "medium" => Some(VulnerabilitySeverity::Medium),
            "high" => Some(VulnerabilitySeverity::High),
            "critical" => Some(VulnerabilitySeverity::Critical),
            _ => None,
        }
    }
}

/// Detected vulnerability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String, // CVE ID or internal identifier
    pub title: String,
    pub description: String,
    pub severity: VulnerabilitySeverity,
    pub affected_version: String,
    pub fixed_version: Option<String>,
    pub advisory_url: Option<String>,
    pub published_date: String,
    pub discovered_at: String,
}

/// License type for compliance checking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LicenseType {
    MIT,
    Apache2,
    GPL2,
    GPL3,
    BSD,
    ISC,
    MPL2,
    Custom(String),
    Unknown,
}

impl LicenseType {
    pub fn from_spdx(spdx_id: &str) -> Self {
        match spdx_id.to_uppercase().as_str() {
            "MIT" => LicenseType::MIT,
            "APACHE-2.0" => LicenseType::Apache2,
            "GPL-2.0" => LicenseType::GPL2,
            "GPL-3.0" => LicenseType::GPL3,
            "BSD-2-CLAUSE" | "BSD-3-CLAUSE" => LicenseType::BSD,
            "ISC" => LicenseType::ISC,
            "MPL-2.0" => LicenseType::MPL2,
            _ => LicenseType::Custom(spdx_id.to_string()),
        }
    }

    pub fn is_permissive(&self) -> bool {
        matches!(
            self,
            LicenseType::MIT | LicenseType::Apache2 | LicenseType::BSD | LicenseType::ISC
        )
    }

    pub fn is_copyleft(&self) -> bool {
        matches!(
            self,
            LicenseType::GPL2 | LicenseType::GPL3 | LicenseType::MPL2
        )
    }
}

/// License compliance issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseCompliance {
    pub dependency: String,
    pub version: String,
    pub license: LicenseType,
    pub is_approved: bool,
    pub issue: Option<String>,
}

/// Scan result for a single plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScanResult {
    pub plugin_id: String,
    pub plugin_version: String,
    pub scan_timestamp: String,
    pub vulnerabilities: Vec<Vulnerability>,
    pub license_issues: Vec<LicenseCompliance>,
    pub dependency_count: usize,
    pub vulnerable_dependency_count: usize,
    pub high_severity_count: usize,
    pub critical_severity_count: usize,
    pub overall_risk: RiskLevel,
}

/// Overall risk assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn description(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "No vulnerabilities detected",
            RiskLevel::Low => "Minor vulnerabilities, low risk",
            RiskLevel::Medium => "Moderate vulnerabilities present",
            RiskLevel::High => "Significant security concerns",
            RiskLevel::Critical => "Critical vulnerabilities - do not use",
        }
    }
}

/// Vulnerability scanner
pub struct VulnerabilityScanner {
    known_vulnerabilities: Vec<Vulnerability>,
    approved_licenses: Vec<LicenseType>,
    max_allowed_severity: VulnerabilitySeverity,
}

impl VulnerabilityScanner {
    /// Create a new vulnerability scanner
    pub fn new() -> Self {
        Self {
            known_vulnerabilities: Vec::new(),
            approved_licenses: vec![
                LicenseType::MIT,
                LicenseType::Apache2,
                LicenseType::BSD,
                LicenseType::ISC,
            ],
            max_allowed_severity: VulnerabilitySeverity::High,
        }
    }

    /// Create scanner with custom severity threshold
    pub fn with_severity_threshold(max_severity: VulnerabilitySeverity) -> Self {
        let mut scanner = Self::new();
        scanner.max_allowed_severity = max_severity;
        scanner
    }

    /// Register a known vulnerability
    pub fn register_vulnerability(&mut self, vuln: Vulnerability) {
        self.known_vulnerabilities.push(vuln);
    }

    /// Add an approved license
    pub fn approve_license(&mut self, license: LicenseType) {
        if !self.approved_licenses.contains(&license) {
            self.approved_licenses.push(license);
        }
    }

    /// Scan a plugin for vulnerabilities
    pub fn scan_plugin(
        &self,
        plugin_id: &str,
        version: &str,
        dependencies: Vec<(&str, &str)>, // (name, version)
    ) -> SecurityScanResult {
        let mut vulnerabilities = Vec::new();
        let mut high_count = 0;
        let mut critical_count = 0;

        // Check for known vulnerabilities in dependencies
        for (_dep_name, dep_version) in &dependencies {
            for vuln in &self.known_vulnerabilities {
                if vuln.affected_version == *dep_version {
                    if vuln.severity >= VulnerabilitySeverity::High {
                        high_count += 1;
                    }
                    if vuln.severity == VulnerabilitySeverity::Critical {
                        critical_count += 1;
                    }
                    vulnerabilities.push(vuln.clone());
                }
            }
        }

        let vulnerable_dep_count = dependencies.len();
        let risk_level = self.assess_risk_level(
            vulnerabilities.len(),
            high_count,
            critical_count,
            vulnerable_dep_count,
        );

        SecurityScanResult {
            plugin_id: plugin_id.to_string(),
            plugin_version: version.to_string(),
            scan_timestamp: chrono::Utc::now().to_rfc3339(),
            vulnerabilities,
            license_issues: Vec::new(),
            dependency_count: dependencies.len(),
            vulnerable_dependency_count: vulnerable_dep_count,
            high_severity_count: high_count,
            critical_severity_count: critical_count,
            overall_risk: risk_level,
        }
    }

    /// Check license compliance for dependencies
    pub fn check_license_compliance(
        &self,
        dependencies: Vec<(&str, &str, &str)>, // (name, version, license_spdx)
    ) -> Vec<LicenseCompliance> {
        dependencies
            .into_iter()
            .map(|(name, version, license_spdx)| {
                let license_type = LicenseType::from_spdx(license_spdx);
                let is_approved = self.approved_licenses.contains(&license_type);

                LicenseCompliance {
                    dependency: name.to_string(),
                    version: version.to_string(),
                    license: license_type,
                    is_approved,
                    issue: if !is_approved {
                        Some(format!("License {} not approved", license_spdx))
                    } else {
                        None
                    },
                }
            })
            .collect()
    }

    /// Assess overall risk level
    fn assess_risk_level(
        &self,
        total_vulns: usize,
        high_count: usize,
        critical_count: usize,
        _dep_count: usize,
    ) -> RiskLevel {
        if critical_count > 0 {
            RiskLevel::Critical
        } else if high_count > 2 {
            RiskLevel::High
        } else if high_count > 0 {
            RiskLevel::Medium
        } else if total_vulns > 5 {
            RiskLevel::Low
        } else {
            RiskLevel::Safe
        }
    }

    /// Check if scan result is acceptable based on configuration
    pub fn is_acceptable(&self, result: &SecurityScanResult) -> bool {
        if result.critical_severity_count > 0 {
            return false;
        }

        result.high_severity_count
            <= (match self.max_allowed_severity {
                VulnerabilitySeverity::Informational => 10,
                VulnerabilitySeverity::Low => 5,
                VulnerabilitySeverity::Medium => 2,
                VulnerabilitySeverity::High => 1,
                VulnerabilitySeverity::Critical => 0,
            })
    }
}

impl Default for VulnerabilityScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Audit report combining security scan and compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditReport {
    pub plugin_id: String,
    pub plugin_version: String,
    pub report_timestamp: String,
    pub scan_result: SecurityScanResult,
    pub license_compliances: Vec<LicenseCompliance>,
    pub recommendations: Vec<String>,
    pub approved: bool,
}

impl SecurityAuditReport {
    /// Generate recommendations based on scan results
    pub fn generate_recommendations(&mut self) {
        self.recommendations.clear();

        // Vulnerability recommendations
        if self.scan_result.critical_severity_count > 0 {
            self.recommendations.push(
                "CRITICAL: Do not publish. Address critical vulnerabilities immediately."
                    .to_string(),
            );
        }

        if self.scan_result.high_severity_count > 2 {
            self.recommendations.push(
                "HIGH RISK: Multiple high-severity vulnerabilities detected. Consider patching."
                    .to_string(),
            );
        }

        // License recommendations
        let unapproved_licenses: Vec<_> = self
            .license_compliances
            .iter()
            .filter(|lc| !lc.is_approved)
            .collect();

        if !unapproved_licenses.is_empty() {
            self.recommendations.push(format!(
                "LICENSE: {} unapproved license(s) found. Review and update.",
                unapproved_licenses.len()
            ));
        }

        // Dependency count recommendations
        if self.scan_result.dependency_count > 50 {
            self.recommendations.push(
                "DEPENDENCY: High number of dependencies increases attack surface. Consider minimizing."
                    .to_string(),
            );
        }

        // Overall approval
        self.approved = self.scan_result.critical_severity_count == 0
            && unapproved_licenses.is_empty()
            && self.scan_result.high_severity_count <= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vulnerability_severity_ordering() {
        assert!(VulnerabilitySeverity::Critical > VulnerabilitySeverity::High);
        assert!(VulnerabilitySeverity::High > VulnerabilitySeverity::Medium);
        assert!(VulnerabilitySeverity::Medium > VulnerabilitySeverity::Low);
    }

    #[test]
    fn test_vulnerability_severity_to_str() {
        assert_eq!(VulnerabilitySeverity::Critical.as_str(), "critical");
        assert_eq!(VulnerabilitySeverity::Low.as_str(), "low");
    }

    #[test]
    fn test_vulnerability_severity_try_parse() {
        assert_eq!(
            VulnerabilitySeverity::try_parse("critical"),
            Some(VulnerabilitySeverity::Critical)
        );
        assert_eq!(VulnerabilitySeverity::try_parse("invalid"), None);
    }

    #[test]
    fn test_license_type_permissive() {
        assert!(LicenseType::MIT.is_permissive());
        assert!(LicenseType::Apache2.is_permissive());
        assert!(!LicenseType::GPL3.is_permissive());
    }

    #[test]
    fn test_license_type_copyleft() {
        assert!(LicenseType::GPL3.is_copyleft());
        assert!(LicenseType::MPL2.is_copyleft());
        assert!(!LicenseType::MIT.is_copyleft());
    }

    #[test]
    fn test_license_from_spdx() {
        assert_eq!(LicenseType::from_spdx("MIT"), LicenseType::MIT);
        assert_eq!(LicenseType::from_spdx("Apache-2.0"), LicenseType::Apache2);
        assert_eq!(LicenseType::from_spdx("GPL-3.0"), LicenseType::GPL3);
    }

    #[test]
    fn test_scanner_creation() {
        let scanner = VulnerabilityScanner::new();
        assert_eq!(scanner.approved_licenses.len(), 4); // MIT, Apache2, BSD, ISC
    }

    #[test]
    fn test_scanner_register_vulnerability() {
        let mut scanner = VulnerabilityScanner::new();
        let vuln = Vulnerability {
            id: "CVE-2024-0001".to_string(),
            title: "Test Vulnerability".to_string(),
            description: "A test vulnerability".to_string(),
            severity: VulnerabilitySeverity::High,
            affected_version: "1.0.0".to_string(),
            fixed_version: Some("1.0.1".to_string()),
            advisory_url: Some("https://example.com".to_string()),
            published_date: "2024-01-01".to_string(),
            discovered_at: chrono::Utc::now().to_rfc3339(),
        };

        scanner.register_vulnerability(vuln);
        assert_eq!(scanner.known_vulnerabilities.len(), 1);
    }

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Critical > RiskLevel::High);
        assert!(RiskLevel::High > RiskLevel::Medium);
        assert!(RiskLevel::Safe < RiskLevel::Low);
    }

    #[test]
    fn test_risk_level_descriptions() {
        assert!(!RiskLevel::Safe.description().is_empty());
        assert!(!RiskLevel::Critical.description().is_empty());
    }

    #[test]
    fn test_scan_plugin_no_vulnerabilities() {
        let scanner = VulnerabilityScanner::new();
        let result = scanner.scan_plugin("test-plugin", "1.0.0", vec![]);

        assert_eq!(result.critical_severity_count, 0);
        assert_eq!(result.high_severity_count, 0);
        assert_eq!(result.overall_risk, RiskLevel::Safe);
    }

    #[test]
    fn test_scan_result_acceptable() {
        let result = SecurityScanResult {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            scan_timestamp: "2024-01-01".to_string(),
            vulnerabilities: Vec::new(),
            license_issues: Vec::new(),
            dependency_count: 5,
            vulnerable_dependency_count: 0,
            high_severity_count: 0,
            critical_severity_count: 0,
            overall_risk: RiskLevel::Safe,
        };

        let scanner = VulnerabilityScanner::new();
        assert!(scanner.is_acceptable(&result));
    }

    #[test]
    fn test_license_compliance_check() {
        let scanner = VulnerabilityScanner::new();
        let deps = vec![("dep1", "1.0.0", "MIT"), ("dep2", "2.0.0", "GPL-3.0")];

        let compliances = scanner.check_license_compliance(deps);
        assert_eq!(compliances.len(), 2);
        assert!(compliances[0].is_approved); // MIT is approved
        assert!(!compliances[1].is_approved); // GPL-3.0 is not approved
    }

    #[test]
    fn test_audit_report_recommendations() {
        let mut report = SecurityAuditReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            report_timestamp: chrono::Utc::now().to_rfc3339(),
            scan_result: SecurityScanResult {
                plugin_id: "test".to_string(),
                plugin_version: "1.0.0".to_string(),
                scan_timestamp: chrono::Utc::now().to_rfc3339(),
                vulnerabilities: Vec::new(),
                license_issues: Vec::new(),
                dependency_count: 5,
                vulnerable_dependency_count: 0,
                high_severity_count: 0,
                critical_severity_count: 0,
                overall_risk: RiskLevel::Safe,
            },
            license_compliances: Vec::new(),
            recommendations: Vec::new(),
            approved: false,
        };

        report.generate_recommendations();
        assert!(report.approved);
    }

    #[test]
    fn test_audit_report_critical_vulnerability() {
        let mut report = SecurityAuditReport {
            plugin_id: "test".to_string(),
            plugin_version: "1.0.0".to_string(),
            report_timestamp: chrono::Utc::now().to_rfc3339(),
            scan_result: SecurityScanResult {
                plugin_id: "test".to_string(),
                plugin_version: "1.0.0".to_string(),
                scan_timestamp: chrono::Utc::now().to_rfc3339(),
                vulnerabilities: Vec::new(),
                license_issues: Vec::new(),
                dependency_count: 5,
                vulnerable_dependency_count: 0,
                high_severity_count: 0,
                critical_severity_count: 1, // Critical vulnerability
                overall_risk: RiskLevel::Critical,
            },
            license_compliances: Vec::new(),
            recommendations: Vec::new(),
            approved: false,
        };

        report.generate_recommendations();
        assert!(!report.approved);
        assert!(!report.recommendations.is_empty());
    }
}
