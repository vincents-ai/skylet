// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Service testing utilities for the test framework
//!
//! This module provides mock implementations of plugin services for testing.
//!
//! NOTE: This is a stub module. The implementation is tracked by task:
//! c7c4f29f-95b0-4263-94f0-f1b95e478d33

/// Mock plugin for testing plugin interactions
///
/// # Example (when implemented)
/// ```ignore
/// let plugin = MockPlugin::builder("test-plugin")
///     .with_version("1.0.0")
///     .with_capability("test.action")
///     .build();
/// ```
pub struct MockPlugin {
    name: String,
    version: String,
    capabilities: Vec<String>,
}

impl MockPlugin {
    /// Creates a new builder for constructing a MockPlugin
    pub fn builder(name: &str) -> MockPluginBuilder {
        MockPluginBuilder {
            name: name.to_string(),
            version: "0.0.0".to_string(),
            capabilities: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

pub struct MockPluginBuilder {
    name: String,
    version: String,
    capabilities: Vec<String>,
}

impl MockPluginBuilder {
    pub fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn with_capability(mut self, capability: &str) -> Self {
        self.capabilities.push(capability.to_string());
        self
    }

    pub fn build(self) -> MockPlugin {
        MockPlugin {
            name: self.name,
            version: self.version,
            capabilities: self.capabilities,
        }
    }
}

/// Mock service registry for testing service discovery
pub struct MockServiceRegistry {
    services: Vec<(String, String, *mut std::ffi::c_void)>,
}

impl MockServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
        }
    }

    pub fn register_service(&mut self, name: &str, type_name: &str, ptr: *mut std::ffi::c_void) {
        self.services
            .push((name.to_string(), type_name.to_string(), ptr));
    }

    pub fn has_service(&self, name: &str) -> bool {
        self.services.iter().any(|(n, _, _)| n == name)
    }

    pub fn service_count(&self) -> usize {
        self.services.len()
    }
}

impl Default for MockServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Lifecycle event for simulation
#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    PluginLoaded { name: String },
    PluginInitialized { name: String },
    PluginStarted { name: String },
    PluginStopped { name: String },
    PluginUnloaded { name: String },
}

/// Lifecycle simulator for testing plugin lifecycle handling
pub struct LifecycleSimulator {
    events: Vec<LifecycleEvent>,
}

impl LifecycleSimulator {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn emit_event(&mut self, event: LifecycleEvent) {
        self.events.push(event);
    }

    pub fn event_history(&self) -> &[LifecycleEvent] {
        &self.events
    }
}

impl Default for LifecycleSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_plugin_has_capability() {
        let plugin = MockPlugin::builder("test")
            .with_capability("test.action")
            .build();
        assert!(plugin.has_capability("test.action"));
        assert!(!plugin.has_capability("other.action"));
    }

    #[test]
    fn test_mock_plugin_version() {
        let plugin = MockPlugin::builder("test").with_version("1.0.0").build();
        assert_eq!(plugin.version(), "1.0.0");
    }

    #[test]
    fn test_mock_service_registry() {
        let mut registry = MockServiceRegistry::new();
        assert_eq!(registry.service_count(), 0);

        registry.register_service("test.service", "test.v1.Service", std::ptr::null_mut());
        assert!(registry.has_service("test.service"));
        assert!(!registry.has_service("other.service"));
        assert_eq!(registry.service_count(), 1);
    }

    #[test]
    fn test_lifecycle_simulator() {
        let mut simulator = LifecycleSimulator::new();

        simulator.emit_event(LifecycleEvent::PluginLoaded {
            name: "test".to_string(),
        });
        simulator.emit_event(LifecycleEvent::PluginInitialized {
            name: "test".to_string(),
        });

        let events = simulator.event_history();
        assert_eq!(events.len(), 2);
    }
}
