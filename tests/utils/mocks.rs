//! Mock services and dependencies for comprehensive testing

use anyhow::Result;
use async_trait::async_trait;
use mockall::{automock, mock};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use uuid::Uuid;

/// Mock plugin manager for testing
#[automock]
#[async_trait]
pub trait PluginManager: Send + Sync {
    async fn load_plugin(&mut self, name: &str) -> Result<()>;
    async fn unload_plugin(&mut self, name: &str) -> Result<()>;
    async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus>;
    async fn list_plugins(&self) -> Result<Vec<PluginInfo>>;
    async fn reload_plugin(&mut self, name: &str) -> Result<()>;
}

/// Mock configuration service
#[automock]
#[async_trait]
pub trait ConfigService: Send + Sync {
    async fn get_config(&self, key: &str) -> Result<Option<String>>;
    async fn set_config(&mut self, key: &str, value: &str) -> Result<()>;
    async fn load_config(&mut self) -> Result<()>;
    async fn save_config(&self) -> Result<()>;
}

/// Mock event bus
#[automock]
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn subscribe(&self, event_type: &str) -> Result<EventSubscription>;
    async fn publish(&self, event_type: &str, data: &[u8]) -> Result<()>;
    async fn unsubscribe(&self, subscription: EventSubscription) -> Result<()>;
}

/// Mock registry service
#[automock]
#[async_trait]
pub trait RegistryService: Send + Sync {
    async fn register_service(&mut self, service_name: &str, service_info: ServiceInfo) -> Result<()>;
    async fn unregister_service(&mut self, service_name: &str) -> Result<()>;
    async fn lookup_service(&self, service_name: &str) -> Result<Option<ServiceInfo>>;
    async fn list_services(&self) -> Result<Vec<ServiceInfo>>;
}

/// Mock database service
#[automock]
#[async_trait]
pub trait DatabaseService: Send + Sync {
    async fn execute_query(&self, query: &str) -> Result<QueryResult>;
    async fn begin_transaction(&self) -> Result<Transaction>;
    async fn commit_transaction(&self, tx: Transaction) -> Result<()>;
    async fn rollback_transaction(&self, tx: Transaction) -> Result<()>;
}

/// Mock security service
#[automock]
#[async_trait]
pub trait SecurityService: Send + Sync {
    async fn check_permission(&self, plugin_id: &str, resource: &str, action: &str) -> Result<bool>;
    async fn enforce_policy(&self, policy: &SecurityPolicy) -> Result<()>;
    async fn log_security_event(&self, event: SecurityEvent) -> Result<()>;
    async fn validate_plugin_sandbox(&self, plugin_id: &str) -> Result<bool>;
}

/// Plugin status for testing
#[derive(Debug, Clone)]
pub struct PluginStatus {
    pub name: String,
    pub status: String,
    pub loaded_at: String,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

/// Plugin information for testing
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub status: String,
    pub dependencies: Vec<String>,
}

/// Event subscription for testing
#[derive(Debug, Clone)]
pub struct EventSubscription {
    pub id: Uuid,
    pub event_type: String,
}

/// Service information for testing
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub endpoint: String,
    pub version: String,
    pub metadata: HashMap<String, String>,
}

/// Query result for testing
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Transaction handle for testing
#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: Uuid,
}

/// Security policy for testing
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub name: String,
    pub permissions: Vec<Permission>,
    pub resource_limits: ResourceLimits,
}

/// Permission for testing
#[derive(Debug, Clone)]
pub struct Permission {
    pub resource: String,
    pub action: String,
    pub conditions: HashMap<String, String>,
}

/// Resource limits for testing
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_mb: usize,
    pub max_cpu_percent: f64,
    pub max_file_handles: usize,
    pub max_network_connections: usize,
}

/// Security event for testing
#[derive(Debug, Clone)]
pub struct SecurityEvent {
    pub event_type: String,
    pub plugin_id: String,
    pub timestamp: String,
    pub details: HashMap<String, String>,
}

/// In-memory implementation of mock plugin manager
pub struct MockPluginManager {
    plugins: Arc<RwLock<HashMap<String, PluginStatus>>>,
}

