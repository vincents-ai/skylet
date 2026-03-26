# ABI Version Stability and Compatibility

This document specifies the versioning scheme and stability guarantees for the Execution Engine ABI.

## Version Numbering

The Execution Engine uses semantic versioning for ABI versions:

**Format:** `MAJOR.MINOR.PATCH`

- **MAJOR**: Breaking changes (require plugin recompilation)
- **MINOR**: Backward-compatible additions
- **PATCH**: Bug fixes and internal improvements

## Current Status

- **Release Version**: v0.5.0 (Beta)
- **ABI Version**: v2.0.0 (Stable)
- **Release Date**: 2024-02-20
- **ABI Support**: Stable until v3.0.0 (no breaking changes)

Note: The Release Version (v0.5.0) and ABI Version (v2.0.0) are independent.
The ABI is stable and frozen; the release version follows a typical v0.x → v1.0 → v2.0+ progression.

## Stability Guarantees

### ABI v2.x (Current)

**No Breaking Changes** until ABI v3.0.

Breaking changes include:

- Removing required entry points
- Changing function signatures (parameters or return types)
- Removing fields from public structures
- Changing field order or sizes
- Changing error codes

**Forward-Compatible Changes** ARE allowed in v2.x:

- Adding new optional entry points
- Adding new services to the context structure
- Adding new fields to structures (at the end)
- Adding new error codes
- Improving documentation
- Removing deprecated features (with 1 release notice)

### Example: Adding a New Service

A future v2.1.0 release might add a new service:

**v2.0.0:**
```c
struct PluginContextV2 {
    ServiceRegistry* service_registry;
    Logger* logger;
    ConfigManager* config_manager;
    SecretsProvider* secrets_provider;
    // ... more services
};
```

**v2.1.0 (backward compatible):**
```c
struct PluginContextV2 {
    ServiceRegistry* service_registry;
    Logger* logger;
    ConfigManager* config_manager;
    SecretsProvider* secrets_provider;
    // ... more services
    MetricsCollector* metrics;  // NEW (null-safe for v2.0 plugins)
};
```

Plugins compiled for v2.0 will:
1. Continue to work with v2.1 engines
2. See `metrics` as NULL (can check before use)
3. Not need recompilation

### Patch Releases (v2.0.x)

Patch releases only contain:

- Bug fixes for security issues
- Bug fixes for correctness issues
- Documentation updates
- Internal optimizations (no observable behavior change)

Example issues appropriate for patches:

- Null pointer dereference
- Memory leak
- Race condition
- Incorrect error reporting
- Performance regression (within 10%)

Examples NOT appropriate for patches:

- New features
- API additions
- Behavior changes
- Interface modifications

## Deprecation Policy

### Deprecation Notice Period

New deprecations require:

1. **Announcement Release**: Feature marked deprecated in v2.x
2. **Grace Period**: At least 2 minor versions before removal
3. **Removal Release**: Removal only in v3.0 (major version bump)

Example timeline for hypothetical deprecation in v2.0:

- v2.0.0: New feature introduced
- v2.1.0: If deemed problematic, mark as deprecated (notice in docs)
- v2.2.0, v2.3.0: Still available (grace period)
- v3.0.0: Can be removed (major version bump)

### Current Deprecations

None. ABI v2 has no deprecated features.

## Compatibility Checking

### At Build Time

Check Cargo.toml for ABI version:

```toml
[dependencies]
skylet-abi = "2.0"  # Accept 2.0 through 2.x
skylet-abi = "2.0.0"  # Exact version
skylet-abi = ">=2.0.0, <3.0"  # Explicit range
```

### At Runtime

Plugins can check engine capabilities:

```rust
unsafe {
    if let Some(context) = PLUGIN_CONTEXT {
        let version = CStr::from_ptr((*context).engine_version);
        println!("Engine version: {}", version.to_string_lossy());
        
        // Check for optional services
        if !(*context).metrics.is_null() {
            // Use metrics service if available
        }
    }
}
```

### Version Detection

Engine provides version at initialization:

```rust
use skylet_abi::v2_spec::{PluginContextV2, PluginResultV2};

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    unsafe {
        if context.is_null() {
            return PluginResultV2::InvalidRequest;
        }
        
        let version = CStr::from_ptr((*context).engine_version);
        match version.to_str() {
            Ok("2.0.0") => { /* v2.0 specific code */ }
            Ok(v) if v.starts_with("2.") => { /* v2.x code */ }
            _ => { /* unknown version, proceed cautiously */ }
        }
    }
    PluginResultV2::Success
}
```

