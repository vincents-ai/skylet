# Testing Guide

This guide explains how to run, extend, and maintain the Skylet test suite.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Running Tests](#running-tests)
3. [Test Organization](#test-organization)
4. [Writing Tests](#writing-tests)
5. [Test Utilities](#test-utilities)
6. [Continuous Integration](#continuous-integration)
7. [Troubleshooting](#troubleshooting)

## Quick Start

### Running All Tests

```bash
# Run all tests
nix develop --command cargo test

# Run tests with output
nix develop --command cargo test -- --nocapture

# Run tests in release mode
nix develop --command cargo test --release
```

### Running Specific Test Suites

```bash
# Run unit tests only
nix develop --command cargo test --lib

# Run integration tests
nix develop --command cargo test --test '*'

# Run chaos tests
nix develop --command cargo test chaos -- --test-threads=1

# Run E2E tests
nix develop --command cargo test e2e -- --test-threads=1
```

### Running Specific Tests

```bash
# Run a specific test
nix develop --command cargo test test_plugin_loading

# Run tests matching a pattern
nix develop --command cargo test loading

# Run tests in a specific file
nix develop --command cargo test --test plugin_loading
```

## Running Tests

### Test Execution Modes

**Parallel Execution (Default)**
```bash
cargo test
```
Tests run in parallel for speed. Use `--test-threads=1` for sequential execution.

**Sequential Execution**
```bash
cargo test -- --test-threads=1
```
Useful for tests that share resources or have race conditions.

**Verbose Output**
```bash
cargo test -- --nocapture -- --show-output
```
Shows all test output including print statements.

**Filter by Test Name**
```bash
cargo test config
```
Runs all tests with "config" in the name.

### Running Benchmarks

```bash
# Run all benchmarks
nix develop --command cargo bench

# Run specific benchmark
nix develop --command cargo bench plugin_loading

# Run benchmarks with specific criterion options
nix develop --command cargo bench -- -- --save-baseline main
```

### Code Coverage

```bash
# Generate coverage report
nix develop --command cargo tarpaulin --out Xml

# Generate HTML coverage
nix develop --command cargo tarpaulin --out Html

# Exclude files from coverage
nix develop --command cargo tarpaulin --exclude-files 'plugins/*' --out Html
```

### Linting and Formatting

```bash
# Check formatting
nix develop --command cargo fmt -- --check

# Format code
nix develop --command cargo fmt

# Run clippy
nix develop --command cargo clippy -- -D warnings
```

## Test Organization

```
tests/
├── unit/                          # Unit tests
│   ├── config_system.rs            # Configuration system tests
│   ├── event_bus.rs              # Event system tests
│   ├── metrics.rs                # Metrics system tests
│   └── plugin_manager.rs        # Plugin manager tests
├── integration/                   # Integration tests
│   ├── plugin_loading.rs          # Plugin loading tests
│   ├── plugin_communication.rs    # Plugin communication tests
│   └── service_integration.rs     # Service integration tests
├── chaos/                        # Chaos engineering tests
│   ├── fault_injection.rs        # Fault injection tests
│   └── recovery_tests.rs         # Recovery mechanism tests
├── e2e/                          # End-to-end tests
│   └── full_lifecycle.rs         # Full lifecycle validation
├── utils/                        # Test utilities
│   ├── assertions.rs             # Custom assertions
│   ├── fixtures.rs               # Test fixtures
│   ├── mocks.rs                  # Mock implementations
│   ├── mod.rs                   # Utility exports
│   ├── helpers.rs               # Helper functions
│   ├── performance.rs            # Performance utilities
│   └── security.rs              # Security testing utilities
└── data/                         # Test data
    └── plugins/                 # Mock plugins
        └── test-plugin-mock/
```

### Test Types

**Unit Tests** (`tests/unit/`)
- Test individual components in isolation
- Fast execution (typically < 100ms)
- Use mocking for dependencies
- Example: `test_schema_validation_valid_config()`

**Integration Tests** (`tests/integration/`)
- Test multiple components working together
- Medium execution time (100ms - 1s)
- Use real dependencies where possible
- Example: `test_plugin_to_plugin_communication()`

**Chaos Tests** (`tests/chaos/`)
- Test system resilience to failures
- Variable execution time (1s - 10s)
- Simulate real-world failure scenarios
- Example: `test_random_plugin_failures()`

**E2E Tests** (`tests/e2e/`)
- Test complete user workflows
- Longer execution time (1s - 30s)
- Use production-like configurations
- Example: `test_basic_lifecycle()`

## Writing Tests

### Basic Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_something() {
        // Arrange: Set up test
        let manager = PluginManager::new(temp_dir);

        // Act: Execute operation
        let result = manager.load_plugin("test").await;

        // Assert: Verify result
        assert!(result.is_ok());
    }
}
```

### Async Tests

```rust
#[tokio::test]
async fn test_async_operation() {
    let mut manager = PluginManager::new(temp_dir);

    // Use .await for async operations
    let result = manager.load_plugin("test").await;

    assert!(result.is_ok());
}
```

### Using Test Utilities

```rust
use crate::tests::utils::*;

#[tokio::test]
async fn test_with_utilities() {
    // Create temporary directory
    let temp_dir = TestSetup::temp_env();

    // Create test plugin
    let plugin_path = TestPlugin::create_plugin_dir(&temp_dir);

    // Use custom assertions
    assert_plugin_loaded(&manager, "test_plugin").await;

    // Use mock services
    let mock_service = MockDatabase::new();
    mock_service.expect_query().returning(Ok(data));
}
```

### Using Fixtures

```rust
use crate::tests::utils::fixtures::*;

#[tokio::test]
async fn test_with_fixtures() {
    // Load test plugin
    let plugin = FixtureLoader::load_mock_plugin("test_plugin");

    // Use test configuration
    let config = FixtureLoader::load_test_config("test_config");

    // Test with fixture
    let result = manager.load_with_config(&plugin, &config).await;

    assert!(result.is_ok());
}
```

## Test Utilities

### Creating Mock Services

```rust
use mockall::mock;
use crate::plugin_manager::database::DatabaseService;

// Create mock
let mock_db = MockDatabaseService::new();

// Set expectations
mock_db
    .expect_query()
    .with(eq("SELECT * FROM test"))
    .returning(Ok(vec![record]));

// Use in test
let result = manager.load_with_db(mock_db).await;
```

### Custom Assertions

```rust
use crate::tests::utils::assertions::*;

// Assert plugin is loaded
assert_plugin_loaded(&manager, "test_plugin").await;

// Assert plugin is not loaded
assert_plugin_not_loaded(&manager, "test_plugin").await;

// Assert plugin is healthy
assert_plugin_healthy(&manager, "test_plugin").await;
```

### Test Fixtures

```rust
use crate::tests::utils::fixtures::*;

// Load plugin fixture
let plugin = FixtureLoader::load_plugin_from_disk("test_plugin");

// Load config fixture
let config = FixtureLoader::load_config("test_config");

// Load event fixture
let event = FixtureLoader::load_event("test_event");
```

### Performance Testing

```rust
use crate::tests::utils::performance::*;

// Measure execution time
let duration = measure_execution_time(|| {
    manager.load_plugin("test").await
}).await;

assert!(duration < Duration::from_millis(100));

// Measure memory usage
let memory = measure_memory(|| {
    manager.load_plugin("test").await
}).await;

assert!(memory < 1024 * 1024); // 1MB
```

## Continuous Integration

### GitHub Actions

The test suite runs on every push and PR via GitHub Actions:

```yaml
# .github/workflows/test-suite.yml
on:
  push:
    branches: [ main, develop, feature/* ]
  pull_request:
    branches: [ main, develop ]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run tests
        run: nix develop --command cargo test
```

### Quality Gates

Tests must pass before merging:
- ✅ All unit tests pass
- ✅ All integration tests pass
- ✅ Code coverage ≥ 80%
- ✅ No clippy warnings
- ✅ No audit findings
- ✅ No formatting issues

### Coverage Reporting

Coverage is reported to Codecov:

```bash
# Generate coverage
nix develop --command cargo tarpaulin --out Xml

# Upload to Codecov
# (Done automatically in CI)
```

## Troubleshooting

### Test Compilation Failures

**Problem**: Tests don't compile

```bash
error[E0433]: failed to resolve: use of undeclared type
```

**Solution**: Check imports and module structure
```rust
// Add missing import
use crate::plugin_manager::PluginManager;
```

### Test Timeouts

**Problem**: Tests timeout

```bash
test result: TIMEOUT. duration: 60s
```

**Solution**: Reduce test complexity or increase timeout
```rust
#[tokio::test]
async fn test_with_timeout() {
    // Use tokio::time::timeout
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        manager.load_plugin("test")
    ).await;

    assert!(result.is_ok());
}
```

### Flaky Tests

**Problem**: Tests sometimes fail

**Solution**: Add retries or improve isolation
```rust
#[tokio::test]
async fn test_with_retry() {
    let mut attempts = 0;
    loop {
        let result = manager.load_plugin("test").await;

        if result.is_ok() || attempts >= 3 {
            assert!(result.is_ok());
            break;
        }

        attempts += 1;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

### Mock Not Working

**Problem**: Mock expectations not met

```bash
thread 'test_name' panicked at 'mock_db.query(): no expectation set'
```

**Solution**: Verify mock setup
```rust
// Make sure expectations are set before use
mock_db.expect_query().returning(Ok(vec![]));

let result = manager.load_with_db(&mock_db).await;
```

### Resource Cleanup

**Problem**: Resources not cleaned up

**Solution**: Use test lifecycle hooks
```rust
#[tokio::test]
async fn test_with_cleanup() {
    let temp_dir = tempfile::TempDir::new()?;

    // Test code here

    // temp_dir is automatically cleaned up when dropped
}
```

### Parallel Test Conflicts

**Problem**: Tests conflict when run in parallel

```bash`
test suite failed: multiple tests use the same resource
```

**Solution**: Use unique resources per test
```rust
#[tokio::test]
async fn test_with_unique_resources() {
    // Create unique temp directory
    let temp_dir = tempfile::TempDir::new()?;

    let manager = PluginManager::new(temp_dir.path());

    // Test continues...
}
```

## Best Practices

### 1. Test Isolation
- Each test should be independent
- Use fresh resources for each test
- Don't share state between tests

### 2. Clear Test Names
- Names should describe what's being tested
- Use descriptive, specific names
- Example: `test_plugin_loading_with_dependencies()`

### 3. Arrange-Act-Assert Pattern
```rust
#[tokio::test]
async fn test_plugin_loading() {
    // Arrange: Set up test data
    let temp_dir = tempfile::TempDir::new()?;

    // Act: Execute operation
    let manager = PluginManager::new(temp_dir.path());
    let result = manager.load_plugin("test").await;

    // Assert: Verify result
    assert!(result.is_ok());
}
```

### 4. Test Edge Cases
- Test empty inputs
- Test boundary conditions
- Test error conditions
- Example: `test_plugin_loading_with_empty_name()`

### 5. Use Meaningful Assertions
- Don't just assert true
- Use specific assertions
- Example: `assert_eq!(result, expected)`

### 6. Keep Tests Fast
- Unit tests: < 100ms
- Integration tests: < 1s
- E2E tests: < 10s

## Contributing Tests

### Adding New Tests

1. Identify test type (unit/integration/chaos/e2e)
2. Create test file in appropriate directory
3. Write test following conventions
4. Run tests locally
5. Submit PR with tests

### Test Checklist

- [ ] Test compiles
- [ ] Test runs successfully
- [ ] Test is isolated
- [ ] Test has descriptive name
- [ ] Test follows conventions
- [ ] Test covers edge cases
- [ ] Test is documented

## Additional Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Documentation](https://tokio.rs/tokio/test/)
- [Mockall Documentation](https://docs.rs/mockall/latest/mockall/)
- [Criterion Documentation](https://bheisler.github.io/criterion/criterion/)

## Getting Help

### Troubleshooting Issues

Check existing tests for examples:
```bash
grep -r "async fn test_" tests/
```

Review test utilities:
```bash
cat tests/utils/mod.rs
```

Ask questions in issues:
https://github.com/vincents-ai/skylet/issues
