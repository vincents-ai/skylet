// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use flate2::read::GzDecoder;
use plugin_packager::{
    pack_dir, pack_dir_with_target, verify_artifact, BackupManager, DependencyResolver,
    LocalRegistry, PluginDependency, PluginRegistryEntry, RegistryPersistence, SemanticVersion,
    UpgradeInfo, VersionRequirement,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tar::Archive;
use tempfile::tempdir;

#[test]
fn test_package_extract_verify_roundtrip() -> Result<()> {
    // 1. Create a temporary source directory with all required files
    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin.toml (v2 format)
    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "roundtrip-test-plugin"
version = "0.2.0"
description = "Integration test plugin"
author = "test"
license = "MIT"
abi_version = "2.0"

[capabilities]
handles_requests = true
provides_health_checks = true

[requirements]
max_concurrency = 5
"#,
    )?;

    // Create README.md
    fs::write(
        base.join("README.md"),
        "# Test Plugin\n\nThis is a test plugin for integration testing.",
    )?;

    // Create LICENSE
    fs::write(
        base.join("LICENSE"),
        "MIT License\n\nCopyright (c) 2026 Test",
    )?;

    // Create plugin binary
    let mut plugin_binary = File::create(base.join("plugin.so"))?;
    plugin_binary.write_all(b"INTEGRATION_TEST_PLUGIN_BINARY_DATA")?;
    drop(plugin_binary);

    // 2. Package the plugin
    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("roundtrip-test-plugin-0.2.0.tar.gz");
    let checksum_path = pack_dir(base, &out_path)?;

    assert!(out_path.exists(), "Package artifact should exist");
    assert!(checksum_path.exists(), "Checksum file should exist");

    // 3. Verify the packaged artifact
    verify_artifact(&out_path, Some(&checksum_path))?;

    // 4. Extract the package to a new directory
    let extract_dir = tempdir()?;
    let extract_path = extract_dir.path();

    let tar_file = File::open(&out_path)?;
    let decoder = GzDecoder::new(tar_file);
    let mut archive = Archive::new(decoder);
    archive.unpack(extract_path)?;

    // 5. Verify extracted contents
    let plugin_root = extract_path.join("roundtrip-test-plugin-0.2.0");
    assert!(plugin_root.exists(), "Plugin root directory should exist");
    assert!(
        plugin_root.join("plugin.toml").exists(),
        "plugin.toml should exist"
    );
    assert!(
        plugin_root.join("plugin.so").exists(),
        "plugin.so should exist"
    );
    assert!(
        plugin_root.join("README.md").exists(),
        "README.md should exist"
    );
    assert!(plugin_root.join("LICENSE").exists(), "LICENSE should exist");

    // 6. Verify binary content
    let binary_content = fs::read(plugin_root.join("plugin.so"))?;
    assert_eq!(
        binary_content, b"INTEGRATION_TEST_PLUGIN_BINARY_DATA",
        "Binary content should match"
    );

    // 7. Verify manifest can be parsed
    let manifest_content = fs::read_to_string(plugin_root.join("plugin.toml"))?;
    assert!(
        manifest_content.contains("[package]"),
        "Should be v2 format with [package] section"
    );
    assert!(
        manifest_content.contains(r#"name = "roundtrip-test-plugin""#),
        "Name should match"
    );
    assert!(
        manifest_content.contains(r#"version = "0.2.0""#),
        "Version should match"
    );

    Ok(())
}

#[test]
fn test_v1_format_compatibility() -> Result<()> {
    // Create a temporary source directory with v1 format (flat TOML)
    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin.toml (v1 flat format)
    fs::write(
        base.join("plugin.toml"),
        r#"name = "legacy-plugin"
version = "0.1.0"
abi_version = "1.0"
entrypoint = "init_plugin"
"#,
    )?;

    fs::write(
        base.join("README.md"),
        "# Legacy Plugin\n\nOld format plugin.",
    )?;

    fs::write(base.join("LICENSE"), "MIT License")?;
    fs::write(base.join("plugin.so"), b"LEGACY_BINARY")?;

    // Package the v1 format plugin
    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("legacy-plugin-0.1.0.tar.gz");
    let checksum_path = pack_dir(base, &out_path)?;

    // Verify it works
    verify_artifact(&out_path, Some(&checksum_path))?;

    // Extract and verify
    let extract_dir = tempdir()?;
    let tar_file = File::open(&out_path)?;
    let decoder = GzDecoder::new(tar_file);
    let mut archive = Archive::new(decoder);
    archive.unpack(extract_dir.path())?;

    let plugin_root = extract_dir.path().join("legacy-plugin-0.1.0");
    assert!(plugin_root.exists());
    assert!(plugin_root.join("plugin.toml").exists());

    Ok(())
}

#[test]
fn test_checksum_verification_failure() -> Result<()> {
    let src_dir = tempdir()?;
    let base = src_dir.path();

    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "checksum-test"
version = "0.1.0"
abi_version = "2.0"
"#,
    )?;

    fs::write(base.join("README.md"), "Test")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"BINARY")?;

    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("checksum-test-0.1.0.tar.gz");
    let checksum_path = pack_dir(base, &out_path)?;

    // Verify passes initially
    assert!(verify_artifact(&out_path, Some(&checksum_path)).is_ok());

    // Modify the artifact
    let mut file = std::fs::OpenOptions::new().write(true).open(&out_path)?;
    file.write_all(b"CORRUPTED")?;
    drop(file);

    // Verify should now fail
    assert!(
        verify_artifact(&out_path, Some(&checksum_path)).is_err(),
        "Verification should fail for corrupted artifact"
    );

    Ok(())
}

