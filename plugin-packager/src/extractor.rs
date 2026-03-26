// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plugin artifact extraction with security checks
//!
//! This module provides secure extraction of plugin .tar.gz artifacts with:
//! - Path traversal prevention
//! - Symlink handling
//! - Permission preservation
//! - Size limits
//! - File type validation

use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

/// Maximum allowed extracted file size (100 MB)
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum allowed total extracted size (1 GB)
const MAX_TOTAL_SIZE: u64 = 1024 * 1024 * 1024;

/// Maximum allowed path length
const MAX_PATH_LENGTH: usize = 4096;

/// Maximum allowed number of entries in archive
const MAX_ENTRIES: usize = 10000;

/// Extraction configuration options
#[derive(Debug, Clone)]
pub struct ExtractorConfig {
    /// Maximum single file size in bytes
    pub max_file_size: u64,
    /// Maximum total extracted size in bytes
    pub max_total_size: u64,
    /// Maximum path length
    pub max_path_length: usize,
    /// Maximum number of entries
    pub max_entries: usize,
    /// Allow symlinks (dangerous if from untrusted sources)
    pub allow_symlinks: bool,
    /// Allow absolute paths (usually dangerous)
    pub allow_absolute_paths: bool,
    /// Set executable permissions on .so/.dll/.dylib files
    pub set_executable: bool,
    /// Overwrite existing files
    pub overwrite: bool,
}

impl Default for ExtractorConfig {
    fn default() -> Self {
        ExtractorConfig {
            max_file_size: MAX_FILE_SIZE,
            max_total_size: MAX_TOTAL_SIZE,
            max_path_length: MAX_PATH_LENGTH,
            max_entries: MAX_ENTRIES,
            allow_symlinks: false,
            allow_absolute_paths: false,
            set_executable: true,
            overwrite: false,
        }
    }
}

impl ExtractorConfig {
    /// Create a new config with secure defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a permissive config for trusted sources
    pub fn permissive() -> Self {
        ExtractorConfig {
            max_file_size: MAX_FILE_SIZE * 10,
            max_total_size: MAX_TOTAL_SIZE * 10,
            max_path_length: MAX_PATH_LENGTH,
            max_entries: MAX_ENTRIES * 10,
            allow_symlinks: true,
            allow_absolute_paths: false, // Still dangerous
            set_executable: true,
            overwrite: true,
        }
    }
}

/// Extraction result with statistics
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// Path to the extracted plugin directory
    pub plugin_dir: PathBuf,
    /// Number of files extracted
    pub files_extracted: usize,
    /// Number of directories created
    pub directories_created: usize,
    /// Total bytes extracted
    pub total_bytes: u64,
    /// Plugin name from manifest
    pub plugin_name: String,
    /// Plugin version from manifest
    pub plugin_version: String,
}

/// Plugin artifact extractor
pub struct PluginExtractor {
    config: ExtractorConfig,
}

impl PluginExtractor {
    /// Create a new extractor with the given configuration
    pub fn new(config: ExtractorConfig) -> Self {
        PluginExtractor { config }
    }

    /// Create an extractor with secure default settings
    pub fn secure() -> Self {
        PluginExtractor::new(ExtractorConfig::new())
    }

