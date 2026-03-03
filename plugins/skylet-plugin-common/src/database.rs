// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Database abstraction layer for skylet-plugin-common v0.3.0
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};

/// Database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub connection_string: String,
    pub max_connections: u32,
    pub timeout_seconds: u64,
    pub min_connections: u32,
    pub connection_timeout_ms: u64,
    pub idle_timeout_ms: Option<u64>,
    pub max_lifetime_ms: Option<u64>,
}

impl DatabaseConfig {
    pub fn new(connection_string: &str) -> Self {
        Self {
            connection_string: connection_string.to_string(),
            max_connections: 10,
            timeout_seconds: 30,
            min_connections: 1,
            connection_timeout_ms: 5000,
            idle_timeout_ms: Some(300000),  // 5 minutes
            max_lifetime_ms: Some(1800000), // 30 minutes
        }
    }

    pub fn with_max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn with_pool_config(mut self, min: u32, max: u32) -> Self {
        self.min_connections = min;
        self.max_connections = max;
        self
    }
}

/// Database row representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseRow {
    pub columns: HashMap<String, DatabaseValue>,
}

impl DatabaseRow {
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            columns: HashMap::with_capacity(capacity),
        }
    }

    pub fn insert(mut self, column: &str, value: DatabaseValue) -> Self {
        self.columns.insert(column.to_string(), value);
        self
    }

    pub fn get(&self, column: &str) -> Option<&DatabaseValue> {
        self.columns.get(column)
    }

    pub fn get_string(&self, column: &str) -> Option<&String> {
        self.columns.get(column).and_then(|v| match v {
            DatabaseValue::Text(s) => Some(s),
            _ => None,
        })
    }

    pub fn get_i64(&self, column: &str) -> Option<i64> {
        self.columns.get(column).and_then(|v| match v {
            DatabaseValue::Integer(i) => Some(*i),
            _ => None,
        })
    }

    pub fn get_f64(&self, column: &str) -> Option<f64> {
        self.columns.get(column).and_then(|v| match v {
            DatabaseValue::Float(f) => Some(*f),
            DatabaseValue::Integer(i) => Some(*i as f64),
            _ => None,
        })
    }

    pub fn get_bool(&self, column: &str) -> Option<bool> {
        self.columns.get(column).and_then(|v| match v {
            DatabaseValue::Boolean(b) => Some(*b),
            _ => None,
        })
    }

    pub fn get_bytes(&self, column: &str) -> Option<&Vec<u8>> {
        self.columns.get(column).and_then(|v| match v {
            DatabaseValue::Bytes(b) => Some(b),
            _ => None,
        })
    }
}

/// Database value representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DatabaseValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Bytes(Vec<u8>),
    Null,
}

impl DatabaseValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn integer(value: i64) -> Self {
        Self::Integer(value)
    }

    pub fn float(value: f64) -> Self {
        Self::Float(value)
    }

    pub fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    pub fn bytes(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }

    pub fn null() -> Self {
        Self::Null
    }

    pub fn as_sql_literal(&self) -> String {
        match self {
            DatabaseValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
            DatabaseValue::Integer(i) => i.to_string(),
            DatabaseValue::Float(f) => f.to_string(),
            DatabaseValue::Boolean(b) => (if *b { "1" } else { "0" }).to_string(),
            DatabaseValue::Bytes(_) => "'<BINARY_DATA>'".to_string(), // Simplified for now
            DatabaseValue::Null => "NULL".to_string(),
        }
    }
}

/// Database query parameters
pub type QueryParams = Vec<Box<dyn ToSql + Send + Sync>>;

/// Trait for SQL parameter conversion
pub trait ToSql: std::fmt::Debug {
    fn to_sql(&self) -> DatabaseValue;
    fn as_text(&self) -> Option<String> {
        match self.to_sql() {
            DatabaseValue::Text(s) => Some(s),
            _ => None,
        }
    }
}

// Implement ToSql for common types
impl ToSql for String {
    fn to_sql(&self) -> DatabaseValue {
        DatabaseValue::text(self.clone())
    }
}

impl ToSql for &str {
    fn to_sql(&self) -> DatabaseValue {
        DatabaseValue::text(*self)
    }
}

impl ToSql for i64 {
    fn to_sql(&self) -> DatabaseValue {
        DatabaseValue::integer(*self)
    }
}

impl ToSql for i32 {
    fn to_sql(&self) -> DatabaseValue {
        DatabaseValue::integer(*self as i64)
    }
}

impl ToSql for f64 {
    fn to_sql(&self) -> DatabaseValue {
        DatabaseValue::float(*self)
    }
}

impl ToSql for f32 {
    fn to_sql(&self) -> DatabaseValue {
        DatabaseValue::float(*self as f64)
    }
}