// Registry integration tests

#[test]
fn test_registry_save_and_load() -> Result<()> {
    let temp_dir = tempdir()?;
    let registry_file = temp_dir.path().join("registry.json");

    // Create and populate registry
    let mut registry = LocalRegistry::new();

    for i in 0..3 {
        let entry = PluginRegistryEntry {
            plugin_id: format!("plugin-{}", i),
            name: format!("plugin-{}", i),
            version: format!("{}.0.0", i + 1),
            abi_version: "2.0".to_string(),
            description: Some(format!("Test plugin {}", i)),
            author: Some("test-author".to_string()),
            license: Some("MIT".to_string()),
            keywords: Some(vec!["test".to_string()]),
            dependencies: None,
        };
        registry.register(entry)?;
    }

    // Save registry
    RegistryPersistence::save(&registry, &registry_file)?;
    assert!(registry_file.exists(), "Registry file should be created");

    // Load registry
    let loaded = RegistryPersistence::load(&registry_file)?;

    // Verify contents
    assert_eq!(loaded.count(), 3, "Should have 3 plugins");
    for i in 0..3 {
        let name = format!("plugin-{}", i);
        assert!(
            loaded.find_by_name(&name).is_some(),
            "Plugin {} should exist",
            name
        );
    }

    Ok(())
}

#[test]
fn test_dependency_resolution_integration() -> Result<()> {
    // Create a registry with a dependency chain:
    // plugin-a depends on plugin-b
    // plugin-b depends on plugin-c
    // plugin-c has no dependencies

    let mut registry = LocalRegistry::new();

    // Register plugin-c (no dependencies)
    let entry_c = PluginRegistryEntry {
        plugin_id: "plugin-c".to_string(),
        name: "plugin-c".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "2.0".to_string(),
        description: Some("Base plugin".to_string()),
        author: None,
        license: None,
        keywords: None,
        dependencies: None,
    };
    registry.register(entry_c)?;

    // Register plugin-b (depends on plugin-c)
    let entry_b = PluginRegistryEntry {
        plugin_id: "plugin-b".to_string(),
        name: "plugin-b".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "2.0".to_string(),
        description: Some("Middle plugin".to_string()),
        author: None,
        license: None,
        keywords: None,
        dependencies: Some(vec![PluginDependency {
            name: "plugin-c".to_string(),
            version_requirement: "1.0.0".to_string(),
        }]),
    };
    registry.register(entry_b)?;

    // Register plugin-a (depends on plugin-b)
    let entry_a = PluginRegistryEntry {
        plugin_id: "plugin-a".to_string(),
        name: "plugin-a".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "2.0".to_string(),
        description: Some("Top plugin".to_string()),
        author: None,
        license: None,
        keywords: None,
        dependencies: Some(vec![PluginDependency {
            name: "plugin-b".to_string(),
            version_requirement: "1.0.0".to_string(),
        }]),
    };
    registry.register(entry_a)?;

    // Resolve dependencies for plugin-a
    let resolver = DependencyResolver::new(registry);
    let resolution = resolver.resolve("plugin-a", "1.0.0")?;

    // Should have install order: [c, b, a] (dependencies first)
    assert_eq!(
        resolution.install_order,
        vec!["plugin-c", "plugin-b", "plugin-a"]
    );

    // Should have no unmet dependencies
    assert_eq!(resolution.unmet_dependencies.len(), 0);

    Ok(())
}

#[test]
fn test_dependency_resolution_with_version_matching() -> Result<()> {
    let mut registry = LocalRegistry::new();

    // Register multiple versions of dependency
    for version in &["0.9.0", "1.0.0", "1.5.0", "2.0.0"] {
        let entry = PluginRegistryEntry {
            plugin_id: "base-plugin".to_string(),
            name: "base-plugin".to_string(),
            version: version.to_string(),
            abi_version: "2.0".to_string(),
            description: None,
            author: None,
            license: None,
            keywords: None,
            dependencies: None,
        };
        registry.register(entry)?;
    }

    // Plugin depends on "^1.0.0" (>=1.0.0, <2.0.0)
    let entry_consumer = PluginRegistryEntry {
        plugin_id: "consumer-plugin".to_string(),
        name: "consumer-plugin".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "2.0".to_string(),
        description: None,
        author: None,
        license: None,
        keywords: None,
        dependencies: Some(vec![PluginDependency {
            name: "base-plugin".to_string(),
            version_requirement: "^1.0.0".to_string(),
        }]),
    };
    registry.register(entry_consumer)?;

    // Resolve - should pick latest matching version (1.5.0)
    let resolver = DependencyResolver::new(registry);
    let resolution = resolver.resolve("consumer-plugin", "1.0.0")?;

    assert_eq!(
        resolution.version_map.get("base-plugin"),
        Some(&"1.5.0".to_string()),
        "Should resolve to latest matching version"
    );
    assert_eq!(resolution.unmet_dependencies.len(), 0);

    Ok(())
}