    /// Extract a plugin artifact to the specified directory
    ///
    /// # Arguments
    /// * `artifact_path` - Path to the .tar.gz artifact
    /// * `dest_dir` - Destination directory for extraction
    ///
    /// # Returns
    /// The path to the extracted plugin directory (inside dest_dir)
    pub fn extract(&self, artifact_path: &Path, dest_dir: &Path) -> Result<ExtractionResult> {
        // Validate artifact exists
        if !artifact_path.exists() {
            bail!("Artifact not found: {}", artifact_path.display());
        }

        // Create destination if it doesn't exist
        fs::create_dir_all(dest_dir)
            .with_context(|| format!("Creating destination directory {}", dest_dir.display()))?;

        // Open and decompress
        let file = File::open(artifact_path)
            .with_context(|| format!("Opening artifact {}", artifact_path.display()))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        let mut files_extracted = 0;
        let mut directories_created = 0;
        let mut total_bytes = 0u64;
        let mut plugin_name = String::new();
        let mut plugin_version = String::new();
        let mut root_dir: Option<PathBuf> = None;
        let mut entry_count = 0;

        // Process each entry
        for entry_result in archive.entries()? {
            let mut entry = entry_result?;
            entry_count += 1;

            if entry_count > self.config.max_entries {
                bail!(
                    "Archive contains too many entries (max {})",
                    self.config.max_entries
                );
            }

            let entry_path = entry.path()?.to_path_buf();

            // Validate path for security
            self.validate_path(&entry_path)?;

            // Determine the root directory
            if root_dir.is_none() {
                if let Some(first_component) = entry_path.components().next() {
                    root_dir = Some(PathBuf::from(first_component.as_os_str()));
                }
            }

            // Calculate destination path
            let dest_path = dest_dir.join(&entry_path);

            // Check file size limit
            let size = entry.size();
            if size > self.config.max_file_size {
                bail!(
                    "File {} exceeds maximum size ({} > {} bytes)",
                    entry_path.display(),
                    size,
                    self.config.max_file_size
                );
            }

            // Handle different entry types
            let header = entry.header();
            match header.entry_type() {
                tar::EntryType::Directory => {
                    self.extract_directory(&dest_path)?;
                    directories_created += 1;
                }
                tar::EntryType::Regular | tar::EntryType::Continuous => {
                    self.extract_file(&mut entry, &dest_path, size)?;
                    files_extracted += 1;
                    total_bytes += size;
                }
                tar::EntryType::Symlink => {
                    if !self.config.allow_symlinks {
                        bail!("Symlinks are not allowed: {}", entry_path.display());
                    }
                    #[cfg(unix)]
                    {
                        let target = entry.link_name()?.context("Symlink target missing")?;
                        self.extract_symlink(&dest_path, &target)?;
                    }
                    #[cfg(not(unix))]
                    {
                        bail!("Symlinks not supported on this platform");
                    }
                }
                tar::EntryType::Link => {
                    // Hard links - skip for now, treat as regular file copy
                    // This is a simplification; proper hard link handling is complex
                }
                _ => {
                    // Skip other types (block devices, char devices, etc.)
                }
            }

            // Check for manifest to extract plugin name/version
            // Read from the extracted file on disk rather than the tar entry,
            // since extract_file() already consumed the entry's data stream.
            if entry_path.ends_with("plugin.toml") && dest_path.exists() {
                let content = fs::read_to_string(&dest_path)?;
                if let Some((name, version)) = self.parse_manifest_basic(&content)? {
                    plugin_name = name;
                    plugin_version = version;
                }
            }

            // Check total size
            if total_bytes > self.config.max_total_size {
                bail!(
                    "Total extracted size exceeds limit ({} > {} bytes)",
                    total_bytes,
                    self.config.max_total_size
                );
            }
        }

        // Verify we found a valid root directory
        let plugin_dir = match root_dir {
            Some(root) => dest_dir.join(root),
            None => bail!("Archive has no root directory"),
        };

        // Verify plugin.toml exists
        let manifest_path = plugin_dir.join("plugin.toml");
        if !manifest_path.exists() {
            bail!("Extracted archive missing plugin.toml");
        }

        Ok(ExtractionResult {
            plugin_dir,
            files_extracted,
            directories_created,
            total_bytes,
            plugin_name,
            plugin_version,
        })
    }

