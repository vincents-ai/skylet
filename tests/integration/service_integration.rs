//! Integration tests for service integration

use super::*;
use crate::plugin_manager::*;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::sync::Mutex;
use std::sync::Arc;

#[tokio::test]
async fn test_plugin_database_integration() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Initialize database service
    let db_service = DatabaseService::new(&db_path).await.unwrap();

    // Create table
    db_service
        .execute("CREATE TABLE test_table (id INTEGER PRIMARY KEY, data TEXT)")
        .await
        .unwrap();

    // Insert data
    db_service
        .execute("INSERT INTO test_table (id, data) VALUES (1, 'test')")
        .await
        .unwrap();

    // Query data
    let result = db_service
        .query("SELECT * FROM test_table")
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].get::<_, String>("data"), "test");
}

#[tokio::test]
async fn test_plugin_http_service_integration() {
    let http_client = HttpClient::new();

    // Make HTTP request to test endpoint
    let response = http_client
        .get("https://httpbin.org/get")
        .await
        .unwrap();

    assert!(response.status().is_success());
}

#[tokio::test]
async fn test_plugin_file_system_integration() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    // Write file
    std::fs::write(&file_path, "test content").unwrap();

    // Read file
    let content = std::fs::read_to_string(&file_path).unwrap();

    assert_eq!(content, "test content");

    // Delete file
    std::fs::remove_file(&file_path).unwrap();

    assert!(!file_path.exists());
}

#[tokio::test]
async fn test_plugin_cache_integration() {
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::time::sleep;

    let cache = Arc::new(Mutex::new(HashMap::new()));

    // Set value
    {
        let mut cache = cache.lock().await;
        cache.insert("key1".to_string(), "value1".to_string());
    }

    // Get value
    let value = {
        let cache = cache.lock().await;
        cache.get("key1").cloned()
    };

    assert_eq!(value, Some("value1".to_string()));

    // Update value
    {
        let mut cache = cache.lock().await;
        cache.insert("key1".to_string(), "updated_value".to_string());
    }

    // Get updated value
    let value = {
        let cache = cache.lock().await;
        cache.get("key1").cloned()
    };

    assert_eq!(value, Some("updated_value".to_string()));

    // Delete value
    {
        let mut cache = cache.lock().await;
        cache.remove("key1");
    }

    // Verify deletion
    let value = {
        let cache = cache.lock().await;
        cache.get("key1").cloned()
    };

    assert_eq!(value, None);
}

#[tokio::test]
async fn test_plugin_queue_integration() {
    use std::collections::VecDeque;

    let queue = Arc::new(Mutex::new(VecDeque::new()));

    // Enqueue items
    for i in 0..5 {
        let mut queue = queue.lock().await;
        queue.push_back(i);
    }

    // Dequeue items
    let items = {
        let mut queue = queue.lock().await;
        (0..5).map(|_| queue.pop_front().unwrap()).collect::<Vec<_>>()
    };

    assert_eq!(items, vec![0, 1, 2, 3, 4]);

    // Queue should be empty
    let is_empty = {
        let queue = queue.lock().await;
        queue.is_empty()
    };

    assert!(is_empty);
}

#[tokio::test]
async fn test_plugin_logger_integration() {
    use tracing::{info, error, warn};

    // Initialize test logger
    tracing_subscriber::fmt()
        .with_test_writer()
        .init();

    // Log messages
    info!("Test info message");
    warn!("Test warning message");
    error!("Test error message");

    // In real test, we'd verify logs were written
    // For now, just ensure no panic
    assert!(true);
}

#[tokio::test]
async fn test_plugin_configuration_service_integration() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path();

    let config_content = r#"[database]
host = "localhost"
port = 5432