#[test]
fn test_dependency_resolution_missing_dependency() -> Result<()> {
    let mut registry = LocalRegistry::new();

    // Register plugin that depends on non-existent plugin
    let entry = PluginRegistryEntry {
        plugin_id: "consumer".to_string(),
        name: "consumer".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "2.0".to_string(),
        description: None,
        author: None,
        license: None,
        keywords: None,
        dependencies: Some(vec![PluginDependency {
            name: "non-existent".to_string(),
            version_requirement: ">=1.0.0".to_string(),
        }]),
    };
    registry.register(entry)?;

    // Resolve
    let resolver = DependencyResolver::new(registry);
    let resolution = resolver.resolve("consumer", "1.0.0")?;

    // Should report unmet dependency
    assert_eq!(resolution.unmet_dependencies.len(), 1);
    assert_eq!(resolution.unmet_dependencies[0].plugin_name, "non-existent");
    assert_eq!(resolution.unmet_dependencies[0].required_by, "consumer");

    Ok(())
}

#[test]
fn test_version_requirement_various_formats() -> Result<()> {
    // Test exact match
    let req = VersionRequirement::new("1.0.0".to_string());
    assert!(req.matches("1.0.0"));
    assert!(!req.matches("1.0.1"));

    // Test greater than
    let req = VersionRequirement::new(">1.0.0".to_string());
    assert!(req.matches("1.0.1"));
    assert!(!req.matches("1.0.0"));

    // Test caret (major version locked)
    let req = VersionRequirement::new("^1.2.3".to_string());
    assert!(req.matches("1.2.3"));
    assert!(req.matches("1.5.0"));
    assert!(!req.matches("2.0.0"));

    // Test tilde (minor version locked)
    let req = VersionRequirement::new("~1.2.3".to_string());
    assert!(req.matches("1.2.3"));
    assert!(req.matches("1.2.5"));
    assert!(!req.matches("1.3.0"));

    // Test wildcard
    let req = VersionRequirement::new("1.2.x".to_string());
    assert!(req.matches("1.2.0"));
    assert!(req.matches("1.2.999"));
    assert!(!req.matches("1.3.0"));

    Ok(())
}

#[test]
fn test_registry_search_functionality() -> Result<()> {
    let mut registry = LocalRegistry::new();

    // Register plugins with different metadata
    for i in 0..5 {
        let keywords = if i % 2 == 0 {
            Some(vec!["search".to_string(), "test".to_string()])
        } else {
            Some(vec!["other".to_string()])
        };

        let entry = PluginRegistryEntry {
            plugin_id: format!("plugin-{}", i),
            name: format!("plugin-{}", i),
            version: "1.0.0".to_string(),
            abi_version: "2.0".to_string(),
            description: Some(format!("This is a searchable plugin number {}", i)),
            author: None,
            license: None,
            keywords,
            dependencies: None,
        };
        registry.register(entry)?;
    }

    // Search by name
    let results = registry.search("plugin-0");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "plugin-0");

    // Search by keyword
    let results = registry.search("keyword-search");
    assert_eq!(
        results.len(),
        0,
        "Should not find plugins with keyword-search"
    );

    // Search by exact keyword
    let results = registry.search("test");
    assert_eq!(results.len(), 3); // plugins 0, 2, 4 have "test" keyword

    // Search by description substring (searchable)
    let results = registry.search("searchable");
    assert_eq!(results.len(), 5); // all have "searchable" in description

    Ok(())
}

#[test]
fn test_upgrade_version_detection() -> Result<()> {
    // Test semantic version parsing
    let v1 = SemanticVersion::parse("1.0.0")?;
    let v2 = SemanticVersion::parse("1.1.0")?;
    let v3 = SemanticVersion::parse("2.0.0")?;

    assert!(!v1.is_newer_than(&v1));
    assert!(v2.is_newer_than(&v1));
    assert!(v3.is_newer_than(&v2));

    // Test breaking change detection
    assert!(!v2.is_breaking_change(&v1)); // Minor bump
    assert!(v3.is_breaking_change(&v2)); // Major bump

    Ok(())
}

#[test]
fn test_upgrade_info_creation() -> Result<()> {
    let upgrade = UpgradeInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        "1.1.0".to_string(),
    )?;

    assert_eq!(upgrade.name, "test-plugin");
    assert_eq!(upgrade.current_version, "1.0.0");
    assert_eq!(upgrade.new_version, "1.1.0");
    assert!(!upgrade.is_breaking);

    // Test breaking change
    let breaking_upgrade = UpgradeInfo::new(
        "test-plugin".to_string(),
        "1.0.0".to_string(),
        "2.0.0".to_string(),
    )?;

    assert!(breaking_upgrade.is_breaking);

    Ok(())
}