    /// Validate a path for security issues
    fn validate_path(&self, path: &Path) -> Result<()> {
        // Check path length
        let path_str = path.to_string_lossy();
        if path_str.len() > self.config.max_path_length {
            bail!(
                "Path too long ({} > {})",
                path_str.len(),
                self.config.max_path_length
            );
        }

        // Check for path traversal
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    bail!("Path traversal detected: {}", path.display());
                }
                Component::RootDir => {
                    if !self.config.allow_absolute_paths {
                        bail!("Absolute path not allowed: {}", path.display());
                    }
                }
                Component::Prefix(_) => {
                    if !self.config.allow_absolute_paths {
                        bail!("Absolute path (prefix) not allowed: {}", path.display());
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Extract a directory
    fn extract_directory(&self, path: &Path) -> Result<()> {
        if path.exists() {
            if !path.is_dir() {
                bail!("Path exists but is not a directory: {}", path.display());
            }
            return Ok(());
        }

        fs::create_dir_all(path)
            .with_context(|| format!("Creating directory {}", path.display()))?;

        // Set directory permissions
        #[cfg(unix)]
        {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }

        Ok(())
    }

    /// Extract a regular file
    fn extract_file(
        &self,
        entry: &mut tar::Entry<impl Read>,
        path: &Path,
        expected_size: u64,
    ) -> Result<()> {
        // Check if file exists
        if path.exists() && !self.config.overwrite {
            bail!("File already exists: {}", path.display());
        }

        // Create parent directories
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Creating parent directory {}", parent.display()))?;
            }
        }

        // Extract file with size validation
        let mut file =
            File::create(path).with_context(|| format!("Creating file {}", path.display()))?;

        let mut bytes_written = 0u64;
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = entry.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            file.write_all(&buffer[..bytes_read])?;
            bytes_written += bytes_read as u64;

            // Check for size mismatch (potential zip bomb)
            if bytes_written > expected_size {
                bail!(
                    "File size mismatch during extraction: {} > {}",
                    bytes_written,
                    expected_size
                );
            }
        }

