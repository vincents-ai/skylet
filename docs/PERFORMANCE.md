# Performance Tuning Guide

This guide covers performance optimization techniques for Skylet plugins and the execution engine.

## Overview

Key performance considerations:

- **FFI Overhead**: Minimizing foreign function interface costs
- **Async/Await**: Efficient async patterns for concurrent workloads
- **Memory Management**: Reducing allocations and improving locality
- **Profiling**: Identifying bottlenecks with profiling tools
- **Benchmarking**: Measuring and comparing performance
- **Resource Pools**: Reusing expensive resources

## Understanding FFI Overhead

### FFI Call Costs

Each FFI (Foreign Function Interface) call between Rust and the Skylet engine has overhead:

```
Native Rust call:     ~5 nanoseconds
FFI call (best case): ~50-100 nanoseconds
FFI call (typical):   ~200-500 nanoseconds
```

Factors affecting FFI performance:

- **Pointer validation**: Engine validates all pointers crossing boundary
- **Context lookup**: Finding service registry and accessing services
- **Error handling**: Checking return codes and translating errors
- **Memory barriers**: CPU instruction barriers for safety

### Strategies to Reduce FFI Overhead

#### 1. Batch Operations

```rust
// SLOW: Multiple FFI calls
for item in items {
    log_message(&format!("Processing item: {}", item))?;  // FFI call per item
}

// FAST: Single batch FFI call
let messages: Vec<String> = items
    .iter()
    .map(|item| format!("Processing item: {}", item))
    .collect();
batch_log_messages(&messages)?;  // Single FFI call ✅
```

#### 2. Minimize Cross-Boundary Data

```rust
// SLOW: Passing large data across FFI
fn process_large_data(data: &[u8]) -> Result<Vec<u8>> {
    let validated = validate_via_ffi(data)?;  // FFI validates all data
    Ok(validated)
}

// FAST: Validate on plugin side
fn process_large_data(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() > MAX_SIZE || !is_valid_utf8(data) {
        return Err(anyhow!("Invalid data"));
    }
    Ok(data.to_vec())  // No FFI call needed ✅
}
```

#### 3. Cache Service References

```rust
// SLOW: Looking up service on every call
fn process_items(items: &[String]) -> Result<()> {
    for item in items {
        let logger = context.service_registry.get_service("logger")?;  // Lookup each time
        logger.log(item)?;
    }
    Ok(())
}

// FAST: Cache service reference
fn process_items(items: &[String]) -> Result<()> {
    let logger = context.service_registry.get_service("logger")?;  // Lookup once ✅
    for item in items {
        logger.log(item)?;
    }
    Ok(())
}
```

## Async/Await Performance

### Effective Async Patterns

#### 1. Concurrent Processing

```rust
use tokio::task;

// SLOW: Sequential processing
async fn process_urls(urls: &[&str]) -> Result<Vec<String>> {
    let mut results = Vec::new();
    for url in urls {
        let response = fetch_url(url).await?;  // Wait for each sequentially
        results.push(response);
    }
    Ok(results)
}

// FAST: Concurrent processing
async fn process_urls(urls: &[&str]) -> Result<Vec<String>> {
    let tasks: Vec<_> = urls
        .iter()
        .map(|url| fetch_url(*url))  // All start concurrently ✅
        .collect();
    
    let results = futures::future::try_join_all(tasks).await?;
    Ok(results)
}
```

#### 2. Use Bounded Concurrency

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

// UNBOUNDED: Can spawn unlimited concurrent tasks
async fn fetch_all(urls: &[&str]) -> Result<Vec<String>> {
    futures::future::try_join_all(
        urls.iter().map(|url| fetch_url(*url))
    ).await
    // Could spawn thousands of concurrent tasks ❌
}

// BOUNDED: Limit concurrent connections
async fn fetch_all(urls: &[&str]) -> Result<Vec<String>> {
    let semaphore = Arc::new(Semaphore::new(10));  // Max 10 concurrent ✅
    
    let tasks: Vec<_> = urls
        .iter()
        .map(|url| {
            let sem = Arc::clone(&semaphore);
            async move {
                let _permit = sem.acquire().await;
                fetch_url(*url).await
            }
        })
        .collect();
    
    futures::future::try_join_all(tasks).await
}
```

#### 3. Avoid Blocking in Async Context

```rust
use std::thread;

// SLOW: Blocking call blocks entire thread pool
async fn process_request(request: Request) -> Result<Response> {
    // This blocks the tokio worker thread!
    thread::sleep(Duration::from_secs(1));  // ❌
    
    Ok(Response::ok())
}