#[test]
fn test_backup_manager_lifecycle() -> Result<()> {
    let backup_dir = tempdir()?;
    let plugin_dir = tempdir()?;
    let mut manager = BackupManager::new(backup_dir.path().to_path_buf())?;

    // Create a simple plugin directory
    let plugin_path = plugin_dir.path().join("test-plugin");
    fs::create_dir(&plugin_path)?;
    fs::write(plugin_path.join("plugin.so"), b"binary data")?;
    fs::write(
        plugin_path.join("plugin.toml"),
        "[package]\nname = \"test\"",
    )?;

    // Test backup creation
    let backup_path = manager.backup_plugin("test-plugin", "1.0.0", &plugin_path)?;
    assert!(backup_path.exists());
    assert!(backup_path.join("plugin.so").exists());
    assert!(backup_path.join("plugin.toml").exists());

    // Test listing backups
    let backups = manager.list_backups("test-plugin");
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].version, "1.0.0");

    // Test backup count
    assert_eq!(manager.count_backups(), 1);

    Ok(())
}

#[test]
fn test_upgrade_availability_check() -> Result<()> {
    // Test newer version available
    assert!(UpgradeInfo::is_available("1.0.0", "1.1.0")?);
    assert!(UpgradeInfo::is_available("1.0.0", "2.0.0")?);

    // Test same or older version
    assert!(!UpgradeInfo::is_available("1.0.0", "1.0.0")?);
    assert!(!UpgradeInfo::is_available("1.1.0", "1.0.0")?);
    assert!(!UpgradeInfo::is_available("2.0.0", "1.9.9")?);

    Ok(())
}

#[test]
fn test_upgrade_with_breaking_changes() -> Result<()> {
    let minor_upgrade = UpgradeInfo::new(
        "plugin".to_string(),
        "1.5.0".to_string(),
        "1.6.0".to_string(),
    )?;

    let major_upgrade = UpgradeInfo::new(
        "plugin".to_string(),
        "1.5.0".to_string(),
        "2.0.0".to_string(),
    )?;

    // Minor version bump is not breaking
    assert!(!minor_upgrade.is_breaking);

    // Major version bump is breaking
    assert!(major_upgrade.is_breaking);

    Ok(())
}

#[test]
fn test_version_prerelease_handling() -> Result<()> {
    let release = SemanticVersion::parse("1.0.0")?;
    let prerelease = SemanticVersion::parse("1.0.0-beta")?;

    // Release version is newer than prerelease
    assert!(release.is_newer_than(&prerelease));
    assert!(!prerelease.is_newer_than(&release));

    Ok(())
}

#[test]
fn test_metadata_abi_v2_conversion() -> Result<()> {
    use plugin_packager::PluginMetadata;

    let metadata = PluginMetadata {
        name: "test-plugin".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "2.0".to_string(),
        description: Some("Test plugin for ABI v2".to_string()),
        authors: Some(vec!["Test Author".to_string()]),
        license: Some("MIT".to_string()),
        keywords: Some(vec!["test".to_string()]),
        categories: None,
        repository: Some("https://github.com/test/plugin".to_string()),
        homepage: None,
        documentation: None,
        capabilities: Some(vec!["http_request".to_string()]),
        requirements: None,
        dependencies: None,
    };

    // Convert to ABI compatible
    let abi_info = metadata.to_abi_compatible()?;

    assert_eq!(abi_info.name, "test-plugin");
    assert_eq!(abi_info.version, "1.0.0");
    assert!(abi_info.capabilities.len() > 0);
    assert_eq!(abi_info.author, Some("Test Author".to_string()));

    Ok(())
}

#[test]
fn test_metadata_abi_v2_compatibility_check() -> Result<()> {
    use plugin_packager::PluginMetadata;

    // V2.0 plugin
    let v2_plugin = PluginMetadata {
        name: "v2-plugin".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "2.0".to_string(),
        description: None,
        authors: None,
        license: None,
        keywords: None,
        categories: None,
        repository: None,
        homepage: None,
        documentation: None,
        capabilities: None,
        requirements: None,
        dependencies: None,
    };

    assert!(v2_plugin.is_abi_v2_compatible()?);

    // V1.0 plugin (not compatible)
    let v1_plugin = PluginMetadata {
        name: "v1-plugin".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "1.0".to_string(),
        description: None,
        authors: None,
        license: None,
        keywords: None,
        categories: None,
        repository: None,
        homepage: None,
        documentation: None,
        capabilities: None,
        requirements: None,
        dependencies: None,
    };

    assert!(!v1_plugin.is_abi_v2_compatible()?);

    Ok(())
}

