// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct LazyPluginLoader {
    loaded: Arc<AtomicBool>,
    plugin_path: PathBuf,
    load_on_demand: bool,
}

impl LazyPluginLoader {
    pub fn new(plugin_path: PathBuf) -> Self {
        Self {
            loaded: Arc::new(AtomicBool::new(false)),
            plugin_path,
            load_on_demand: true,
        }
    }

    pub fn with_eager_loading(mut self) -> Self {
        self.load_on_demand = false;
        self
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    pub fn mark_loaded(&self) {
        self.loaded.store(true, Ordering::SeqCst);
    }

    pub fn should_load(&self) -> bool {
        self.load_on_demand && !self.is_loaded()
    }

    pub fn plugin_path(&self) -> &PathBuf {
        &self.plugin_path
    }
}

pub struct PluginLoaderRegistry {
    loaders: Arc<RwLock<Vec<LazyPluginLoader>>>,
    parallel_loading: bool,
}

impl PluginLoaderRegistry {
    pub fn new(parallel_loading: bool) -> Self {
        Self {
            loaders: Arc::new(RwLock::new(Vec::new())),
            parallel_loading,
        }
    }

    pub async fn register(&self, loader: LazyPluginLoader) {
        let mut loaders = self.loaders.write().await;
        loaders.push(loader);
    }

    pub async fn load_all(&self) {
        let loaders = self.loaders.read().await.clone();
        
        if self.parallel_loading {
            use tokio::task;
            
            let handles: Vec<_> = loaders
                .iter()
                .filter(|l| !l.is_loaded())
                .map(|loader| {
                    let _path = loader.plugin_path().clone();
                    task::spawn_blocking(move || {
                        // Actual loading logic would go here
                        // For now, just mark as loaded
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    })
                })
                .collect();

            for handle in handles {
                let _ = handle.await;
            }
        } else {
            for loader in &loaders {
                if !loader.is_loaded() {
                    // Synchronous loading
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    loader.mark_loaded();
                }
            }
        }
    }

    pub async fn get_pending_count(&self) -> usize {
        let loaders = self.loaders.read().await;
        loaders.iter().filter(|l| !l.is_loaded()).count()
    }

    pub async fn get_loaded_count(&self) -> usize {
        let loaders = self.loaders.read().await;
        loaders.iter().filter(|l| l.is_loaded()).count()
    }
}

pub struct StartupConfig {
    pub parallel_discovery: bool,
    pub lazy_plugin_loading: bool,
    pub max_concurrent_loads: usize,
    pub discovery_timeout_ms: u64,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            parallel_discovery: true,
            lazy_plugin_loading: true,
            max_concurrent_loads: 4,
            discovery_timeout_ms: 5000,
        }
    }
}

pub struct StartupOptimizer {
    config: StartupConfig,
}

impl StartupOptimizer {
    pub fn new(config: StartupConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(StartupConfig::default())
    }

    pub fn config(&self) -> &StartupConfig {
        &self.config
    }

    pub fn should_parallel_discovery(&self) -> bool {
        self.config.parallel_discovery
    }

    pub fn should_lazy_load(&self) -> bool {
        self.config.lazy_plugin_loading
    }

    pub fn max_concurrent_loads(&self) -> usize {
        self.config.max_concurrent_loads
    }
}

pub struct DelayedInitializer<T> {
    initializer: std::sync::Mutex<Option<Box<dyn FnOnce() -> T + Send + Sync>>>,
    value: Arc<RwLock<Option<T>>>,
    initialized: AtomicBool,
}

impl<T: 'static> DelayedInitializer<T> {
    pub fn new<F>(initializer: F) -> Self
    where
        F: FnOnce() -> T + Send + Sync + 'static,
    {
        Self {
            initializer: std::sync::Mutex::new(Some(Box::new(initializer))),
            value: Arc::new(RwLock::new(None)),
            initialized: AtomicBool::new(false),
        }
    }

    pub async fn get(&self) -> &Arc<RwLock<Option<T>>> {
        if !self.initialized.load(Ordering::SeqCst) {
            if let Some(init) = self.initializer.lock().unwrap().take() {
                let val = init();
                *self.value.write().await = Some(val);
                self.initialized.store(true, Ordering::SeqCst);
            }
        }
        &self.value
    }

    pub async fn is_ready(&self) -> bool {
        if self.initialized.load(Ordering::SeqCst) {
            return true;
        }
        let value = self.value.read().await;
        value.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_lazy_plugin_loader() {
        let loader = LazyPluginLoader::new(PathBuf::from("/tmp/plugin"));
        
        assert!(!loader.is_loaded());
        assert!(loader.should_load());
        
        loader.mark_loaded();
        
        assert!(loader.is_loaded());
        assert!(!loader.should_load());
    }

    #[test]
    fn test_lazy_plugin_loader_eager() {
        let loader = LazyPluginLoader::new(PathBuf::from("/tmp/plugin"))
            .with_eager_loading();
        
        assert!(!loader.should_load());
    }

    #[tokio::test]
    async fn test_plugin_loader_registry() {
        let registry = PluginLoaderRegistry::new(true);
        
        let loader1 = LazyPluginLoader::new(PathBuf::from("/plugin1"));
        let loader2 = LazyPluginLoader::new(PathBuf::from("/plugin2"));
        
        registry.register(loader1).await;
        registry.register(loader2).await;
        
        assert_eq!(registry.get_pending_count().await, 2);
        assert_eq!(registry.get_loaded_count().await, 0);
    }

    #[tokio::test]
    async fn test_delayed_initializer() {
        let init = DelayedInitializer::new(|| 42);
        
        assert!(!init.is_ready().await);
        
        let value_ref = init.get().await;
        let value = value_ref.read().await;
        
        assert!(init.is_ready().await);
        assert_eq!(*value, Some(42));
    }
}
