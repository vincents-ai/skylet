// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use execution_engine_core::auth::UserContext;

#[test]
fn test_usercontext_roles_and_permissions() {
    let uc = UserContext::new(
        Some("user1".to_string()),
        vec!["user".to_string()],
        vec!["read".to_string()],
    );
    assert!(!uc.is_admin());
    assert!(uc.has_role("user"));
    assert!(uc.has_permission("read"));

    let admin = UserContext::new(
        Some("admin1".to_string()),
        vec!["admin".to_string()],
        vec![],
    );
    assert!(admin.is_admin());
}
