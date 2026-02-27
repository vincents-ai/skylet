//! Unit tests for core components

pub mod config_system;
pub mod event_bus;
pub mod metrics;
pub mod plugin_manager;

// Re-export common test utilities
pub use crate::testing_comprehensive::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_imports() {
        // Ensure all test modules are accessible
        // This is a placeholder test to verify module structure
        assert!(true);
    }
}
