// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plugin lifecycle and action step definitions
//!
//! These steps handle plugin loading, initialization, execution, and assertions.

use cucumber::{given, then, when};
use std::time::Duration;

use crate::test_world::PluginTestWorld;
use crate::{TestResult, TestStatus};

// ============================================================================
// Given Steps - Setup and preconditions
// ============================================================================

/// Load a plugin by path
#[given(regex = r#"^the plugin "([^"]+)" is loaded$"#)]
async fn plugin_loaded(world: &mut PluginTestWorld, plugin_path: String) {
    world.setup(&plugin_path).expect("Failed to setup world");
    world.load_plugin().await.expect("Failed to load plugin");
}

/// Load a plugin from the target directory by name
#[given(regex = r#"^the plugin named "([^"]+)" is loaded$"#)]
async fn plugin_loaded_by_name(world: &mut PluginTestWorld, plugin_name: String) {
    // Try common paths
    let possible_paths = vec![
        format!("./target/release/lib{}.so", plugin_name),
        format!("./target/release/lib{}.dylib", plugin_name),
        format!("./target/debug/lib{}.so", plugin_name),
        format!("./target/debug/lib{}.dylib", plugin_name),
        format!(
            "../plugins/{}/target/release/lib{}.so",
            plugin_name, plugin_name
        ),
    ];

    for path in &possible_paths {
        if std::path::Path::new(path).exists() {
            world.setup(path).expect("Failed to setup world");
            world.load_plugin().await.expect("Failed to load plugin");
            return;
        }
    }

    panic!(
        "Could not find plugin '{}' in any expected location",
        plugin_name
    );
}

/// Plugin is initialized (already done during load, but explicit step)
#[given("the plugin is initialized")]
async fn plugin_initialized(world: &mut PluginTestWorld) {
    // Plugin is already initialized during load
    // This step is for documentation in scenarios
    assert!(
        world.plugin.is_some(),
        "Plugin should be loaded and initialized"
    );
}

/// Configure the plugin with key-value pairs
#[given(regex = r#"^the plugin is configured with:$"#)]
async fn plugin_configured(world: &mut PluginTestWorld, config: String) {
    // Parse config lines (format: key = "value")
    for line in config.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            world.set_config(key, value);
        }
    }
}

/// Set a specific config value
#[given(regex = r#"^the config "([^"]+)" is set to "([^"]+)"$"#)]
async fn config_value_set(world: &mut PluginTestWorld, key: String, value: String) {
    world.set_config(&key, &value);
}

/// Set test data
#[given(regex = r#"^the test data "([^"]+)" is "([^"]+)"$"#)]
async fn test_data_set(world: &mut PluginTestWorld, key: String, value: String) {
    world.set_data(&key, &value);
}

/// Create a test file
#[given(regex = r#"^a test file "([^"]+)" with content:$"#)]
async fn test_file_created(world: &mut PluginTestWorld, filename: String, content: String) {
    let path = world
        .create_test_file(&filename, &content)
        .expect("Failed to create test file");
    world.set_data(&format!("file:{}", filename), &path.to_string_lossy());
}

// ============================================================================
// When Steps - Actions
// ============================================================================

/// Execute an action with no arguments
#[when(regex = r#"^I execute the action "([^"]+)"$"#)]
async fn execute_action(world: &mut PluginTestWorld, action: String) {
    let start = std::time::Instant::now();
    match world.execute_action(&action, "{}") {
        Ok(_) => {
            world.add_result(TestResult::passed(
                format!("Execute action: {}", action),
                start.elapsed(),
            ));
        }
        Err(e) => {
            world.add_result(TestResult::failed(
                format!("Execute action: {}", action),
                e.to_string(),
                start.elapsed(),
            ));
        }
    }
}

/// Execute an action with JSON arguments
#[when(regex = r#"^I execute the action "([^"]+)" with arguments:$"#)]
async fn execute_action_with_args(world: &mut PluginTestWorld, action: String, args: String) {
    let start = std::time::Instant::now();
    match world.execute_action(&action, &args) {
        Ok(_) => {
            world.add_result(TestResult::passed(
                format!("Execute action: {}", action),
                start.elapsed(),
            ));
        }
        Err(e) => {
            world.add_result(TestResult::failed(
                format!("Execute action: {}", action),
                e.to_string(),
                start.elapsed(),
            ));
        }
    }
}

/// Execute an action with inline JSON arguments
#[when(regex = r#"^I execute "([^"]+)" with '([^']+)'$"#)]
async fn execute_action_inline(world: &mut PluginTestWorld, action: String, args: String) {
    let start = std::time::Instant::now();
    match world.execute_action(&action, &args) {
        Ok(_) => {
            world.add_result(TestResult::passed(
                format!("Execute action: {}", action),
                start.elapsed(),
            ));
        }
        Err(e) => {
            world.add_result(TestResult::failed(
                format!("Execute action: {}", action),
                e.to_string(),
                start.elapsed(),
            ));
        }
    }
}

/// Get plugin info
#[when("I request the plugin info")]
async fn request_plugin_info(world: &mut PluginTestWorld) {
    match world.get_plugin_info() {
        Ok(info) => {
            world.last_response = Some(info);
        }
        Err(e) => {
            world.last_error = Some(e.to_string());
        }
    }
}

