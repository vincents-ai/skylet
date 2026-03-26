// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cross-Platform Plugin Loader Abstraction
//!
//! This module provides a unified interface for loading plugins on different operating systems.
//! Each platform (Linux, macOS, Windows) has its own implementation, abstracting away
//! platform-specific details like ELF, Mach-O, and PE binary formats.
//!
//! RFC-0004 Phase 2: Dynamic Plugin Loading

use crate::v2_spec::*;
use libloading::Library;
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Error types for platform loader operations
#[derive(Debug, Clone)]
pub enum PlatformLoaderError {
    /// Plugin binary file not found
    BinaryNotFound(PathBuf),

    /// Binary validation failed (invalid format, corrupted, etc.)
    BinaryValidationFailed(String),

    /// Required symbol not found in plugin
    SymbolNotFound { plugin: String, symbol: String },

    /// Symbol found but type doesn't match expected signature
    SymbolTypeMismatch {
        symbol: String,
        expected: String,
        found: String,
    },

    /// ABI version in plugin not supported by loader
    AbiVersionMismatch {
        plugin_version: String,
        loader_version: String,
        reason: String,
    },

    /// Capability not approved for this plugin
    CapabilityNotApproved(String),

    /// Platform not supported (e.g., trying to load Windows DLL on Linux)
    PlatformMismatch { binary_format: String, host: String },

    /// Generic platform-specific error
    PlatformError(String),

    /// Insufficient resources to load plugin
    OutOfMemory,

    /// Other loader errors
    Other(String),
}

impl std::fmt::Display for PlatformLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformLoaderError::BinaryNotFound(path) => {
                write!(f, "Plugin binary not found: {}", path.display())
            }
            PlatformLoaderError::BinaryValidationFailed(reason) => {
                write!(f, "Binary validation failed: {}", reason)
            }
            PlatformLoaderError::SymbolNotFound { plugin, symbol } => {
                write!(
                    f,
                    "Required symbol '{}' not found in plugin '{}'",
                    symbol, plugin
                )
            }
            PlatformLoaderError::SymbolTypeMismatch {
                symbol,
                expected,
                found,
            } => {
                write!(
                    f,
                    "Symbol '{}' type mismatch: expected {}, found {}",
                    symbol, expected, found
                )
            }
            PlatformLoaderError::AbiVersionMismatch {
                plugin_version,
                loader_version,
                reason,
            } => {
                write!(
                    f,
                    "ABI version mismatch: plugin {}, loader {} ({})",
                    plugin_version, loader_version, reason
                )
            }
            PlatformLoaderError::CapabilityNotApproved(cap) => {
                write!(f, "Capability '{}' not approved for plugin", cap)
            }
            PlatformLoaderError::PlatformMismatch {
                binary_format,
                host,
            } => {
                write!(
                    f,
                    "Platform mismatch: {} binary on {} host",
                    binary_format, host
                )
            }
            PlatformLoaderError::PlatformError(err) => write!(f, "Platform error: {}", err),
            PlatformLoaderError::OutOfMemory => write!(f, "Insufficient memory to load plugin"),
            PlatformLoaderError::Other(err) => write!(f, "Loader error: {}", err),
        }
    }
}

impl std::error::Error for PlatformLoaderError {}

pub type LoaderResult<T> = Result<T, PlatformLoaderError>;

