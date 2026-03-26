// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

// Registry integration module
pub mod registry;
pub use registry::{
    DependencyResolution, DependencyResolver, LocalRegistry, PluginDependency, PluginRegistryEntry,
    RegistryPersistence, VersionRequirement,
};

// Configuration module
pub mod config;
pub use config::Config;

// Metadata extraction module
pub mod metadata;
pub use metadata::{DependencyMetadata, PluginMetadata, PluginRequirements, PluginStats};

// Remote registry integration module
pub mod remote;
pub use remote::{CacheStats, HybridRegistry, RemoteRegistry, RemoteRegistryConfig};

// Plugin upgrade and rollback module
pub mod upgrade;
pub use upgrade::{BackupManager, BackupRecord, SemanticVersion, UpgradeInfo, UpgradeResult};

// ABI v2.0 compatibility module
pub mod abi_compat;
pub use abi_compat::{
    ABICompatibleInfo, ABIValidationResult, ABIValidator, ABIVersion, CapabilityInfo,
    DependencyInfo, MaturityLevel, PluginCategory,
    ResourceRequirements,
};

// Plugin signature verification and cryptographic signing module
pub mod signature;
pub use signature::{
    KeyInfo, PluginSignature, SignatureAlgorithm, SignatureAuditLog, SignatureManager, TrustLevel,
    VerificationResult,
};

// Vulnerability scanning and security auditing module
pub mod security;
pub use security::{
    LicenseCompliance, LicenseType, RiskLevel, SecurityAuditReport, SecurityScanResult,
    Vulnerability, VulnerabilityScanner, VulnerabilitySeverity,
};

// Plugin manifest validation framework module
pub mod validation;
pub use validation::{
    ManifestValidator, ValidationIssue, ValidationReport, ValidationRule, ValidationSeverity,
};

// Plugin health check and verification framework module
pub mod health_check;
pub use health_check::{
    Architecture, BinaryCompatibility, HealthCheckResult, HealthReport, HealthScore,
    HealthSeverity, HealthStatus, PerformanceBaseline, PerformanceThresholds, Platform,
    PluginHealthChecker, SymbolRequirement,
};

// Plugin compatibility matrix and analysis module
pub mod compat_matrix;
pub use compat_matrix::{
    AbiCompatibilityEntry, AbiVersion, BreakingChange, CompatibilityAnalysis, CompatibilityLevel,
    CompatibilityReport, DependencyCompatibility, PlatformArch, PlatformSupportEntry,
    PluginCompatibilityMatrix,
};

// Plugin sandbox verification and security analysis module
pub mod sandbox;
pub use sandbox::{
    Permission, PluginCapability, PluginSandboxVerifier, ResourceLimits, SandboxCheckResult,
    SandboxRiskLevel, SandboxSeverity, SandboxVerificationReport, SystemCallInfo,
};

// Dependency tree visualization and graph analysis module
pub mod dep_tree;
pub use dep_tree::{
    CircularDependency, DependencyEdge, DependencyGraph, DependencyMetrics, DependencyNode,
};

// Plugin composition and meta-package support module
pub mod composition;
pub use composition::{
    BundleMetadata, BundleType, CompositePlugin, CompositeSize, CompositionManager,
    ConflictResolution, DependencyResolutionResult, PluginBundle, PluginComponent,
    ValidationResult, VersionConflict,
};

// Optional dependencies and feature gates support module
pub mod optional_deps;
pub use optional_deps::{
    ConditionType, DependencyCondition, FeatureGate, OptionalDependency, OptionalDependencyManager,
    PlatformSpecific,
};

// RFC-0003: Plugin artifact extraction with security checks
pub mod extractor;
pub use extractor::{extract_artifact, ExtractionResult, ExtractorConfig, PluginExtractor};

// RFC-0003: Cross-platform plugin artifact support
pub mod platform;
// Note: Access Platform enum via plugin_packager::platform::Platform
// to avoid conflict with health_check::Platform
pub use platform::{
    get_valid_artifact_filenames, is_valid_artifact_extension, is_valid_artifact_filename,
    validate_platform_artifact, ArtifactMetadata, SUPPORTED_ARTIFACT_EXTENSIONS,
    SUPPORTED_ARTIFACT_FILENAMES,
};

// RFC-0003: Registry publishing support
pub mod publish;
pub use publish::{ArtifactPublishResult, ArtifactPublisher, LocalArtifact, PublishConfig};

