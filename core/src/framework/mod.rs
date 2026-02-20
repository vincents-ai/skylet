// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

// Framework module for setting up test environments

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

pub mod service;
pub mod assertions;
pub mod database;

pub struct TestEnvironment {
    name: String,
    temp_dir: TempDir,
    isolated: bool,
    cleanup_on_drop: bool,
}

impl TestEnvironment {
    pub fn new(name: &str) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        Ok(Self {
            name: name.to_string(),
            temp_dir,
            isolated: true,
            cleanup_on_drop: true,
        })
    }

    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    pub fn is_isolated(&self) -> bool {
        self.isolated
    }

    pub fn set_cleanup_on_drop(&mut self, cleanup: bool) {
        self.cleanup_on_drop = cleanup;
    }

    pub fn max_test_duration_secs(&self) -> u64 {
        300 // 5 minutes default
    }
}

pub struct TestConfiguration {
    pub temp_dir: Option<PathBuf>,
    pub cleanup_on_drop: bool,
    pub max_test_duration_secs: u64,
}

impl Default for TestConfiguration {
    fn default() -> Self {
        Self {
            temp_dir: None,
            cleanup_on_drop: true,
            max_test_duration_secs: 300,
        }
    }
}

pub struct TestFramework {
    environments: Vec<TestEnvironment>,
    current_env: Option<String>,
}

impl TestFramework {
    /// Provide a shared ServiceRegistry for tests/plugins
    pub fn service_registry(&self) -> Arc<crate::service_registry::ServiceRegistry> {
        // In tests we create a fresh registry when needed. For simplicity return a new one.
        Arc::new(crate::service_registry::ServiceRegistry::new())
    }
}

impl TestFramework {
    pub fn new() -> Self {
        Self {
            environments: Vec::new(),
            current_env: None,
        }
    }

    pub fn create_test_environment(&mut self, name: &str) -> Result<String> {
        let env = TestEnvironment::new(name)?;
        let env_name = env.name.clone();
        self.environments.push(env);
        Ok(env_name)
    }

    pub fn get_environment(&self, name: &str) -> Option<&TestEnvironment> {
        self.environments.iter().find(|env| env.name == name)
    }

    pub fn set_current_environment(&mut self, name: &str) {
        self.current_env = Some(name.to_string());
    }

    pub fn cleanup_all(&mut self) -> Result<()> {
        for _env in self.environments.drain(..) {
            // Cleanup happens when TempDir is dropped
        }
        self.current_env = None;
        Ok(())
    }
}

impl Default for TestFramework {
    fn default() -> Self {
        Self::new()
    }
}
