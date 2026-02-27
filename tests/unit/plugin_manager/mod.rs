//! Unit tests for plugin manager components

use super::*;
use mockall::predicate::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

mod plugin_manager_tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_manager_initialization() {
        let mut manager = PluginManagerImpl::new();
        
        // Test manager initialization
        assert_eq!(manager.get_plugin_count().await, 0);
        
        // Test manager state
        assert!(!manager.has_plugins().await);
        assert!(manager.get_plugin_status("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn test_plugin_lifecycle() {
        let mut manager = PluginManagerImpl::new();
        
        // Test plugin loading
        let result = manager.load_plugin("test-plugin").await;
        assert!(result.is_ok());
        
        // Verify plugin was loaded
        assert_eq!(manager.get_plugin_count().await, 1);
        assert!(manager.has_plugins().await);
        
        // Test plugin status
        let status = manager.get_plugin_status("test-plugin").await;
        assert!(status.is_ok());
        let status = status.unwrap();
        assert_eq!(status.name, "test-plugin");
        assert_eq!(status.status, "loaded");
        
        // Test plugin unloading
        let result = manager.unload_plugin("test-plugin").await;
        assert!(result.is_ok());
        
        // Verify plugin was unloaded
        assert_eq!(manager.get_plugin_count().await, 0);
        assert!(!manager.has_plugins().await);
        assert!(manager.get_plugin_status("test-plugin").await.is_err());
    }

    #[tokio::test]
    async fn test_plugin_duplicate_loading() {
        let mut manager = PluginManagerImpl::new();
        
        // Load plugin first time
        let result = manager.load_plugin("duplicate-plugin").await;
        assert!(result.is_ok());
        
        // Try to load same plugin again
        let result = manager.load_plugin("duplicate-plugin").await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("already loaded"));
    }

    #[tokio::test]
    async fn test_plugin_unloading_nonexistent() {
        let mut manager = PluginManagerImpl::new();
        
        // Try to unload nonexistent plugin
        let result = manager.unload_plugin("nonexistent-plugin").await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_plugin_list() {
        let mut manager = PluginManagerImpl::new();
        
        // Initially empty
        let plugins = manager.list_plugins().await;
        assert!(plugins.is_empty());
        
        // Load multiple plugins
        manager.load_plugin("plugin-1").await.unwrap();
        manager.load_plugin("plugin-2").await.unwrap();
        manager.load_plugin("plugin-3").await.unwrap();
        
        // Verify list
        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 3);
        
        let plugin_names: Vec<String> = plugins.iter().map(|p| p.name.clone()).collect();
        assert!(plugin_names.contains(&"plugin-1".to_string()));
        assert!(plugin_names.contains(&"plugin-2".to_string()));
        assert!(plugin_names.contains(&"plugin-3".to_string()));
    }

    #[tokio::test]
    async fn test_plugin_reloading() {
        let mut manager = PluginManagerImpl::new();
        
        // Load plugin
        manager.load_plugin("reload-test").await.unwrap();
        
        // Reload plugin
        let result = manager.reload_plugin("reload-test").await;
        assert!(result.is_ok());
        
        // Verify status changed
        let status = manager.get_plugin_status("reload-test").await.unwrap();
        assert_eq!(status.status, "reloaded");
    }

    #[tokio::test]
    async fn test_plugin_dependency_resolution() {
        let mut manager = PluginManagerImpl::new();
        
        // Create test dependencies
        let plugin_1 = PluginInfo::new("plugin-1", "0.1.0", vec![]);
        let plugin_2 = PluginInfo::new("plugin-2", "0.1.0", vec!["plugin-1".to_string()]);
        let plugin_3 = PluginInfo::new("plugin-3", "0.1.0", vec!["plugin-2".to_string()]);
        
        // Test dependency resolution
        let dependencies = vec![
            ("plugin-3".to_string(), vec!["plugin-2".to_string()]),
            ("plugin-2".to_string(), vec!["plugin-1".to_string()]),
            ("plugin-1".to_string(), vec![]),
        ];
        
        let resolved_order = manager.resolve_dependencies(&dependencies).unwrap();
        assert_eq!(resolved_order.len(), 3);
        assert_eq!(resolved_order[0], "plugin-1");
        assert_eq!(resolved_order[1], "plugin-2");
        assert_eq!(resolved_order[2], "plugin-3");
    }