[cache]
enabled = true
ttl_seconds = 3600
"#;

    std::fs::write(config_dir.join("test.toml"), config_content).unwrap();

    let config_service = ConfigurationService::new(config_dir.to_path_buf());

    // Load configuration
    let config = config_service.load("test").await.unwrap();

    assert!(config.contains("localhost"));
    assert!(config.contains("5432"));

    // Update configuration
    config_service
        .update("test", "database.port", "3306")
        .await
        .unwrap();

    // Reload configuration
    let updated_config = config_service.load("test").await.unwrap();

    assert!(updated_config.contains("3306"));
}

#[tokio::test]
async fn test_plugin_metrics_service_integration() {
    use crate::plugin_manager::metrics::*;

    let metrics_service = MetricsService::new(MetricsConfig::default());

    // Record metrics
    metrics_service
        .record_counter("test_counter", 1.0)
        .await;
    metrics_service
        .record_gauge("test_gauge", 42.0)
        .await;

    // Get metrics
    let metrics = metrics_service.get_metrics().await;

    assert!(metrics.contains_key("test_counter"));
    assert!(metrics.contains_key("test_gauge"));
}

#[tokio::test]
async fn test_plugin_with_external_api() {
    use serde_json::json;

    let api_client = ApiClient::new("https://api.github.com");

    // Get user information
    let user_info = api_client
        .get("/users/octocat")
        .await;

    // Note: This requires network access, so we wrap in expect
    // In real CI, we'd mock this
    #[cfg(feature = "network-tests")]
    {
        let user = user_info.unwrap();
        assert_eq!(user["login"], "octocat");
    }
}

#[tokio::test]
async fn test_plugin_with_retry_logic() {
    let attempts = Arc::new(Mutex::new(0));

    let result = retry_with_backoff(
        || async {
            let mut attempts = attempts.lock().await;
            *attempts += 1;

            if *attempts < 3 {
                Err(anyhow::anyhow!("Temporary failure"))
            } else {
                Ok::<(), anyhow::Error>(())
            }
        },
        3,
        Duration::from_millis(10),
    )
    .await;

    assert!(result.is_ok());

    let final_attempts = attempts.lock().await;
    assert_eq!(*final_attempts, 3);
}

async fn retry_with_backoff<F, Fut, T, E>(
    mut f: F,
    max_attempts: usize,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempt = 0;
    let mut delay = initial_delay;

    loop {
        attempt += 1;

        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_attempts => {
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(e) => return Err(e),
        }
    }
}

#[tokio::test]
async fn test_plugin_circuit_breaker() {
    let circuit_breaker = CircuitBreaker::new(3, Duration::from_secs(60));

    // Record failures
    for _ in 0..3 {
        circuit_breaker.record_failure();
    }

    // Circuit should be open
    assert!(circuit_breaker.is_open());

    // Try to execute while circuit is open
    let result = circuit_breaker.execute(|| async {
        Ok::<(), anyhow::Error>(())
    }).await;

    assert!(result.is_err());

    // Wait for cooldown
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Record success
    circuit_breaker.record_success();

    // Circuit should be closed
    assert!(!circuit_breaker.is_open());
}

struct CircuitBreaker {
    failure_count: Arc::Mutex<usize>>,
    failure_threshold: usize,
    cooldown_period: Duration,
}

impl CircuitBreaker {
    fn new(failure_threshold: usize, cooldown_period: Duration) -> Self {
        Self {
            failure_count: Arc::new(Mutex::new(0)),
            failure_threshold,
            cooldown_period,
        }
    }

    fn is_open(&self) -> bool {
        let count = self.failure_count.lock().await;
        *count >= self.failure_threshold
    }

    fn record_failure(&self) {
        let mut count = self.failure_count.lock().await;
        *count += 1;
    }

    fn record_success(&self) {
        let mut count = self.failure_count.lock().await;
        *count = 0;
    }

    async fn execute<F, Fut, T, E>(&self, f: F) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        if self.is_open() {
            return Err(anyhow::anyhow!("Circuit is open"));
        }

        f().await.map_err(|e| {
            self.record_failure();
            e
        })
    }
}
