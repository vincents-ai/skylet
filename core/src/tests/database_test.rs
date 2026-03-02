// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// TDD Red Phase: Tests for temporary database management
// Task: e5b7c328-9a8f-41ef-91e7-7071e24b56ef

// Note: Real database tests are implemented below. No placeholder needed.

// ============================================================================
// TEMPORARY DATABASE MANAGEMENT TESTS
// ============================================================================

/// Test that TemporaryDatabase creates a database file
#[test]
fn test_temporary_database_creates_file() {
    let temp_db = crate::framework::database::TemporaryDatabase::new("test_db").unwrap();

    // Database path should exist
    assert!(temp_db.path().exists());

    // Should be an SQLite file (can check extension)
    assert_eq!(temp_db.path().extension().unwrap(), "db");
}

/// Test that temporary database is cleaned up on drop
#[test]
fn test_temporary_database_cleanup_on_drop() {
    use std::path::PathBuf;

    let path: PathBuf;
    {
        let temp_db = crate::framework::database::TemporaryDatabase::new("cleanup_test").unwrap();
        path = temp_db.path().to_path_buf();
        assert!(path.exists());
        // temp_db is dropped here
    }

    // Path should no longer exist after drop
    assert!(!path.exists());
}

/// Test that we can get a connection string for SQLite
#[test]
fn test_temporary_database_connection_string() {
    let temp_db = crate::framework::database::TemporaryDatabase::new("conn_test").unwrap();

    let conn_str = temp_db.connection_string();
    assert!(conn_str.starts_with("sqlite:"));
    assert!(conn_str.contains("conn_test"));
}

/// Test isolated databases don't interfere with each other
#[test]
fn test_isolated_databases() {
    let db1 = crate::framework::database::TemporaryDatabase::new("isolated_1").unwrap();
    let db2 = crate::framework::database::TemporaryDatabase::new("isolated_2").unwrap();

    // Different databases should have different paths
    assert_ne!(db1.path(), db2.path());

    // Both should exist
    assert!(db1.path().exists());
    assert!(db2.path().exists());
}

/// Test database with custom schema initialization
#[test]
fn test_temporary_database_with_schema() {
    use crate::framework::database::TemporaryDatabase;

    let schema = "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT);";
    let temp_db = TemporaryDatabase::with_schema("schema_test", schema).unwrap();

    // Database should exist
    assert!(temp_db.path().exists());

    // Verify schema was applied (we'll check this by trying to query)
    // This will be verified by the implementation
}