impl ToSql for bool {
    fn to_sql(&self) -> DatabaseValue {
        DatabaseValue::boolean(*self)
    }
}

impl ToSql for Vec<u8> {
    fn to_sql(&self) -> DatabaseValue {
        DatabaseValue::bytes(self.clone())
    }
}

impl<T: ToSql> ToSql for Option<T> {
    fn to_sql(&self) -> DatabaseValue {
        match self {
            Some(v) => v.to_sql(),
            None => DatabaseValue::null(),
        }
    }
}

/// Database transaction trait
pub trait DatabaseTransaction {
    fn execute(&mut self, query: &str, params: &[&dyn ToSql]) -> Result<u64, DatabaseError>;
    fn query(
        &mut self,
        query: &str,
        params: &[&dyn ToSql],
    ) -> Result<Vec<DatabaseRow>, DatabaseError>;
    fn query_one(
        &mut self,
        query: &str,
        params: &[&dyn ToSql],
    ) -> Result<Option<DatabaseRow>, DatabaseError>;
    fn commit(self: Box<Self>) -> Result<(), DatabaseError>;
    fn rollback(self: Box<Self>) -> Result<(), DatabaseError>;
}

/// Database connection trait
pub trait DatabaseConnection: Send + Sync {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, DatabaseError>;
    fn query(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<DatabaseRow>, DatabaseError>;
    fn query_one(
        &self,
        query: &str,
        params: &[&dyn ToSql],
    ) -> Result<Option<DatabaseRow>, DatabaseError>;
    fn transaction<F, R>(&self, f: F) -> Result<R, DatabaseError>
    where
        F: FnOnce(&mut dyn DatabaseTransaction) -> Result<R, DatabaseError>;
    fn ping(&self) -> Result<bool, DatabaseError>;
    fn close(&self) -> Result<(), DatabaseError>;
}

/// Database connection pool
pub struct DatabasePool<T: DatabaseConnection> {
    connections: Arc<RwLock<Vec<PooledConnection<T>>>>,
    config: DatabaseConfig,
    max_connections: usize,
    min_connections: usize,
    connection_factory: Box<dyn Fn() -> Result<T, DatabaseError> + Send + Sync>,
    metrics: Arc<Mutex<PoolMetrics>>,
}

/// Pooled connection wrapper
pub struct PooledConnection<T: DatabaseConnection> {
    connection: Option<T>,
    created_at: std::time::Instant,
    last_used: std::time::Instant,
    in_use: bool,
}

/// Pool metrics
#[derive(Debug, Default, Clone)]
pub struct PoolMetrics {
    pub total_created: u64,
    pub total_acquired: u64,
    pub total_released: u64,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub waiting_requests: usize,
}

impl<T: DatabaseConnection> DatabasePool<T> {
    /// Create a new database pool
    pub fn new<F>(config: DatabaseConfig, factory: F) -> Result<Self, DatabaseError>
    where
        F: Fn() -> Result<T, DatabaseError> + Send + Sync + 'static,
    {
        let pool = Self {
            connections: Arc::new(RwLock::new(Vec::new())),
            max_connections: config.max_connections as usize,
            min_connections: config.min_connections as usize,
            connection_factory: Box::new(factory),
            config,
            metrics: Arc::new(Mutex::new(PoolMetrics::default())),
        };

        // Initialize minimum connections
        pool.ensure_min_connections()?;

        Ok(pool)
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<PooledConnection<T>, DatabaseError> {
        // Pool implementation is not yet complete
        // For now, return an error indicating this feature is not available
        Err(DatabaseError::Connection(
            "Connection pooling not yet implemented".to_string(),
        ))
    }

    /// Ensure minimum number of connections
    fn ensure_min_connections(&self) -> Result<(), DatabaseError> {
        // Simplified sync version - in real async version, this would use async
        Ok(())
    }

    /// Get pool metrics
    pub async fn get_metrics(&self) -> PoolMetrics {
        (*self.metrics.lock().await).clone()
    }

    /// Close all connections
    pub async fn close(&self) -> Result<(), DatabaseError> {
        let mut connections = self.connections.write().await;
        for conn in connections.iter_mut() {
            if let Some(ref mut connection) = conn.connection {
                connection.close()?;
            }
        }
        connections.clear();
        Ok(())
    }
}

impl<T: DatabaseConnection> Drop for DatabasePool<T> {
    fn drop(&mut self) {
        // Close all connections when pool is dropped
        // Note: This is a simplified implementation
    }
}

/// Pooled connection implementation
pub struct PooledConnectionHandle<T: DatabaseConnection> {
    index: usize,
    connections: Arc<RwLock<Vec<PooledConnection<T>>>>,
}

impl<T: DatabaseConnection> PooledConnectionHandle<T> {
    fn new(index: usize, connections: Arc<RwLock<Vec<PooledConnection<T>>>>) -> Self {
        Self { index, connections }
    }