#[test]
fn test_metadata_abi_validation() -> Result<()> {
    use plugin_packager::PluginMetadata;

    let metadata = PluginMetadata {
        name: "test-plugin".to_string(),
        version: "1.0.0".to_string(),
        abi_version: "2.0".to_string(),
        description: Some("Complete test plugin".to_string()),
        authors: Some(vec!["Author".to_string()]),
        license: Some("MIT".to_string()),
        keywords: None,
        categories: None,
        repository: None,
        homepage: None,
        documentation: None,
        capabilities: Some(vec!["http_request".to_string(), "file_read".to_string()]),
        requirements: None,
        dependencies: None,
    };

    let validation = metadata.validate_abi_compatibility()?;

    // Should be compatible
    assert!(validation.is_compatible);

    Ok(())
}

// RFC-0003: Cross-Platform Plugin Packaging Tests

#[test]
fn test_cross_platform_packaging_linux() -> Result<()> {
    use plugin_packager::platform::Platform;
    use plugin_packager::{is_valid_artifact_filename, pack_dir, verify_artifact};

    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin.toml
    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "linux-plugin"
version = "1.0.0"
abi_version = "2.0"
description = "Linux test plugin"
"#,
    )?;

    fs::write(base.join("README.md"), "# Linux Plugin")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"LINUX_BINARY")?;

    // Package
    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("linux-plugin-1.0.0.tar.gz");
    let checksum_path = pack_dir(base, &out_path)?;

    // Verify
    verify_artifact(&out_path, Some(&checksum_path))?;

    // Verify platform detection
    assert!(is_valid_artifact_filename("plugin.so"));
    assert_eq!(Platform::Linux.artifact_filename(), "plugin.so");

    Ok(())
}

#[test]
fn test_cross_platform_packaging_windows() -> Result<()> {
    use plugin_packager::platform::Platform;
    use plugin_packager::{is_valid_artifact_filename, pack_dir, verify_artifact};

    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin.toml
    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "windows-plugin"
version = "1.0.0"
abi_version = "2.0"
description = "Windows test plugin"
"#,
    )?;

    fs::write(base.join("README.md"), "# Windows Plugin")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.dll"), b"WINDOWS_BINARY")?;

    // Package
    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("windows-plugin-1.0.0.tar.gz");
    let checksum_path = pack_dir(base, &out_path)?;

    // Verify
    verify_artifact(&out_path, Some(&checksum_path))?;

    // Verify platform detection
    assert!(is_valid_artifact_filename("plugin.dll"));
    assert_eq!(Platform::Windows.artifact_filename(), "plugin.dll");

    Ok(())
}

#[test]
fn test_cross_platform_packaging_macos() -> Result<()> {
    use plugin_packager::platform::Platform;
    use plugin_packager::{is_valid_artifact_filename, pack_dir, verify_artifact};

    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin.toml
    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "macos-plugin"
version = "1.0.0"
abi_version = "2.0"
description = "macOS test plugin"
"#,
    )?;

    fs::write(base.join("README.md"), "# macOS Plugin")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.dylib"), b"MACOS_BINARY")?;

    // Package
    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("macos-plugin-1.0.0.tar.gz");
    let checksum_path = pack_dir(base, &out_path)?;

    // Verify
    verify_artifact(&out_path, Some(&checksum_path))?;

    // Verify platform detection
    assert!(is_valid_artifact_filename("plugin.dylib"));
    assert_eq!(Platform::Macos.artifact_filename(), "plugin.dylib");

    Ok(())
}

#[test]
fn test_artifact_metadata_parsing() -> Result<()> {
    use plugin_packager::platform::Platform;
    use plugin_packager::ArtifactMetadata;

    // Linux artifact
    let meta = ArtifactMetadata::parse("myplugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz")?;
    assert_eq!(meta.name, "myplugin");
    assert_eq!(meta.version, "1.0.0");
    assert_eq!(meta.target_triple, "x86_64-unknown-linux-gnu");
    assert_eq!(meta.platform, Platform::Linux);

    // Windows artifact
    let meta = ArtifactMetadata::parse("myplugin-v2.0.0-x86_64-pc-windows-gnu.tar.gz")?;
    assert_eq!(meta.platform, Platform::Windows);

    // macOS artifact (Intel)
    let meta = ArtifactMetadata::parse("myplugin-v0.1.0-x86_64-apple-darwin.tar.gz")?;
    assert_eq!(meta.platform, Platform::Macos);

    // macOS artifact (ARM)
    let meta = ArtifactMetadata::parse("myplugin-v0.1.0-aarch64-apple-darwin.tar.gz")?;
    assert_eq!(meta.platform, Platform::Macos);

    Ok(())
}

#[test]
fn test_artifact_metadata_to_filename() -> Result<()> {
    use plugin_packager::platform::Platform;
    use plugin_packager::ArtifactMetadata;

    let meta = ArtifactMetadata {
        name: "test-plugin".to_string(),
        version: "1.2.3".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        platform: Platform::Linux,
    };

    assert_eq!(
        meta.to_filename(),
        "test-plugin-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"
    );

    Ok(())
}

