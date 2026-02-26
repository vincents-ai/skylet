# Example Plugins

This section provides example plugins demonstrating various features and patterns for developing Skylet plugins.

## Available Examples

### Hello Plugin
**Location**: [examples/hello-plugin/](../examples/hello-plugin/)

A minimal plugin demonstrating:
- Basic plugin structure
- Simple request handling
- Health check implementation
- Plugin lifecycle management

**Features**:
- Returns "Hello from Hello Plugin!"
- Demonstrates minimal V2 ABI implementation
- Includes integration tests

**See**: [Hello Plugin README](../examples/hello-plugin/README.md)

---

### Echo Plugin
**Location**: [examples/echo-plugin/](../examples/echo-plugin/)

An advanced plugin showing:
- Request body processing
- String manipulation
- Event handling
- Metrics collection
- Request counter

**Features**:
- Echoes back request body or query string
- Demonstrates body reading from FFI
- Includes event handler implementation
- Tracks request count with metrics
- Includes integration tests

**See**: [Echo Plugin README](../examples/echo-plugin/README.md)

---

### Counter Plugin
**Location**: [examples/counter-plugin/](../examples/counter-plugin/)

A comprehensive plugin demonstrating:
- State management with atomic operations
- Multiple request actions
- Configuration via request body
- Error handling and validation
- Complex request handling

**Features**:
- Atomic counter with increment/decrement
- Maximum value enforcement
- Multiple actions: increment, decrement, reset, get, set
- Error tracking and validation
- Comprehensive integration tests (10 tests)

**See**: [Counter Plugin README](../examples/counter-plugin/README.md)

---

## Running Examples

### Build All Examples

```bash
# Build all example plugins
cargo build --release
```

### Test with Plugin Test Harness

```bash
cd plugin-test-harness

# Build the test harness
cargo build --release

# Test a specific plugin
./target/release/plugin-test-harness test \
  --plugin-path ../examples/counter-plugin/target/release/libcounter_plugin.so

# Run with verbose output
./target/release/plugin-test-harness test \
  --plugin-path ../examples/echo-plugin/target/release/libecho_plugin.so \
  --verbose
```

### Run Integration Tests

Each example plugin includes comprehensive integration tests:

```bash
# Hello plugin tests
cd examples/hello-plugin
cargo test --test integration

# Echo plugin tests
cd ../echo-plugin
cargo test --test integration

# Counter plugin tests
cd ../counter-plugin
cargo test --test integration
```

## Plugin Architecture Patterns

### Pattern 1: Stateless Processing (Hello, Echo)
- No persistent state
- Simple request-response handling
- Ideal for: data transformation, logging, monitoring

### Pattern 2: Stateful Operations (Counter)
- Atomic state management
- Multiple operations on shared state
- Ideal for: counters, caches, aggregators

### Pattern 3: Event-Driven (Echo)
- Event handler implementation
- Asynchronous processing
- Ideal for: event processing, notifications

## Integration Testing

All example plugins include integration tests using the [plugin-test-harness](../plugin-test-harness/):

### Test Coverage

| Plugin | Test Count | Coverage |
|--------|-----------|----------|
| Hello Plugin | 4 | Lifecycle, Request, Health |
| Echo Plugin | 5 | Basic, Body, JSON, Health, Sequence |
| Counter Plugin | 10 | Full feature set with edge cases |

### Test Categories

1. **Lifecycle Tests**: Initialization, shutdown
2. **Functional Tests**: Plugin-specific functionality
3. **Error Handling Tests**: Invalid inputs, error conditions
4. **Health Check Tests**: Health status reporting
5. **Integration Tests**: End-to-end workflows

## Learning Path

We recommend the following learning path for new plugin developers:

1. **Hello Plugin**: Understand basic structure and FFI
2. **Echo Plugin**: Learn request handling and events
3. **Counter Plugin**: Master state management and complex operations

## Contributing Examples

Have a great example plugin? Consider contributing it!

### Submission Guidelines

- Follow the existing project structure
- Include comprehensive integration tests
- Add README with usage examples
- Document all features and APIs
- Follow code style guidelines

### Example Templates

See [Plugin Development Guide](PLUGIN_DEVELOPMENT.md) for:
- Recommended project structure
- Code style guidelines
- Documentation requirements
- Testing best practices

## Additional Resources

- [Plugin Development Guide](PLUGIN_DEVELOPMENT.md) - Complete tutorial
- [API Reference](API_REFERENCE.md) - Detailed API docs
- [Plugin Contract](PLUGIN_CONTRACT.md) - FFI specification
- [Testing Framework](../plugin-test-harness/README.md) - How to write tests
