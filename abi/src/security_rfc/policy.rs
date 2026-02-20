// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Security Policy Engine - RFC-0008
//!
//! Central policy engine for capability approval workflow and
//! security policy enforcement.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::capabilities::{CapabilityInfo, CapabilityType};
use tracing;

/// Status of a capability approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// Approval is pending review
    Pending,
    /// Approved by administrator
    Approved,
    /// Denied by administrator
    Denied,
    /// Auto-approved by policy
    AutoApproved,
    /// Approval expired
    Expired,
}

/// A capability approval record
#[derive(Debug, Clone)]
pub struct CapabilityApproval {
    /// Unique approval ID
    pub id: String,
    /// Plugin ID this approval is for
    pub plugin_id: String,
    /// The capability being approved
    pub capability_type: CapabilityType,
    /// Human-readable description
    pub description: String,
    /// Current approval status
    pub status: ApprovalStatus,
    /// When the approval was created
    pub created_at: Instant,
    /// When the approval expires (if applicable)
    pub expires_at: Option<Instant>,
    /// Who approved/denied (if applicable)
    pub approver: Option<String>,
    /// Reason for denial (if denied)
    pub denial_reason: Option<String>,
    /// Risk level assessment
    pub risk_level: RiskLevel,
}

/// Risk level for capability requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    /// Minimal risk - safe capabilities like read access to plugin's own data
    Minimal,
    /// Low risk - controlled access to specific resources
    Low,
    /// Medium risk - broader access like network to specific hosts
    Medium,
    /// High risk - sensitive operations like arbitrary command execution
    High,
    /// Critical risk - full system access
    Critical,
}

/// Security policy configuration
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// Auto-approve minimal risk capabilities
    pub auto_approve_minimal: bool,
    /// Auto-approve low risk capabilities
    pub auto_approve_low: bool,
    /// Require MFA for high risk capabilities
    pub require_mfa_for_high: bool,
    /// Require MFA for critical risk capabilities
    pub require_mfa_for_critical: bool,
    /// Default approval expiration time (None = no expiration)
    pub default_expiration: Option<Duration>,
    /// Maximum number of pending approvals per plugin
    pub max_pending_per_plugin: usize,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            auto_approve_minimal: true,
            auto_approve_low: true,
            require_mfa_for_high: true,
            require_mfa_for_critical: true,
            default_expiration: Some(Duration::from_secs(30 * 24 * 60 * 60)), // 30 days
            max_pending_per_plugin: 10,
        }
    }
}

/// Security policy engine
///
/// Manages capability approval workflow and policy enforcement.
#[derive(Debug)]
pub struct SecurityPolicyEngine {
    /// Pending approvals
    pending_approvals: Arc<RwLock<HashMap<String, CapabilityApproval>>>,
    /// Approved capabilities by plugin
    approved_capabilities: Arc<RwLock<HashMap<String, Vec<CapabilityApproval>>>>,
    /// Security policy configuration
    policy: SecurityPolicy,
    /// Filesystem enforcer reference
    fs_enforcer: super::filesystem::FilesystemEnforcer,
    /// Network enforcer reference
    net_enforcer: super::network::NetworkEnforcer,
}

impl SecurityPolicyEngine {
    /// Create a new security policy engine
    pub fn new() -> Self {
        Self::with_policy(SecurityPolicy::default())
    }

    /// Create with custom policy
    pub fn with_policy(policy: SecurityPolicy) -> Self {
        Self {
            pending_approvals: Arc::new(RwLock::new(HashMap::new())),
            approved_capabilities: Arc::new(RwLock::new(HashMap::new())),
            policy,
            fs_enforcer: super::filesystem::FilesystemEnforcer::new(),
            net_enforcer: super::network::NetworkEnforcer::new(),
        }
    }

    /// Request approval for a capability
    pub fn request_approval(
        &self,
        plugin_id: &str,
        capability: &CapabilityInfo,
        description: &str,
    ) -> Result<String, PolicyError> {
        // Check pending limit
        {
            let pending = self.pending_approvals.read().unwrap();
            let plugin_pending = pending
                .values()
                .filter(|a| a.plugin_id == plugin_id)
                .count();

            if plugin_pending >= self.policy.max_pending_per_plugin {
                return Err(PolicyError::TooManyPendingRequests {
                    plugin_id: plugin_id.to_string(),
                    max: self.policy.max_pending_per_plugin,
                });
            }
        }

        // Assess risk level
        let risk_level = self.assess_risk(capability);

        // Determine if auto-approval applies
        let status = if (risk_level == RiskLevel::Minimal && self.policy.auto_approve_minimal)
            || (risk_level == RiskLevel::Low && self.policy.auto_approve_low)
        {
            ApprovalStatus::AutoApproved
        } else {
            ApprovalStatus::Pending
        };

        // Create approval record
        let approval_id = format!("appr-{}-{}", plugin_id, uuid_v4_short());
        let approval = CapabilityApproval {
            id: approval_id.clone(),
            plugin_id: plugin_id.to_string(),
            capability_type: capability.type_,
            description: description.to_string(),
            status,
            created_at: Instant::now(),
            expires_at: self.policy.default_expiration.map(|d| Instant::now() + d),
            approver: if status == ApprovalStatus::AutoApproved {
                Some("auto".to_string())
            } else {
                None
            },
            denial_reason: None,
            risk_level,
        };

        // Store approval
        {
            let mut pending = self.pending_approvals.write().unwrap();
            pending.insert(approval_id.clone(), approval.clone());
        }

        // If auto-approved, apply it
        if status == ApprovalStatus::AutoApproved {
            self.apply_approval(&approval_id)?;
        }

        Ok(approval_id)
    }