#[test]
fn test_platform_from_target_triple() -> Result<()> {
    use plugin_packager::platform::Platform;

    // Linux targets
    assert_eq!(
        Platform::from_target_triple("x86_64-unknown-linux-gnu"),
        Some(Platform::Linux)
    );
    assert_eq!(
        Platform::from_target_triple("aarch64-unknown-linux-gnu"),
        Some(Platform::Linux)
    );
    assert_eq!(
        Platform::from_target_triple("i686-unknown-linux-musl"),
        Some(Platform::Linux)
    );

    // Windows targets
    assert_eq!(
        Platform::from_target_triple("x86_64-pc-windows-gnu"),
        Some(Platform::Windows)
    );
    assert_eq!(
        Platform::from_target_triple("x86_64-pc-windows-msvc"),
        Some(Platform::Windows)
    );
    assert_eq!(
        Platform::from_target_triple("i686-pc-windows-gnu"),
        Some(Platform::Windows)
    );

    // macOS targets
    assert_eq!(
        Platform::from_target_triple("x86_64-apple-darwin"),
        Some(Platform::Macos)
    );
    assert_eq!(
        Platform::from_target_triple("aarch64-apple-darwin"),
        Some(Platform::Macos)
    );

    // Unknown
    assert_eq!(Platform::from_target_triple("unknown-unknown"), None);

    Ok(())
}

#[test]
fn test_artifact_metadata_invalid_cases() -> Result<()> {
    use plugin_packager::ArtifactMetadata;

    // Missing .tar.gz extension
    assert!(ArtifactMetadata::parse("myplugin-v1.0.0-x86_64-unknown-linux-gnu").is_err());

    // Missing 'v' prefix on version
    assert!(ArtifactMetadata::parse("myplugin-1.0.0-x86_64-unknown-linux-gnu.tar.gz").is_err());

    // Uppercase plugin name
    assert!(ArtifactMetadata::parse("MyPlugin-v1.0.0-x86_64-unknown-linux-gnu.tar.gz").is_err());

    // Invalid version format
    assert!(ArtifactMetadata::parse("myplugin-v1.0-x86_64-unknown-linux-gnu.tar.gz").is_err());

    // Unknown platform
    assert!(ArtifactMetadata::parse("myplugin-v1.0.0-unknown-unknown.tar.gz").is_err());

    Ok(())
}

#[test]
fn test_cross_compilation_packaging() -> Result<()> {
    use plugin_packager::platform::Platform;
    use plugin_packager::{pack_dir, verify_artifact};

    // Test packaging a Windows DLL on Linux (cross-compilation scenario)
    let src_dir = tempdir()?;
    let base = src_dir.path();

    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "cross-compiled"
version = "0.1.0"
abi_version = "2.0"
description = "Cross-compiled plugin"
"#,
    )?;

    fs::write(base.join("README.md"), "# Cross-compiled")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    // Even though we're on Linux, we can package a Windows DLL
    fs::write(base.join("plugin.dll"), b"WINDOWS_CROSS_COMPILED")?;

    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("cross-compiled-0.1.0.tar.gz");
    let checksum_path = pack_dir(base, &out_path)?;

    // Verify should accept .dll even on Linux
    verify_artifact(&out_path, Some(&checksum_path))?;

    // Confirm Windows platform detection works
    assert_eq!(Platform::Windows.artifact_extension(), "dll");

    Ok(())
}

// RFC-0003 Task 0003.2: Naming Convention Validation Tests

#[test]
fn test_pack_dir_rejects_uppercase_name() -> Result<()> {
    use plugin_packager::pack_dir;

    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin with uppercase name (invalid)
    fs::write(
        base.join("plugin.toml"),
        r#"name = "InvalidName"
version = "1.0.0"
abi_version = "2.0"
"#,
    )?;
    fs::write(base.join("README.md"), "Test")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"binary")?;

    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("invalid.tar.gz");

    // Should fail with helpful error message
    let result = pack_dir(base, &out_path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("RFC-0003") || err.contains("lowercase"));

    Ok(())
}

#[test]
fn test_pack_dir_rejects_bad_version() -> Result<()> {
    use plugin_packager::pack_dir;

    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin with bad version (invalid)
    fs::write(
        base.join("plugin.toml"),
        r#"name = "test-plugin"
version = "1.0"
abi_version = "2.0"
"#,
    )?;
    fs::write(base.join("README.md"), "Test")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"binary")?;

    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("invalid.tar.gz");

    // Should fail with helpful error message
    let result = pack_dir(base, &out_path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("RFC-0003") || err.contains("semantic versioning"));

    Ok(())
}

#[test]
fn test_pack_dir_includes_optional_changelog() -> Result<()> {
    use flate2::read::GzDecoder;
    use plugin_packager::pack_dir;
    use tar::Archive;

    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin with CHANGELOG.md
    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "with-changelog"
version = "1.0.0"
abi_version = "2.0"
"#,
    )?;
    fs::write(base.join("README.md"), "Test")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"binary")?;
    fs::write(
        base.join("CHANGELOG.md"),
        "# Changelog\n\n## v1.0.0\n- Initial release",
    )?;

    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("with-changelog-1.0.0.tar.gz");
    pack_dir(base, &out_path)?;

    // Verify CHANGELOG.md is included
    let f = File::open(&out_path)?;
    let dec = GzDecoder::new(f);
    let mut ar = Archive::new(dec);

    let mut found_changelog = false;
    for entry in ar.entries()? {
        let entry = entry?;
        if entry.path()?.ends_with("CHANGELOG.md") {
            found_changelog = true;
            break;
        }
    }
    assert!(
        found_changelog,
        "CHANGELOG.md should be included in artifact"
    );

    Ok(())
}