impl MockPluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_plugin(&self, name: &str, status: &str) {
        let mut plugins = self.plugins.write().await;
        plugins.insert(
            name.to_string(),
            PluginStatus {
                name: name.to_string(),
                status: status.to_string(),
                loaded_at: chrono::Utc::now().to_rfc3339(),
                memory_usage_mb: 10.0,
                cpu_usage_percent: 5.0,
            },
        );
    }

    pub async fn remove_plugin(&self, name: &str) {
        let mut plugins = self.plugins.write().await;
        plugins.remove(name);
    }
}

#[async_trait]
impl PluginManager for MockPluginManager {
    async fn load_plugin(&mut self, name: &str) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        plugins.insert(
            name.to_string(),
            PluginStatus {
                name: name.to_string(),
                status: "loaded".to_string(),
                loaded_at: chrono::Utc::now().to_rfc3339(),
                memory_usage_mb: 10.0,
                cpu_usage_percent: 5.0,
            },
        );
        Ok(())
    }

    async fn unload_plugin(&mut self, name: &str) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        plugins.remove(name);
        Ok(())
    }

    async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus> {
        let plugins = self.plugins.read().await;
        plugins.get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Plugin not found: {}", name))
    }

    async fn list_plugins(&self) -> Result<Vec<PluginInfo>> {
        let plugins = self.plugins.read().await;
        let mut plugin_list = Vec::new();
        
        for (name, status) in plugins.iter() {
            plugin_list.push(PluginInfo {
                name: name.clone(),
                version: "0.1.0".to_string(),
                status: status.status.clone(),
                dependencies: vec![],
            });
        }
        
        Ok(plugin_list)
    }

    async fn reload_plugin(&mut self, name: &str) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        if let Some(plugin) = plugins.get_mut(name) {
            plugin.status = "reloaded".to_string();
            plugin.loaded_at = chrono::Utc::now().to_rfc3339();
        }
        Ok(())
    }
}

/// In-memory implementation of mock config service
pub struct MockConfigService {
    config: Arc<RwLock<HashMap<String, String>>>,
}

impl MockConfigService {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_test_config(&self, key: &str, value: &str) {
        let mut config = self.config.write().await;
        config.insert(key.to_string(), value.to_string());
    }

    pub async fn get_test_config(&self, key: &str) -> Option<String> {
        let config = self.config.read().await;
        config.get(key).cloned()
    }
}

#[async_trait]
impl ConfigService for MockConfigService {
    async fn get_config(&self, key: &str) -> Result<Option<String>> {
        let config = self.config.read().await;
        Ok(config.get(key).cloned())
    }

    async fn set_config(&mut self, key: &str, value: &str) -> Result<()> {
        let mut config = self.config.write().await;
        config.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn load_config(&mut self) -> Result<()> {
        // Simulate loading config
        let mut config = self.config.write().await;
        config.insert("test.key".to_string(), "test.value".to_string());
        Ok(())
    }

    async fn save_config(&self) -> Result<()> {
        // Simulate saving config
        Ok(())
    }
}

/// In-memory implementation of mock event bus
pub struct MockEventBus {
    subscribers: Arc<RwLock<HashMap<String, Vec<EventSubscription>>>>,
    events: Arc<RwLock<Vec<(String, Vec<u8>)>>>,
}

impl MockEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn get_published_events(&self) -> Vec<(String, Vec<u8>)> {
        self.events.read().await.clone()
    }

    pub async fn clear_events(&self) {
        let mut events = self.events.write().await;
        events.clear();
    }
}

#[async_trait]
impl EventBus for MockEventBus {
    async fn subscribe(&self, event_type: &str) -> Result<EventSubscription> {
        let subscription = EventSubscription {
            id: Uuid::new_v4(),
            event_type: event_type.to_string(),
        };

        let mut subscribers = self.subscribers.write().await;
        subscribers
            .entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(subscription.clone());

        Ok(subscription)
    }

    async fn publish(&self, event_type: &str, data: &[u8]) -> Result<()> {
        let event_data = data.to_vec();
        
        // Store the event for verification
        let mut events = self.events.write().await;
        events.push((event_type.to_string(), event_data));

        // Notify subscribers (simplified for testing)
        let subscribers = self.subscribers.read().await;
        if let Some(subs) = subscribers.get(event_type) {
            // In a real implementation, we would send to each subscriber
            tracing::info!("Published event '{}' to {} subscribers", event_type, subs.len());
        }

        Ok(())
    }

