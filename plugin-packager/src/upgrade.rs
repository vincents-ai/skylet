// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Plugin upgrade functionality
//!
//! This module provides version comparison, upgrade detection, and rollback support.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Semantic version for comparison
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub prerelease: Option<String>,
}

impl SemanticVersion {
    /// Parse version from string (e.g., "1.2.3" or "1.2.3-beta")
    pub fn parse(version_str: &str) -> Result<Self> {
        let (base, prerelease) = if let Some(dash_pos) = version_str.find('-') {
            (
                &version_str[..dash_pos],
                Some(version_str[dash_pos + 1..].to_string()),
            )
        } else {
            (version_str, None)
        };

        let parts: Vec<&str> = base.split('.').collect();
        if parts.len() < 3 {
            anyhow::bail!("Invalid version format: {}", version_str);
        }

        let major = parts[0].parse::<u32>()?;
        let minor = parts[1].parse::<u32>()?;
        let patch = parts[2].parse::<u32>()?;

        Ok(SemanticVersion {
            major,
            minor,
            patch,
            prerelease,
        })
    }

    /// Check if this version is newer than another
    pub fn is_newer_than(&self, other: &SemanticVersion) -> bool {
        if self.major != other.major {
            return self.major > other.major;
        }
        if self.minor != other.minor {
            return self.minor > other.minor;
        }
        if self.patch != other.patch {
            return self.patch > other.patch;
        }

        // Prerelease versions are lower than release versions
        match (&self.prerelease, &other.prerelease) {
            (None, Some(_)) => true,  // release > prerelease
            (Some(_), None) => false, // prerelease < release
            _ => false,               // equal or prerelease comparison
        }
    }

    /// Check if this is a breaking change (major version bump)
    pub fn is_breaking_change(&self, previous: &SemanticVersion) -> bool {
        self.major != previous.major
    }
}

/// Upgrade information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeInfo {
    /// Plugin name
    pub name: String,
    /// Current installed version
    pub current_version: String,
    /// Available newer version
    pub new_version: String,
    /// Whether this is a breaking change
    pub is_breaking: bool,
    /// Available as of this time
    pub available_since: String,
}

impl UpgradeInfo {
    /// Check if upgrade is available from current to new
    pub fn is_available(current: &str, new: &str) -> Result<bool> {
        let current_ver = SemanticVersion::parse(current)?;
        let new_ver = SemanticVersion::parse(new)?;
        Ok(new_ver.is_newer_than(&current_ver))
    }

    /// Create upgrade info
    pub fn new(name: String, current_version: String, new_version: String) -> Result<Self> {
        let current_ver = SemanticVersion::parse(&current_version)?;
        let new_ver = SemanticVersion::parse(&new_version)?;

        Ok(UpgradeInfo {
            name,
            current_version,
            new_version,
            is_breaking: new_ver.is_breaking_change(&current_ver),
            available_since: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }
}

/// Backup record for rollback support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    /// Plugin name
    pub plugin_name: String,
    /// Version that was backed up
    pub version: String,
    /// Backup location
    pub backup_path: PathBuf,
    /// When backup was created
    pub created_at: String,
    /// Whether this backup is valid for rollback
    pub valid: bool,
}

impl BackupRecord {
    /// Create new backup record
    pub fn new(plugin_name: String, version: String, backup_path: PathBuf) -> Self {
        Self {
            plugin_name,
            version,
            backup_path,
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            valid: true,
        }
    }

    /// Mark backup as invalid (e.g., if source is deleted)
    pub fn invalidate(&mut self) {
        self.valid = false;
    }
}

/// Backup manager for plugin upgrades
pub struct BackupManager {
    backup_dir: PathBuf,
    records: Vec<BackupRecord>,
}

impl BackupManager {
    /// Create new backup manager
    pub fn new(backup_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&backup_dir)?;
        Ok(Self {
            backup_dir,
            records: Vec::new(),
        })
    }

    /// Create backup of plugin directory
    pub fn backup_plugin(
        &mut self,
        plugin_name: &str,
        version: &str,
        plugin_path: &Path,
    ) -> Result<PathBuf> {
        if !plugin_path.exists() {
            anyhow::bail!("Plugin directory does not exist: {}", plugin_path.display());
        }

        // Create backup directory structure: backups/plugin-name/version-timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let backup_path = self
            .backup_dir
            .join(format!("{}-{}-{}", plugin_name, version, timestamp));

        fs::create_dir_all(&backup_path)?;

        // Copy plugin directory to backup
        copy_dir_recursive(plugin_path, &backup_path)?;

        // Record backup
        let record = BackupRecord::new(
            plugin_name.to_string(),
            version.to_string(),
            backup_path.clone(),
        );
        self.records.push(record);

        Ok(backup_path)
    }