#[test]
fn test_pack_dir_includes_optional_doc_directory() -> Result<()> {
    use flate2::read::GzDecoder;
    use plugin_packager::pack_dir;
    use tar::Archive;

    let src_dir = tempdir()?;
    let base = src_dir.path();

    // Create plugin with doc/ directory
    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "with-docs"
version = "1.0.0"
abi_version = "2.0"
"#,
    )?;
    fs::write(base.join("README.md"), "Test")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"binary")?;

    // Create doc directory with files
    fs::create_dir_all(base.join("doc"))?;
    fs::write(base.join("doc/api.md"), "# API Documentation")?;
    fs::write(base.join("doc/guide.md"), "# User Guide")?;

    let out_dir = tempdir()?;
    let out_path = out_dir.path().join("with-docs-1.0.0.tar.gz");
    pack_dir(base, &out_path)?;

    // Verify doc/ files are included
    let f = File::open(&out_path)?;
    let dec = GzDecoder::new(f);
    let mut ar = Archive::new(dec);

    let mut found_api_doc = false;
    let mut found_guide_doc = false;
    for entry in ar.entries()? {
        let entry = entry?;
        let path = entry.path()?;
        if path.ends_with("api.md") {
            found_api_doc = true;
        }
        if path.ends_with("guide.md") {
            found_guide_doc = true;
        }
    }
    assert!(found_api_doc, "doc/api.md should be included");
    assert!(found_guide_doc, "doc/guide.md should be included");

    Ok(())
}

#[test]
fn test_pack_dir_with_target_creates_rfc_compliant_name() -> Result<()> {
    use plugin_packager::pack_dir_with_target;

    let src_dir = tempdir()?;
    let base = src_dir.path();

    fs::write(
        base.join("plugin.toml"),
        r#"[package]
name = "rfc-test"
version = "2.0.0"
abi_version = "2.0"
"#,
    )?;
    fs::write(base.join("README.md"), "Test")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"binary")?;

    let out_dir = tempdir()?;
    let checksum_path = pack_dir_with_target(base, out_dir.path(), "x86_64-unknown-linux-gnu")?;

    // Verify checksum path is correct
    assert!(checksum_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains("rfc-test-v2.0.0-x86_64-unknown-linux-gnu.tar.gz.sha256"));

    // Verify artifact was created with correct name
    let artifact_path = out_dir
        .path()
        .join("rfc-test-v2.0.0-x86_64-unknown-linux-gnu.tar.gz");
    assert!(
        artifact_path.exists(),
        "RFC-0003 compliant artifact should exist"
    );

    Ok(())
}

// ============================================================================
// RFC-0003 Task 3: Registry Publishing and skylet-pack CLI Tests
// ============================================================================

/// Test: ArtifactPublisher creation with config
#[test]
fn test_publish_config_creation() {
    use plugin_packager::publish::{ArtifactPublisher, PublishConfig};

    let config = PublishConfig {
        registry_url: "https://marketplace.example.com".to_string(),
        auth_token: "test-token".to_string(),
        skip_verify: false,
        as_draft: false,
        sign: false,
        key_id: None,
    };

    let publisher = ArtifactPublisher::new(config);
    assert!(
        publisher.is_authenticated(),
        "Publisher should be authenticated with token"
    );
}

/// Test: ArtifactPublisher validate method works
#[test]
fn test_publish_validate_artifact() -> Result<()> {
    use plugin_packager::publish::{ArtifactPublisher, PublishConfig};

    let dir = tempdir()?;
    let base = dir.path();

    // Create a valid plugin
    fs::write(
        base.join("plugin.toml"),
        r#"
[package]
name = "validate-test"
version = "1.0.0"
abi_version = "2"
"#,
    )?;
    fs::write(base.join("README.md"), "Test")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"binary")?;

    let out_dir = tempdir()?;
    let _checksum = pack_dir_with_target(base, out_dir.path(), "x86_64-unknown-linux-gnu")?;

    let artifact_path = out_dir
        .path()
        .join("validate-test-v1.0.0-x86_64-unknown-linux-gnu.tar.gz");
    assert!(artifact_path.exists(), "Artifact should be created");

    // Create publisher with dummy config
    let config = PublishConfig {
        registry_url: "http://localhost".to_string(),
        auth_token: "".to_string(),
        skip_verify: false,
        as_draft: false,
        sign: false,
        key_id: None,
    };

    let publisher = ArtifactPublisher::new(config);

    // Validate should succeed
    let result = publisher.validate(&artifact_path)?;
    assert_eq!(result.metadata.name, "validate-test");
    assert_eq!(result.metadata.version, "1.0.0");
    assert_eq!(result.metadata.target_triple, "x86_64-unknown-linux-gnu");
    assert!(!result.checksum.is_empty(), "Checksum should be computed");

    Ok(())
}

