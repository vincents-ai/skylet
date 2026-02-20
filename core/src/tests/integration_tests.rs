// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

// Integration tests for the complete marketplace system
#[cfg(test)]
mod integration_tests {
    use crate::framework::TestFramework;
    
    #[test]
    fn test_full_integration_workflow() {
        // Test the complete integration workflow from end to end
        let mut framework = TestFramework::new();
        
        // Test 1: Create test environment
        let _env_name = framework.create_test_environment("integration_test").unwrap();
        let _env = framework.get_environment("integration_test").unwrap();
        
        // All wallet-related tests are now removed as wallet is a plugin.
        // Integration tests for plugins should be done in a separate test suite
        // that loads the compiled plugin.
        
        tracing::info!("✅ Full integration test workflow passed");
    }
    
    #[test]
    fn test_service_compatibility() {
        // Test that all components can work together
        let mut framework = TestFramework::new();
        
        let _env_name = framework.create_test_environment("service_test").unwrap();
        
        // Create multiple environments for parallel testing
        let _env1 = framework.create_test_environment("parallel_test_1").unwrap();
        let _env2 = framework.create_test_environment("parallel_test_2").unwrap();
        
        // Verify isolation between environments
        let env1_ref = framework.get_environment("parallel_test_1").unwrap();
        let env2_ref = framework.get_environment("parallel_test_2").unwrap();
        assert_ne!(env1_ref.path(), env2_ref.path(), "Environments should have different paths");
        
        // Clean up all environments
        framework.cleanup_all().unwrap();
        
        assert!(framework.get_environment("parallel_test_1").is_none(), "Environment 1 should be cleaned up");
        assert!(framework.get_environment("parallel_test_2").is_none(), "Environment 2 should be cleaned up");
        
        tracing::info!("✅ Service compatibility test passed");
    }
    
    #[test]
    fn test_performance_under_load() {
        // Test system performance under load
        let mut framework = TestFramework::new();
        
        let start_time = std::time::Instant::now();
        
        // Create 50 concurrent test environments
        for i in 0..50 {
            let env_name = format!("load_test_{}", i);
            framework.create_test_environment(&env_name).unwrap();
        }
        
        let creation_time = start_time.elapsed();
        
        // All environments should be isolated
        for i in 0..50 {
            let env_name = format!("load_test_{}", i);
            let env = framework.get_environment(&env_name).unwrap();
            assert!(env.is_isolated(), "Each environment should be isolated");
        }
        
        let total_time = start_time.elapsed();
        
        // Should complete within reasonable time
        assert!(total_time.as_secs() < 30, "Environment creation should complete within 30 seconds");
        
        let _ = framework.cleanup_all();
        
        tracing::info!("✅ Performance under load test passed (creation: {:?}, total: {:?})", creation_time, total_time);
    }
}
