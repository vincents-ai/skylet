// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::v2_spec::PluginResultV2;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// A shared RPC handler: takes request bytes and returns (PluginResultV2, response bytes)
pub type RpcHandler = Arc<dyn Fn(&[u8]) -> (PluginResultV2, Vec<u8>) + Send + Sync + 'static>;

#[derive(Clone, Default)]
pub struct RpcRegistry {
    inner: Arc<Mutex<HashMap<String, RpcEntry>>>,
}

#[derive(Clone)]
pub struct RpcEntry {
    pub interface: Option<String>,
    pub idl: Option<String>,
    pub handler: RpcHandler,
}

impl RpcRegistry {
    pub fn new() -> Self {
        RpcRegistry {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(
        &self,
        name: &str,
        interface: Option<String>,
        idl: Option<String>,
        handler: RpcHandler,
    ) -> PluginResultV2 {
        let mut m = self.inner.lock().unwrap();
        m.insert(
            name.to_string(),
            RpcEntry {
                interface,
                idl,
                handler,
            },
        );
        PluginResultV2::Success
    }

    pub fn unregister(&self, name: &str) -> PluginResultV2 {
        let mut m = self.inner.lock().unwrap();
        if m.remove(name).is_some() {
            PluginResultV2::Success
        } else {
            PluginResultV2::ServiceUnavailable
        }
    }

    pub fn call(&self, name: &str, request: &[u8]) -> Result<Vec<u8>, PluginResultV2> {
        let handler = {
            let m = self.inner.lock().unwrap();
            m.get(name).map(|entry| entry.handler.clone())
        };

        if let Some(handler) = handler {
            let (res, resp) = handler(request);
            match res {
                PluginResultV2::Success => Ok(resp),
                e => Err(e),
            }
        } else {
            Err(PluginResultV2::ServiceUnavailable)
        }
    }

    pub fn get_idl(&self, name: &str) -> Option<String> {
        let m = self.inner.lock().unwrap();
        m.get(name).and_then(|e| e.idl.clone())
    }

    /// List all registered RPC service names
    pub fn list_services(&self) -> Vec<String> {
        let m = self.inner.lock().unwrap();
        m.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_basic() {
        let reg = RpcRegistry::new();
        reg.register(
            "svc::echo",
            None,
            None,
            Arc::new(|req| {
                // echo back
                (PluginResultV2::Success, req.to_vec())
            }),
        );

        let out = reg.call("svc::echo", b"hello");
        assert!(out.is_ok());
        assert_eq!(out.unwrap(), b"hello".to_vec());

        let idl = reg.get_idl("svc::echo");
        assert!(idl.is_none());
    }
}
