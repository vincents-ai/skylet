# Counter Plugin

A comprehensive example plugin demonstrating:
- State management with atomic operations
- Multiple request actions (increment, decrement, reset, get, set)
- Configuration via request body (max count)
- Error handling and validation
- Health checks
- Metrics collection
- Integration tests

## Features

- **Atomic Counter**: Thread-safe increment/decrement operations
- **Maximum Limit**: Optional maximum value enforcement
- **Error Tracking**: Tracks operation errors
- **Multiple Actions**:
  - `increment`: Increment counter by 1
  - `decrement`: Decrement counter by 1 (saturates at 0)
  - `reset`: Reset counter to 0
  - `get`: Get current counter value, max limit, and error count
  - `set`: Set counter to specific value and optionally set max limit

## Building

```bash
cargo build --release
```

## Testing

### Unit Tests

```bash
cargo test --lib
```

### Integration Tests

```bash
# Build plugin first
cargo build --release

# Run integration tests
cargo test --test integration
```

### Using Plugin Test Harness

```bash
cd ../../plugin-test-harness
cargo build --release

# Test the counter plugin
./target/release/plugin-test-harness test \
  --plugin-path ../examples/counter-plugin/target/release/libcounter_plugin.so
```

## Usage Examples

### Increment Counter

```bash
# Query param: action
plugin-test-harness execute \
  --plugin-path libcounter_plugin.so \
  --action increment \
  --args '{}'
```

Response:
```json
{"value": 1, "action": "incremented"}
```

### Set Counter with Max Limit

```bash
# Body param: JSON value
plugin-test-harness execute \
  --plugin-path libcounter_plugin.so \
  --action set \
  --args '{"value": 10, "max": 15}'
```

Response:
```json
{"value": 10, "action": "set"}
```

### Get Counter Status

```bash
plugin-test-harness execute \
  --plugin-path libcounter_plugin.so \
  --action get \
  --args '{}'
```

Response:
```json
{"value": 10, "max": 15, "errors": 0}
```

### Reset Counter

```bash
plugin-test-harness execute \
  --plugin-path libcounter_plugin.so \
  --action reset \
  --args '{}'
```

Response:
```json
{"value": 0, "action": "reset"}
```

## API Reference

### Actions

| Action | Request Body | Response |
|--------|--------------|----------|
| `increment` | None | `{"value": N, "action": "incremented"}` |
| `decrement` | None | `{"value": N, "action": "decremented"}` |
| `reset` | None | `{"value": 0, "action": "reset"}` |
| `get` | None | `{"value": N, "max": M?, "errors": E}` |
| `set` | `{"value": N, "max": M?}` | `{"value": N, "action": "set"}` |

### Error Responses

- **Unknown Action**: `{"error": "Unknown action: {action}"}`
- **Invalid JSON**: `{"error": "Invalid JSON format"}`
- **Missing Value**: `{"error": "Invalid or missing 'value' field"}`
- **Exceeds Max**: `{"error": "Counter exceeds maximum value of {max}", "value": {max}"}`

## Integration Tests

The plugin includes comprehensive integration tests:

- `test_counter_plugin_initialization`: Tests plugin lifecycle
- `test_counter_increment`: Tests increment functionality
- `test_counter_decrement`: Tests decrement with saturation
- `test_counter_reset`: Tests reset to zero
- `test_counter_set_with_max`: Tests setting with maximum limit
- `test_counter_get`: Tests retrieving counter state
- `test_counter_error_handling`: Tests various error conditions
- `test_counter_health_check`: Tests health status reporting
- `test_counter_sequence`: Tests sequence of operations
- `test_counter_saturated_decrement`: Tests underflow protection

## Implementation Notes

- Uses `lazy_static` for global state initialization
- Atomic operations ensure thread safety
- Mutex protects complex operations
- Validates all inputs
- Tracks errors for metrics

## License

MIT OR Apache-2.0
