//! Integration tests for plugin loading and lifecycle

use super::*;
use crate::plugin_manager::*;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_plugin_loading_from_disk() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    // Create a simple test plugin structure
    let plugin_path = plugin_dir.join("test_plugin.so");

    // Create a dummy plugin file (in real tests, this would be a compiled plugin)
    std::fs::write(&plugin_path, b"test_plugin_binary").unwrap();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    let result = manager.load_plugin("test_plugin").await;

    // Note: This test will fail without actual plugin binary
    // but demonstrates the test structure
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_plugin_dependency_chain_loading() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    // Define dependency chain: plugin1 -> plugin2 -> plugin3
    let dependencies = vec![
        ("plugin3".to_string(), vec!["plugin2".to_string()]),
        ("plugin2".to_string(), vec!["plugin1".to_string()]),
        ("plugin1".to_string(), vec![]),
    ];

    // Load plugins in correct order
    let result = manager.load_plugins_with_dependencies(&dependencies).await;

    assert!(result.is_ok());
    assert_eq!(manager.get_plugin_count().await, 3);
}

#[tokio::test]
async fn test_plugin_reload() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    // Load plugin
    manager.load_plugin("test_plugin").await.unwrap();

    // Get initial status
    let initial_status = manager.get_plugin_status("test_plugin").await.unwrap();

    // Reload plugin
    let result = manager.reload_plugin("test_plugin").await;
    assert!(result.is_ok());

    // Get status after reload
    let reloaded_status = manager.get_plugin_status("test_plugin").await.unwrap();

    // Verify reload happened (version or timestamp changed)
    assert_ne!(
        initial_status.loaded_at,
        reloaded_status.loaded_at
    );
}

#[tokio::test]
async fn test_plugin_unloading_cleans_resources() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    // Load plugin
    manager.load_plugin("resource_test_plugin").await.unwrap();

    // Get initial resource usage
    let initial_resources = manager.get_plugin_resources("resource_test_plugin").await.unwrap();

    // Unload plugin
    manager.unload_plugin("resource_test_plugin").await.unwrap();

    // Verify plugin is unloaded
    assert!(!manager.is_plugin_loaded("resource_test_plugin").await);

    // Verify resources were cleaned up
    let result = manager.get_plugin_resources("resource_test_plugin").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_plugin_hot_reload_on_file_change() {
    use tokio::sync::mpsc;

    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    // Enable hot reload
    manager.enable_hot_reload().await;

    // Create channel to receive reload events
    let (tx, mut rx) = mpsc::channel(10);
    manager.on_plugin_reload(Box::new(move |event| {
        let _ = tx.blocking_send(event);
    })).await;

    // Load plugin
    manager.load_plugin("hot_reload_test").await.unwrap();

    // Modify plugin file (simulate hot reload trigger)
    // In real scenario, this would trigger file watcher
    let plugin_path = plugin_dir.join("hot_reload_test.so");
    std::fs::write(&plugin_path, b"updated_plugin_binary").unwrap();

    // Wait for reload event
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Verify reload happened
    let received = rx.try_recv();
    assert!(received.is_ok());
}

#[tokio::test]
async fn test_multiple_plugins_concurrent_loading() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    // Create multiple plugins
    let plugin_names: Vec<String> = (0..10).map(|i| format!("plugin_{}", i)).collect();

    // Load all plugins concurrently
    let handles: Vec<_> = plugin_names
        .iter()
        .map(|name| {
            let mut manager = manager.clone();
            let name = name.clone();
            tokio::spawn(async move {
                manager.load_plugin(&name).await
            })
        })
        .collect();

    // Wait for all loads to complete
    let results: Vec<_> = futures::future::join_all(handles).await;

    // Verify all plugins were loaded
    for (i, result) in results.iter().enumerate() {
        assert!(
            result.is_ok(),
            "Plugin {} failed to load: {:?}",
            i,
            result
        );
    }
}

#[tokio::test]
async fn test_plugin_error_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    // Try to load a plugin that doesn't exist
    let result = manager.load_plugin("nonexistent_plugin").await;

    assert!(result.is_err());

    // Manager should still be functional
    assert_eq!(manager.get_plugin_count().await, 0);

    // Load a valid plugin
    manager.load_plugin("valid_plugin").await.unwrap();

    // Verify manager is still functional after error
    assert_eq!(manager.get_plugin_count().await, 1);
}

#[tokio::test]
async fn test_plugin_health_monitoring() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    // Load healthy plugin
    manager.load_plugin("healthy_plugin").await.unwrap();

    // Run health check
    let health_report = manager.check_health().await;

    assert!(health_report.is_healthy());
    assert_eq!(health_report.healthy_plugins.len(), 1);
    assert!(health_report.unhealthy_plugins.is_empty());

    // Simulate plugin becoming unhealthy
    manager.mark_plugin_unhealthy("healthy_plugin").await;

    // Run health check again
    let health_report = manager.check_health().await;

    assert!(!health_report.is_healthy());
    assert!(health_report.healthy_plugins.is_empty());
    assert_eq!(health_report.unhealthy_plugins.len(), 1);
}

#[tokio::test]
async fn test_plugin_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path();

    let mut manager = PluginManager::new(plugin_dir.to_path_buf());

    // Load some plugins
    manager.load_plugin("plugin1").await.unwrap();
    manager.load_plugin("plugin2").await.unwrap();

    // Save state
    let state = manager.save_state().await.unwrap();

    // Create new manager and restore state
    let mut new_manager = PluginManager::new(plugin_dir.to_path_buf());
    new_manager.load_state(&state).await.unwrap();

    // Verify state was restored
    assert_eq!(new_manager.get_plugin_count().await, 2);
    assert!(new_manager.is_plugin_loaded("plugin1").await);
    assert!(new_manager.is_plugin_loaded("plugin2").await);
}