/// Test: ArtifactPublisher validate rejects invalid artifact name
#[test]
fn test_publish_validate_rejects_invalid_name() -> Result<()> {
    use plugin_packager::publish::{ArtifactPublisher, PublishConfig};

    let dir = tempdir()?;

    // Create a file with invalid RFC-0003 name
    let bad_artifact = dir.path().join("invalid-artifact-name.tar.gz");
    fs::write(&bad_artifact, "not a real artifact")?;

    let config = PublishConfig {
        registry_url: "http://localhost".to_string(),
        auth_token: "".to_string(),
        skip_verify: false,
        as_draft: false,
        sign: false,
        key_id: None,
    };

    let publisher = ArtifactPublisher::new(config);

    // Validate should fail
    let result = publisher.validate(&bad_artifact);
    assert!(
        result.is_err(),
        "Should reject artifact with invalid name format"
    );

    Ok(())
}

/// Test: ArtifactPublisher validate rejects missing artifact
#[test]
fn test_publish_validate_rejects_missing_file() {
    use plugin_packager::publish::{ArtifactPublisher, PublishConfig};

    let config = PublishConfig {
        registry_url: "http://localhost".to_string(),
        auth_token: "".to_string(),
        skip_verify: false,
        as_draft: false,
        sign: false,
        key_id: None,
    };

    let publisher = ArtifactPublisher::new(config);

    // Validate missing file should fail
    let result = publisher.validate(Path::new("/nonexistent/artifact.tar.gz"));
    assert!(result.is_err(), "Should reject missing artifact file");
}

/// Test: PublishConfig skip_verify option
#[test]
fn test_publish_config_skip_verify() {
    use plugin_packager::publish::PublishConfig;

    let config = PublishConfig {
        registry_url: "https://example.com".to_string(),
        auth_token: "token".to_string(),
        skip_verify: true,
        as_draft: false,
        sign: false,
        key_id: None,
    };

    assert!(config.skip_verify, "skip_verify should be true");
}

/// Test: PublishConfig as_draft option
#[test]
fn test_publish_config_as_draft() {
    use plugin_packager::publish::PublishConfig;

    let config = PublishConfig {
        registry_url: "https://example.com".to_string(),
        auth_token: "token".to_string(),
        skip_verify: false,
        as_draft: true,
        sign: false,
        key_id: None,
    };

    assert!(config.as_draft, "as_draft should be true");
}

/// Test: PublishConfig sign option
#[test]
fn test_publish_config_sign_with_key() {
    use plugin_packager::publish::PublishConfig;

    let config = PublishConfig {
        registry_url: "https://example.com".to_string(),
        auth_token: "token".to_string(),
        skip_verify: false,
        as_draft: false,
        sign: true,
        key_id: Some("key-123".to_string()),
    };

    assert!(config.sign, "sign should be true");
    assert_eq!(config.key_id, Some("key-123".to_string()));
}

/// Test: LocalArtifact contains expected metadata
#[test]
fn test_local_artifact_metadata() -> Result<()> {
    use plugin_packager::publish::{ArtifactPublisher, PublishConfig};

    let dir = tempdir()?;
    let base = dir.path();

    // Create a valid plugin
    fs::write(
        base.join("plugin.toml"),
        r#"
[package]
name = "metadata-test"
version = "3.2.1"
abi_version = "2"
"#,
    )?;
    fs::write(base.join("README.md"), "Test")?;
    fs::write(base.join("LICENSE"), "MIT")?;
    fs::write(base.join("plugin.so"), b"binary")?;

    let out_dir = tempdir()?;
    let _checksum = pack_dir_with_target(base, out_dir.path(), "x86_64-pc-windows-gnu")?;

    let artifact_path = out_dir
        .path()
        .join("metadata-test-v3.2.1-x86_64-pc-windows-gnu.tar.gz");
    assert!(artifact_path.exists(), "Artifact should be created");

    let config = PublishConfig {
        registry_url: "http://localhost".to_string(),
        auth_token: "".to_string(),
        skip_verify: false,
        as_draft: false,
        sign: false,
        key_id: None,
    };

    let publisher = ArtifactPublisher::new(config);
    let local_artifact = publisher.validate(&artifact_path)?;

    // Check metadata is correct
    assert_eq!(local_artifact.metadata.name, "metadata-test");
    assert_eq!(local_artifact.metadata.version, "3.2.1");
    assert_eq!(
        local_artifact.metadata.target_triple,
        "x86_64-pc-windows-gnu"
    );

    // Checksum should be 64 hex characters (SHA256)
    assert_eq!(local_artifact.checksum.len(), 64);
    assert!(local_artifact
        .checksum
        .chars()
        .all(|c| c.is_ascii_hexdigit()));

    Ok(())
}
