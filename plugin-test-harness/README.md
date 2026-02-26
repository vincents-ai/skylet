# Plugin Test Harness

Comprehensive testing framework for Skylet plugins with BDD/Cucumber support.

## Features

- **V2 ABI Compatible** - Tests plugins using the V2 ABI specification
- **BDD Testing** - Cucumber/Gherkin support for behavior-driven testing
- **Mock Context** - Isolated testing without the full Skylet engine
- **Multiple Output Formats** - Pretty, JSON, and JUnit XML output
- **Plugin Validation** - Verify plugins meet Skylet requirements

## Installation

Add to your `Cargo.toml`:

```toml
[dev-dependencies]
plugin-test-harness = { path = "../plugin-test-harness" }
```

## CLI Usage

### Test a Plugin

```bash
# Basic test with API checks
plugin-test-harness test --plugin-path ./target/release/libmy_plugin.so

# Verbose output
plugin-test-harness test --plugin-path ./plugin.so --verbose
```

### Run BDD Tests

```bash
# Run all feature files in ./features
plugin-test-harness bdd

# Run specific feature file
plugin-test-harness bdd --feature-path ./features/my_plugin.feature

# With tags
plugin-test-harness bdd --feature-path ./features --tags "@smoke"

# JSON output for CI/CD
plugin-test-harness bdd --feature-path ./features --format json

# JUnit XML for test reporting
plugin-test-harness bdd --feature-path ./features --format junit
```

### Validate a Plugin

```bash
plugin-test-harness validate --plugin-path ./target/release/libmy_plugin.so
```

### Execute Single Action

```bash
plugin-test-harness execute \
  --plugin-path ./plugin.so \
  --action health \
  --args '{}'
```

### Test Suite

Create a `plugin-suite.toml`:

```toml
[[plugins]]
name = "my-plugin"
path = "./target/release/libmy_plugin.so"
timeout_ms = 5000

[[plugins]]
name = "another-plugin"
path = "./target/release/libanother_plugin.so"
```

Run:

```bash
plugin-test-harness suite --config-file ./plugin-suite.toml
```

## Writing BDD Tests

### Feature Files

Create `.feature` files in your `features/` directory:

```gherkin
Feature: My Plugin
  As a plugin user
  I want to verify the plugin works correctly

  @smoke
  Scenario: Health check
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    And the plugin is initialized
    When I execute the action "health"
    Then the action should succeed
    And the response should contain "ok"

  Scenario: Process data
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    When I execute the action "process" with arguments:
      """
      {"data": "test", "count": 5}
      """
    Then the action should succeed
    And the response should be valid JSON
    And the response should have field "result"
```

### Available Steps

#### Given Steps

```gherkin
# Load plugin by path
Given the plugin "./path/to/plugin.so" is loaded

# Load plugin by name (searches common paths)
Given the plugin named "my-plugin" is loaded

# Plugin is initialized
Given the plugin is initialized

# Configure plugin
Given the plugin is configured with:
  """
  database.path = "/tmp/test.db"
  log.level = "debug"
  """

# Set specific config
Given the config "key" is set to "value"

# Set test data
Given the test data "user_id" is "12345"

# Create test file
Given a test file "config.json" with content:
  """
  {"setting": true}
  """
```

#### When Steps

```gherkin
# Execute action
When I execute the action "health"

# Execute with arguments (multiline)
When I execute the action "process" with arguments:
  """
  {"data": "test"}
  """

# Execute with inline arguments
When I execute "echo" with '{"message": "hello"}'

# Get plugin info
When I request the plugin info

# Wait
When I wait for 100 milliseconds

# Shutdown
When I shutdown the plugin
```

#### Then Steps

```gherkin
# Success assertions
Then the action should succeed
Then the action should fail

# Response content
Then the response should contain "ok"
Then the response should not contain "error"
Then the response should be valid JSON
Then the response should have field "id"
Then the response field "status" should equal "ok"

# Error assertions
Then the error should contain "invalid"

# Plugin info
Then the plugin name should be "my-plugin"

# Test results
Then all tests should pass
Then 3 tests should have passed

# Logs
Then the logs should contain "initialized" at level "INFO"

# Store data
Then I store the response field "id" as "item_id"

# Cleanup
Then the test data is cleaned up
```

## Library Usage

```rust
use plugin_test_harness::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create test configuration
    let config = PluginTestConfig {
        plugin_path: "./target/release/libmy_plugin.so".to_string(),
        test_timeout_ms: 5000,
        enable_logging: true,
        ..Default::default()
    };
    
    // Create harness
    let mut harness = PluginTestHarness::new(config);
    
    // Load plugin
    harness.load_plugin().await?;
    
    // Execute actions
    let response = harness.execute_action("health", "{}")?;
    println!("Health: {}", response);
    
    // Run test cases
    let test_cases = vec![
        PluginTestCase {
            name: "Health Check".to_string(),
            action: "health".to_string(),
            args_json: "{}".to_string(),
            expected_success: true,
            expected_response_contains: Some("ok".to_string()),
        },
    ];
    
    let results = harness.test_plugin_api(test_cases).await?;
    
    for result in &results {
        match result.status {
            TestStatus::Passed => println!("[OK] {}", result.name),
            TestStatus::Failed => println!("[FAIL] {}: {:?}", result.name, result.error_message),
        }
    }
    
    Ok(())
}
```

## Mock Context

For unit testing plugins without the full harness:

```rust
use plugin_test_harness::MockPluginContextV2;

#[test]
fn test_plugin_init() {
    let mock_ctx = MockPluginContextV2::new();
    let ctx_ptr = mock_ctx.as_context_ptr();
    
    // Pass ctx_ptr to your plugin's init function
    unsafe {
        let result = plugin_init_v2(ctx_ptr);
        assert_eq!(result, PluginResultV2::Success);
    }
}
```

## Test World for BDD

The `PluginTestWorld` struct maintains state during BDD scenarios:

```rust
use plugin_test_harness::test_world::PluginTestWorld;

// In step definitions
#[given("the plugin is loaded")]
async fn plugin_loaded(world: &mut PluginTestWorld) {
    world.setup("./plugin.so").unwrap();
    world.load_plugin().await.unwrap();
}

#[when("I execute an action")]
async fn execute_action(world: &mut PluginTestWorld) {
    world.execute_action("test", "{}").unwrap();
}

#[then("it should succeed")]
async fn check_success(world: &mut PluginTestWorld) {
    assert!(world.last_response.is_some());
}
```

## Output Formats

### Pretty (Default)

Human-readable output with colors and symbols.

### JSON

```bash
plugin-test-harness bdd --format json > results.json
```

### JUnit XML

```bash
plugin-test-harness bdd --format junit > results.xml
```

## CI/CD Integration

### GitHub Actions

```yaml
- name: Run Plugin Tests
  run: |
    plugin-test-harness bdd \
      --feature-path ./features \
      --format junit > test-results.xml

- name: Publish Test Results
  uses: EnricoMi/publish-unit-test-result-action@v2
  with:
    files: test-results.xml
```

## License

MIT OR Apache-2.0
