// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Temporary database management for test environments
//!
//! This module provides utilities for creating isolated, temporary SQLite databases
//! for testing purposes. Databases are automatically cleaned up when dropped.
//!
//! # Example
//! ```ignore
//! use framework::database::TemporaryDatabase;
//!
//! let temp_db = TemporaryDatabase::new("my_test").unwrap();
//! let conn_str = temp_db.connection_string(); // "sqlite:/path/to/my_test.db"
//! // Database is cleaned up when temp_db goes out of scope
//! ```

use anyhow::Result;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A temporary SQLite database that is automatically cleaned up on drop.
///
/// The database file is created in a temporary directory that is removed
/// when this struct is dropped, ensuring no test artifacts remain.
pub struct TemporaryDatabase {
    /// The temporary directory containing the database
    temp_dir: TempDir,
    /// Path to the database file
    db_path: PathBuf,
    /// Name of the database
    name: String,
}

impl TemporaryDatabase {
    /// Create a new temporary database with the given name.
    ///
    /// The database file will be created as `{name}.db` in a temporary directory.
    /// The file is touch()ed to ensure it exists on disk.
    ///
    /// # Arguments
    /// * `name` - A name for the database (used in the filename)
    ///
    /// # Returns
    /// A new TemporaryDatabase instance, or an error if creation fails.
    pub fn new(name: &str) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join(format!("{}.db", name));

        // Create the database file
        std::fs::File::create(&db_path)?;

        Ok(Self {
            temp_dir,
            db_path,
            name: name.to_string(),
        })
    }

    /// Create a new temporary database with an initial schema.
    ///
    /// The schema SQL will be executed against the database after creation.
    ///
    /// # Arguments
    /// * `name` - A name for the database
    /// * `schema` - SQL statements to initialize the database
    ///
    /// # Returns
    /// A new TemporaryDatabase with the schema applied.
    pub fn with_schema(name: &str, schema: &str) -> Result<Self> {
        let temp_db = Self::new(name)?;

        // Apply the schema using rusqlite
        let conn = rusqlite::Connection::open(temp_db.path())?;
        conn.execute_batch(schema)?;

        Ok(temp_db)
    }

    /// Get the path to the database file.
    pub fn path(&self) -> &Path {
        &self.db_path
    }

    /// Get a connection string suitable for SQLx or other SQLite clients.
    ///
    /// Returns a string in the format `sqlite:/path/to/database.db`
    pub fn connection_string(&self) -> String {
        format!("sqlite:{}", self.db_path.display())
    }

    /// Get the name of the database.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the directory containing the database.
    pub fn directory(&self) -> &Path {
        self.temp_dir.path()
    }
}

/// A manager for multiple temporary databases.
///
/// Useful when tests need multiple isolated databases that should all
/// be cleaned up together.
pub struct TemporaryDatabaseManager {
    databases: Vec<TemporaryDatabase>,
}

impl TemporaryDatabaseManager {
    /// Create a new empty database manager.
    pub fn new() -> Self {
        Self {
            databases: Vec::new(),
        }
    }

    /// Create a new temporary database and track it for cleanup.
    pub fn create_database(&mut self, name: &str) -> Result<&TemporaryDatabase> {
        let db = TemporaryDatabase::new(name)?;
        self.databases.push(db);
        Ok(self.databases.last().unwrap())
    }

    /// Create a database with schema and track it for cleanup.
    pub fn create_database_with_schema(
        &mut self,
        name: &str,
        schema: &str,
    ) -> Result<&TemporaryDatabase> {
        let db = TemporaryDatabase::with_schema(name, schema)?;
        self.databases.push(db);
        Ok(self.databases.last().unwrap())
    }

    /// Get the number of managed databases.
    pub fn count(&self) -> usize {
        self.databases.len()
    }

    /// Get a database by name.
    pub fn get(&self, name: &str) -> Option<&TemporaryDatabase> {
        self.databases.iter().find(|db| db.name() == name)
    }

    /// Remove and drop a specific database by name.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(pos) = self.databases.iter().position(|db| db.name() == name) {
            self.databases.remove(pos);
            true
        } else {
            false
        }
    }

    /// Clear all databases (they will be dropped and cleaned up).
    pub fn clear(&mut self) {
        self.databases.clear();
    }
}

impl Default for TemporaryDatabaseManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporary_database_exists() {
        let db = TemporaryDatabase::new("test").unwrap();
        assert!(db.path().exists());
    }

    #[test]
    fn test_temporary_database_connection_string_format() {
        let db = TemporaryDatabase::new("mydb").unwrap();
        let conn_str = db.connection_string();
        assert!(conn_str.starts_with("sqlite:"));
        assert!(conn_str.contains("mydb"));
    }

    #[test]
    fn test_temporary_database_manager() {
        let mut manager = TemporaryDatabaseManager::new();

        manager.create_database("db1").unwrap();
        manager.create_database("db2").unwrap();

        assert_eq!(manager.count(), 2);
        assert!(manager.get("db1").is_some());
        assert!(manager.get("db2").is_some());
        assert!(manager.get("db3").is_none());

        manager.remove("db1");
        assert_eq!(manager.count(), 1);
        assert!(manager.get("db1").is_none());
    }
}
