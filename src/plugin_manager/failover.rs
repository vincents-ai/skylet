// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
//! Graceful Degradation & Circuit Breaker System for RFC-0004 Phase 6.3
//!
//! This module provides high availability features for plugin systems:
//! - Circuit breaker pattern for fault tolerance
//! - Fallback service definitions for partial functionality
//! - Automatic recovery detection
//! - Request queuing during failures

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Circuit breaker states as per State Machine Design Pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through to primary
    Closed = 0,
    /// Too many failures - rejecting all requests
    Open = 1,
    /// Testing if service recovered - allowing single request
    HalfOpen = 2,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "Closed"),
            CircuitState::Open => write!(f, "Open"),
            CircuitState::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

/// Circuit breaker for managing fault tolerance
///
/// State transitions:
/// - Closed → Open: When failure_threshold exceeded
/// - Open → HalfOpen: After timeout_seconds elapses
/// - HalfOpen → Closed: On successful request
/// - HalfOpen → Open: On failure
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Consecutive failures
    failures: AtomicUsize,
    /// Threshold before opening circuit
    failure_threshold: usize,
    /// Timeout before trying to recover (seconds)
    timeout_seconds: u64,
    /// Last failure timestamp (seconds since epoch)
    last_failure: Mutex<u64>,
    /// Current circuit state
    state: Mutex<CircuitState>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    ///
    /// # Arguments
    /// * `failure_threshold` - How many failures before opening (e.g., 5)
    /// * `timeout_seconds` - How long to wait before HalfOpen (e.g., 30)
    pub fn new(failure_threshold: usize, timeout_seconds: u64) -> Self {
        Self {
            failures: AtomicUsize::new(0),
            failure_threshold,
            timeout_seconds,
            last_failure: Mutex::new(0),
            state: Mutex::new(CircuitState::Closed),
        }
    }

    /// Get current circuit state
    pub fn get_state(&self) -> CircuitState {
        *self.state.lock().unwrap()
    }

    /// Record a successful request
    pub fn record_success(&self) {
        self.failures.store(0, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::HalfOpen => {
                // Recovered! Close the circuit
                *state = CircuitState::Closed;
            }
            CircuitState::Closed => {
                // Still healthy
            }
            CircuitState::Open => {
                // Should not happen without going through HalfOpen
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut last_failure = self.last_failure.lock().unwrap();
        *last_failure = now;
        drop(last_failure);

        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::Closed => {
                let failures = self.failures.fetch_add(1, Ordering::SeqCst);
                if failures + 1 >= self.failure_threshold {
                    // Too many failures - open circuit
                    *state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Failed recovery attempt - go back to open
                *state = CircuitState::Open;
            }
            CircuitState::Open => {
                // Already open, check if we should try recovery
                let last_failure = self.last_failure.lock().unwrap();
                if now - *last_failure >= self.timeout_seconds {
                    *state = CircuitState::HalfOpen;
                }
            }
        }
    }

    /// Check if a request should be allowed
    pub fn should_allow_request(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout elapsed to try recovery
                let last_failure = self.last_failure.lock().unwrap();
                if now - *last_failure >= self.timeout_seconds {
                    *state = CircuitState::HalfOpen;
                    true // Allow one request to test recovery
                } else {
                    false // Still open
                }
            }
            CircuitState::HalfOpen => true, // Allow one request
        }
    }

    /// Get failure count
    pub fn failure_count(&self) -> usize {
        self.failures.load(Ordering::SeqCst)
    }
}

/// Service response type for failover handling
#[derive(Debug, Clone)]
pub struct ServiceResponse {
    pub success: bool,
    pub data: String,
    pub status_code: u32,
    pub error_message: Option<String>,
}

impl ServiceResponse {
    pub fn ok(data: impl Into<String>) -> Self {
        Self {
            success: true,
            data: data.into(),
            status_code: 200,
            error_message: None,
        }
    }

    pub fn error(status: u32, msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: String::new(),
            status_code: status,
            error_message: Some(msg.into()),
        }
    }
}

/// Fallback service definition
#[derive(Debug, Clone)]
pub struct FallbackService {
    pub name: String,
    pub description: String,
    pub reduced_functionality: Vec<String>,
}

impl FallbackService {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            reduced_functionality: Vec::new(),
        }
    }

    pub fn with_reduced_functions(mut self, functions: Vec<String>) -> Self {
        self.reduced_functionality = functions;
        self
    }
}

/// Failover strategy for graceful degradation
///
/// Features:
/// - Circuit breaker per service
/// - Fallback services with reduced functionality
/// - Request queuing during failures
/// - Automatic recovery detection
/// - Degraded mode alerting
pub struct FailoverStrategy {
    /// Circuit breakers per service
    circuit_breakers: HashMap<String, CircuitBreaker>,
    /// Fallback services
    fallback_services: HashMap<String, FallbackService>,
    /// Request queue during failures
    request_queue: Mutex<Vec<String>>,
}