#[derive(Deserialize, Debug)]
pub struct ManifestPackage {
    pub name: String,
    pub version: String,
    pub abi_version: String,
    pub entrypoint: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub package: Option<ManifestPackage>,
    // Support legacy flat format
    pub name: Option<String>,
    pub version: Option<String>,
    pub abi_version: Option<String>,
    pub entrypoint: Option<String>,
}

pub fn read_manifest(path: &Path) -> Result<Manifest> {
    let s =
        fs::read_to_string(path).with_context(|| format!("reading manifest {}", path.display()))?;
    let m: Manifest = toml::from_str(&s).context("parsing plugin.toml")?;

    // Get the effective values - prefer [package] section, fallback to flat fields
    let name = if let Some(pkg) = &m.package {
        pkg.name.clone()
    } else {
        m.name.clone().unwrap_or_default()
    };

    let version = if let Some(pkg) = &m.package {
        pkg.version.clone()
    } else {
        m.version.clone().unwrap_or_default()
    };

    let abi_version = if let Some(pkg) = &m.package {
        pkg.abi_version.clone()
    } else {
        m.abi_version.clone().unwrap_or_default()
    };

    // basic validation
    if name.trim().is_empty() || version.trim().is_empty() || abi_version.trim().is_empty() {
        bail!("manifest must have name, version, and abi_version (either in [package] section or at top level)");
    }
    Ok(m)
}

/// Create a .tar.gz artifact from a plugin directory. The archive will contain a single
/// root directory named "<name>-<version>/" and all files from `src_dir` will be placed
/// under that root preserving relative layout.
///
/// # RFC-0003 Compliance
/// - Validates plugin name is lowercase with hyphens/underscores only
/// - Validates version follows semantic versioning (major.minor.patch)
/// - Includes optional CHANGELOG.md and doc/ directory if present
pub fn pack_dir(src_dir: &Path, out_path: &Path) -> Result<PathBuf> {
    // ensure manifest exists
    let manifest_path = src_dir.join("plugin.toml");
    let manifest = read_manifest(&manifest_path)?;

    // Extract effective name and version
    let (name, version) = if let Some(pkg) = &manifest.package {
        (pkg.name.clone(), pkg.version.clone())
    } else {
        (
            manifest.name.clone().unwrap_or_default(),
            manifest.version.clone().unwrap_or_default(),
        )
    };

    // RFC-0003: Validate name format
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        bail!(
            "Plugin name must be lowercase with hyphens/underscores only (RFC-0003)\n\
             Got: '{}'\n\
             Example: 'my-plugin' or 'my_plugin'",
            name
        );
    }

    // RFC-0003: Validate version format
    let version_parts: Vec<&str> = version.split('.').collect();
    if version_parts.len() < 3 {
        bail!(
            "Version must follow semantic versioning (RFC-0003)\n\
             Got: '{}'\n\
             Expected format: major.minor.patch (e.g., '1.0.0')",
            version
        );
    }
    for part in &version_parts[..3] {
        if part.parse::<u32>().is_err() {
            bail!(
                "Version parts must be numeric (RFC-0003)\n\
                 Got: '{}'\n\
                 Expected format: major.minor.patch (e.g., '1.0.0')",
                version
            );
        }
    }

    let file = File::create(out_path)
        .with_context(|| format!("creating output {}", out_path.display()))?;
    let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(enc);

    let root = format!("{}-{}", name, version);

    // Create deterministic root directory entry
    {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_mode(0o755);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_size(0);
        header.set_cksum();
        builder.append_data(&mut header, Path::new(&root), std::io::empty())?;
    }

    // Track optional files included
    let mut optional_files: Vec<&str> = Vec::new();

    // append files under root with deterministic metadata
    for entry in walkdir::WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path == src_dir {
            continue;
        }
        let rel = path.strip_prefix(src_dir).unwrap();
        let target_path = Path::new(&root).join(rel);

        // Track optional files
        if let Some(fname) = rel.file_name().and_then(|s| s.to_str()) {
            if fname == "CHANGELOG.md" {
                optional_files.push("CHANGELOG.md");
            }
        }
        if rel.starts_with("doc") && !optional_files.contains(&"doc/") {
            optional_files.push("doc/");
        }

        if path.is_dir() {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_mode(0o755);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_size(0);
            header.set_cksum();
            builder.append_data(&mut header, &target_path, std::io::empty())?;
        } else if path.is_file() {
            let mut f = File::open(path)?;
            let meta = f.metadata()?;
            let mut header = tar::Header::new_gnu();
            header.set_size(meta.len());
            // set executable bit for plugin binary names, otherwise 0644
            let mut mode = 0o644;
            if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                if fname == "plugin.so" || fname == "plugin.dll" || fname == "plugin.dylib" {
                    mode = 0o755;
                }
            }
            // preserve executable bit on unix if present
            #[cfg(unix)]
            {
                let p = meta.permissions();
                if (p.mode() & 0o111) != 0 {
                    mode = 0o755;
                }
            }
            header.set_mode(mode);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_cksum();
            builder.append_data(&mut header, &target_path, &mut f)?;
        }
    }

    // finish to flush encoder
    let enc = builder.into_inner()?;
    enc.finish()?;

    // compute sha256
    let sha = compute_sha256(out_path)?;
    // produce sidecar file named '<artifact>.sha256' (e.g. artifact.tar.gz.sha256)
    let checksum_name = format!("{}.sha256", out_path.file_name().unwrap().to_string_lossy());
    let checksum_path = out_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(checksum_name);
    let mut f = File::create(&checksum_path)?;
    writeln!(
        f,
        "{}  {}",
        hex::encode(sha),
        out_path.file_name().unwrap().to_string_lossy()
    )?;

    // RFC-0003: Log optional files included
    if !optional_files.is_empty() {
        tracing::error!(
            "RFC-0003: Optional files included: {}",
            optional_files.join(", ")
        );
    }

    Ok(checksum_path)
}