    async fn unsubscribe(&self, subscription: EventSubscription) -> Result<()> {
        let mut subscribers = self.subscribers.write().await;
        
        if let Some(subs) = subscribers.get_mut(&subscription.event_type) {
            subs.retain(|sub| sub.id != subscription.id);
        }

        Ok(())
    }
}

/// Test helper for setting up mock services
pub struct MockServiceFactory;

impl MockServiceFactory {
    pub fn create_plugin_manager() -> MockPluginManager {
        MockPluginManager::new()
    }

    pub fn create_config_service() -> MockConfigService {
        MockConfigService::new()
    }

    pub fn create_event_bus() -> MockEventBus {
        MockEventBus::new()
    }

    pub fn create_plugin_manager_with_plugins() -> MockPluginManager {
        let manager = MockPluginManager::new();
        
        // Add some test plugins
        tokio::spawn({
            let manager = manager.clone();
            async move {
                manager.add_plugin("test-plugin-1", "loaded").await;
                manager.add_plugin("test-plugin-2", "loaded").await;
                manager.add_plugin("test-plugin-3", "failed").await;
            }
        });

        manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_plugin_manager() {
        let mut manager = MockPluginManager::new();
        
        // Test loading plugins
        manager.load_plugin("test-plugin").await.unwrap();
        let status = manager.get_plugin_status("test-plugin").await.unwrap();
        assert_eq!(status.status, "loaded");
        
        // Test listing plugins
        let plugins = manager.list_plugins().await.unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "test-plugin");
        
        // Test reloading plugin
        manager.reload_plugin("test-plugin").await.unwrap();
        let reloaded_status = manager.get_plugin_status("test-plugin").await.unwrap();
        assert_eq!(reloaded_status.status, "reloaded");
        
        // Test unloading plugin
        manager.unload_plugin("test-plugin").await.unwrap();
        let unloaded_status = manager.get_plugin_status("test-plugin");
        assert!(unloaded_status.is_err());
    }

    #[tokio::test]
    async fn test_mock_config_service() {
        let mut service = MockConfigService::new();
        
        // Test setting and getting config
        service.set_config("test.key", "test.value").await.unwrap();
        let value = service.get_config("test.key").await.unwrap();
        assert_eq!(value, Some("test.value".to_string()));
        
        // Test loading config
        service.load_config().await.unwrap();
        let loaded_value = service.get_config("test.key").await.unwrap();
        assert_eq!(loaded_value, Some("test.value".to_string()));
    }

    #[tokio::test]
    async fn test_mock_event_bus() {
        let event_bus = MockEventBus::new();
        
        // Subscribe to events
        let subscription = event_bus.subscribe("test.event").await.unwrap();
        
        // Publish event
        let event_data = b"test event data";
        event_bus.publish("test.event", event_data).await.unwrap();
        
        // Verify event was published
        let events = event_bus.get_published_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "test.event");
        assert_eq!(events[0].1, event_data);
        
        // Unsubscribe
        event_bus.unsubscribe(subscription).await.unwrap();
    }

    #[tokio::test]
    async fn test_mock_service_factory() {
        let manager = MockServiceFactory::create_plugin_manager();
        let config_service = MockServiceFactory::create_config_service();
        let event_bus = MockServiceFactory::create_event_bus();
        
        assert!(manager.get_plugin_status("nonexistent").await.is_err());
        assert!(config_service.get_config("nonexistent").await.unwrap().is_none());
        assert_eq!(event_bus.get_published_events().len(), 0);
    }

    #[tokio::test]
    async fn test_mock_plugin_manager_with_plugins() {
        let manager = MockServiceFactory::create_plugin_manager_with_plugins();
        
        // Give it a moment to add plugins
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let plugins = manager.list_plugins().await.unwrap();
        assert_eq!(plugins.len(), 3);
        
        let plugin_names: Vec<String> = plugins.iter().map(|p| p.name.clone()).collect();
        assert!(plugin_names.contains(&"test-plugin-1".to_string()));
        assert!(plugin_names.contains(&"test-plugin-2".to_string()));
        assert!(plugin_names.contains(&"test-plugin-3".to_string()));
    }
}