// FAST: Use async sleep
async fn process_request(request: Request) -> Result<Response> {
    tokio::time::sleep(Duration::from_secs(1)).await;  // ✅
    Ok(Response::ok())
}
```

## Memory Optimization

### Reducing Allocations

#### 1. Use String References

```rust
// SLOW: Multiple allocations
fn process_string(input: String) -> Result<String> {
    let uppercase = input.to_uppercase();  // Allocates new String
    let trimmed = uppercase.trim().to_string();  // Another allocation
    Ok(trimmed)
}

// FAST: Use references and stack allocation
fn process_string(input: &str) -> Result<String> {
    input
        .trim()
        .to_uppercase()  // Single allocation at end
        .try_into()
}
```

#### 2. Pool Objects

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

// Reuse expensive objects
struct ConnectionPool {
    connections: Arc<Mutex<Vec<Connection>>>,
    max_size: usize,
}

impl ConnectionPool {
    pub async fn get_connection(&self) -> Result<PooledConnection> {
        let mut conns = self.connections.lock().await;
        if let Some(conn) = conns.pop() {
            Ok(PooledConnection { conn, pool: self })  // ✅ Reuse connection
        } else if conns.len() < self.max_size {
            let conn = Connection::new().await?;
            Ok(PooledConnection { conn, pool: self })
        } else {
            Err(anyhow!("Connection pool exhausted"))
        }
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        // Return connection to pool for reuse
    }
}
```

#### 3. Use Arenas for Batch Processing

```rust
// SLOW: Allocating individually
fn process_items(items: &[Item]) -> Result<Vec<Processed>> {
    items
        .iter()
        .map(|item| {
            let mut processed = Vec::new();
            // ... process ...
            Ok(processed)  // Allocates for each item
        })
        .collect()
}

// FAST: Single arena allocation
fn process_items(items: &[Item]) -> Result<Vec<Processed>> {
    let mut arena = Vec::with_capacity(items.len() * 10);  // Pre-allocate ✅
    
    items
        .iter()
        .map(|item| {
            let start = arena.len();
            // ... process into arena ...
            let end = arena.len();
            Processed(&arena[start..end])
        })
        .collect()
}
```

### Cache Locality

```rust
// SLOW: Poor cache locality
struct Item {
    id: u64,
    name: String,
    data: Vec<u8>,
}

fn process_items(items: &[Item]) {
    for item in items {
        let id = item.id;  // Different cache line
        let name = &item.name;  // Different cache line
        let data = &item.data;  // Different cache line
        // Many cache misses ❌
    }
}

// FAST: Improve data locality
struct ItemRef<'a> {
    id: u64,
    name: &'a str,
    data: &'a [u8],
}

fn process_items(items: &[ItemRef]) {
    for item in items {
        let id = item.id;
        let name = item.name;
        let data = item.data;  // Better cache locality ✅
    }
}
```

## Profiling and Benchmarking

### Setup Flamegraph Profiling

```bash
# Install flamegraph
cargo install flamegraph

# Build release binary
cargo build --release

# Run with profiling
cargo flamegraph --bin my-plugin -o flamegraph.svg

# View results
open flamegraph.svg
```

### Benchmark with Criterion

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn process_benchmark(c: &mut Criterion) {
    c.bench_function("process_10_items", |b| {
        b.iter(|| {
            let items = black_box(vec![1; 10]);
            process_items(&items)
        })
    });
}

criterion_group!(benches, process_benchmark);
criterion_main!(benches);
```

Run benchmarks:

```bash
cargo bench --bench my_benchmark
```

### Profile with perf (Linux)

```bash
# Record performance data
perf record --call-graph=dwarf ./target/release/my-plugin

# View results
perf report

# Generate flamegraph
perf script | stackcollapse-perf.pl | flamegraph.pl > my-perf.svg
```

### Memory Profiling with Valgrind

```bash
# Check for memory leaks
valgrind --leak-check=full --show-leak-kinds=all ./target/release/my-plugin

# Profile allocations
valgrind --tool=massif ./target/release/my-plugin
ms_print massif.out.<pid>
```

## Common Bottlenecks

### 1. String Allocations in Loops

```rust
// SLOW: String allocation in loop
for i in 0..1000 {
    let label = format!("Item {}", i);  // 1000 allocations ❌
    process(label);
}

// FAST: Reuse buffer
use std::fmt::Write;
let mut label = String::with_capacity(20);  // Pre-allocate ✅
for i in 0..1000 {
    label.clear();
    write!(&mut label, "Item {}", i)?;
    process(&label);
}
```

### 2. JSON Serialization

```rust
use serde_json::json;