        // Set executable permission for plugin binaries
        if self.config.set_executable {
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                let lower = filename.to_lowercase();
                if lower == "plugin.so"
                    || lower == "plugin.dll"
                    || lower == "plugin.dylib"
                    || lower.ends_with(".so")
                    || lower.ends_with(".dll")
                    || lower.ends_with(".dylib")
                {
                    #[cfg(unix)]
                    {
                        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract a symlink (Unix only)
    #[cfg(unix)]
    fn extract_symlink(&self, link_path: &Path, target: &Path) -> Result<()> {
        // Check if file exists
        if link_path.exists() && !self.config.overwrite {
            bail!("Symlink already exists: {}", link_path.display());
        }

        // Remove existing symlink if overwrite is set
        if link_path.exists() {
            fs::remove_file(link_path)?;
        }

        // Create parent directories
        if let Some(parent) = link_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        // Create symlink
        std::os::unix::fs::symlink(target, link_path).with_context(|| {
            format!(
                "Creating symlink {} -> {}",
                link_path.display(),
                target.display()
            )
        })?;

        Ok(())
    }

    /// Basic manifest parsing to extract name and version
    fn parse_manifest_basic(&self, content: &str) -> Result<Option<(String, String)>> {
        // Simple TOML parsing for [package] section
        let mut in_package = false;
        let mut name = String::new();
        let mut version = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed == "[package]" {
                in_package = true;
                continue;
            }

            if trimmed.starts_with('[') && trimmed != "[package]" {
                in_package = false;
                continue;
            }

            if in_package {
                if let Some((key, value)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"');

                    match key {
                        "name" => name = value.to_string(),
                        "version" => version = value.to_string(),
                        _ => {}
                    }
                }
            } else if !in_package {
                // Try flat format (top-level keys outside any section)
                if let Some((key, value)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"');

                    match key {
                        "name" if name.is_empty() => name = value.to_string(),
                        "version" if version.is_empty() => version = value.to_string(),
                        _ => {}
                    }
                }
            }
        }

        if !name.is_empty() && !version.is_empty() {
            Ok(Some((name, version)))
        } else {
            Ok(None)
        }
    }
}

/// Convenience function to extract an artifact with secure defaults
pub fn extract_artifact(artifact_path: &Path, dest_dir: &Path) -> Result<ExtractionResult> {
    let extractor = PluginExtractor::secure();
    extractor.extract(artifact_path, dest_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_artifact(dir: &Path) -> PathBuf {
        use std::fs;

        // Create test plugin structure
        fs::write(
            dir.join("plugin.toml"),
            r#"[package]
name = "test-plugin"
version = "1.0.0"
abi_version = "2.0""#,
        )
        .unwrap();
        fs::write(dir.join("plugin.so"), b"binary content").unwrap();
        fs::write(dir.join("LICENSE"), "MIT").unwrap();
        fs::write(dir.join("README.md"), "Test plugin").unwrap();

        // Create artifact
        let artifact_path = dir.parent().unwrap().join("test-plugin.tar.gz");
        let file = File::create(&artifact_path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(enc);

        // Add root directory
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_mode(0o755);
        header.set_size(0);
        header.set_cksum();
        builder
            .append_data(
                &mut header,
                Path::new("test-plugin-1.0.0"),
                std::io::empty(),
            )
            .unwrap();

        // Add files
        for (name, content) in [
            (
                "plugin.toml",
                fs::read_to_string(dir.join("plugin.toml")).unwrap(),
            ),
            (
                "plugin.so",
                fs::read_to_string(dir.join("plugin.so")).unwrap(),
            ),
            ("LICENSE", fs::read_to_string(dir.join("LICENSE")).unwrap()),
            (
                "README.md",
                fs::read_to_string(dir.join("README.md")).unwrap(),
            ),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            let path = format!("test-plugin-1.0.0/{}", name);
            builder
                .append_data(&mut header, Path::new(&path), content.as_bytes())
                .unwrap();
        }

        let enc = builder.into_inner().unwrap();
        enc.finish().unwrap();

        artifact_path
    }

    #[test]
    fn test_extract_artifact() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugin_src");
        fs::create_dir_all(&plugin_dir).unwrap();

        let artifact_path = create_test_artifact(&plugin_dir);
        let dest_dir = temp_dir.path().join("extracted");

        let result = extract_artifact(&artifact_path, &dest_dir).unwrap();

        assert!(result.plugin_dir.exists());
        assert!(result.plugin_dir.join("plugin.toml").exists());
        assert_eq!(result.plugin_name, "test-plugin");
        assert_eq!(result.plugin_version, "1.0.0");
        assert!(result.files_extracted > 0);
    }

    #[test]
    fn test_path_traversal_detection() {
        let config = ExtractorConfig::default();
        let extractor = PluginExtractor::new(config);

        // Should reject path traversal
        assert!(extractor.validate_path(Path::new("../etc/passwd")).is_err());
        assert!(extractor
            .validate_path(Path::new("safe/../../../etc/passwd"))
            .is_err());
    }

    #[test]
    fn test_absolute_path_rejection() {
        let config = ExtractorConfig::default();
        let extractor = PluginExtractor::new(config);

        // Should reject absolute paths
        assert!(extractor.validate_path(Path::new("/etc/passwd")).is_err());
    }

    #[test]
    fn test_valid_path() {
        let config = ExtractorConfig::default();
        let extractor = PluginExtractor::new(config);

        // Should accept valid relative paths
        assert!(extractor
            .validate_path(Path::new("plugin-1.0.0/plugin.toml"))
            .is_ok());
        assert!(extractor
            .validate_path(Path::new("plugin-1.0.0/lib/plugin.so"))
            .is_ok());
    }

    #[test]
    fn test_config_defaults() {
        let config = ExtractorConfig::default();

        assert_eq!(config.max_file_size, MAX_FILE_SIZE);
        assert_eq!(config.max_total_size, MAX_TOTAL_SIZE);
        assert!(!config.allow_symlinks);
        assert!(!config.allow_absolute_paths);
        assert!(config.set_executable);
        assert!(!config.overwrite);
    }

    #[test]
    fn test_parse_manifest_basic() {
        let extractor = PluginExtractor::secure();

        let toml = r#"[package]
name = "my-plugin"
version = "2.0.0"
abi_version = "2.0""#;

        let result = extractor.parse_manifest_basic(toml).unwrap();
        assert_eq!(result, Some(("my-plugin".to_string(), "2.0.0".to_string())));
    }

    #[test]
    fn test_parse_manifest_flat() {
        let extractor = PluginExtractor::secure();

        let toml = r#"name = "flat-plugin"
version = "1.5.0""#;

        let result = extractor.parse_manifest_basic(toml).unwrap();
        assert_eq!(
            result,
            Some(("flat-plugin".to_string(), "1.5.0".to_string()))
        );
    }
}
