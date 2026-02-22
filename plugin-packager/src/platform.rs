// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Cross-platform plugin artifact support
//!
//! This module provides platform detection and artifact type validation for
//! cross-platform plugin packaging as specified in RFC-0003.
//!
//! ## Supported Platforms
//! - Linux: `.so` (shared object)
//! - Windows: `.dll` (dynamic link library)
//! - macOS: `.dylib` (dynamic library)
//!
//! ## Naming Convention
//! Artifacts follow the pattern: `<plugin-name>-v<version>-<target-triple>.tar.gz`
//!
//! Examples:
//! - `myplugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz`
//! - `myplugin-v1.0.0-x86_64-pc-windows-gnu.tar.gz`
//! - `myplugin-v1.0.0-x86_64-apple-darwin.tar.gz`
//! - `myplugin-v1.0.0-aarch64-apple-darwin.tar.gz`

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

/// Supported platform types for plugin artifacts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Linux,
    Windows,
    Macos,
}

impl Platform {
    /// Get the expected artifact extension for this platform
    pub fn artifact_extension(&self) -> &'static str {
        match self {
            Platform::Linux => "so",
            Platform::Windows => "dll",
            Platform::Macos => "dylib",
        }
    }

    /// Get the expected artifact filename for this platform
    pub fn artifact_filename(&self) -> &'static str {
        match self {
            Platform::Linux => "plugin.so",
            Platform::Windows => "plugin.dll",
            Platform::Macos => "plugin.dylib",
        }
    }

    /// Detect platform from target triple
    pub fn from_target_triple(target: &str) -> Option<Self> {
        if target.contains("linux") {
            Some(Platform::Linux)
        } else if target.contains("windows") {
            Some(Platform::Windows)
        } else if target.contains("apple") || target.contains("darwin") {
            Some(Platform::Macos)
        } else {
            None
        }
    }

    /// Get current host platform
    pub fn host() -> Self {
        #[cfg(target_os = "linux")]
        {
            Platform::Linux
        }
        #[cfg(target_os = "windows")]
        {
            Platform::Windows
        }
        #[cfg(target_os = "macos")]
        {
            Platform::Macos
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Platform::Linux // Default fallback
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Linux => write!(f, "linux"),
            Platform::Windows => write!(f, "windows"),
            Platform::Macos => write!(f, "macos"),
        }
    }
}

/// All supported artifact extensions
pub const SUPPORTED_ARTIFACT_EXTENSIONS: &[&str] = &["so", "dll", "dylib"];

/// All supported artifact filenames
pub const SUPPORTED_ARTIFACT_FILENAMES: &[&str] = &["plugin.so", "plugin.dll", "plugin.dylib"];

/// Information extracted from an artifact filename
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactMetadata {
    /// Plugin name (lowercase with hyphens/underscores)
    pub name: String,
    /// Plugin version (semver with 'v' prefix)
    pub version: String,
    /// Target triple (e.g., "x86_64-unknown-linux-gnu")
    pub target_triple: String,
    /// Detected platform
    pub platform: Platform,
}

impl ArtifactMetadata {
    /// Parse artifact filename to extract metadata
    ///
    /// Expected format: `<plugin-name>-v<version>-<target-triple>.tar.gz`
    ///
    /// # Examples
    /// ```
    /// use plugin_packager::platform::{ArtifactMetadata, Platform};
    ///
    /// let meta = ArtifactMetadata::parse("myplugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz").unwrap();
    /// assert_eq!(meta.name, "myplugin");
    /// assert_eq!(meta.version, "1.0.0");
    /// assert_eq!(meta.target_triple, "x86_64-unknown-linux-gnu");
    /// assert_eq!(meta.platform, Platform::Linux);
    /// ```
    pub fn parse(filename: &str) -> Result<Self> {
        // Must end with .tar.gz
        if !filename.ends_with(".tar.gz") {
            bail!("Artifact filename must end with .tar.gz: {}", filename);
        }

        // Strip .tar.gz suffix
        let base = &filename[..filename.len() - 7];

        // Split into parts by '-'
        let parts: Vec<&str> = base.split('-').collect();

        if parts.len() < 4 {
            bail!(
                "Artifact filename must follow pattern: <name>-v<version>-<target>.tar.gz\n\
                 Got: {}\n\
                 Example: myplugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz",
                filename
            );
        }

        // Find the version part (starts with 'v' followed by digit)
        let version_idx = parts
            .iter()
            .position(|p| {
                p.starts_with('v')
                    && p.len() > 1
                    && p[1..]
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Artifact filename must contain version with 'v' prefix (e.g., -v1.0.0-)\n\
                 Got: {}",
                    filename
                )
            })?;