/// Create a .tar.gz artifact with RFC-0003 compliant naming.
///
/// The artifact will be named: `<plugin-name>-v<version>-<target-triple>.tar.gz`
///
/// # Arguments
/// * `src_dir` - Source directory containing plugin files
/// * `output_dir` - Directory to write the artifact to
/// * `target_triple` - Target triple (e.g., "x86_64-unknown-linux-gnu")
///
/// # Returns
/// * Path to the checksum file on success
///
/// # Example
/// ```no_run
/// use plugin_packager::pack_dir_with_target;
/// use std::path::Path;
///
/// let checksum = pack_dir_with_target(
///     Path::new("./my-plugin"),
///     Path::new("./dist"),
///     "x86_64-unknown-linux-gnu"
/// ).unwrap();
/// // Creates: ./dist/my-plugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
/// ```
pub fn pack_dir_with_target(
    src_dir: &Path,
    output_dir: &Path,
    target_triple: &str,
) -> Result<PathBuf> {
    // Ensure manifest exists and get name/version
    let manifest_path = src_dir.join("plugin.toml");
    let manifest = read_manifest(&manifest_path)?;

    let (name, version) = if let Some(pkg) = &manifest.package {
        (pkg.name.clone(), pkg.version.clone())
    } else {
        (
            manifest.name.clone().unwrap_or_default(),
            manifest.version.clone().unwrap_or_default(),
        )
    };

    // Validate target triple
    let _platform =
        crate::platform::Platform::from_target_triple(target_triple).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown platform in target triple: {}\n\
             Supported: linux, windows, apple/darwin",
                target_triple
            )
        })?;

    // Construct RFC-0003 compliant filename
    let artifact_name = format!("{}-v{}-{}.tar.gz", name, version, target_triple);
    let out_path = output_dir.join(&artifact_name);

    // Create output directory if needed
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    // Pack using the standard pack_dir
    pack_dir(src_dir, &out_path)?;

    // Verify the artifact name is parseable
    let _meta = crate::platform::ArtifactMetadata::parse(&artifact_name).with_context(|| {
        format!(
            "Generated artifact name is not RFC-0003 compliant: {}",
            artifact_name
        )
    })?;

    // Return checksum path
    let checksum_name = format!("{}.sha256", artifact_name);
    Ok(output_dir.join(checksum_name))
}

fn compute_sha256(path: &Path) -> Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_vec())
}