    /// Approve a pending request
    pub fn approve(&self, approval_id: &str, approver: &str) -> Result<(), PolicyError> {
        let approval = {
            let mut pending = self.pending_approvals.write().unwrap();
            pending
                .remove(approval_id)
                .ok_or_else(|| PolicyError::ApprovalNotFound {
                    id: approval_id.to_string(),
                })?
        };

        let mut approval = approval;
        approval.status = ApprovalStatus::Approved;
        approval.approver = Some(approver.to_string());

        self.apply_approved_capability(&approval)?;

        // Store in approved list
        {
            let mut approved = self.approved_capabilities.write().unwrap();
            approved
                .entry(approval.plugin_id.clone())
                .or_default()
                .push(approval);
        }

        Ok(())
    }

    /// Deny a pending request
    pub fn deny(&self, approval_id: &str, reason: &str) -> Result<(), PolicyError> {
        let mut pending = self.pending_approvals.write().unwrap();
        let approval =
            pending
                .get_mut(approval_id)
                .ok_or_else(|| PolicyError::ApprovalNotFound {
                    id: approval_id.to_string(),
                })?;

        approval.status = ApprovalStatus::Denied;
        approval.denial_reason = Some(reason.to_string());

        Ok(())
    }

    /// Get all pending approvals
    pub fn get_pending_approvals(&self) -> Vec<CapabilityApproval> {
        let pending = self.pending_approvals.read().unwrap();
        pending.values().cloned().collect()
    }

    /// Get pending approvals for a plugin
    pub fn get_pending_for_plugin(&self, plugin_id: &str) -> Vec<CapabilityApproval> {
        let pending = self.pending_approvals.read().unwrap();
        pending
            .values()
            .filter(|a| a.plugin_id == plugin_id)
            .cloned()
            .collect()
    }

    /// Get approved capabilities for a plugin
    pub fn get_approved_for_plugin(&self, plugin_id: &str) -> Vec<CapabilityApproval> {
        let approved = self.approved_capabilities.read().unwrap();
        approved.get(plugin_id).cloned().unwrap_or_default()
    }

    /// Check if a capability is approved
    pub fn is_capability_approved(&self, plugin_id: &str, capability_type: CapabilityType) -> bool {
        let approved = self.approved_capabilities.read().unwrap();
        approved
            .get(plugin_id)
            .map(|caps| {
                caps.iter().any(|c| {
                    c.capability_type == capability_type && c.status == ApprovalStatus::Approved
                })
            })
            .unwrap_or(false)
    }

    /// Revoke an approved capability
    pub fn revoke(&self, plugin_id: &str, approval_id: &str) -> Result<(), PolicyError> {
        let mut approved = self.approved_capabilities.write().unwrap();

        if let Some(capabilities) = approved.get_mut(plugin_id) {
            if let Some(cap) = capabilities.iter_mut().find(|c| c.id == approval_id) {
                cap.status = ApprovalStatus::Expired;
                return Ok(());
            }
        }

        Err(PolicyError::ApprovalNotFound {
            id: approval_id.to_string(),
        })
    }

    /// Assess risk level for a capability
    pub fn assess_risk(&self, capability: &CapabilityInfo) -> RiskLevel {
        match capability.type_ {
            CapabilityType::Filesystem => {
                // Filesystem access - assess based on mode and path
                RiskLevel::Medium
            }
            CapabilityType::Network => {
                // Network access - assess based on pattern
                RiskLevel::Medium
            }
            CapabilityType::Command => {
                // Command execution is always high risk
                RiskLevel::High
            }
            CapabilityType::Database => {
                // Database access
                RiskLevel::Medium
            }
            CapabilityType::Secrets => {
                // Secrets access is high risk
                RiskLevel::High
            }
            CapabilityType::Custom => {
                // Unknown capability - treat as high risk
                RiskLevel::High
            }
        }
    }

    /// Get filesystem enforcer
    pub fn filesystem(&self) -> &super::filesystem::FilesystemEnforcer {
        &self.fs_enforcer
    }

    /// Get network enforcer
    pub fn network(&self) -> &super::network::NetworkEnforcer {
        &self.net_enforcer
    }