    async fn get_connection(&self) -> Option<&T> {
        // Cannot return reference to locked data - this method signature is unsound
        // The connection pool implementation needs to be redesigned
        None
    }

    async fn get_connection_mut(&mut self) -> Option<&mut T> {
        // Cannot return mutable reference to locked data - this method signature is unsound
        // The connection pool implementation needs to be redesigned
        None
    }
}

impl<T: DatabaseConnection> Drop for PooledConnectionHandle<T> {
    fn drop(&mut self) {
        // Mark connection as available when handle is dropped
        // This would need to be async in real implementation
    }
}

impl<T: DatabaseConnection> DatabaseConnection for PooledConnectionHandle<T> {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, DatabaseError> {
        // Simplified sync implementation
        Err(DatabaseError::NotImplemented)
    }

    fn query(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<DatabaseRow>, DatabaseError> {
        // Simplified sync implementation
        Err(DatabaseError::NotImplemented)
    }

    fn query_one(
        &self,
        query: &str,
        params: &[&dyn ToSql],
    ) -> Result<Option<DatabaseRow>, DatabaseError> {
        // Simplified sync implementation
        Err(DatabaseError::NotImplemented)
    }

    fn transaction<F, R>(&self, f: F) -> Result<R, DatabaseError>
    where
        F: FnOnce(&mut dyn DatabaseTransaction) -> Result<R, DatabaseError>,
    {
        // Simplified sync implementation
        Err(DatabaseError::NotImplemented)
    }

    fn ping(&self) -> Result<bool, DatabaseError> {
        // Simplified sync implementation
        Err(DatabaseError::NotImplemented)
    }

    fn close(&self) -> Result<(), DatabaseError> {
        // Simplified sync implementation
        Err(DatabaseError::NotImplemented)
    }
}

/// Database error types
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Pool exhausted")]
    PoolExhausted,

    #[error("Not implemented")]
    NotImplemented,

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Type error: {0}")]
    Type(String),
}

impl DatabaseError {
    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    pub fn query(msg: impl Into<String>) -> Self {
        Self::Query(msg.into())
    }

    pub fn transaction(msg: impl Into<String>) -> Self {
        Self::Transaction(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    pub fn schema(msg: impl Into<String>) -> Self {
        Self::Schema(msg.into())
    }

    pub fn type_error(msg: impl Into<String>) -> Self {
        Self::Type(msg.into())
    }
}

/// Convenience function to create a database config
pub fn create_database_config(connection_string: &str) -> DatabaseConfig {
    DatabaseConfig::new(connection_string)
}

/// Convenience function to create a database pool
pub fn create_database_pool<T: DatabaseConnection, F>(
    connection_string: &str,
    factory: F,
) -> Result<DatabasePool<T>, DatabaseError>
where
    F: Fn() -> Result<T, DatabaseError> + Send + Sync + 'static,
{
    let config = DatabaseConfig::new(connection_string);
    DatabasePool::new(config, factory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_config() {
        let config = DatabaseConfig::new("postgresql://localhost/test")
            .with_max_connections(20)
            .with_timeout(60)
            .with_pool_config(2, 20);

        assert_eq!(config.max_connections, 20);
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.min_connections, 2);
    }

    #[test]
    fn test_database_row() {
        let row = DatabaseRow::new()
            .insert("name", DatabaseValue::text("test"))
            .insert("age", DatabaseValue::integer(25))
            .insert("active", DatabaseValue::boolean(true));

        assert_eq!(row.get_string("name"), Some(&"test".to_string()));
        assert_eq!(row.get_i64("age"), Some(25));
        assert_eq!(row.get_bool("active"), Some(true));
    }

    #[test]
    fn test_database_value() {
        let text_val = DatabaseValue::text("hello");
        let int_val = DatabaseValue::integer(42);
        let bool_val = DatabaseValue::boolean(true);

        assert_eq!(text_val.as_sql_literal(), "'hello'");
        assert_eq!(int_val.as_sql_literal(), "42");
        assert_eq!(bool_val.as_sql_literal(), "1");
    }

    #[test]
    fn test_to_sql_implementations() {
        assert_eq!("hello".to_sql(), DatabaseValue::text("hello"));
        assert_eq!(42i64.to_sql(), DatabaseValue::integer(42));
        assert_eq!(3.14f64.to_sql(), DatabaseValue::float(3.14));
        assert_eq!(true.to_sql(), DatabaseValue::boolean(true));
    }
}