## Breaking Changes Roadmap

### Planned for ABI v3.0

The following breaking changes are being considered for v3.0 (future):

- [ ] Async context traits instead of function pointers
- [ ] Stronger type safety at FFI boundary
- [ ] New resource management model
- [ ] Enhanced error propagation

### Not Planned

- Migration to Rust-native plugin model (would fragment ecosystem)
- Complete API redesign (backward compatibility remains core value)
- Network protocol changes (can be done in v2.x)

## Vendor-Specific Extensions

Third-party systems can extend ABI v2 safely:

### Reserved Namespace

The context structure reserves a field for vendor extensions:

```c
struct PluginContextV2 {
    // ... standard fields ...
    void* vendor_context;  // Reserved for extensions
};
```

### Example: Vendor Extensions

Plugins can access vendor-specific features through the `vendor_context`:

```rust
unsafe {
    if let Some(context) = PLUGIN_CONTEXT {
        if !(*context).vendor_context.is_null() {
            // Cast to vendor-specific context
            let ext_ctx = *(context).vendor_context as *mut VendorExtensionContext;
            // Use vendor-specific services
        }
    }
}
```

This allows:
- Vendor-specific plugins to use extended services
- General plugins to remain portable
- No conflicts or version skew

## Maintenance and Support

### Support Timeline

| Version | Released | End of Life | Status |
|---------|----------|-------------|--------|
| v1.x | (before) | 2024-01-01 | Unsupported |
| v2.0.x | 2024-02-20 | 2026-02-20 | Active |
| v2.1+ | TBD | TBD | Future |
| v3.0 | TBD | TBD | Future |

### How to Report Compatibility Issues

Found a compatibility issue?

1. **Check ABI Documentation**: Ensure you're following the contract correctly
2. **Search Issues**: Check if the issue has been reported
3. **Report Issue**: Include:
   - ABI version
   - Plugin code (minimal example)
   - Engine version
   - Platform (OS, architecture)
   - Error message and logs

## Version Query APIs

### Engine Version

Engines expose their version through the context:

```c
// In PluginContextV2
const char* engine_version;  // e.g., "2.0.0"
```

### Plugin Reporting Version

Plugins should report their ABI compatibility:

```rust
static PLUGIN_INFO: PluginInfoV2 = PluginInfoV2 {
    name: "my-plugin\0".as_ptr() as *const c_char,
    version: "1.0.0\0".as_ptr() as *const c_char,
    abi_version: "2.0\0".as_ptr() as *const c_char,
    // ...
};
```

## Testing Compatibility

### Unit Tests

```rust
#[test]
fn test_abi_compatibility() {
    // Verify required symbols exist
    // Verify structure sizes
    // Verify type alignment
}
```

### Integration Tests

```rust
#[test]
fn test_with_engine_v2_0() {
    let engine = TestEngine::with_version("2.0.0");
    // Test plugin behavior
}

#[test]
fn test_with_engine_v2_1() {
    let engine = TestEngine::with_version("2.1.0");
    // Test plugin behavior (should still work)
}
```

## FAQ

### Q: Can a v2.0 plugin run on v2.1 engine?

**A:** Yes, absolutely. The v2 series maintains forward compatibility.

### Q: Can a v2.1 plugin run on v2.0 engine?

**A:** Maybe. If the plugin uses only v2.0 features, yes. If it uses v2.1 features, no.

### Q: When will v3.0 be released?

**A:** No release date is set. Expect at least 2 years from v2.0 release (2026).

### Q: What if I find a critical bug?

**A:** Report it immediately. Critical bugs in v2.0 will be fixed in patches (v2.0.1, etc).

### Q: Can I use the same plugin binary for multiple ABI versions?

**A:** No. Plugins must be compiled/linked against the specific ABI version. However, the source code should be portable.

### Q: How do I know if my plugin is compatible?

**A:** Check:
1. Compiled against ABI v2.x headers
2. All required entry points exported
3. Passes integration tests with target engine version

## See Also

- [Plugin Contract](./PLUGIN_CONTRACT.md)
- [API Reference](./API_REFERENCE.md)
- [Changelog](../CHANGELOG.md)