        // Name is everything before the version (joined by '-')
        let name = parts[..version_idx].join("-");

        // Validate name format
        if name.is_empty() {
            bail!(
                "Plugin name cannot be empty in artifact filename: {}",
                filename
            );
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            bail!(
                "Plugin name must be lowercase with hyphens/underscores only: {}\n\
                 Got: {}",
                filename,
                name
            );
        }

        // Version is the version part without 'v' prefix
        let version = parts[version_idx][1..].to_string();

        // Validate version format (basic semver check)
        let version_parts: Vec<&str> = version.split('.').collect();
        if version_parts.len() < 3 {
            bail!(
                "Version must follow semantic versioning (major.minor.patch)\n\
                 Got: {}",
                version
            );
        }
        for part in &version_parts[..3] {
            if part.parse::<u32>().is_err() {
                bail!("Version parts must be numeric: {}", version);
            }
        }

        // Target triple is everything after version (joined by '-')
        let target_triple = parts[version_idx + 1..].join("-");

        if target_triple.is_empty() {
            bail!(
                "Target triple cannot be empty in artifact filename: {}",
                filename
            );
        }

        // Detect platform from target triple
        let platform = Platform::from_target_triple(&target_triple).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown platform in target triple: {}\n\
                 Supported: linux, windows, apple/darwin",
                target_triple
            )
        })?;

        Ok(ArtifactMetadata {
            name,
            version,
            target_triple,
            platform,
        })
    }

    /// Generate artifact filename from metadata
    pub fn to_filename(&self) -> String {
        format!(
            "{}-v{}-{}.tar.gz",
            self.name, self.version, self.target_triple
        )
    }
}

/// Validate that a path contains a valid artifact for the given platform
pub fn validate_platform_artifact(path: &Path, platform: Platform) -> Result<()> {
    let expected_filename = platform.artifact_filename();

    if !path.exists() {
        bail!("Artifact file not found: {}", path.display());
    }

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", path.display()))?;

    // Check for any supported artifact filename
    if !SUPPORTED_ARTIFACT_FILENAMES.contains(&filename) {
        bail!(
            "Invalid artifact filename: {}\n\
             Expected one of: {}",
            filename,
            SUPPORTED_ARTIFACT_FILENAMES.join(", ")
        );
    }

    // For the expected platform, the file should match
    if filename != expected_filename {
        // This is a warning-level issue - the artifact exists but is for a different platform
        // We still allow it for cross-compilation scenarios
    }

    Ok(())
}

/// Get all valid artifact filenames for verification
pub fn get_valid_artifact_filenames() -> &'static [&'static str] {
    SUPPORTED_ARTIFACT_FILENAMES
}

/// Check if a filename is a valid plugin artifact
pub fn is_valid_artifact_filename(filename: &str) -> bool {
    SUPPORTED_ARTIFACT_FILENAMES.contains(&filename)
}

