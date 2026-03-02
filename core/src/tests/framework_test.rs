// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Framework tests - Testing mock plugin framework utilities
// Task: c7c4f29f-95b0-4263-94f0-f1b95e478d33

use crate::framework::TestFramework;

#[test]
fn test_framework_default() {
    let f = TestFramework::new();
    assert!(f.get_environment("anything").is_none());
}

// ============================================================================
// MOCK PLUGIN TESTS
// ============================================================================

/// Test that framework can create a mock plugin for testing
#[test]
fn test_framework_mock_plugin() {
    use crate::framework::service::MockPlugin;

    let plugin = MockPlugin::builder("test-plugin")
        .with_version("1.0.0")
        .with_capability("test.action")
        .build();

    assert_eq!(plugin.name(), "test-plugin");
    assert_eq!(plugin.version(), "1.0.0");
    assert!(plugin.has_capability("test.action"));
    assert!(!plugin.has_capability("other.action"));
}

/// Test that framework can create a mock service registry
#[test]
fn test_framework_mock_service_registry() {
    use crate::framework::service::MockServiceRegistry;

    let mut registry = MockServiceRegistry::new();
    assert_eq!(registry.service_count(), 0);

    // Register a mock service
    registry.register_service("test.service", "test.v1.Service", std::ptr::null_mut());

    assert!(registry.has_service("test.service"));
    assert!(!registry.has_service("nonexistent.service"));
    assert_eq!(registry.service_count(), 1);
}

/// Test that framework can simulate plugin lifecycle events
#[test]
fn test_framework_lifecycle_simulator() {
    use crate::framework::service::{LifecycleEvent, LifecycleSimulator};

    let mut simulator = LifecycleSimulator::new();

    // Simulate plugin load
    simulator.emit_event(LifecycleEvent::PluginLoaded {
        name: "test-plugin".to_string(),
    });

    // Simulate plugin init
    simulator.emit_event(LifecycleEvent::PluginInitialized {
        name: "test-plugin".to_string(),
    });

    let events = simulator.event_history();
    assert_eq!(events.len(), 2);

    // Verify event order
    match &events[0] {
        LifecycleEvent::PluginLoaded { name } => assert_eq!(name, "test-plugin"),
        _ => panic!("Expected PluginLoaded event"),
    }
    match &events[1] {
        LifecycleEvent::PluginInitialized { name } => assert_eq!(name, "test-plugin"),
        _ => panic!("Expected PluginInitialized event"),
    }
}

/// Test that framework can assert on plugin state
#[test]
fn test_framework_plugin_assertions() {
    use crate::framework::assertions::PluginAssertions;
    use crate::framework::service::MockPlugin;

    let plugin = MockPlugin::builder("test-plugin")
        .with_capability("test.action")
        .build();

    // These assertions should pass
    plugin.assert_state("Loaded");
    plugin.assert_has_capability("test.action");
    plugin.assert_health_status("healthy");
}
