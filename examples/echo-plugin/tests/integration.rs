use plugin_test_harness::*;
use std::path::PathBuf;

#[tokio::test]
async fn test_echo_plugin_initialization() {
    let plugin_path = PathBuf::from("../../target/release/libecho_plugin.so");
    
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
async fn test_echo_plugin_basic() {
    let plugin_path = PathBuf::from("../../target/release/libecho_plugin.so");
    
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
    assert!(response.contains("Hello from Echo Plugin!"));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_echo_plugin_with_body() {
    let plugin_path = PathBuf::from("../../target/release/libecho_plugin.so");
    
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

    let test_message = "Test message for echo";
    let response = harness.execute_action("request", test_message).await.unwrap();
    assert_eq!(response, test_message);

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_echo_plugin_with_json() {
    let plugin_path = PathBuf::from("../../target/release/libecho_plugin.so");
    
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

    let json_data = r#"{"key": "value", "number": 42}"#;
    let response = harness.execute_action("request", json_data).await.unwrap();
    assert_eq!(response, json_data);

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_echo_plugin_health_check() {
    let plugin_path = PathBuf::from("../../target/release/libecho_plugin.so");
    
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
async fn test_echo_plugin_multiple_requests() {
    let plugin_path = PathBuf::from("../../target/release/libecho_plugin.so");
    
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

    let messages = vec![
        "First message",
        "Second message",
        "Third message",
        "Fourth message",
        "Fifth message",
    ];

    for (i, msg) in messages.iter().enumerate() {
        let response = harness.execute_action("request", msg).await.unwrap();
        assert_eq!(response, *msg, "Echo failed for message {}", i + 1);
    }

    harness.shutdown_plugin().await.unwrap();
}