/// Check if an extension is a valid artifact extension
pub fn is_valid_artifact_extension(ext: &str) -> bool {
    SUPPORTED_ARTIFACT_EXTENSIONS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_artifact_extensions() {
        assert_eq!(Platform::Linux.artifact_extension(), "so");
        assert_eq!(Platform::Windows.artifact_extension(), "dll");
        assert_eq!(Platform::Macos.artifact_extension(), "dylib");
    }

    #[test]
    fn test_platform_artifact_filenames() {
        assert_eq!(Platform::Linux.artifact_filename(), "plugin.so");
        assert_eq!(Platform::Windows.artifact_filename(), "plugin.dll");
        assert_eq!(Platform::Macos.artifact_filename(), "plugin.dylib");
    }

    #[test]
    fn test_platform_from_target_triple() {
        assert_eq!(
            Platform::from_target_triple("x86_64-unknown-linux-gnu"),
            Some(Platform::Linux)
        );
        assert_eq!(
            Platform::from_target_triple("x86_64-pc-windows-gnu"),
            Some(Platform::Windows)
        );
        assert_eq!(
            Platform::from_target_triple("x86_64-apple-darwin"),
            Some(Platform::Macos)
        );
        assert_eq!(
            Platform::from_target_triple("aarch64-apple-darwin"),
            Some(Platform::Macos)
        );
        assert_eq!(Platform::from_target_triple("unknown-unknown"), None);
    }

    #[test]
    fn test_artifact_metadata_parse_linux() {
        let meta =
            ArtifactMetadata::parse("myplugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz").unwrap();
        assert_eq!(meta.name, "myplugin");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.target_triple, "x86_64-unknown-linux-gnu");
        assert_eq!(meta.platform, Platform::Linux);
    }

    #[test]
    fn test_artifact_metadata_parse_windows() {
        let meta = ArtifactMetadata::parse("myplugin-v2.3.4-x86_64-pc-windows-gnu.tar.gz").unwrap();
        assert_eq!(meta.name, "myplugin");
        assert_eq!(meta.version, "2.3.4");
        assert_eq!(meta.target_triple, "x86_64-pc-windows-gnu");
        assert_eq!(meta.platform, Platform::Windows);
    }

    #[test]
    fn test_artifact_metadata_parse_macos() {
        let meta = ArtifactMetadata::parse("myplugin-v0.1.0-aarch64-apple-darwin.tar.gz").unwrap();
        assert_eq!(meta.name, "myplugin");
        assert_eq!(meta.version, "0.1.0");
        assert_eq!(meta.target_triple, "aarch64-apple-darwin");
        assert_eq!(meta.platform, Platform::Macos);
    }

    #[test]
    fn test_artifact_metadata_parse_with_hyphenated_name() {
        let meta =
            ArtifactMetadata::parse("my-awesome-plugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz")
                .unwrap();
        assert_eq!(meta.name, "my-awesome-plugin");
        assert_eq!(meta.version, "1.0.0");
    }

    #[test]
    fn test_artifact_metadata_parse_invalid_no_extension() {
        let result = ArtifactMetadata::parse("myplugin-v1.0.0-x86_64-unknown-linux-gnu");
        assert!(result.is_err());
    }

    #[test]
    fn test_artifact_metadata_parse_invalid_no_v_prefix() {
        let result = ArtifactMetadata::parse("myplugin-1.0.0-x86_64-unknown-linux-gnu.tar.gz");
        assert!(result.is_err());
    }

    #[test]
    fn test_artifact_metadata_parse_invalid_uppercase_name() {
        let result = ArtifactMetadata::parse("MyPlugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz");
        assert!(result.is_err());
    }

    #[test]
    fn test_artifact_metadata_parse_invalid_bad_version() {
        let result = ArtifactMetadata::parse("myplugin-v1.0-x86_64-unknown-linux-gnu.tar.gz");
        assert!(result.is_err());
    }

    #[test]
    fn test_artifact_metadata_to_filename() {
        let meta = ArtifactMetadata {
            name: "myplugin".to_string(),
            version: "1.0.0".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            platform: Platform::Linux,
        };
        assert_eq!(
            meta.to_filename(),
            "myplugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn test_is_valid_artifact_filename() {
        assert!(is_valid_artifact_filename("plugin.so"));
        assert!(is_valid_artifact_filename("plugin.dll"));
        assert!(is_valid_artifact_filename("plugin.dylib"));
        assert!(!is_valid_artifact_filename("plugin.bin"));
        assert!(!is_valid_artifact_filename("plugin"));
    }

    #[test]
    fn test_is_valid_artifact_extension() {
        assert!(is_valid_artifact_extension("so"));
        assert!(is_valid_artifact_extension("dll"));
        assert!(is_valid_artifact_extension("dylib"));
        assert!(!is_valid_artifact_extension("bin"));
        assert!(!is_valid_artifact_extension("exe"));
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", Platform::Linux), "linux");
        assert_eq!(format!("{}", Platform::Windows), "windows");
        assert_eq!(format!("{}", Platform::Macos), "macos");
    }

    #[test]
    fn test_supported_extensions_constant() {
        assert_eq!(SUPPORTED_ARTIFACT_EXTENSIONS, &["so", "dll", "dylib"]);
    }

    #[test]
    fn test_supported_filenames_constant() {
        assert_eq!(
            SUPPORTED_ARTIFACT_FILENAMES,
            &["plugin.so", "plugin.dll", "plugin.dylib"]
        );
    }
}
