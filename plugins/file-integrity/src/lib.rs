// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! File Integrity Monitoring Plugin
//!
//! Computes SHA-256 hashes of files and directories
//! Stores baseline hashes and detects changes
//!
//! Supports: Linux, macOS, Windows

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skylet_abi::v2_spec::{PluginContextV2, PluginInfoV2, PluginResultV2};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHash {
    pub path: String,
    pub hash: String,
    pub size: u64,
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub status: String,
    pub hash: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheckResult {
    pub baseline_path: String,
    pub files: Vec<FileStatus>,
    pub summary: IntegritySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegritySummary {
    pub total_files: usize,
    pub unchanged: usize,
    pub modified: usize,
    pub added: usize,
    pub deleted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineConfig {
    pub paths: Vec<String>,
    pub recursive: bool,
}

static mut BASELINE: Option<HashMap<String, FileHash>> = None;

fn compute_sha256(path: &Path) -> Option<(String, u64, u64)> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let size = metadata.len();
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer).ok()?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = format!("{:x}", hasher.finalize());
    Some((hash, size, modified))
}

fn scan_directory(path: &Path, recursive: bool) -> Vec<FileHash> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some((hash, size, modified)) = compute_sha256(&entry_path) {
                    files.push(FileHash {
                        path: entry_path.to_string_lossy().to_string(),
                        hash,
                        size,
                        modified,
                    });
                }
            } else if recursive && entry_path.is_dir() {
                files.extend(scan_directory(&entry_path, recursive));
            }
        }
    }

    files
}

fn compute_hashes(config: &BaselineConfig) -> HashMap<String, FileHash> {
    let mut hash_map = HashMap::new();

    for path_str in &config.paths {
        let path = Path::new(path_str);
        if path.is_file() {
            if let Some((hash, size, modified)) = compute_sha256(path) {
                let file_hash = FileHash {
                    path: path_str.clone(),
                    hash,
                    size,
                    modified,
                };
                hash_map.insert(path_str.clone(), file_hash);
            }
        } else if path.is_dir() {
            for file_hash in scan_directory(path, config.recursive) {
                hash_map.insert(file_hash.path.clone(), file_hash);
            }
        }
    }

    hash_map
}

fn check_integrity(
    baseline: &HashMap<String, FileHash>,
    config: &BaselineConfig,
) -> IntegrityCheckResult {
    let current_hashes = compute_hashes(config);
    let mut files = Vec::new();
    let mut unchanged = 0;
    let mut modified = 0;
    let mut added = 0;
    let mut deleted = 0;

    for (path, baseline_hash) in baseline {
        if let Some(current_hash) = current_hashes.get(path) {
            if current_hash.hash == baseline_hash.hash {
                files.push(FileStatus {
                    path: path.clone(),
                    status: "unchanged".to_string(),
                    hash: Some(current_hash.hash.clone()),
                    size: Some(current_hash.size),
                });
                unchanged += 1;
            } else {
                files.push(FileStatus {
                    path: path.clone(),
                    status: "modified".to_string(),
                    hash: Some(current_hash.hash.clone()),
                    size: Some(current_hash.size),
                });
                modified += 1;
            }
        } else {
            files.push(FileStatus {
                path: path.clone(),
                status: "deleted".to_string(),
                hash: None,
                size: None,
            });
            deleted += 1;
        }
    }

    for (path, current_hash) in &current_hashes {
        if !baseline.contains_key(path) {
            files.push(FileStatus {
                path: path.clone(),
                status: "added".to_string(),
                hash: Some(current_hash.hash.clone()),
                size: Some(current_hash.size),
            });
            added += 1;
        }
    }

    let summary = IntegritySummary {
        total_files: files.len(),
        unchanged,
        modified,
        added,
        deleted,
    };

    IntegrityCheckResult {
        baseline_path: config.paths.join(","),
        files,
        summary,
    }
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    let config = BaselineConfig {
        paths: vec![],
        recursive: true,
    };

    let baseline = compute_hashes(&config);

    unsafe {
        BASELINE = Some(baseline);
    }

    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    std::ptr::null()
}