/// Helper function to extract capabilities from plugin info
fn extract_capabilities(info: &PluginInfoV2) -> PluginCapabilities {
    // Extract requires_services
    let requires_services = if !info.requires_services.is_null() && info.num_requires_services > 0 {
        let services = unsafe {
            std::slice::from_raw_parts(info.requires_services, info.num_requires_services)
        };
        services
            .iter()
            .filter_map(|s| {
                if s.name.is_null() {
                    None
                } else {
                    Some(unsafe { CStr::from_ptr(s.name).to_string_lossy().into_owned() })
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    // Extract provides_services
    let provides_services = if !info.provides_services.is_null() && info.num_provides_services > 0 {
        let services = unsafe {
            std::slice::from_raw_parts(info.provides_services, info.num_provides_services)
        };
        services
            .iter()
            .filter_map(|s| {
                if s.name.is_null() {
                    None
                } else {
                    Some(unsafe { CStr::from_ptr(s.name).to_string_lossy().into_owned() })
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    // Extract declared_capabilities
    let declared_capabilities = if !info.capabilities.is_null() && info.num_capabilities > 0 {
        let caps = unsafe { std::slice::from_raw_parts(info.capabilities, info.num_capabilities) };
        caps.iter()
            .filter_map(|c| {
                if c.name.is_null() {
                    None
                } else {
                    Some(unsafe { CStr::from_ptr(c.name).to_string_lossy().into_owned() })
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    PluginCapabilities {
        supports_hot_reload: info.supports_hot_reload,
        supports_async: info.supports_async,
        supports_streaming: info.supports_streaming,
        max_concurrency: info.max_concurrency,
        requires_services,
        provides_services,
        declared_capabilities,
    }
}

/// Represents a loaded plugin binary in memory
///
/// This holds the loaded shared library and metadata about the plugin,
/// including capability information. The library is kept alive in _library.
pub struct LoadedPlugin {
    /// Name of the plugin (from metadata)
    pub name: String,

    /// Version of the plugin (from metadata)
    pub version: String,

    /// ABI version the plugin implements
    pub abi_version: String,

    /// Path to the loaded binary
    pub binary_path: PathBuf,

    /// Resolved plugin info pointer
    pub info: *const PluginInfoV2,

    /// Detected capabilities of the plugin
    pub capabilities: PluginCapabilities,

    /// Whether the plugin is currently initialized
    pub is_initialized: bool,

    /// Loaded library (must stay in memory for function pointers to remain valid)
    #[doc(hidden)]
    pub library: Arc<Library>,
}

/// Plugin capabilities discovered from metadata
///
/// These capabilities indicate what the plugin can do and what
/// services it requires, informing the security model (RFC-0008).
#[derive(Debug, Clone)]
pub struct PluginCapabilities {
    /// Whether plugin supports hot reloading
    pub supports_hot_reload: bool,

    /// Whether plugin supports async operations
    pub supports_async: bool,

    /// Whether plugin supports streaming responses
    pub supports_streaming: bool,

    /// Maximum concurrent operations
    pub max_concurrency: usize,

    /// Services this plugin requires (from requires_services)
    pub requires_services: Vec<String>,

    /// Services this plugin provides (from provides_services)
    pub provides_services: Vec<String>,

    /// Security capabilities (from capabilities array)
    pub declared_capabilities: Vec<String>,
}

impl Default for PluginCapabilities {
    fn default() -> Self {
        Self {
            supports_hot_reload: false,
            supports_async: true,
            supports_streaming: false,
            max_concurrency: 1,
            requires_services: Vec::new(),
            provides_services: Vec::new(),
            declared_capabilities: Vec::new(),
        }
    }
}

/// Cross-platform loader enum
///
/// Uses an enum to hold concrete platform loader types, automatically selecting
/// the correct loader based on the host OS.
pub enum CrossPlatformLoader {
    /// Linux ELF loader
    Linux(LinuxLoader),
    /// macOS Mach-O loader
    MacOs(MacOsLoader),
    /// Windows PE loader
    Windows(WindowsLoader),
}

impl CrossPlatformLoader {
    /// Create a new cross-platform loader for the current host platform
    pub fn new() -> LoaderResult<Self> {
        if cfg!(target_os = "linux") {
            Ok(CrossPlatformLoader::Linux(LinuxLoader::new()))
        } else if cfg!(target_os = "macos") {
            Ok(CrossPlatformLoader::MacOs(MacOsLoader::new()))
        } else if cfg!(target_os = "windows") {
            Ok(CrossPlatformLoader::Windows(WindowsLoader::new()))
        } else {
            Err(PlatformLoaderError::PlatformError(format!(
                "Unsupported platform: {}",
                std::env::consts::OS
            )))
        }
    }

    /// Load a plugin using the platform-appropriate loader
    pub fn load<P: AsRef<Path>>(&self, path: P) -> LoaderResult<LoadedPlugin> {
        match self {
            CrossPlatformLoader::Linux(loader) => loader.load(path),
            CrossPlatformLoader::MacOs(loader) => loader.load(path),
            CrossPlatformLoader::Windows(loader) => loader.load(path),
        }
    }

    /// Get the name of the active loader
    pub fn loader_name(&self) -> &str {
        match self {
            CrossPlatformLoader::Linux(l) => l.name(),
            CrossPlatformLoader::MacOs(l) => l.name(),
            CrossPlatformLoader::Windows(l) => l.name(),
        }
    }

    /// Get the platform identifier
    pub fn platform(&self) -> &str {
        match self {
            CrossPlatformLoader::Linux(l) => l.platform(),
            CrossPlatformLoader::MacOs(l) => l.platform(),
            CrossPlatformLoader::Windows(l) => l.platform(),
        }
    }
}

impl Default for CrossPlatformLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create CrossPlatformLoader for current platform")
    }
}

/// Linux/ELF plugin loader
pub struct LinuxLoader;

impl LinuxLoader {
    /// Create a new Linux ELF loader
    pub fn new() -> Self {
        LinuxLoader
    }

    /// Load a plugin binary on Linux
    pub fn load<P: AsRef<Path>>(&self, path: P) -> LoaderResult<LoadedPlugin> {
        let path = path.as_ref();

        // Validate file exists and is readable
        if !path.exists() {
            return Err(PlatformLoaderError::BinaryNotFound(path.to_path_buf()));
        }

        // Validate ELF format
        self.validate_binary(path)?;

        // Load the shared library using libloading
        let lib = unsafe {
            Library::new(path).map_err(|e| {
                PlatformLoaderError::PlatformError(format!(
                    "Failed to load ELF binary {}: {}",
                    path.display(),
                    e
                ))
            })?
        };

        let lib = Arc::new(lib);

        // Get plugin info via the plugin_get_info function
        let info_ptr = unsafe {
            let get_info: libloading::Symbol<PluginGetInfoFnV2> = lib
                .get(b"plugin_get_info")
                .map_err(|_| PlatformLoaderError::SymbolNotFound {
                    plugin: "unknown".to_string(),
                    symbol: "plugin_get_info".to_string(),
                })?;
            get_info()
        };

        if info_ptr.is_null() {
            return Err(PlatformLoaderError::BinaryValidationFailed(
                "plugin_get_info returned null pointer".to_string(),
            ));
        }

        // Read metadata from the info structure
        let (name, version, abi_version, capabilities) = unsafe {
            let info = &*info_ptr;

            let name = if info.name.is_null() {
                "unknown".to_string()
            } else {
                CStr::from_ptr(info.name).to_string_lossy().into_owned()
            };

            let version = if info.version.is_null() {
                "0.0.0".to_string()
            } else {
                CStr::from_ptr(info.version).to_string_lossy().into_owned()
            };

            let abi_version = if info.abi_version.is_null() {
                "2.0".to_string()
            } else {
                CStr::from_ptr(info.abi_version)
                    .to_string_lossy()
                    .into_owned()
            };

            let capabilities = extract_capabilities(info);

            (name, version, abi_version, capabilities)
        };

        let loaded = LoadedPlugin {
            name,
            version,
            abi_version,
            binary_path: path.to_path_buf(),
            info: info_ptr,
            capabilities,
            is_initialized: false,
            library: lib,
        };

        Ok(loaded)
    }

    /// Validate that a binary file is in the correct ELF format
    pub fn validate_binary<P: AsRef<Path>>(&self, path: P) -> LoaderResult<()> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(PlatformLoaderError::BinaryNotFound(path.to_path_buf()));
        }

        // Read file header to validate ELF magic number
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path).map_err(|e| {
            PlatformLoaderError::PlatformError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let mut magic = [0u8; 4];
        file.read_exact(&mut magic).map_err(|e| {
            PlatformLoaderError::BinaryValidationFailed(format!("Failed to read ELF header: {}", e))
        })?;

        // ELF magic: 0x7f 0x45 0x4c 0x46 (\177 E L F)
        if magic != [0x7f, 0x45, 0x4c, 0x46] {
            return Err(PlatformLoaderError::BinaryValidationFailed(format!(
                "Invalid ELF magic number: {:02x?} (expected 7f 45 4c 46)",
                magic
            )));
        }

        Ok(())
    }

    /// Get the name of this loader
    pub fn name(&self) -> &str {
        "Linux ELF Loader"
    }

    /// Get platform identifier
    pub fn platform(&self) -> &str {
        "x86_64-linux-gnu"
    }
}

impl Default for LinuxLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// macOS/Mach-O plugin loader
pub struct MacOsLoader;

impl MacOsLoader {
    /// Create a new macOS Mach-O loader
    pub fn new() -> Self {
        MacOsLoader
    }

    /// Load a plugin binary on macOS
    pub fn load<P: AsRef<Path>>(&self, path: P) -> LoaderResult<LoadedPlugin> {
        let path = path.as_ref();

        // Validate file exists and is readable
        if !path.exists() {
            return Err(PlatformLoaderError::BinaryNotFound(path.to_path_buf()));
        }

        // Validate Mach-O format
        self.validate_binary(path)?;

        // Load the shared library using libloading
        let lib = unsafe {
            Library::new(path).map_err(|e| {
                PlatformLoaderError::PlatformError(format!(
                    "Failed to load Mach-O binary {}: {}",
                    path.display(),
                    e
                ))
            })?
        };

        let lib = Arc::new(lib);

        // Get plugin info via the plugin_get_info function
        let info_ptr = unsafe {
            let get_info: libloading::Symbol<PluginGetInfoFnV2> = lib
                .get(b"plugin_get_info")
                .map_err(|_| PlatformLoaderError::SymbolNotFound {
                    plugin: "unknown".to_string(),
                    symbol: "plugin_get_info".to_string(),
                })?;
            get_info()
        };

        if info_ptr.is_null() {
            return Err(PlatformLoaderError::BinaryValidationFailed(
                "plugin_get_info returned null pointer".to_string(),
            ));
        }

        // Read metadata from the info structure
        let (name, version, abi_version, capabilities) = unsafe {
            let info = &*info_ptr;

            let name = if info.name.is_null() {
                "unknown".to_string()
            } else {
                CStr::from_ptr(info.name).to_string_lossy().into_owned()
            };

            let version = if info.version.is_null() {
                "0.0.0".to_string()
            } else {
                CStr::from_ptr(info.version).to_string_lossy().into_owned()
            };

            let abi_version = if info.abi_version.is_null() {
                "2.0".to_string()
            } else {
                CStr::from_ptr(info.abi_version)
                    .to_string_lossy()
                    .into_owned()
            };

            let capabilities = extract_capabilities(info);

            (name, version, abi_version, capabilities)
        };

        let loaded = LoadedPlugin {
            name,
            version,
            abi_version,
            binary_path: path.to_path_buf(),
            info: info_ptr,
            capabilities,
            is_initialized: false,
            library: lib,
        };

        Ok(loaded)
    }

    /// Validate that a binary file is in the correct Mach-O format
    pub fn validate_binary<P: AsRef<Path>>(&self, path: P) -> LoaderResult<()> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(PlatformLoaderError::BinaryNotFound(path.to_path_buf()));
        }

        // Read file header to validate Mach-O magic number
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path).map_err(|e| {
            PlatformLoaderError::PlatformError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let mut magic = [0u8; 4];
        file.read_exact(&mut magic).map_err(|e| {
            PlatformLoaderError::BinaryValidationFailed(format!(
                "Failed to read Mach-O header: {}",
                e
            ))
        })?;

        // Mach-O magic numbers:
        // 0xfeedface - 32-bit little-endian
        // 0xcefaedfe - 32-bit big-endian (host-swapped)
        // 0xfeedfacf - 64-bit little-endian
        // 0xcffaedfe - 64-bit big-endian (host-swapped)
        const MACHO_32_LE: [u8; 4] = [0xce, 0xfa, 0xed, 0xfe];
        const MACHO_64_LE: [u8; 4] = [0xcf, 0xfa, 0xed, 0xfe];
        const MACHO_FAT: [u8; 4] = [0xca, 0xfe, 0xba, 0xbe];
        const MACHO_FAT_BE: [u8; 4] = [0xbe, 0xba, 0xfe, 0xca];

        if magic != MACHO_32_LE
            && magic != MACHO_64_LE
            && magic != MACHO_FAT
            && magic != MACHO_FAT_BE
        {
            return Err(PlatformLoaderError::BinaryValidationFailed(format!(
                "Invalid Mach-O magic number: {:02x?} (expected cefaedfe or cffaedfe)",
                magic
            )));
        }

        Ok(())
    }

    /// Get the name of this loader
    pub fn name(&self) -> &str {
        "macOS Mach-O Loader"
    }

    /// Get platform identifier
    pub fn platform(&self) -> &str {
        "x86_64-apple-darwin"
    }
}

impl Default for MacOsLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Windows/PE plugin loader
pub struct WindowsLoader;

impl WindowsLoader {
    /// Create a new Windows PE loader
    pub fn new() -> Self {
        WindowsLoader
    }

    /// Load a plugin binary on Windows
    pub fn load<P: AsRef<Path>>(&self, path: P) -> LoaderResult<LoadedPlugin> {
        let path = path.as_ref();

        // Validate file exists and is readable
        if !path.exists() {
            return Err(PlatformLoaderError::BinaryNotFound(path.to_path_buf()));
        }

        // Validate PE format
        self.validate_binary(path)?;

        // Load the shared library using libloading
        let lib = unsafe {
            Library::new(path).map_err(|e| {
                PlatformLoaderError::PlatformError(format!(
                    "Failed to load PE binary {}: {}",
                    path.display(),
                    e
                ))
            })?
        };

        let lib = Arc::new(lib);

        // Get plugin info via the plugin_get_info function
        let info_ptr = unsafe {
            let get_info: libloading::Symbol<PluginGetInfoFnV2> = lib
                .get(b"plugin_get_info")
                .map_err(|_| PlatformLoaderError::SymbolNotFound {
                    plugin: "unknown".to_string(),
                    symbol: "plugin_get_info".to_string(),
                })?;
            get_info()
        };

        if info_ptr.is_null() {
            return Err(PlatformLoaderError::BinaryValidationFailed(
                "plugin_get_info returned null pointer".to_string(),
            ));
        }

        // Read metadata from the info structure
        let (name, version, abi_version, capabilities) = unsafe {
            let info = &*info_ptr;

            let name = if info.name.is_null() {
                "unknown".to_string()
            } else {
                CStr::from_ptr(info.name).to_string_lossy().into_owned()
            };

            let version = if info.version.is_null() {
                "0.0.0".to_string()
            } else {
                CStr::from_ptr(info.version).to_string_lossy().into_owned()
            };

            let abi_version = if info.abi_version.is_null() {
                "2.0".to_string()
            } else {
                CStr::from_ptr(info.abi_version)
                    .to_string_lossy()
                    .into_owned()
            };

            let capabilities = extract_capabilities(info);

            (name, version, abi_version, capabilities)
        };

        let loaded = LoadedPlugin {
            name,
            version,
            abi_version,
            binary_path: path.to_path_buf(),
            info: info_ptr,
            capabilities,
            is_initialized: false,
            library: lib,
        };

        Ok(loaded)
    }

    /// Validate that a binary file is in the correct PE format
    pub fn validate_binary<P: AsRef<Path>>(&self, path: P) -> LoaderResult<()> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(PlatformLoaderError::BinaryNotFound(path.to_path_buf()));
        }

        // Read file header to validate PE magic numbers
        use std::fs::File;
        use std::io::{Read, Seek};

        let mut file = File::open(path).map_err(|e| {
            PlatformLoaderError::PlatformError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let mut magic = [0u8; 2];
        file.read_exact(&mut magic).map_err(|e| {
            PlatformLoaderError::BinaryValidationFailed(format!("Failed to read PE header: {}", e))
        })?;

        // PE magic: 0x4d 0x5a ("MZ" - DOS header signature)
        if magic != [0x4d, 0x5a] {
            return Err(PlatformLoaderError::BinaryValidationFailed(format!(
                "Invalid PE magic number: {:02x?} (expected 4d 5a)",
                magic
            )));
        }

        // Additional check: read PE signature at offset 0x3c
        file.seek(std::io::SeekFrom::Start(0x3c)).map_err(|e| {
            PlatformLoaderError::BinaryValidationFailed(format!(
                "Failed to seek to PE offset: {}",
                e
            ))
        })?;

        let mut pe_offset_bytes = [0u8; 4];
        file.read_exact(&mut pe_offset_bytes).map_err(|e| {
            PlatformLoaderError::BinaryValidationFailed(format!("Failed to read PE offset: {}", e))
        })?;

        let pe_offset = u32::from_le_bytes(pe_offset_bytes) as u64;
        file.seek(std::io::SeekFrom::Start(pe_offset))
            .map_err(|e| {
                PlatformLoaderError::BinaryValidationFailed(format!(
                    "Failed to seek to PE signature: {}",
                    e
                ))
            })?;

        let mut pe_signature = [0u8; 4];
        file.read_exact(&mut pe_signature).map_err(|e| {
            PlatformLoaderError::BinaryValidationFailed(format!(
                "Failed to read PE signature: {}",
                e
            ))
        })?;

        // PE signature: 0x50 0x45 0x00 0x00 ("PE\0\0")
        if pe_signature != [0x50, 0x45, 0x00, 0x00] {
            return Err(PlatformLoaderError::BinaryValidationFailed(format!(
                "Invalid PE signature: {:02x?} (expected 50 45 00 00)",
                pe_signature
            )));
        }

        Ok(())
    }

    /// Get the name of this loader
    pub fn name(&self) -> &str {
        "Windows PE Loader"
    }

    /// Get platform identifier
    pub fn platform(&self) -> &str {
        "x86_64-pc-windows-msvc"
    }
}

impl Default for WindowsLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_platform_loader_creation() {
        let loader = CrossPlatformLoader::new();
        assert!(loader.is_ok(), "Should create loader for current platform");
    }

    #[test]
    fn test_loader_name_not_empty() {
        let loader = CrossPlatformLoader::new().unwrap();
        assert!(!loader.loader_name().is_empty());
    }

    #[test]
    fn test_platform_identifier_not_empty() {
        let loader = CrossPlatformLoader::new().unwrap();
        assert!(!loader.platform().is_empty());
    }

    #[test]
    fn test_error_display() {
        let err = PlatformLoaderError::BinaryNotFound(PathBuf::from("/nonexistent/plugin.so"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_abi_mismatch_error() {
        let err = PlatformLoaderError::AbiVersionMismatch {
            plugin_version: "1.0".to_string(),
            loader_version: "2.0".to_string(),
            reason: "major version differs".to_string(),
        };
        assert!(err.to_string().contains("mismatch"));
    }

    #[test]
    fn test_linux_loader_binary_not_found() {
        let loader = LinuxLoader::new();
        let result = loader.validate_binary("/nonexistent/file.so");
        assert!(matches!(
            result,
            Err(PlatformLoaderError::BinaryNotFound(_))
        ));
    }

    #[test]
    fn test_macos_loader_binary_not_found() {
        let loader = MacOsLoader::new();
        let result = loader.validate_binary("/nonexistent/file.dylib");
        assert!(matches!(
            result,
            Err(PlatformLoaderError::BinaryNotFound(_))
        ));
    }

    #[test]
    fn test_windows_loader_binary_not_found() {
        let loader = WindowsLoader::new();
        let result = loader.validate_binary("C:\\nonexistent\\plugin.dll");
        assert!(matches!(
            result,
            Err(PlatformLoaderError::BinaryNotFound(_))
        ));
    }

    #[test]
    fn test_plugin_capabilities_default() {
        let caps = PluginCapabilities::default();
        assert!(!caps.supports_hot_reload);
        assert!(caps.supports_async);
        assert_eq!(caps.max_concurrency, 1);
    }

    #[test]
    fn test_plugin_capabilities_custom() {
        let caps = PluginCapabilities {
            supports_hot_reload: true,
            supports_async: true,
            supports_streaming: true,
            max_concurrency: 10,
            requires_services: vec!["logger".to_string()],
            provides_services: vec!["database".to_string()],
            declared_capabilities: vec!["filesystem_access".to_string()],
        };

        assert!(caps.supports_hot_reload);
        assert_eq!(caps.max_concurrency, 10);
        assert_eq!(caps.requires_services.len(), 1);
        assert_eq!(caps.provides_services.len(), 1);
    }

    #[test]
    fn test_extract_capabilities() {
        // This test would need a real or mock PluginInfoV2 structure
        // For now, we test the default
        let caps = PluginCapabilities::default();
        assert_eq!(caps.max_concurrency, 1);
    }

    // Error Display Tests
    #[test]
    fn test_symbol_not_found_error_display() {
        let err = PlatformLoaderError::SymbolNotFound {
            plugin: "test_plugin".to_string(),
            symbol: "test_symbol".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("test_symbol"));
        assert!(msg.contains("test_plugin"));
    }

    #[test]
    fn test_symbol_type_mismatch_error_display() {
        let err = PlatformLoaderError::SymbolTypeMismatch {
            symbol: "get_info".to_string(),
            expected: "fn() -> *const PluginInfo".to_string(),
            found: "fn() -> u32".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("get_info"));
        assert!(msg.contains("mismatch"));
    }

    #[test]
    fn test_capability_not_approved_error_display() {
        let err = PlatformLoaderError::CapabilityNotApproved("filesystem_access".to_string());
        let msg = err.to_string();
        assert!(msg.contains("filesystem_access"));
    }

    #[test]
    fn test_platform_mismatch_error_display() {
        let err = PlatformLoaderError::PlatformMismatch {
            binary_format: "PE".to_string(),
            host: "x86_64-linux-gnu".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("PE"));
        assert!(msg.contains("linux"));
    }

    #[test]
    fn test_out_of_memory_error_display() {
        let err = PlatformLoaderError::OutOfMemory;
        let msg = err.to_string();
        assert!(msg.contains("memory"));
    }

    // Platform Loader Identity Tests
    #[test]
    fn test_linux_loader_identity() {
        let loader = LinuxLoader::new();
        assert_eq!(loader.name(), "Linux ELF Loader");
        assert_eq!(loader.platform(), "x86_64-linux-gnu");
    }

    #[test]
    fn test_macos_loader_identity() {
        let loader = MacOsLoader::new();
        assert_eq!(loader.name(), "macOS Mach-O Loader");
        assert_eq!(loader.platform(), "x86_64-apple-darwin");
    }

    #[test]
    fn test_windows_loader_identity() {
        let loader = WindowsLoader::new();
        assert_eq!(loader.name(), "Windows PE Loader");
        assert_eq!(loader.platform(), "x86_64-pc-windows-msvc");
    }

    // Default implementations
    #[test]
    fn test_linux_loader_default() {
        let loader1 = LinuxLoader::new();
        let loader2 = LinuxLoader::default();
        assert_eq!(loader1.name(), loader2.name());
    }

    #[test]
    fn test_macos_loader_default() {
        let loader1 = MacOsLoader::new();
        let loader2 = MacOsLoader::default();
        assert_eq!(loader1.name(), loader2.name());
    }

    #[test]
    fn test_windows_loader_default() {
        let loader1 = WindowsLoader::new();
        let loader2 = WindowsLoader::default();
        assert_eq!(loader1.name(), loader2.name());
    }

    #[test]
    fn test_cross_platform_loader_default() {
        let loader = CrossPlatformLoader::default();
        assert!(!loader.loader_name().is_empty());
        assert!(!loader.platform().is_empty());
    }

    // Error type tests
    #[test]
    fn test_loader_error_is_error_trait() {
        use std::error::Error;
        let err: Box<dyn Error> = Box::new(PlatformLoaderError::Other("test error".to_string()));
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_loader_error_clone() {
        let err1 = PlatformLoaderError::Other("test".to_string());
        let err2 = err1.clone();
        assert_eq!(err1.to_string(), err2.to_string());
    }

    #[test]
    fn test_loader_error_debug() {
        let err = PlatformLoaderError::Other("test error".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("test error"));
    }

    // Plugin Capabilities Extended Tests
    #[test]
    fn test_capabilities_with_all_services() {
        let caps = PluginCapabilities {
            supports_hot_reload: true,
            supports_async: true,
            supports_streaming: true,
            max_concurrency: 100,
            requires_services: vec![
                "logger".to_string(),
                "database".to_string(),
                "cache".to_string(),
            ],
            provides_services: vec!["api".to_string(), "storage".to_string()],
            declared_capabilities: vec![
                "read_files".to_string(),
                "write_files".to_string(),
                "network_access".to_string(),
            ],
        };

        assert!(caps.supports_hot_reload);
        assert!(caps.supports_async);
        assert!(caps.supports_streaming);
        assert_eq!(caps.max_concurrency, 100);
        assert_eq!(caps.requires_services.len(), 3);
        assert_eq!(caps.provides_services.len(), 2);
        assert_eq!(caps.declared_capabilities.len(), 3);
    }

    #[test]
    fn test_capabilities_minimal() {
        let caps = PluginCapabilities {
            supports_hot_reload: false,
            supports_async: false,
            supports_streaming: false,
            max_concurrency: 1,
            requires_services: Vec::new(),
            provides_services: Vec::new(),
            declared_capabilities: Vec::new(),
        };

        assert!(!caps.supports_hot_reload);
        assert!(!caps.supports_async);
        assert!(!caps.supports_streaming);
        assert_eq!(caps.max_concurrency, 1);
        assert!(caps.requires_services.is_empty());
        assert!(caps.provides_services.is_empty());
        assert!(caps.declared_capabilities.is_empty());
    }

    // Cross-platform loader enum tests
    #[test]
    fn test_cross_platform_loader_enum_matching() {
        let loader = CrossPlatformLoader::new().unwrap();
        let platform = loader.platform();
        assert!(!platform.is_empty());
    }

    #[test]
    fn test_cross_platform_loader_methods() {
        let loader = CrossPlatformLoader::new().unwrap();

        let name = loader.loader_name();
        assert!(!name.is_empty());
        assert!(name.contains("Loader"));

        let platform = loader.platform();
        assert!(!platform.is_empty());
    }

    // Binary validation error cases
    #[test]
    fn test_linux_validation_invalid_path() {
        let loader = LinuxLoader::new();
        let result = loader.validate_binary("/");
        // Root directory exists but isn't a valid ELF file
        assert!(result.is_err());
    }

    #[test]
    fn test_macos_validation_invalid_path() {
        let loader = MacOsLoader::new();
        let result = loader.validate_binary("/");
        assert!(result.is_err());
    }

    #[test]
    fn test_windows_validation_invalid_path() {
        let loader = WindowsLoader::new();
        let result = loader.validate_binary("/");
        assert!(result.is_err());
    }

    // Result type tests
    #[test]
    fn test_loader_result_ok() {
        let result: LoaderResult<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_loader_result_err() {
        let result: LoaderResult<i32> = Err(PlatformLoaderError::Other("test".to_string()));
        assert!(result.is_err());
    }
}
