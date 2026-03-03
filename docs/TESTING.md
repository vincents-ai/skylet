# Testing Guide

This guide explains how to run, extend, and maintain the Skylet test suite.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Running Tests](#running-tests)
3. [Test Organization](#test-organization)
4. [Writing Tests](#writing-tests)
5. [Continuous Integration](#continuous-integration)
6. [Troubleshooting](#troubleshooting)

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

### Running Tests for a Specific Crate

```bash
# Run tests for a single crate
nix develop --command cargo test -p permissions
nix develop --command cargo test -p job-queue
nix develop --command cargo test -p execution-engine

# Run only unit tests (lib tests)
nix develop --command cargo test -p permissions --lib

# Run a specific test by name
nix develop --command cargo test -p permissions -- auth::tests::test_token_refresh
```

### Running Tests Matching a Pattern

```bash
# Run all tests with "config" in the name
nix develop --command cargo test config

# Run all tests in a module
nix develop --command cargo test -p permissions -- auth::tests
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
cargo test -- --nocapture --show-output
```
Shows all test output including print statements.

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

Tests are co-located with the code they test, following Rust's standard `#[cfg(test)]` convention. Each workspace crate contains its own test modules.

### Workspace Crates with Tests

| Crate | Package Name | Test Location |
|-------|-------------|---------------|
| Main binary | `execution-engine` | `src/` (inline `#[cfg(test)]` modules) |
| ABI definitions | `skylet-abi` | `abi/src/` |
| Core framework | `skylet-core` | `core/src/` |
| Permissions | `permissions` | `permissions/src/` |
| Job Queue | `job-queue` | `job-queue/src/` |
| HTTP Router | `http-router` | `http-router/src/` |
| Plugin Packager | `plugin-packager` | `plugin-packager/src/` |
| Config Manager | `config-manager` | `plugins/config-manager/src/` |
| Logging Plugin | `logging` | `plugins/logging/src/` |
| Registry Plugin | `registry` | `plugins/registry/src/` |
| Secrets Manager | `secrets-manager` | `plugins/secrets-manager/src/` |
| Plugin Test Harness | `plugin-test-harness` | `plugin-test-harness/src/` |

### Test Types

**Unit Tests** (inline `#[cfg(test)]` modules)
- Co-located with the code they test
- Fast execution (typically < 100ms)
- Example: `permissions/src/auth.rs` contains `mod tests { ... }`

**Integration Tests** (per-crate)
- Located in `<crate>/src/tests/` or as inline modules
- Test multiple components working together
- Example: `core/src/tests/integration_tests.rs`

**Test Utilities**
- `plugin-test-harness/` — Dedicated workspace crate for plugin testing infrastructure
- `src/testing_comprehensive.rs` — Testing framework for the execution engine
- `src/plugin_test_utils.rs` — Plugin-specific test utilities

## Writing Tests

### Basic Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // Arrange: Set up test data
        let config = AppConfig::default();

        // Act: Execute operation
        let result = config.validate();

        // Assert: Verify result
        assert!(result.is_ok());
    }
}
```

### Async Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_operation() {
        let queue = JobQueue::new(":memory:").await.unwrap();

        let result = queue.push_job("task", b"payload").await;

        assert!(result.is_ok());
    }
}
```

### Test Best Practices

1. **Test Isolation** — Each test should be independent; use fresh resources
2. **Clear Names** — `test_duplicate_password_registration()` not `test_dup()`
3. **Arrange-Act-Assert** — Structure tests in three clear phases
4. **Test Edge Cases** — Empty inputs, boundary conditions, error paths
5. **Keep Tests Fast** — Unit tests < 100ms, integration tests < 1s

## Continuous Integration

### GitHub Actions

The test suite runs on every push and PR:

```yaml
# .github/workflows/ci.yml
on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: cachix/install-nix-action@v27
      - name: Run tests
        run: nix develop --command cargo test
```

### Quality Gates

Tests must pass before merging:
- All unit tests pass
- All integration tests pass
- No clippy warnings
- No formatting issues

## Troubleshooting

### Test Compilation Failures

**Problem**: Tests don't compile

```bash
error[E0433]: failed to resolve: use of undeclared type
```

**Solution**: Check imports and ensure the crate's dependencies include what you need in `[dev-dependencies]`.

### Test Timeouts

**Problem**: Tests timeout

**Solution**: Use `tokio::time::timeout` to bound async operations:
```rust
#[tokio::test]
async fn test_with_timeout() {
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        some_async_operation()
    ).await;

    assert!(result.is_ok());
}
```

### Parallel Test Conflicts

**Problem**: Tests conflict when run in parallel

**Solution**: Use unique resources per test (e.g., separate temp directories, in-memory databases):
```rust
#[tokio::test]
async fn test_with_unique_resources() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    // ...
}
```

## Additional Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Testing](https://tokio.rs/tokio/test/)
