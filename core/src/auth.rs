// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

/// Simple user context for request-level identity and roles
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContext {
    /// Optional user id (None for anonymous)
    pub user_id: Option<String>,
    /// Roles assigned to the user (e.g. "admin")
    pub roles: Vec<String>,
    /// Fine-grained permissions
    pub permissions: Vec<String>,
}

impl UserContext {
    pub fn new(user_id: Option<String>, roles: Vec<String>, permissions: Vec<String>) -> Self {
        Self {
            user_id,
            roles,
            permissions,
        }
    }

    pub fn is_admin(&self) -> bool {
        self.roles.iter().any(|r| r == "admin")
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.iter().any(|p| p == perm)
    }
}