// SLOW: Multiple serialization passes
let response = json!({
    "items": items,
    "count": items.len(),
    "timestamp": now(),
});
let json_str = response.to_string();  // Allocate string

// FAST: Direct serialization
let response_bytes = serde_json::to_vec(&ResponseData {
    items,
    count: items.len(),
    timestamp: now(),
})?;
```

### 3. Regex Compilation

```rust
use regex::Regex;
use once_cell::sync::Lazy;

// SLOW: Compile regex in loop
fn validate_email(emails: &[&str]) -> bool {
    for email in emails {
        let re = Regex::new(EMAIL_PATTERN)?;  // Compile each time ❌
        if !re.is_match(email) {
            return false;
        }
    }
    true
}

// FAST: Compile once, reuse
static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(EMAIL_PATTERN).unwrap()
});

fn validate_email(emails: &[&str]) -> bool {
    emails.iter().all(|email| EMAIL_RE.is_match(email))  // ✅
}
```

### 4. Lock Contention

```rust
use std::sync::Mutex;

// SLOW: Hold lock too long
fn process_many(items: &[Item]) -> Result<()> {
    let mut shared = SHARED_STATE.lock().unwrap();  // Lock acquired
    for item in items {
        let result = expensive_computation(item)?;  // Still holding lock! ❌
        shared.push(result);
    }
    Ok(())
}

// FAST: Minimize critical section
fn process_many(items: &[Item]) -> Result<()> {
    let results: Vec<_> = items
        .iter()
        .map(|item| expensive_computation(item))
        .collect::<Result<_>>()?;  // No lock held during computation ✅
    
    let mut shared = SHARED_STATE.lock().unwrap();  // Lock only for update
    shared.extend(results);
    Ok(())
}
```

## Configuration Performance Tips

### 1. Cache Configuration Values

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

struct PluginState {
    config: Arc<RwLock<PluginConfig>>,
}

impl PluginState {
    // SLOW: Re-read config on every access
    fn get_timeout_slow(&self) -> Duration {
        let config = self.config.blocking_read();
        config.timeout  // ❌ Acquires lock
    }
    
    // FAST: Cache frequently used values
    fn get_timeout_fast(&self) -> Duration {
        // Cache timeout in thread-local or atomic
        CACHED_TIMEOUT.load(std::sync::atomic::Ordering::Relaxed)  // ✅
    }
}
```

### 2. Lazy Load Large Configuration

```rust
use once_cell::sync::Lazy;

// SLOW: Load entire config on startup
static CONFIG: Lazy<LargeConfig> = Lazy::new(|| {
    LargeConfig::load_from_disk().unwrap()  // Blocks initialization
});

// FAST: Lazy load on first use
static CONFIG: Lazy<LargeConfig> = Lazy::new(|| {
    LargeConfig::load_from_disk().unwrap()  // Only when first accessed ✅
});

// Or load in background
fn init_with_background_load() {
    tokio::spawn(async {
        CONFIG.clone();  // Triggers load in background
    });
}
```

## Performance Checklist

- [ ] Profiled with flamegraph to identify bottlenecks
- [ ] Benchmarked critical paths with criterion
- [ ] Minimized FFI call frequency
- [ ] Batched FFI operations where possible
- [ ] Cached service references
- [ ] Used async/await for concurrent I/O
- [ ] Limited concurrent operations with semaphores
- [ ] Pre-allocated buffers for known sizes
- [ ] Avoided allocations in hot loops
- [ ] Used connection/object pools
- [ ] Compiled regexes once and reused
- [ ] Minimized lock contention
- [ ] Used appropriate data structures
- [ ] Profile memory usage with valgrind

## Performance Targets

Recommended performance targets for Skylet plugins:

| Operation | Target | Notes |
|-----------|--------|-------|
| FFI call | < 1µs | Native call overhead |
| Request processing | < 50ms | P99 latency |
| Memory per connection | < 10MB | For pooled connections |
| Startup time | < 5s | Plugin initialization |
| Config reload | < 1s | Hot reload latency |
| JSON parse (1KB) | < 100µs | Common bottleneck |
| Regex match | < 10µs | Compiled regex |
| Lock acquisition | < 100ns | Uncontended lock |

## References

- [The Rust Book - Performance](https://doc.rust-lang.org/book/ch13-03-performance.html)
- [Flamegraph](https://www.brendangregg.com/flamegraphs.html)
- [Criterion.rs](https://bheisler.github.io/criterion.rs/book/)
- [Perf Tutorial](https://perf.wiki.kernel.org/index.php/Main_Page)

## See Also

- [Security Best Practices](./SECURITY.md)
- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [Configuration Reference](./CONFIG_REFERENCE.md)