/// Wait for a duration
#[when(regex = r#"^I wait for (\d+) milliseconds$"#)]
async fn wait_duration(_world: &mut PluginTestWorld, ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// Shutdown the plugin
#[when("I shutdown the plugin")]
async fn shutdown_plugin(world: &mut PluginTestWorld) {
    match world.shutdown_plugin() {
        Ok(_) => {
            world.add_result(TestResult::passed(
                "Plugin shutdown".to_string(),
                Duration::from_millis(1),
            ));
        }
        Err(e) => {
            world.add_result(TestResult::failed(
                "Plugin shutdown".to_string(),
                e.to_string(),
                Duration::from_millis(1),
            ));
        }
    }
}

// ============================================================================
// Then Steps - Assertions
// ============================================================================

/// Assert response status/success
#[then("the action should succeed")]
async fn action_succeeds(world: &mut PluginTestWorld) {
    assert!(
        world.last_response.is_some() && world.last_error.is_none(),
        "Action should succeed. Error: {:?}",
        world.last_error
    );
}

/// Assert action failure
#[then("the action should fail")]
async fn action_fails(world: &mut PluginTestWorld) {
    assert!(
        world.has_error(),
        "Action should fail. Response: {:?}",
        world.last_response
    );
}

/// Assert response contains text
#[then(regex = r#"^the response should contain "([^"]+)"$"#)]
async fn response_contains(world: &mut PluginTestWorld, expected: String) {
    assert!(
        world.response_contains(&expected),
        "Response should contain '{}'. Actual: {:?}",
        expected,
        world.last_response
    );
}

/// Assert response does not contain text
#[then(regex = r#"^the response should not contain "([^"]+)"$"#)]
async fn response_not_contains(world: &mut PluginTestWorld, unexpected: String) {
    assert!(
        !world.response_contains(&unexpected),
        "Response should not contain '{}'. Actual: {:?}",
        unexpected,
        world.last_response
    );
}

/// Assert response is valid JSON
#[then("the response should be valid JSON")]
async fn response_is_json(world: &mut PluginTestWorld) {
    let result = world.response_as_json();
    assert!(
        result.is_ok(),
        "Response should be valid JSON. Error: {:?}",
        result.err()
    );
}

/// Assert response JSON has a field
#[then(regex = r#"^the response should have field "([^"]+)"$"#)]
async fn response_has_field(world: &mut PluginTestWorld, field: String) {
    let json = world.response_as_json().expect("Response should be JSON");
    assert!(
        json.get(&field).is_some(),
        "Response should have field '{}'. Actual: {:?}",
        field,
        json
    );
}

/// Assert response JSON field equals value
#[then(regex = r#"^the response field "([^"]+)" should equal "([^"]+)"$"#)]
async fn response_field_equals(world: &mut PluginTestWorld, field: String, expected: String) {
    let json = world.response_as_json().expect("Response should be JSON");
    let value = json
        .get(&field)
        .expect(&format!("Field '{}' should exist", field));

    let value_str = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => value.to_string(),
    };

    assert_eq!(
        value_str, expected,
        "Field '{}' should equal '{}'. Actual: '{}'",
        field, expected, value_str
    );
}

/// Assert error message contains text
#[then(regex = r#"^the error should contain "([^"]+)"$"#)]
async fn error_contains(world: &mut PluginTestWorld, expected: String) {
    let error = world.last_error.as_ref().expect("There should be an error");
    assert!(
        error.contains(&expected),
        "Error should contain '{}'. Actual: {}",
        expected,
        error
    );
}

/// Assert plugin info contains name
#[then(regex = r#"^the plugin name should be "([^"]+)"$"#)]
async fn plugin_name_is(world: &mut PluginTestWorld, expected: String) {
    let info = world.get_plugin_info().expect("Should get plugin info");
    assert!(
        info.contains(&expected),
        "Plugin name should be '{}'. Info: {}",
        expected,
        info
    );
}

/// Assert test passed count
#[then(regex = r#"^(\d+) tests? should have passed$"#)]
async fn tests_passed(world: &mut PluginTestWorld, expected: usize) {
    let (passed, _) = world.get_result_summary();
    assert_eq!(
        passed, expected,
        "Expected {} tests to pass, but {} passed",
        expected, passed
    );
}

/// Assert no test failures
#[then("all tests should pass")]
async fn all_tests_pass(world: &mut PluginTestWorld) {
    let (_, failed) = world.get_result_summary();
    assert_eq!(failed, 0, "All tests should pass, but {} failed", failed);

    // Print failures for debugging
    for result in &world.results {
        if result.status == TestStatus::Failed {
            eprintln!("  FAILED: {} - {:?}", result.name, result.error_message);
        }
    }
}

/// Assert logs contain message
#[then(regex = r#"^the logs should contain "([^"]+)" at level "([^"]+)"$"#)]
async fn logs_contain(world: &mut PluginTestWorld, message: String, level: String) {
    assert!(
        world.logs_contain(&level, &message),
        "Logs should contain '{}' at level '{}'. Logs: {:?}",
        message,
        level,
        world.get_logs()
    );
}

/// Store response field in test data
#[then(regex = r#"^I store the response field "([^"]+)" as "([^"]+)"$"#)]
async fn store_response_field(world: &mut PluginTestWorld, field: String, key: String) {
    let json = world.response_as_json().expect("Response should be JSON");
    let value = json
        .get(&field)
        .expect(&format!("Field '{}' should exist", field));

    let value_str = match value {
        serde_json::Value::String(s) => s.clone(),
        _ => value.to_string(),
    };

    world.set_data(&key, &value_str);
}

/// Cleanup test data
#[then("the test data is cleaned up")]
async fn cleanup_test_data(world: &mut PluginTestWorld) {
    world.cleanup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_step_definitions_compile() {
        // This test just verifies that the step definitions compile correctly
        let _world = PluginTestWorld::new();
    }
}
