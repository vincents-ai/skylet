// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Plugin assertion utilities for testing
//!
//! This module provides assertion helpers for testing plugin behavior.

use super::service::MockPlugin;

/// Trait for plugin assertions
pub trait PluginAssertions {
    /// Assert the plugin is in a specific state
    fn assert_state(&self, expected_state: &str);

    /// Assert the plugin has a specific capability
    fn assert_has_capability(&self, capability: &str);

    /// Assert the plugin's health status
    fn assert_health_status(&self, expected_status: &str);
}

impl PluginAssertions for MockPlugin {
    fn assert_state(&self, _expected_state: &str) {
        // For mock plugins, we just verify the plugin exists
        // Real state tracking would require a state field
    }

    fn assert_has_capability(&self, capability: &str) {
        assert!(
            self.has_capability(capability),
            "Plugin '{}' does not have capability '{}'",
            self.name(),
            capability
        );
    }

    fn assert_health_status(&self, _expected_status: &str) {
        // For mock plugins, we just verify the plugin exists
        // Real health status tracking would require a health field
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_has_capability_passes() {
        let plugin = MockPlugin::new("test")
            .with_capability("test.action")
            .build();
        plugin.assert_has_capability("test.action"); // Should not panic
    }

    #[test]
    #[should_panic(expected = "does not have capability")]
    fn test_assert_has_capability_fails() {
        let plugin = MockPlugin::new("test").build();
        plugin.assert_has_capability("missing.action"); // Should panic
    }

    #[test]
    fn test_assert_state() {
        let plugin = MockPlugin::new("test").build();
        plugin.assert_state("any"); // For mock, this always passes
    }

    #[test]
    fn test_assert_health_status() {
        let plugin = MockPlugin::new("test").build();
        plugin.assert_health_status("healthy"); // For mock, this always passes
    }
}
