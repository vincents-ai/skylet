// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Test framework that will handle test environment setup
#[cfg(test)]
mod framework_tests {
    use crate::framework::{TestConfiguration, TestFramework};

    #[test]
    fn test_framework_creation() {
        // This test should pass now that framework exists
        let mut framework = TestFramework::new();

        // Verify framework was created
        // internal state is private; just ensure construction succeeded
        assert!(framework.get_environment("nonexistent").is_none());

        // Create test environment
        let env_name = framework.create_test_environment("test_env").unwrap();
        assert_eq!(env_name, "test_env");

        // Get environment and verify properties
        let env = framework.get_environment("test_env").unwrap();
        assert!(env.is_isolated());
        assert!(env.path().exists(), "Test environment path should exist");

        // Test cleanup
        framework.cleanup_all().unwrap();
        assert!(framework.get_environment("test_env").is_none());
    }

    #[test]
    fn test_framework_configuration() {
        // Test framework configuration
        let config = TestConfiguration::default();

        assert!(
            config.cleanup_on_drop,
            "Default config should cleanup on drop"
        );
        assert_eq!(
            config.max_test_duration_secs, 300,
            "Default max duration should be 300"
        );
    }

    #[test]
    fn test_multiple_environments() {
        // Test managing multiple test environments
        let mut framework = TestFramework::new();

        // Create multiple environments
        let _env1 = framework.create_test_environment("env1").unwrap();
        let _env2 = framework.create_test_environment("env2").unwrap();
        let _env3 = framework.create_test_environment("env3").unwrap();

        // Verify all environments exist and are isolated
        assert!(framework.get_environment("env1").is_some());
        assert!(framework.get_environment("env2").is_some());
        assert!(framework.get_environment("env3").is_some());

        let env1 = framework.get_environment("env1").unwrap();
        let env2 = framework.get_environment("env2").unwrap();
        let env3 = framework.get_environment("env3").unwrap();

        assert!(env1.is_isolated());
        assert!(env2.is_isolated());
        assert!(env3.is_isolated());

        // Verify paths are different
        assert_ne!(env1.path(), env2.path());
        assert_ne!(env2.path(), env3.path());
        assert_ne!(env1.path(), env3.path());

        // Test cleanup
        framework.cleanup_all().unwrap();
        assert!(framework.get_environment("env1").is_none());
        assert!(framework.get_environment("env2").is_none());
        assert!(framework.get_environment("env3").is_none());
    }
}
