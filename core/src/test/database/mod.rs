// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Database management for test environments
use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

pub struct DatabaseManager {
    temp_dir: TempDir,
}

impl DatabaseManager {
    pub fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        Ok(Self { temp_dir })
    }

    pub fn create_database(&self, name: &str) -> Result<PathBuf> {
        let db_path = self.temp_dir.path().join(format!("{}.db", name));
        Ok(db_path)
    }

    pub fn cleanup(&self) -> Result<()> {
        // Cleanup happens when TempDir is dropped
        Ok(())
    }
}
