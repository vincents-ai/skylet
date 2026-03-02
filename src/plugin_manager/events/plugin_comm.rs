// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Request-response pattern for plugin communication
#[derive(Debug, Clone, serde::Serialize)]
pub struct Request<T> {
    pub id: String,
    pub payload: T,
    pub correlation_id: String,
    pub reply_to: String,
    pub timeout_ms: Option<u64>,
}

impl<T> Request<T> {
    pub fn new(payload: T, reply_to: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            payload,
            correlation_id: Uuid::new_v4().to_string(),
            reply_to,
            timeout_ms: None,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

/// Response from a plugin request
#[derive(Debug, Clone)]
pub enum Response<T> {
    Success(T),
    Error(String),
    Timeout(String),
}

impl<T> Response<T> {
    pub fn is_success(&self) -> bool {
        matches!(self, Response::Success(_))
    }

    pub fn into_result(self) -> Result<T, String> {
        match self {
            Response::Success(v) => Ok(v),
            Response::Error(e) | Response::Timeout(e) => Err(e),
        }
    }
}

/// Request-response manager for plugin communication
pub struct RequestResponseManager {
    pending: Arc<RwLock<HashMap<String, (tokio::sync::oneshot::Sender<Response<serde_json::Value>>, std::time::Instant)>>>,
    timeout: std::time::Duration,
}

impl RequestResponseManager {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            timeout: std::time::Duration::from_millis(timeout_ms),
        }
    }

    pub async fn send_request<T: serde::Serialize>(
        &self,
        event_system: Arc<super::EventSystem>,
        target_plugin: String,
        request: T,
    ) -> Result<Response<serde_json::Value>, String> {
        let payload = serde_json::to_value(&request).map_err(|e| e.to_string())?;
        let request_id = Uuid::new_v4().to_string();

        let (sender, receiver) = tokio::sync::oneshot::channel();

        {
            let mut pending = self.pending.write().await;
            pending.insert(request_id.clone(), (sender, std::time::Instant::now()));
        }

        let event = Event::new(
            format!("{}.request", target_plugin),
            "request-manager".to_string(),
            payload,
        )
        .with_metadata(
            EventMetadata::default()
                .with_correlation_id(request_id.clone())
                .with_reply_to(format!("{}.response", target_plugin)),
        );

        event_system.publish(event).await.map_err(|e| e.to_string())?;

        tokio::select! {
            result = receiver => {
                return Ok(result.map_err(|_| "Request canceled".to_string())?);
            }
            _ = tokio::time::sleep(self.timeout) => {
                return Ok(Response::Timeout("Request timed out".to_string()));
            }
        }
    }

    pub async fn handle_response(
        &self,
        response_event: &Event,
    ) -> Result<(), String> {
        let correlation_id = response_event
            .metadata
            .correlation_id
            .as_ref()
            .ok_or("Missing correlation_id")?;

        let mut pending = self.pending.write().await;

        if let Some((sender, _)) = pending.remove(correlation_id) {
            let _ = sender.send(Response::Success(response_event.payload.clone()));
        }

        Ok(())
    }

    pub async fn cleanup_expired(&self) {
        let mut pending = self.pending.write().await;
        let now = std::time::Instant::now();

        let expired: Vec<String> = pending
            .iter()
            .filter(|(_, (_, timestamp))| now.saturating_duration_since(*timestamp) > self.timeout)
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired {
            pending.remove(&id);
        }
    }
}

/// Broadcast manager for sending events to multiple subscribers
pub struct BroadcastManager {
    event_system: Arc<super::EventSystem>,
}

impl BroadcastManager {
    pub fn new(event_system: Arc<super::EventSystem>) -> Self {
        Self { event_system }
    }

    pub async fn broadcast(
        &self,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<usize, String> {
        let event = Event::new(
            event_type.clone(),
            "broadcast-manager".to_string(),
            payload,
        )
        .with_metadata(EventMetadata::default().with_tags(vec!["broadcast".to_string()]));

        let result = self.event_system.publish(event).await.map_err(|e| e.to_string())?;

        match result {
            EventResult::Published { subscriber_count } => Ok(subscriber_count),
            EventResult::Filtered => Ok(0),
        }
    }

    pub async fn multicast(
        &self,
        target_plugins: Vec<String>,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<Vec<Result<usize, String>>, String> {
        let mut results = Vec::new();

        for plugin in target_plugins {
            let event = Event::new(
                format!("{}.{}", event_type, plugin),
                "multicast-manager".to_string(),
                payload.clone(),
            );

            let result = self.event_system.publish(event).await.map_err(|e| e.to_string()).and_then(|r| {
                match r {
                    EventResult::Published { subscriber_count } => Ok(subscriber_count),
                    EventResult::Filtered => Ok(0),
                }
            });
            results.push(result);
        }

        Ok(results)
    }
}

/// Event bus wrapper for easy plugin communication
pub struct EventBus {
    event_system: Arc<super::EventSystem>,
    plugin_name: String,
}

impl EventBus {
    pub fn new(event_system: Arc<super::EventSystem>, plugin_name: String) -> Self {
        Self {
            event_system,
            plugin_name,
        }
    }

    pub async fn publish(&self, event_type: String, payload: serde_json::Value) -> Result<(), String> {
        let event = Event::new(
            event_type,
            self.plugin_name.clone(),
            payload,
        );

        self.event_system
            .publish(event)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn subscribe(
        &self,
        event_types: Vec<String>,
        callback: Arc<dyn EventCallback>,
    ) -> Result<(), String> {
        let subscriber = EventSubscriber::new(
            self.plugin_name.clone(),
            event_types,
            callback,
        );

        self.event_system
            .subscribe(subscriber)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn request<T: serde::Serialize>(
        &self,
        target_plugin: String,
        request: T,
    ) -> Result<Response<serde_json::Value>, String> {
        let manager = RequestResponseManager::new(5000);

        let request_obj = Request::new(request, format!("{}.response", target_plugin));

        let payload = serde_json::to_value(&request_obj).map_err(|e| e.to_string())?;

        let event = Event::new(
            format!("{}.request", target_plugin),
            self.plugin_name.clone(),
            payload,
        );

        self.event_system.publish(event).await.map_err(|e| e.to_string())?;

        manager.send_request(self.event_system.clone(), target_plugin, request_obj).await
    }

    pub fn broadcast_manager(&self) -> BroadcastManager {
        BroadcastManager::new(self.event_system.clone())
    }

    pub fn request_response_manager(&self, timeout_ms: u64) -> RequestResponseManager {
        RequestResponseManager::new(timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_new() {
        let request = Request::new(
            serde_json::json!({"key": "value"}),
            "test.response".to_string(),
        );

        assert!(!request.id.is_empty());
        assert_eq!(request.reply_to, "test.response");
    }

    #[test]
    fn test_request_with_timeout() {
        let request = Request::new(
            serde_json::json!({}),
            "test.response".to_string(),
        )
        .with_timeout(5000);

        assert_eq!(request.timeout_ms, Some(5000));
    }

    #[test]
    fn test_response_success() {
        let response = Response::Success(serde_json::json!({"result": "ok"}));
        assert!(response.is_success());
        assert!(response.into_result().is_ok());
    }

    #[test]
    fn test_response_error() {
        let response: Response<()> = Response::Error("test error".to_string());
        assert!(!response.is_success());
        assert!(response.into_result().is_err());
    }
}
