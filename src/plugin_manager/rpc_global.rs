// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Global RPC Registry for Inter-Plugin Communication
//!
//! This module provides a centralized RPC registry that tracks all plugin
//! RPC registries. This enables plugins to call each other's RPC services
//! by name instead of only being able to call their own registered services.

use skylet_abi::v2_spec::PluginResultV2;
use skylet_abi::RpcRegistry;
use std::sync::{Arc, Mutex};

/// Global RPC registry that tracks all plugin RPC registries
pub struct GlobalRpcRegistry {
    registries: Mutex<Vec<(String, Arc<RpcRegistry>)>>,
}

impl GlobalRpcRegistry {
    pub fn new() -> Self {
        Self {
            registries: Mutex::new(Vec::new()),
        }
    }

    /// Register a plugin's RPC registry
    pub fn register_plugin(&self, plugin_name: &str, registry: Arc<RpcRegistry>) {
        let mut registries = self.registries.lock().unwrap();
        eprintln!(
            "[rpc_global] Registering plugin '{}' with RPC registry",
            plugin_name
        );
        registries.push((plugin_name.to_string(), registry));
        eprintln!(
            "[rpc_global] Total registered plugins: {}",
            registries.len()
        );
    }

    /// Call an RPC service across all plugin registries
    pub fn call(&self, service: &str, params: &[u8]) -> Result<Vec<u8>, PluginResultV2> {
        let registries_clone: Vec<(String, Arc<RpcRegistry>)> = {
            let registries = self.registries.lock().unwrap();
            eprintln!(
                "[rpc_global] Looking for service '{}' across {} registries",
                service,
                registries.len()
            );

            for (plugin_name, registry) in registries.iter() {
                let services = registry.list_services();
                eprintln!(
                    "[rpc_global] Plugin '{}' has services: {:?}",
                    plugin_name, services
                );
            }

            registries.clone()
        };

        for (plugin_name, registry) in registries_clone.iter() {
            eprintln!("[rpc_global] Trying registry for plugin '{}'", plugin_name);
            match registry.call(service, params) {
                Ok(bytes) => {
                    eprintln!(
                        "[rpc_global] Found service '{}' in plugin '{}'",
                        service, plugin_name
                    );
                    return Ok(bytes);
                }
                Err(PluginResultV2::ServiceUnavailable) => {
                    eprintln!(
                        "[rpc_global] Service '{}' not found in plugin '{}', trying next",
                        service, plugin_name
                    );
                    continue;
                }
                Err(e) => {
                    eprintln!("[rpc_global] Error from plugin '{}': {:?}", plugin_name, e);
                    return Err(e);
                }
            }
        }

        eprintln!(
            "[rpc_global] Service '{}' not found in any registry",
            service
        );
        Err(PluginResultV2::ServiceUnavailable)
    }
}

// Global instance
lazy_static::lazy_static! {
    static ref GLOBAL_RPC_REGISTRY: GlobalRpcRegistry = GlobalRpcRegistry::new();
}

/// Get the global RPC registry
pub fn get_global_rpc_registry() -> &'static GlobalRpcRegistry {
    &GLOBAL_RPC_REGISTRY
}