/// Verify an artifact: check checksum, archive layout, and manifest fields. `checksum_path`
/// may be None in which case we look for a sibling `.tar.gz.sha256` file.
pub fn verify_artifact(artifact: &Path, checksum_path: Option<&Path>) -> Result<()> {
    let checksum_path = match checksum_path {
        Some(p) => p.to_path_buf(),
        None => {
            let name = format!("{}.sha256", artifact.file_name().unwrap().to_string_lossy());
            artifact
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(name)
        }
    };

    if !checksum_path.exists() {
        bail!("checksum file not found: {}", checksum_path.display());
    }

    // read checksum file: expect '<hex>  <filename>'
    let s = fs::read_to_string(&checksum_path)?;
    let token = s.split_whitespace().next().context("checksum file empty")?;
    let expected = hex::decode(token.trim()).context("decoding checksum hex")?;

    let computed = compute_sha256(artifact)?;
    if expected != computed {
        bail!(
            "checksum mismatch: expected {} got {}",
            hex::encode(expected),
            hex::encode(computed)
        );
    }

    // open tar.gz and inspect top-level layout
    let f = File::open(artifact)?;
    let dec = flate2::read::GzDecoder::new(f);
    let mut ar = tar::Archive::new(dec);

    let mut roots = std::collections::HashSet::new();
    let mut seen_plugin_toml = false;
    let mut seen_plugin_so = false;
    let mut seen_license = false;
    let mut seen_readme = false;

    for entry in ar.entries()? {
        let entry = entry?;
        let path = match entry.path() {
            Ok(p) => p.into_owned(),
            Err(_) => continue,
        };
        let comps: Vec<_> = path.components().collect();
        if comps.is_empty() {
            continue;
        }
        // first component is the root dir
        if let Some(root_comp) = comps.first() {
            roots.insert(root_comp.as_os_str().to_owned());
        }
        // check for required files at root
        if comps.len() == 2 {
            if let Some(name) = path.file_name() {
                match name.to_string_lossy().to_lowercase().as_str() {
                    "plugin.toml" => seen_plugin_toml = true,
                    "plugin.so" | "plugin.dll" | "plugin.dylib" => seen_plugin_so = true,
                    "license" => seen_license = true,
                    "readme.md" => seen_readme = true,
                    _ => {}
                }
            }
        }
    }

    if roots.len() != 1 {
        bail!("archive must contain a single root directory");
    }

    if !(seen_plugin_toml && seen_plugin_so && seen_license && seen_readme) {
        bail!("archive missing required files: plugin.toml, plugin.so, LICENSE, README.md");
    }

    // Extract and validate manifest from tar (reopen)
    let f = File::open(artifact)?;
    let dec = flate2::read::GzDecoder::new(f);
    let mut ar = tar::Archive::new(dec);
    let root = roots.into_iter().next().unwrap();
    let manifest_path = Path::new(&root).join("plugin.toml");
    for entry in ar.entries()? {
        let mut entry = entry?;
        if entry.path()? == manifest_path {
            let mut s = String::new();
            entry.read_to_string(&mut s)?;
            let m: Manifest = toml::from_str(&s).context("parsing manifest in archive")?;

            // Extract effective name and version (required fields)
            let name = if let Some(pkg) = &m.package {
                pkg.name.clone()
            } else {
                m.name.clone().unwrap_or_default()
            };

            let version = if let Some(pkg) = &m.package {
                pkg.version.clone()
            } else {
                m.version.clone().unwrap_or_default()
            };

            let abi_version = if let Some(pkg) = &m.package {
                pkg.abi_version.clone()
            } else {
                m.abi_version.clone().unwrap_or_default()
            };

            // entrypoint is optional (v2 ABI doesn't use it)
            if name.trim().is_empty() || version.trim().is_empty() || abi_version.trim().is_empty()
            {
                bail!("manifest missing required fields: name, version, abi_version");
            }
            return Ok(());
        }
    }

    bail!("manifest not found inside archive");
}

#[cfg(test)]
mod tests {
    use super::*;
    // no extra imports
    use tempfile::tempdir;

    #[test]
    fn pack_and_verify_roundtrip() -> Result<()> {
        let dir = tempdir()?;
        let base = dir.path();
        // create minimal plugin files
        fs::write(
            base.join("plugin.toml"),
            r#"name = "testplugin"
version = "0.1.0"
abi_version = "1"
entrypoint = "init""#,
        )?;
        fs::write(base.join("plugin.so"), b"binary")?;
        fs::write(base.join("LICENSE"), "MIT")?;
        fs::write(base.join("README.md"), "readme")?;

        let out = dir.path().join("artifact.tar.gz");
        let checksum = pack_dir(base, &out)?;
        // verify
        verify_artifact(&out, Some(&checksum))?;
        Ok(())
    }
}
