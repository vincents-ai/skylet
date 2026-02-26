use plugin_test_harness::*;
use std::path::PathBuf;

#[tokio::test]
async fn test_hello_plugin_initialization() {
    let plugin_path = PathBuf::from("../../target/release/libhello_plugin.so");
    
    if !plugin_path.exists() {
        println!("Warning: Plugin not built yet. Skipping test.");
        return;
    }

    let config = PluginTestConfig {
        plugin_path: plugin_path.to_string_lossy().to_string(),
        test_timeout_ms: 5000,
        enable_logging: true,
        ..Default::default()
    };

    let mut harness = PluginTestHarness::new(config);
    
    harness.load_plugin().await.expect("Failed to load plugin");
    harness.init_plugin().await.expect("Failed to initialize plugin");
    harness.shutdown_plugin().await.expect("Failed to shutdown plugin");
}

#[tokio::test]
async fn test_hello_plugin_request() {
    let plugin_path = PathBuf::from("../../target/release/libhello_plugin.so");
    
    if !plugin_path.exists() {
        println!("Warning: Plugin not built yet. Skipping test.");
        return;
    }

    let config = PluginTestConfig {
        plugin_path: plugin_path.to_string_lossy().to_string(),
        test_timeout_ms: 5000,
        enable_logging: false,
        ..Default::default()
    };

    let mut harness = PluginTestHarness::new(config);
    
    harness.load_plugin().await.unwrap();
    harness.init_plugin().await.unwrap();

    let response = harness.execute_action("request", "").await.unwrap();
    assert!(response.contains("Hello from Hello Plugin!"));
    assert!(!response.contains("error"));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_hello_plugin_health_check() {
    let plugin_path = PathBuf::from("../../target/release/libhello_plugin.so");
    
    if !plugin_path.exists() {
        println!("Warning: Plugin not built yet. Skipping test.");
        return;
    }

    let config = PluginTestConfig {
        plugin_path: plugin_path.to_string_lossy().to_string(),
        test_timeout_ms: 5000,
        enable_logging: false,
        ..Default::default()
    };

    let mut harness = PluginTestHarness::new(config);
    
    harness.load_plugin().await.unwrap();
    harness.init_plugin().await.unwrap();

    let health = harness.health_check().await.unwrap();
    assert!(health.is_healthy());

    harness.shutdown_plugin().await.unwrap();
    
    let health_after_shutdown = harness.health_check().await.unwrap();
    assert!(health_after_shutdown.is_unhealthy());
}

#[tokio::test]
async fn test_hello_plugin_response_format() {
    let plugin_path = PathBuf::from("../../target/release/libhello_plugin.so");
    
    if !plugin_path.exists() {
        println!("Warning: Plugin not built yet. Skipping test.");
        return;
    }

    let config = PluginTestConfig {
        plugin_path: plugin_path.to_string_lossy().to_string(),
        test_timeout_ms: 5000,
        enable_logging: false,
        ..Default::default()
    };

    let mut harness = PluginTestHarness::new(config);
    
    harness.load_plugin().await.unwrap();
    harness.init_plugin().await.unwrap();

    let response = harness.execute_action("request", "").await.unwrap();
    assert!(response.contains("Hello"));
    assert!(response.contains("Plugin"));
    assert_eq!(response, "Hello from Hello Plugin!");

    harness.shutdown_plugin().await.unwrap();
}