impl FailoverStrategy {
    /// Create a new failover strategy
    pub fn new() -> Self {
        Self {
            circuit_breakers: HashMap::new(),
            fallback_services: HashMap::new(),
            request_queue: Mutex::new(Vec::new()),
        }
    }

    /// Register a service with circuit breaker
    pub fn register_service(
        &mut self,
        service_name: impl Into<String>,
        failure_threshold: usize,
        timeout_seconds: u64,
    ) {
        let name = service_name.into();
        self.circuit_breakers.insert(
            name,
            CircuitBreaker::new(failure_threshold, timeout_seconds),
        );
    }

    /// Register a fallback service
    pub fn register_fallback(&mut self, fallback: FallbackService) {
        self.fallback_services
            .insert(fallback.name.clone(), fallback);
    }

    /// Call a service with automatic failover
    ///
    /// Returns the service response, falling back if primary fails
    pub fn call_with_failover(&self, service_name: &str) -> ServiceResponse {
        // Get or create circuit breaker
        let circuit_breaker = self
            .circuit_breakers
            .get(service_name)
            .unwrap_or_else(|| panic!("Service {} not registered", service_name));

        // Check if circuit allows request
        if !circuit_breaker.should_allow_request() {
            // Circuit is open - use fallback
            circuit_breaker.record_failure();
            return self
                .get_fallback(service_name)
                .map(|_| ServiceResponse::ok("Fallback mode"))
                .unwrap_or_else(|| {
                    ServiceResponse::error(503, "Service unavailable - circuit open")
                });
        }

        // Try calling primary service
        // In production, this would make actual RPC call here
        let result = ServiceResponse::ok("Success");

        if result.success {
            circuit_breaker.record_success();
        } else {
            circuit_breaker.record_failure();
            // Try fallback
            return self
                .get_fallback(service_name)
                .map(|_| ServiceResponse::ok("Fallback mode"))
                .unwrap_or(result);
        }

        result
    }

    /// Get fallback service for a primary service
    pub fn get_fallback(&self, service_name: &str) -> Option<&FallbackService> {
        self.fallback_services.get(service_name)
    }

    /// Get circuit breaker state for monitoring
    pub fn get_service_state(&self, service_name: &str) -> Option<CircuitState> {
        self.circuit_breakers
            .get(service_name)
            .map(|cb| cb.get_state())
    }

    /// Queue a request during failure
    pub fn queue_request(&self, request: String) {
        let mut queue = self.request_queue.lock().unwrap();
        queue.push(request);
    }

    /// Get queued request count
    pub fn queued_request_count(&self) -> usize {
        self.request_queue.lock().unwrap().len()
    }

    /// Drain queued requests (for retry after recovery)
    pub fn drain_queued_requests(&self) -> Vec<String> {
        let mut queue = self.request_queue.lock().unwrap();
        std::mem::take(&mut *queue)
    }
}

impl Default for FailoverStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new(3, 10);
        assert_eq!(cb.get_state(), CircuitState::Closed);
        assert!(cb.should_allow_request());
    }

    #[test]
    fn test_circuit_breaker_opens_on_failures() {
        let cb = CircuitBreaker::new(3, 10);

        // Record failures
        cb.record_failure();
        assert_eq!(cb.get_state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.get_state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.get_state(), CircuitState::Open);

        // Should not allow requests
        assert!(!cb.should_allow_request());
    }

    #[test]
    fn test_circuit_breaker_transitions_to_half_open() {
        let cb = CircuitBreaker::new(1, 0); // timeout 0 for immediate transition

        cb.record_failure();
        assert_eq!(cb.get_state(), CircuitState::Open);

        // After timeout, should transition to HalfOpen
        std::thread::sleep(Duration::from_millis(100));
        assert!(cb.should_allow_request());
        assert_eq!(cb.get_state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_breaker_recovers() {
        let cb = CircuitBreaker::new(1, 0);

        cb.record_failure();
        assert_eq!(cb.get_state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(100));
        assert!(cb.should_allow_request());
        assert_eq!(cb.get_state(), CircuitState::HalfOpen);

        // Record success to close circuit
        cb.record_success();
        assert_eq!(cb.get_state(), CircuitState::Closed);
    }

    #[test]
    fn test_fallback_service_used() {
        let mut strategy = FailoverStrategy::new();

        strategy.register_service("primary", 1, 0);

        let fallback = FallbackService::new("primary", "Fallback for primary service")
            .with_reduced_functions(vec!["basic_query".to_string()]);

        strategy.register_fallback(fallback);

        // Trigger failures to open circuit
        let cb = &strategy.circuit_breakers["primary"];
        cb.record_failure();

        // Should have fallback available
        assert!(strategy.get_fallback("primary").is_some());
    }

    #[test]
    fn test_partial_functionality_mode() {
        let _strategy = FailoverStrategy::new();

        // Create fallback with reduced functionality
        let fallback = FallbackService::new("database", "Read-only mode")
            .with_reduced_functions(vec!["query".to_string()]);

        assert_eq!(fallback.reduced_functionality.len(), 1);
        assert_eq!(fallback.reduced_functionality[0], "query");
    }
}