    #[tokio::test]
    async fn test_plugin_dependency_cycle() {
        let mut manager = PluginManagerImpl::new();
        
        // Create circular dependency
        let dependencies = vec![
            ("plugin-1".to_string(), vec!["plugin-2".to_string()]),
            ("plugin-2".to_string(), vec!["plugin-1".to_string()]),
        ];
        
        let result = manager.resolve_dependencies(&dependencies);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("circular dependency"));
    }

    #[tokio::test]
    async fn test_plugin_loading_with_dependencies() {
        let mut manager = PluginManagerImpl::new();
        
        // Define dependencies
        let dependencies = vec![
            ("plugin-3".to_string(), vec!["plugin-1".to_string(), "plugin-2".to_string()]),
            ("plugin-2".to_string(), vec!["plugin-1".to_string()]),
            ("plugin-1".to_string(), vec![]),
        ];
        
        // Load plugins with dependencies
        let result = manager.load_plugins_with_dependencies(&dependencies).await;
        assert!(result.is_ok());
        
        // Verify all plugins were loaded
        assert_eq!(manager.get_plugin_count().await, 3);
        
        let plugins = manager.list_plugins().await;
        let plugin_names: Vec<String> = plugins.iter().map(|p| p.name.clone()).collect();
        assert!(plugin_names.contains(&"plugin-1".to_string()));
        assert!(plugin_names.contains(&"plugin-2".to_string()));
        assert!(plugin_names.contains(&"plugin-3".to_string()));
    }

    #[tokio::test]
    async fn test_plugin_unloading_with_dependents() {
        let mut manager = PluginManagerImpl::new();
        
        // Define dependencies where plugin-2 depends on plugin-1
        let dependencies = vec![
            ("plugin-2".to_string(), vec!["plugin-1".to_string()]),
            ("plugin-1".to_string(), vec![]),
        ];
        
        // Load plugins
        manager.load_plugins_with_dependencies(&dependencies).await.unwrap();
        
        // Try to unload plugin-1 which has dependents
        let result = manager.unload_plugin("plugin-1").await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("has dependent plugins"));
        
        // Unload dependent first
        manager.unload_plugin("plugin-2").await.unwrap();
        
        // Now unload plugin-1
        manager.unload_plugin("plugin-1").await.unwrap();
        
        // Verify both are unloaded
        assert_eq!(manager.get_plugin_count().await, 0);
    }

    #[tokio::test]
    async fn test_plugin_resource_tracking() {
        let mut manager = PluginManagerImpl::new();
        
        // Load a plugin
        manager.load_plugin("resource-test").await.unwrap();
        
        // Get resource usage
        let resources = manager.get_plugin_resources("resource-test").await;
        assert!(resources.is_ok());
        let resources = resources.unwrap();
        
        // Verify resource tracking
        assert!(resources.memory_usage_mb > 0.0);
        assert!(resources.cpu_usage_percent >= 0.0);
        assert!(resources.file_handles_open >= 0);
        assert!(resources.network_connections >= 0);
    }

    #[tokio::test]
    async fn test_plugin_resource_limits() {
        let mut manager = PluginManagerImpl::new();
        
        // Set resource limits
        let limits = ResourceLimits {
            max_memory_mb: 100.0,
            max_cpu_percent: 50.0,
            max_file_handles: 100,
            max_network_connections: 50,
        };
        
        manager.set_resource_limits("resource-test", limits).await;
        
        // Verify limits were set
        let resources = manager.get_plugin_resources("resource-test").await.unwrap();
        assert_eq!(resources.max_memory_mb, 100.0);
        assert_eq!(resources.max_cpu_percent, 50.0);
        assert_eq!(resources.max_file_handles, 100);
        assert_eq!(resources.max_network_connections, 50);
    }

    #[tokio::test]
    async fn test_plugin_performance_tracking() {
        let mut manager = PluginManagerImpl::new();
        
        // Load plugin
        manager.load_plugin("performance-test").await.unwrap();
        
        // Execute plugin (simulated)
        let start = std::time::Instant::now();
        manager.execute_plugin("performance-test", "test-operation").await.unwrap();
        let duration = start.elapsed();
        
        // Verify performance tracking
        let performance = manager.get_plugin_performance("performance-test").await.unwrap();
        assert!(performance.execution_time_ms > 0.0);
        assert!(performance.success_count >= 1);
        assert_eq!(performance.error_count, 0);
    }

    #[tokio::test]
    async fn test_plugin_error_handling() {
        let mut manager = PluginManagerImpl::new();
        
        // Load a plugin that will fail
        let plugin_info = PluginInfo::new("failing-plugin", "0.1.0", vec![]);
        plugin_info.set_failure_probability(1.0); // Always fail
        
        let result = manager.load_plugin_info(plugin_info).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("failed to load"));
        
        // Verify plugin was not loaded
        assert!(!manager.is_plugin_loaded("failing-plugin").await);
        assert!(manager.get_plugin_status("failing-plugin").await.is_err());
    }

    #[tokio::test]
    async fn test_plugin_concurrent_access() {
        let mut manager = PluginManagerImpl::new();
        
        // Create multiple tasks to access the manager concurrently
        let tasks: Vec<_> = (0..10)
            .map(|i| {
                let mut manager_clone = manager.clone();
                async move {
                    manager_clone.load_plugin(&format!("concurrent-plugin-{}", i)).await
                }
            })
            .collect();
        
        // Execute all tasks concurrently
        let results = futures::future::join_all(tasks).await;
        
        // Verify all plugins were loaded successfully
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Plugin {} failed to load", i);
        }
        
        // Verify all plugins are loaded
        assert_eq!(manager.get_plugin_count().await, 10);
        
        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 10);
    }

    #[tokio::test]
    async fn test_plugin_persistence() {
        let mut manager = PluginManagerImpl::new();
        
        // Load some plugins
        manager.load_plugin("persistent-plugin-1").await.unwrap();
        manager.load_plugin("persistent-plugin-2").await.unwrap();
        
        // Save state
        let result = manager.save_state().await;
        assert!(result.is_ok());
        
        // Create new manager and load state
        let mut new_manager = PluginManagerImpl::new();
        let load_result = new_manager.load_state(result.unwrap()).await;
        assert!(load_result.is_ok());
        
        // Verify state was restored
        assert_eq!(new_manager.get_plugin_count().await, 2);
        
        let plugins = new_manager.list_plugins().await;
        let plugin_names: Vec<String> = plugins.iter().map(|p| p.name.clone()).collect();
        assert!(plugin_names.contains(&"persistent-plugin-1".to_string()));
        assert!(plugin_names.contains(&"persistent-plugin-2".to_string()));
    }

    #[tokio::test]
    async fn test_plugin_health_check() {
        let mut manager = PluginManagerImpl::new();
        
        // Load plugins with different health states
        manager.load_plugin("healthy-plugin").await.unwrap();
        
        // Simulate plugin failure
        let mut plugin_info = PluginInfo::new("unhealthy-plugin", "0.1.0", vec![]);
        plugin_info.set_health_status("unhealthy");
        
        let result = manager.load_plugin_info(plugin_info).await;
        assert!(result.is_err());
        
        // Run health check
        let health_report = manager.check_health().await;
        
        // Verify health report
        assert!(!health_report.is_healthy());
        assert_eq!(health_report.healthy_plugins.len(), 1);
        assert_eq!(health_report.unhealthy_plugins.len(), 1);
        assert_eq!(health_report.healthy_plugins[0], "healthy-plugin");
        assert_eq!(health_report.unhealthy_plugins[0], "unhealthy-plugin");
    }

    #[tokio::test]
    async fn test_plugin_cleanup() {
        let mut manager = PluginManagerImpl::new();
        
        // Load multiple plugins
        for i in 0..5 {
            manager.load_plugin(&format!("cleanup-plugin-{}", i)).await.unwrap();
        }
        
        // Verify plugins are loaded
        assert_eq!(manager.get_plugin_count().await, 5);
        
        // Clean up all plugins
        let result = manager.cleanup_all_plugins().await;
        assert!(result.is_ok());
        
        // Verify plugins are cleaned up
        assert_eq!(manager.get_plugin_count().await, 0);
    }
}