    /// Restore plugin from backup
    pub fn restore_plugin(&mut self, backup_path: &Path, restore_to: &Path) -> Result<()> {
        if !backup_path.exists() {
            anyhow::bail!("Backup directory does not exist: {}", backup_path.display());
        }

        // Remove current installation
        if restore_to.exists() {
            fs::remove_dir_all(restore_to)?;
        }

        // Copy backup back
        copy_dir_recursive(backup_path, restore_to)?;

        Ok(())
    }

    /// Get all backups for a plugin
    pub fn list_backups(&self, plugin_name: &str) -> Vec<BackupRecord> {
        self.records
            .iter()
            .filter(|r| r.plugin_name == plugin_name && r.valid)
            .cloned()
            .collect()
    }

    /// Remove old backups (keep last N)
    pub fn prune_backups(&mut self, plugin_name: &str, keep_count: usize) -> Result<usize> {
        let mut backups_for_plugin: Vec<usize> = self
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.plugin_name == plugin_name && r.valid)
            .map(|(idx, _)| idx)
            .collect();

        // Sort by created_at descending (newest first) - get indices of records sorted
        backups_for_plugin.sort_by(|&idx_a, &idx_b| {
            self.records[idx_b]
                .created_at
                .cmp(&self.records[idx_a].created_at)
        });

        let mut removed = 0;

        // Remove old backups beyond keep_count
        for idx in backups_for_plugin.iter().skip(keep_count) {
            if self.records[*idx].backup_path.exists() {
                fs::remove_dir_all(&self.records[*idx].backup_path)?;
            }
            self.records[*idx].invalidate();
            removed += 1;
        }

        Ok(removed)
    }

    /// Get number of backups
    pub fn count_backups(&self) -> usize {
        self.records.iter().filter(|r| r.valid).count()
    }
}

/// Helper function to recursively copy directories
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)?;
        }
    }

    Ok(())
}

/// Upgrade result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResult {
    /// Plugin name
    pub plugin_name: String,
    /// Previous version
    pub from_version: String,
    /// New version
    pub to_version: String,
    /// Whether upgrade was successful
    pub success: bool,
    /// Backup path (if backup was created)
    pub backup_path: Option<PathBuf>,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl UpgradeResult {
    /// Create successful upgrade result
    pub fn success(
        plugin_name: String,
        from_version: String,
        to_version: String,
        backup_path: Option<PathBuf>,
    ) -> Self {
        Self {
            plugin_name,
            from_version,
            to_version,
            success: true,
            backup_path,
            error: None,
        }
    }

    /// Create failed upgrade result
    pub fn failure(
        plugin_name: String,
        from_version: String,
        to_version: String,
        error: String,
    ) -> Self {
        Self {
            plugin_name,
            from_version,
            to_version,
            success: false,
            backup_path: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_version_parse() {
        let v = SemanticVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerelease, None);
    }

    #[test]
    fn test_semantic_version_parse_prerelease() {
        let v = SemanticVersion::parse("1.2.3-beta").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.prerelease, Some("beta".to_string()));
    }

    #[test]
    fn test_version_comparison() {
        let v1 = SemanticVersion::parse("1.2.4").unwrap();
        let v2 = SemanticVersion::parse("1.2.3").unwrap();
        assert!(v1.is_newer_than(&v2));
        assert!(!v2.is_newer_than(&v1));
    }

    #[test]
    fn test_breaking_change_detection() {
        let v1 = SemanticVersion::parse("2.0.0").unwrap();
        let v2 = SemanticVersion::parse("1.5.0").unwrap();
        assert!(v1.is_breaking_change(&v2));
    }

    #[test]
    fn test_upgrade_info_creation() {
        let info = UpgradeInfo::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            "1.1.0".to_string(),
        )
        .unwrap();
        assert_eq!(info.name, "test-plugin");
        assert_eq!(info.current_version, "1.0.0");
        assert_eq!(info.new_version, "1.1.0");
        assert!(!info.is_breaking);
    }

    #[test]
    fn test_upgrade_info_breaking_change() {
        let info = UpgradeInfo::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            "2.0.0".to_string(),
        )
        .unwrap();
        assert!(info.is_breaking);
    }

    #[test]
    fn test_backup_record_creation() {
        let record = BackupRecord::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            PathBuf::from("/backups/test-plugin-1.0.0"),
        );
        assert_eq!(record.plugin_name, "test-plugin");
        assert_eq!(record.version, "1.0.0");
        assert!(record.valid);
    }

    #[test]
    fn test_upgrade_result_success() {
        let result = UpgradeResult::success(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            "1.1.0".to_string(),
            None,
        );
        assert!(result.success);
        assert_eq!(result.from_version, "1.0.0");
        assert_eq!(result.to_version, "1.1.0");
    }

    #[test]
    fn test_upgrade_result_failure() {
        let result = UpgradeResult::failure(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            "1.1.0".to_string(),
            "Installation failed".to_string(),
        );
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_backup_manager_creation() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let manager = BackupManager::new(temp_dir.path().to_path_buf())?;
        assert_eq!(manager.count_backups(), 0);
        Ok(())
    }
}
