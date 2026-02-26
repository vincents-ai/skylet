use plugin_test_harness::*;
use std::path::PathBuf;

#[tokio::test]
async fn test_counter_plugin_initialization() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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
async fn test_counter_increment() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    let response = harness.execute_action_with_query("increment", "").await.unwrap();
    assert!(response.contains("\"value\": 1"));
    assert!(response.contains("\"action\": \"incremented\""));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_counter_decrement() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    harness.execute_action_with_query("set", "{\"value\": 5}").await.unwrap();
    let response = harness.execute_action_with_query("decrement", "").await.unwrap();
    assert!(response.contains("\"value\": 4"));
    assert!(response.contains("\"action\": \"decremented\""));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_counter_reset() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    for _ in 0..10 {
        harness.execute_action_with_query("increment", "").await.unwrap();
    }

    let response = harness.execute_action_with_query("reset", "").await.unwrap();
    assert!(response.contains("\"value\": 0"));
    assert!(response.contains("\"action\": \"reset\""));

    let get_response = harness.execute_action_with_query("get", "").await.unwrap();
    assert!(get_response.contains("\"value\": 0"));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_counter_set_with_max() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    let response = harness.execute_action_with_query(
        "set",
        r#"{"value": 10, "max": 15}"#
    ).await.unwrap();
    assert!(response.contains("\"value\": 10"));
    assert!(response.contains("\"action\": \"set\""));

    harness.execute_action_with_query("increment", "").await.unwrap();
    harness.execute_action_with_query("increment", "").await.unwrap();
    harness.execute_action_with_query("increment", "").await.unwrap();

    let increment_response = harness.execute_action_with_query("increment", "").await.unwrap();
    assert!(increment_response.contains("\"value\": 14"));

    let exceed_response = harness.execute_action_with_query("increment", "").await.unwrap();
    assert!(exceed_response.contains("\"error\""));
    assert!(exceed_response.contains("maximum value of 15"));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_counter_get() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    let initial = harness.execute_action_with_query("get", "").await.unwrap();
    assert!(initial.contains("\"value\": 0"));

    for _ in 0..5 {
        harness.execute_action_with_query("increment", "").await.unwrap();
    }

    let after_increments = harness.execute_action_with_query("get", "").await.unwrap();
    assert!(after_increments.contains("\"value\": 5"));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_counter_error_handling() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    let invalid_json = harness.execute_action_with_query("set", "not json").await.unwrap();
    assert!(invalid_json.contains("\"error\""));
    assert!(invalid_json.contains("Invalid JSON format"));

    let missing_value = harness.execute_action_with_query("set", "{}").await.unwrap();
    assert!(missing_value.contains("\"error\""));
    assert!(missing_value.contains("missing 'value' field"));

    let unknown_action = harness.execute_action_with_query("unknown", "").await.unwrap();
    assert!(unknown_action.contains("\"error\""));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_counter_health_check() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    harness.execute_action_with_query("set", r#"{"value": 10, "max": 15}"#).await.unwrap();
    let health_after_set = harness.health_check().await.unwrap();
    assert!(health_after_set.is_healthy());

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_counter_sequence() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    let r1 = harness.execute_action_with_query("get", "").await.unwrap();
    assert!(r1.contains("\"value\": 0"));

    let r2 = harness.execute_action_with_query("increment", "").await.unwrap();
    assert!(r2.contains("\"value\": 1"));

    let r3 = harness.execute_action_with_query("increment", "").await.unwrap();
    assert!(r3.contains("\"value\": 2"));

    let r4 = harness.execute_action_with_query("decrement", "").await.unwrap();
    assert!(r4.contains("\"value\": 1"));

    let r5 = harness.execute_action_with_query("reset", "").await.unwrap();
    assert!(r5.contains("\"value\": 0"));

    let r6 = harness.execute_action_with_query("get", "").await.unwrap();
    assert!(r6.contains("\"value\": 0"));

    harness.shutdown_plugin().await.unwrap();
}

#[tokio::test]
async fn test_counter_saturated_decrement() {
    let plugin_path = PathBuf::from("../../target/release/libcounter_plugin.so");
    
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

    let response = harness.execute_action_with_query("decrement", "").await.unwrap();
    assert!(response.contains("\"value\": 0"));

    harness.shutdown_plugin().await.unwrap();
}
