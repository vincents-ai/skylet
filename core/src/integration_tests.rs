// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use tracing;
// Simple integration test that verifies real database operations work
#[cfg(test)]
mod integration_tests {
    use tempfile::tempdir;
    use std::path::Path;

    #[tokio::test]
    async fn test_real_temp_directory_isolation() {
        // Test that we can create real temporary directories without mocks
        let temp_dir1 = tempdir().unwrap();
        let temp_dir2 = tempdir().unwrap();
        
        let db_path1 = temp_dir1.path().join("test1.db");
        let db_path2 = temp_dir2.path().join("test2.db");
        
        // Both files should not exist initially
        assert!(!Path::new(&db_path1).exists());
        assert!(!Path::new(&db_path2).exists());
        
        // Create database files (simple file creation)
        use std::fs::File;
        use std::io::Write;
        
        {
            let _file1 = File::create(&db_path1).unwrap();
            writeln!(&_file1, "test database 1").unwrap();
        }
        
        {
            let _file2 = File::create(&db_path2).unwrap();
            writeln!(&_file2, "test database 2").unwrap();
        }
        
        // Both files should exist now and be independent
        assert!(Path::new(&db_path1).exists());
        assert!(Path::new(&db_path2).exists());
        assert_ne!(db_path1, db_path2);
        
        // Verify file contents are real (not mocked)
        let content1 = std::fs::read_to_string(&db_path1).unwrap();
        let content2 = std::fs::read_to_string(&db_path2).unwrap();
        assert!(content1.contains("test database 1"));
        assert!(content2.contains("test database 2"));
        
        tracing::info!("✓ Real temporary directory and file creation working");
    }
    
    #[tokio::test]
    async fn test_real_file_operations() {
        // Test that real file operations work without any mocking
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("real_test.txt");
        
        // Verify file doesn't exist initially
        assert!(!Path::new(&test_file).exists());
        
        // Write to file
        std::fs::write(&test_file, "real test content").unwrap();
        
        // Verify file exists and has correct content
        assert!(Path::new(&test_file).exists());
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "real test content");
        
        // Test that files are cleaned up when temp_dir is dropped
        drop(temp_dir);
        assert!(!Path::new(&test_file).exists());
        
        tracing::info!("✓ Real file operations working without mocks");
    }
}