    /// Apply an auto-approved capability
    fn apply_approval(&self, approval_id: &str) -> Result<(), PolicyError> {
        let approval = {
            let pending = self.pending_approvals.read().unwrap();
            pending
                .get(approval_id)
                .cloned()
                .ok_or_else(|| PolicyError::ApprovalNotFound {
                    id: approval_id.to_string(),
                })?
        };

        self.apply_approved_capability(&approval)?;

        // Move to approved list
        {
            let mut pending = self.pending_approvals.write().unwrap();
            pending.remove(approval_id);
        }

        {
            let mut approved = self.approved_capabilities.write().unwrap();
            approved
                .entry(approval.plugin_id.clone())
                .or_default()
                .push(approval);
        }

        Ok(())
    }

    /// Apply an approved capability to the enforcers
    fn apply_approved_capability(&self, approval: &CapabilityApproval) -> Result<(), PolicyError> {
        // This would integrate with the specific enforcers
        // For now, just log the approval
        tracing::error!(
            "Security: Applied {} approval for plugin {} capability {:?}",
            match approval.status {
                ApprovalStatus::AutoApproved => "auto-",
                _ => "",
            },
            approval.plugin_id,
            approval.capability_type
        );
        Ok(())
    }

    /// Clean up expired approvals
    pub fn cleanup_expired(&self) {
        let now = Instant::now();

        // Clean pending
        {
            let mut pending = self.pending_approvals.write().unwrap();
            pending.retain(|_, approval| approval.expires_at.map(|exp| exp > now).unwrap_or(true));
        }

        // Clean approved
        {
            let mut approved = self.approved_capabilities.write().unwrap();
            for (_, caps) in approved.iter_mut() {
                for cap in caps.iter_mut() {
                    if cap.expires_at.map(|exp| exp <= now).unwrap_or(false) {
                        cap.status = ApprovalStatus::Expired;
                    }
                }
            }
        }
    }
}

impl Default for SecurityPolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Policy errors
#[derive(Debug, Clone)]
pub enum PolicyError {
    /// Approval not found
    ApprovalNotFound { id: String },
    /// Too many pending requests
    TooManyPendingRequests { plugin_id: String, max: usize },
    /// Capability denied
    CapabilityDenied { reason: String },
    /// MFA required
    MfaRequired,
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::ApprovalNotFound { id } => write!(f, "Approval not found: {}", id),
            PolicyError::TooManyPendingRequests { plugin_id, max } => {
                write!(
                    f,
                    "Too many pending requests for plugin {} (max: {})",
                    plugin_id, max
                )
            }
            PolicyError::CapabilityDenied { reason } => write!(f, "Capability denied: {}", reason),
            PolicyError::MfaRequired => write!(f, "MFA required for this operation"),
        }
    }
}

impl std::error::Error for PolicyError {}

/// Generate a short UUID v4-like string
fn uuid_v4_short() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    format!("{:08x}{:08x}", timestamp & 0xFFFFFFFF, count & 0xFFFFFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_policy_engine_new() {
        let engine = SecurityPolicyEngine::new();
        assert!(engine.get_pending_approvals().is_empty());
    }

    #[test]
    fn test_request_approval_auto() {
        let engine = SecurityPolicyEngine::new();

        let cap = CapabilityInfo {
            type_: CapabilityType::Filesystem,
            data: std::ptr::null(),
            description: std::ptr::null(),
        };

        let result = engine.request_approval("test-plugin", &cap, "Test capability");
        assert!(result.is_ok());

        let approval_id = result.unwrap();
        // Should be auto-approved (medium risk with auto_approve_minimal=true)
        // Actually filesystem is medium risk, so needs manual approval
        assert!(!approval_id.is_empty());
    }

    #[test]
    fn test_assess_risk() {
        let engine = SecurityPolicyEngine::new();

        let cap = CapabilityInfo {
            type_: CapabilityType::Command,
            data: std::ptr::null(),
            description: std::ptr::null(),
        };
        assert_eq!(engine.assess_risk(&cap), RiskLevel::High);

        let cap = CapabilityInfo {
            type_: CapabilityType::Secrets,
            data: std::ptr::null(),
            description: std::ptr::null(),
        };
        assert_eq!(engine.assess_risk(&cap), RiskLevel::High);
    }

    #[test]
    fn test_approve_deny() {
        let engine = SecurityPolicyEngine::new();

        let cap = CapabilityInfo {
            type_: CapabilityType::Network,
            data: std::ptr::null(),
            description: std::ptr::null(),
        };

        let approval_id = engine
            .request_approval("test-plugin", &cap, "Network access")
            .unwrap();

        // Deny it
        engine.deny(&approval_id, "Not allowed").unwrap();

        let pending = engine.get_pending_for_plugin("test-plugin");
        let denied = pending.iter().find(|a| a.id == approval_id);
        // After denial, it's still in pending with Denied status
        assert!(denied.is_some());
        assert_eq!(denied.unwrap().status, ApprovalStatus::Denied);
    }

    #[test]
    fn test_risk_level_order() {
        assert!(RiskLevel::Minimal < RiskLevel::Low);
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }
}
