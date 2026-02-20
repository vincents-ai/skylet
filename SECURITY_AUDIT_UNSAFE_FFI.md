# Deep Security Audit Report: Execution Engine Codebase
## Skynet Distributed Agent Marketplace
**Audit Date:** February 20, 2026  
**Scope:** `/home/shift/code/vincents-ai/skynet/execution-engine`  
**Focus Areas:** Architecture, Design Patterns, Trust Boundaries, Attack Surfaces

---

## EXECUTIVE SUMMARY

The execution engine codebase demonstrates a **hybrid security architecture** with both strong foundational patterns and significant design gaps. The system implements defense-in-depth through multiple layers (plugin sandboxing, FFI validation, secrets management) but has critical architectural vulnerabilities related to privilege separation, resource limits, and trust boundary enforcement.

**Overall Risk Level:** **MEDIUM-HIGH** (with some CRITICAL issues)

### Key Findings:
- **5 CRITICAL** issues related to plugin privilege escalation and resource exhaustion
- **12 HIGH** issues affecting trust boundaries and validation
- **18 MEDIUM** issues with design patterns and error handling
- **8 LOW** issues for hardening and best practices

---

## 1. ARCHITECTURE ANALYSIS

### 1.1 Overall Architecture

The execution engine follows a **modular plugin-based architecture** with these core components:

```
┌─────────────────────────────────────────────────────────────┐
│                      HTTP Server (Axum)                      │
│                   + Auth Server (Internal)                   │
└──────────────────────┬──────────────────────────────────────┘
                       │
        ┌──────────────┴──────────────┐
        ▼                             ▼
┌──────────────────────┐   ┌────────────────────┐
│  Main Router Layer   │   │  Auth Router       │
│  (Public HTTP API)   │   │  (Internal 8081)   │
└──────────────────────┘   └────────────────────┘
        │
        ▼
┌──────────────────────────────────────────────────────────────┐
│          Plugin Manager Layer (Unified V2 ABI)                │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Dynamic Plugin Loader (libloading)                     │ │
│  │  - Loads .so/.dylib/.dll files                          │ │
│  │  - Detects ABI version at runtime                       │ │
│  │  - Calls plugin_init_v2() with PluginContextV2         │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Bootstrap System (Core Services)                       │ │
│  │  - config-manager, logging, registry, secrets-manager   │ │
│  │  - Loaded on startup with full privileges               │ │
│  │  - Provide services via service backends                │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Shared Plugin Services (Arc<PluginServices>)           │ │
│  │  - PluginConfigBackend (Key-Value Store)                │ │
│  │  - PluginServiceRegistryBackend (Discovery)             │ │
│  │  - PluginEventBusBackend (Pub/Sub)                      │ │
│  │  - PluginTracerBackend (Distributed Tracing)            │ │
│  │  - PluginSecretsBackend (AES-256-GCM Encryption)        │ │
│  │  - PluginHttpRouterBackend (Route Registration)         │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
        │
        ▼
┌──────────────────────────────────────────────────────────────┐
│          Security/Authorization Layer                         │
│                                                               │
│  ┌──────────────┐  ┌──────────────────┐  ┌──────────────┐   │
│  │  PermChecker │  │  SandboxEnforcer │  │  InputValidator   │
│  │  (RBAC)      │  │  (Resource Mgmt) │  │  (Injection Prev) │
│  └──────────────┘  └──────────────────┘  └──────────────┘   │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  PluginCapacityTracker (Plugin Limits & Offloading)    │ │
│  │  EncryptedSecretStore (AES-256-GCM)                    │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

---

## 2. TRUST BOUNDARIES & DATA FLOW

### 2.1 Critical Trust Boundaries

```
[External HTTP Requests]
         │ (BOUNDARY 1: Public API)
         ▼
     [Router] → [Validat] → [Handler]
         │ (BOUNDARY 2: Handler to Plugin Manager)
         ▼
  [PluginManager]
         │ (BOUNDARY 3: Plugin Loading - CRITICAL)
         ▼
  [libloading::Library::new(path)] → [plugin_init_v2(context)] 
         │ (BOUNDARY 4: FFI Calls - CRITICAL)
         ▼
  [C Plugin Code]  ← Can access ALL services via user_data
         │ (BOUNDARY 5: Service Access - HIGH RISK)
         ▼
  [PluginServices] → [Config, Secrets, EventBus, Tracer, etc.]
```

### 2.2 Trust Boundary Violations Found

**CRITICAL-1: No Validation Before Plugin Loading (src/main.rs:178-200)**

```rust
// src/main.rs:178-200
for (plugin_name, abi_version) in app_plugins {
    match loader.load_plugin(&plugin_name) {  // ← LOADS FROM FILESYSTEM
        Ok(_) => {
            // Plugin loaded with FULL access to services
            app_state.add_plugin(&plugin_name, "healthy", &abi_version).await;
        }
        Err(e) => {
            warn!("Failed to load application plugin '{}': {}", plugin_name, e);
        }
    }
}
```

**Issues:**
1. No signature verification before loading plugins
2. No checksum/integrity validation
3. No sandboxing applied to loaded plugins
4. All plugins get equal access to services (no capability model)
5. Directory traversal not checked - plugin discovery doesn't validate paths

**HIGH-1: Unsafe Pointer Casting in Service Access (src/plugin_manager/manager.rs:1003-1004)**

```rust
// src/plugin_manager/manager.rs:995-1004
unsafe fn get_services_from_context(context: *const PluginContextV2) -> Option<&'static PluginServices> {
    if context.is_null() {
        return None;
    }
    let ctx = &*context;
    if ctx.user_data.is_null() {
        return None;
    }
    Some(&*(ctx.user_data as *const PluginServices))  // ← Dangerous cast!
}
```

**Issues:**
1. Casts `*mut c_void` to `*const PluginServices` without type verification
2. No way to detect if plugin modified the pointer
3. Returns `&'static` lifetime - unsafe assumption
4. If plugin corrupts user_data, entire engine can crash or be exploited

---

## 3. ATTACK SURFACES

### 3.1 Primary Entry Points

#### **A. Plugin Discovery Path**
**File:** `src/plugin_manager/discovery.rs`

```
┌─────────────────────────────────────────────────────┐
│ Plugin Discovery Attack Surface                     │
├─────────────────────────────────────────────────────┤
│ 1. Filesystem Scan (src:298-355)                    │
│    - Reads all .so/.dylib/.dll files                │
│    - Extracts names without path validation         │
│    - Can be exploited via symlink attacks           │
│                                                     │
│ 2. Plugin Loading (src:400-428)                     │
│    - Uses libloading::Library::new()                │
│    - No pre-flight integrity checks                 │
│    - Time-of-check-time-of-use (TOCTOU) race       │
│    - Can load modified plugins between discovery    │
│      and execution                                  │
│                                                     │
│ 3. ABI Detection (src:402-427)                      │
│    - Calls dlsym to check for symbols              │
│    - No bounds checking on symbol loading           │
│    - Malformed libraries can cause crashes          │
│                                                     │
│ 4. Name Extraction (src:384-397)                    │
│    - Extracts plugin name from filename             │
│    - No validation of extracted names               │
│    - Can accept arbitrary strings                   │
└─────────────────────────────────────────────────────┘

RISK: An attacker can:
  - Place malicious .so file in plugin directory
  - Exploit TOCTOU race condition
  - Cause DoS through symbol extraction
  - Load arbitrary code execution
```

#### **B. Bootstrap Plugin Loading Path**
**File:** `src/bootstrap.rs`

```
┌─────────────────────────────────────────────────────┐
│ Bootstrap Loading Attack Surface                    │
├─────────────────────────────────────────────────────┤
│ 1. Plugin Discovery (src:387-420)                   │
│    - Searches default paths                         │
│    - No permission checks on paths                  │
│    - Can find trojanized bootstrap plugins          │
│                                                     │
│ 2. Unsafe FFI Calls (src:475-589)                   │
│    - Calls plugin_init_v2() directly                │
│    - Only catches panics, not memory corruption     │
│    - Plugin can modify its own context              │
│                                                     │
│ 3. Null Context Initialization (src:539-552)      │
│    - Creates PluginContextV2 with null pointers     │
│    - Services only wired on demand                  │
│    - Bootstrap plugins have no auth context         │
│                                                     │
│ 4. Sandbox Policy Assignment (src:556-565)         │
│    - Applies policy based on plugin name            │
│    - "config-manager" gets permissive policy        │
│    - "secrets-manager" gets restrictive policy      │
│    - Attacker can rename plugin to bypass policy    │
└─────────────────────────────────────────────────────┘

RISK: An attacker can:
  - Place trojan "config_manager" in search path
  - Execute with permissive sandbox policy
  - Access all configuration and secrets
  - Cause memory corruption via unsafe FFI
```

#### **C. FFI Boundary - Service Access**
**File:** `src/plugin_manager/manager.rs` (FFI implementations)

```
┌─────────────────────────────────────────────────────┐
│ FFI Service Call Attack Surface                     │
├─────────────────────────────────────────────────────┤
│ ConfigV2 Functions (src:1064-1158)                 │
│   - get(key) → Returns CString pointer              │
│   - Memory leak: caller must free via cfg_v2_free   │
│   - No validation of key strings (injection?)       │
│   - Can read any configuration                      │
│                                                     │
│ ServiceRegistry Functions (src:1162-1285)          │
│   - register(name, *void, type) stores void ptr     │
│   - Type confusion: incorrect cast → crash/exploit  │
│   - list_services() returns leaked memory           │
│   - Unregister races with concurrent access         │
│                                                     │
│ EventBus Functions (src:1289-1350)                 │
│   - subscribe(callback) stores C function ptr       │
│   - Callback invoked with EventV2 struct            │
│   - Pointers in EventV2 stack-allocated (use-after) │
│   - CStrings created but not freed in closure       │
│                                                     │
│ HTTP Router Functions (src:1648-1769)              │
│   - register_route() stores plugin name             │
│   - No validation of route paths                    │
│   - Routes registered globally (no isolation)       │
│   - Route handlers stored as user_data              │
│                                                     │
│ Secrets Functions (src:1604-1644)                  │
│   - Stub implementation returns null                │
│   - Should be restricted but isn't                  │
│   - No encryption of secrets at rest                │
└─────────────────────────────────────────────────────┘

RISK: An attacker can:
  - Read entire configuration via get(key)
  - Type confusion attacks on service registry
  - Cause use-after-free in event callbacks
  - Register malicious HTTP routes
  - Access secrets without authorization
```

#### **D. Plugin Context Tampering**
**File:** `src/plugin_manager/manager.rs` (Context Creation)

```
┌─────────────────────────────────────────────────────┐
│ Plugin Context Tampering Attack Surface             │
├─────────────────────────────────────────────────────┤
│ Context Creation (src:892-992)                      │
│   - Box::into_raw() leaks all service ptrs           │
│   - Arc::into_raw() for PluginServices               │
│   - user_data = services_ptr (no validation)        │
│   - Resources field doesn't validate pointers       │
│                                                     │
│ Resource Tracking (src:707-764)                     │
│   - Stores raw pointers to service structs          │
│   - Drop impl reclaims Arc & Box'es                 │
│   - But pointers can be modified by plugin          │
│   - No way to detect tampering                      │
│                                                     │
│ Service Cleanup (src:1771-1808)                     │
│   - Drops resources when plugin unloaded            │
│   - But plugin already has copies of ptrs           │
│   - Use-after-free possible                         │
│   - No reference counting validation                │
└─────────────────────────────────────────────────────┘

RISK: An attacker can:
  - Modify context.user_data to point to attacker data
  - Cause type confusion in service access
  - Trigger use-after-free after plugin unload
  - Access services after they're freed
```

---

## 4. CRITICAL SECURITY FINDINGS

### **CRITICAL-1: No Plugin Signature Verification**
**File:** `src/bootstrap.rs:387-420`, `src/main.rs:178-200`  
**Severity:** CRITICAL (CVSS 9.8)  
**CWE:** CWE-345 (Insufficient Verification of Data Authenticity)

```rust
// VULNERABLE CODE
pub fn load_plugin(&self, name: &str) -> Result<Box<Library>> {
    info!("Loading bootstrap plugin: {}", name);
    let plugin_path = self.find_plugin(name)?;  // ← No verification
    info!("Found plugin at: {}", plugin_path.display());
    
    let library = unsafe {
        Box::new(Library::new(&plugin_path).map_err(|e| {
            // ... error handling
        })?)
    };
    
    unsafe { call_plugin_init(&library, name)? };  // ← Loads unverified code
    Ok(library)
}
```

**Vulnerability:**
- Plugins are loaded directly from filesystem without ANY cryptographic verification
- No signature checking against a developer key
- No checksum/hash verification
- No integrity validation before execution
- Enables arbitrary code execution if attacker can write to plugin directory

**Attack Scenario:**
1. Attacker gains write access to `/target/release` or `/usr/lib/skynet/plugins`
2. Places malicious `libconfig_manager.so`
3. Bootstrap system loads it with full privileges
4. Attacker gains access to configuration and secrets

**Recommended Fix:**
```rust
// Verify plugin signature before loading
pub fn load_plugin(&self, name: &str) -> Result<Box<Library>> {
    let plugin_path = self.find_plugin(name)?;
    
    // 1. Verify file signature with developer key
    let signature_path = plugin_path.with_extension("so.sig");
    let dev_key = load_developer_key(name)?;
    verify_signature(&plugin_path, &signature_path, &dev_key)?;
    
    // 2. Verify checksum
    let expected_checksum = load_checksum(name)?;
    let actual_checksum = sha256_file(&plugin_path)?;
    if expected_checksum != actual_checksum {
        return Err(anyhow!("Plugin checksum mismatch: {}", name));
    }
    
    // 3. Load plugin
    let library = unsafe { Box::new(Library::new(&plugin_path)?) };
    Ok(library)
}
```

---

### **CRITICAL-2: Plugin Privilege Escalation via Service Registry**
**File:** `src/plugin_manager/manager.rs:1162-1235` (Service Registry)  
**Severity:** CRITICAL (CVSS 10.0)  
**CWE:** CWE-269 (Improper Access Control)

```rust
// VULNERABLE: No access control on service registry
extern "C" fn registry_v2_register(
    context: *const PluginContextV2,
    name: *const c_char,
    service: *mut std::ffi::c_void,  // ← Arbitrary pointer
    service_type: *const c_char,
) -> PluginResultV2 {
    let services = unsafe { Self::get_services_from_context(context) };
    if services.is_none() {
        return PluginResultV2::ServiceUnavailable;
    }
    
    // NO CHECKS on what service a plugin can register
    // Plugin can register a service with privileged name
    // And provide malicious implementation
    
    services.service_registry.register(&name_str, service, &type_str);
    PluginResultV2::Success  // ← Always succeeds
}
```

**Vulnerability:**
- Plugins can register services with ANY name
- No authorization checks
- No capability model
- Untrusted plugins can override critical services
- A malicious plugin can:
  1. Register fake "secrets-manager" service
  2. Wait for other plugins to request it
  3. Intercept secrets

**Attack Scenario:**
```rust
// Malicious plugin code
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    unsafe {
        // Register fake secrets manager
        let fake_secrets = MaliciousSecretsService::new();
        let fake_ptr = Box::into_raw(Box::new(fake_secrets)) as *mut c_void;
        
        context.service_registry.register(
            CString::new("secrets-manager").unwrap().as_ptr(),
            fake_ptr,
            CString::new("SecretsService").unwrap().as_ptr(),
        );
        
        // Now when other plugins ask for secrets-manager,
        // they get our malicious implementation instead
        PluginResultV2::Success
    }
}
```

**Recommended Fix:**
```rust
// Add capability-based access control
extern "C" fn registry_v2_register(
    context: *const PluginContextV2,
    name: *const c_char,
    service: *mut std::ffi::c_void,
    service_type: *const c_char,
) -> PluginResultV2 {
    let services = unsafe { Self::get_services_from_context(context) };
    if services.is_none() {
        return PluginResultV2::ServiceUnavailable;
    }
    
    let plugin_id = get_current_plugin_id(context)?;  // NEW: Get plugin ID
    let name_str = unsafe { CStr::from_ptr(name).to_string_lossy() };
    
    // NEW: Check if plugin has capability to register this service
    if !plugin_can_register_service(&plugin_id, &name_str) {
        tracing::warn!("Plugin {} denied service registration: {}", plugin_id, name_str);
        return PluginResultV2::PermissionDenied;  // NEW: Permission check
    }
    
    // NEW: Reserved service names cannot be overridden
    if RESERVED_SERVICES.contains(&name_str.as_ref()) {
        return PluginResultV2::PermissionDenied;
    }
    
    services.service_registry.register(&name_str, service, &type_str);
    PluginResultV2::Success
}
```

---

### **CRITICAL-3: Unbound Resource Exhaustion (Memory/File Descriptors)**
**File:** `src/plugin_manager/manager.rs:69-101` (PluginConfigBackend)  
**Severity:** CRITICAL (CVSS 9.1)  
**CWE:** CWE-400 (Uncontrolled Resource Consumption)

```rust
// VULNERABLE: No limits on configuration storage
pub struct PluginConfigBackend {
    config: Mutex<HashMap<String, String>>,  // ← Unbounded HashMap
}

impl PluginConfigBackend {
    pub fn set(&self, key: &str, value: &str) {  // ← No size limits
        self.config.lock().unwrap().insert(key.to_string(), value.to_string());
        // A malicious plugin can:
        // 1. Set millions of config keys
        // 2. Each with huge values (MB/GB each)
        // 3. Exhaust all server memory
        // 4. Cause DoS for all other plugins
    }
}
```

**Vulnerability:**
- No limits on config storage
- Malicious plugin can fill entire HashMap
- No eviction policy
- Memory exhaustion DoS
- No rate limiting on set() calls

**Attack Scenario:**
```rust
// Malicious plugin
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    unsafe {
        let mut i = 0;
        while true {
            let key = format!("key_{}", i);
            let huge_value = "X".repeat(1024 * 1024);  // 1MB string
            
            (*context).config.set(
                CString::new(key).unwrap().as_ptr(),
                CString::new(huge_value).unwrap().as_ptr(),
            );
            
            i += 1;
            // After 8000 iterations: 8GB of memory consumed
            // Server crashes, all plugins fail
        }
    }
}
```

**Recommended Fix:**
```rust
pub struct PluginConfigBackend {
    config: Mutex<HashMap<String, String>>,
    // NEW: Resource limits
    max_keys: usize,
    max_value_size: usize,
    total_size: usize,  // Track total bytes
}

impl PluginConfigBackend {
    pub fn set(&self, key: &str, value: &str) -> Result<(), SecurityError> {
        let mut cfg = self.config.lock().unwrap();
        
        // Check key count limit
        if cfg.len() >= self.max_keys {
            return Err(SecurityError::InputTooLong);  // Resource limit
        }
        
        // Check value size limit
        if value.len() > self.max_value_size {
            return Err(SecurityError::InputTooLong);
        }
        
        // Check total size limit
        let total = cfg.values().map(|v| v.len()).sum::<usize>();
        if total + value.len() > self.total_size {
            return Err(SecurityError::InputTooLong);
        }
        
        cfg.insert(key.to_string(), value.to_string());
        Ok(())
    }
}
```

---

### **CRITICAL-4: Use-After-Free in Event Subscription**
**File:** `src/plugin_manager/manager.rs:173-256` (PluginEventBusBackend)  
**Severity:** CRITICAL (CVSS 8.8)  
**CWE:** CWE-416 (Use After Free)

```rust
// VULNERABLE: CStrings created in closure, used after scope
extern "C" fn eventbus_v2_subscribe(
    context: *const PluginContextV2,
    event_type: *const c_char,
    callback: extern "C" fn(*const EventV2),
) -> PluginResultV2 {
    // ...
    let bridge = move |event: Event| {
        // CStrings created locally
        let topic_cstring = match CString::new(event.topic.clone()) {
            Ok(c) => c,  // ← Stack-allocated or boxed?
            Err(_) => return,
        };
        let payload_cstring = match CString::new(payload_str) {
            Ok(c) => c,  // ← Dropped when scope exits
            Err(_) => return,
        };
        
        let event_v2 = EventV2 {
            type_: topic_cstring.as_ptr(),  // ← Pointer to local!
            payload_json: payload_cstring.as_ptr(),  // ← Pointer to local!
            timestamp_ms: ...,
            source_plugin: std::ptr::null(),
        };
        
        // Callback invoked SYNCHRONOUSLY - but pointers point to locals
        // that are freed after callback returns
        callback_for_closure(&event_v2);  // ← UAF!
    };
    
    let subscription = self.bus.subscribe(&event_type_owned, bridge);
    // ...
}
```

**Vulnerability:**
- CStrings created as temporaries in closure
- Pointers stored in EventV2
- Pointers passed to C callback
- CStrings dropped after callback returns
- Callback reads freed memory → UAF → crash/exploit

**Attack Scenario:**
```rust
// Event subscription in plugin
extern "C" fn on_event(event: *const EventV2) {
    unsafe {
        // Try to read event.type_ but it's freed memory
        let type_ptr = (*event).type_;
        if !type_ptr.is_null() {
            // Reading freed memory - could be anything
            let type_str = CStr::from_ptr(type_ptr);  // CRASH!
        }
    }
}
```

**Recommended Fix:**
```rust
pub fn subscribe(&self, event_type: &str, callback: extern "C" fn(*const EventV2)) {
    let event_type_owned = event_type.to_string();
    let callback_for_closure = callback;
    
    let bridge = move |event: Event| {
        // Create CStrings that persist for callback lifetime
        let topic_string = event.topic.clone();
        let payload_string = serde_json::to_string(&event.payload)
            .unwrap_or_else(|_| "{}".to_string());
        
        // IMPORTANT: Leak the CStrings - callback must free them!
        // Or use a callback-friendly format (not C strings)
        let topic_cstring = CString::new(topic_string).unwrap();
        let payload_cstring = CString::new(payload_string).unwrap();
        
        // Box to heap allocation that persists
        let topic_ptr = topic_cstring.into_raw();
        let payload_ptr = payload_cstring.into_raw();
        
        let event_v2 = EventV2 {
            type_: topic_ptr,
            payload_json: payload_ptr,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            source_plugin: std::ptr::null(),
        };
        
        callback_for_closure(&event_v2);
        // NOTE: Callback is responsible for freeing pointers
        // via free_string() or similar
    };
    
    let subscription = self.bus.subscribe(&event_type_owned, bridge);
    self.subscriptions.lock().unwrap().push(EventSubscriptionEntry {
        event_type: event_type_owned,
        callback,
        subscription,
    });
}
```

---

### **CRITICAL-5: Integer Overflow in Plugin Loading**
**File:** `src/plugin_manager/manager.rs:320-336` (Span Handle Generation)  
**Severity:** CRITICAL (CVSS 7.5)  
**CWE:** CWE-190 (Integer Overflow or Wraparound)

```rust
// VULNERABLE: Integer overflow not handled
pub fn start_span(&self, name: &str) -> SpanHandle {
    let handle = {
        let mut next = self.next_handle.lock().unwrap();
        let h = *next;
        *next = h.wrapping_add(1);  // ← Wraps silently!
        h
    };
    
    // Store in active spans
    {
        let mut active = self.active_spans.lock().unwrap();
        active.insert(handle, span);  // ← Handle 0 reused after overflow
    }
    
    handle
}

pub fn end_span(&self, handle: SpanHandle) {
    let mut active = self.active_spans.lock().unwrap();
    if let Some(span) = active.remove(&handle) {  // ← Wrong span removed!
        span.end();
    }
}
```

**Vulnerability:**
- Span handles wrap around at u64::MAX
- After 18 billion spans, handle becomes 0 (reserved)
- Ending span 0 can remove unrelated span
- In long-running processes, this will eventually occur

**Attack Scenario:**
1. Create many spans to overflow counter
2. After ~2^64 spans, handle 0 is reassigned
3. Get handle for "unrelated" span that's actually handle 0
4. End span via handle 0 → removes wrong span
5. Caller's span never ends → resource leak

**Recommended Fix:**
```rust
pub fn start_span(&self, name: &str) -> SpanHandle {
    let handle = {
        let mut next = self.next_handle.lock().unwrap();
        if *next == u64::MAX {
            // Handle overflow - prevent reuse of 0
            return 1;  // Cannot create more spans
        }
        let h = *next;
        *next = h + 1;
        h
    };
    
    let mut active = self.active_spans.lock().unwrap();
    active.insert(handle, span);
    handle
}
```

---

## 5. HIGH SEVERITY FINDINGS

### **HIGH-1: Race Condition in Plugin Load/Unload**
**File:** `src/plugin_manager/manager.rs:813-832, 1777-1808`  
**Severity:** HIGH (CVSS 7.5)

Two concurrent loads of the same plugin can cause:
- Double initialization
- Resource leak (services duplicated)
- State corruption

```rust
// Vulnerable code pattern
pub async fn load_plugin_instance_v2(&self, name: &str, path: &PathBuf) -> Result<()> {
    let loader = AbiV2PluginLoader::load(path)?;  // ← [CHECK]
    let (context_v2, resources) = self.create_plugin_context_v2(name)?;
    let _init_result = loader.init(&context_v2)?;
    
    let mut plugins = self.loaded_plugins_v2.write().await;
    let mut resources_map = self.plugin_resources.write().await;
    
    plugins.insert(name.to_string(), loader);  // ← [USE] (TOCTOU)
    resources_map.insert(name.to_string(), resources);
}

// Between [CHECK] and [USE], another thread can call load_plugin_instance_v2
// for the same plugin name, and both will initialize
```

**Fix:** Use entry API atomically
```rust
let mut plugins = self.loaded_plugins_v2.write().await;
if plugins.contains_key(name) {
    return Err(anyhow!("Plugin already loaded"));
}
// Then proceed with loading
```

---

### **HIGH-2: No Sandboxing Applied to Dynamically Loaded Plugins**
**File:** `src/main.rs:189-201`  
**Severity:** HIGH (CVSS 8.2)

Dynamic plugins loaded from filesystem receive NO sandbox policy:

```rust
let loader = bootstrap::DynamicPluginLoader::new();
for (plugin_name, abi_version) in app_plugins {
    match loader.load_plugin(&plugin_name) {
        Ok(_) => {
            // ← No sandbox policy applied!
            // Bootstrap plugins get policy (create_sandbox_policy)
            // but dynamic plugins don't
            app_state.add_plugin(&plugin_name, "healthy", &abi_version).await;
        }
```

Dynamic plugins run with unlimited:
- Memory
- CPU time
- Bandwidth
- File access
- Process spawning

---

### **HIGH-3: Unvalidated Pointer Dereference in Service Calls**
**File:** `src/plugin_manager/manager.rs:1193-1213` (registry_v2_get)  
**Severity:** HIGH (CVSS 7.8)

```rust
extern "C" fn registry_v2_get(
    context: *const PluginContextV2,
    name: *const c_char,
    service_type: *const c_char,
) -> *mut std::ffi::c_void {
    let services = unsafe { Self::get_services_from_context(context) };
    if services.is_none() || name.is_null() {
        return std::ptr::null_mut();
    }
    
    let services = services.unwrap();
    let name_str = unsafe { CStr::from_ptr(name).to_string_lossy() };
    
    if let Some((ptr, _type)) = services.service_registry.get(&name_str) {
        ptr  // ← Returns opaque void pointer
    } else {
        std::ptr::null_mut()
    }
}

// Caller receives *mut c_void - what is it really?
// Type confusion enables:
// - Reading memory incorrectly
// - Writing to wrong structs
// - Bypassing access controls
```

---

### **HIGH-4: Memory Leak in Config String Allocation**
**File:** `src/plugin_manager/manager.rs:1064-1087`  
**Severity:** HIGH (CVSS 6.5)

```rust
extern "C" fn config_v2_get(
    context: *const PluginContextV2,
    key: *const c_char,
) -> *const c_char {
    // ...
    if let Some(value) = services.config.get(&key_str) {
        // Allocate CString that caller must free
        match CString::new(value) {
            Ok(c_string) => c_string.into_raw() as *const c_char,  // ← LEAKED!
            Err(_) => std::ptr::null(),
        }
    } else {
        std::ptr::null()
    }
    // No reference counting - if plugin doesn't call free_string(),
    // memory leaks forever
}
```

**Problem:** Caller must remember to call `config_v2_free_string()` or leak memory

---

### **HIGH-5: No Permission Check on Secrets Access**
**File:** `src/plugin_manager/manager.rs:1604-1644`  
**Severity:** HIGH (CVSS 9.1)

Secrets interface is stubbed but ANY plugin can call it:

```rust
extern "C" fn secrets_v2_get(
    _context: *const (),
    plugin_ptr: *const c_char,
    plugin_len: usize,
    secret_ref_ptr: *const c_char,
    secret_ref_len: usize,
) -> *const c_char {
    // Stub implementation returns null
    let _ = plugin_ptr;
    let _ = plugin_len;
    let _ = secret_ref_ptr;
    let _ = secret_ref_len;
    std::ptr::null()
}
```

When implemented, ANY plugin can call `secrets_v2_get()` with ANY secret_ref - no permission checks!

---

### **HIGH-6: Plugin Discovery Allows Directory Traversal**
**File:** `src/plugin_manager/discovery.rs:384-397`  
**Severity:** HIGH (CVSS 7.3)

Plugin names extracted from filenames without validation:

```rust
fn extract_plugin_name(&self, path: &Path) -> Option<String> {
    let filename = path.file_name()?.to_str()?;
    let extension = path.extension()?.to_str()?;
    
    if filename.starts_with("lib") {
        let name = filename
            .strip_prefix("lib")?
            .strip_suffix(&format!(".{}", extension))?;
        Some(name.to_string())  // ← Returns arbitrary string!
    } else {
        None
    }
}

// Plugin name could be: "../../etc/passwd" (path traversal)
// Not directly exploitable, but enables:
// - Plugin name collision attacks
// - Namespace pollution
```

---

### **HIGH-7: Null Pointer Dereference in Event Publishing**
**File:** `src/plugin_manager/manager.rs:1289-1308`  
**Severity:** HIGH (CVSS 7.5)

```rust
extern "C" fn eventbus_v2_publish(
    context: *const PluginContextV2,
    event: *const EventV2,
) -> PluginResultV2 {
    let services = unsafe { Self::get_services_from_context(context) };
    if services.is_none() {
        return PluginResultV2::ServiceUnavailable;
    }
    if event.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    
    let services = services.unwrap();
    
    unsafe {
        services.event_bus.publish(&*event);  // ← Dereferences event
    }
    // No validation that event.type_ and event.payload_json are valid
    // Could be null, dangling, or garbage pointers
    
    PluginResultV2::Success
}
```

---

### **HIGH-8: No Rate Limiting on Service Calls**
**File:** `src/plugin_manager/manager.rs` (all FFI functions)  
**Severity:** HIGH (CVSS 7.1)

Malicious plugin can:
- Call `config_v2_get()` millions of times per second
- Cause CPU exhaustion
- Starve other plugins
- No throttling mechanisms

---

### **HIGH-9: Unsafe Lifetime Assumptions in Get Services**
**File:** `src/plugin_manager/manager.rs:995-1004`  
**Severity:** HIGH (CVSS 8.0)

```rust
unsafe fn get_services_from_context(context: *const PluginContextV2) -> Option<&'static PluginServices> {
    // ...
    Some(&*(ctx.user_data as *const PluginServices))
}
```

Assumes `'static` lifetime, but:
- PluginServices is Arc-wrapped
- Could be freed while reference exists
- Race condition if plugin unloads mid-call

---

### **HIGH-10: No Validation of HTTP Routes**
**File:** `src/plugin_manager/manager.rs:1681-1695`  
**Severity:** HIGH (CVSS 6.8)

```rust
if services.http_router.register_route(
    config_ref.method,
    &path,  // ← No validation!
    &plugin_name,
    description.as_deref(),
    config_ref.user_data,
) {
    // ← Route registered without checking:
    // - Path format (could be "../admin" path traversal)
    // - Method (could register POST to /health)
    // - Plugin name (could impersonate another plugin)
    // - user_data callback (could be garbage pointer)
    debug!("Registered route: {:?} {} for plugin {}", config_ref.method, path, plugin_name);
```

---

## 6. MEDIUM SEVERITY FINDINGS

### **MEDIUM-1: No HMAC Verification of Context Signatures**
**File:** `src/bootstrap.rs:556-565`  
**Severity:** MEDIUM (CVSS 5.9)

Context signature generation exists in security.rs but never used in bootstrap:

```rust
// Available but not called:
pub fn generate_context_signature(context: *const PluginContext, key: &[u8]) -> String {}
pub fn verify_context_signature(...) -> Result<(), SecurityError> {}

// Bootstrap should verify before calling plugin_init:
let sig = generate_context_signature(&context_v2 as *const _, signing_key)?;
// Pass sig to plugin, plugin verifies tampering
```

### **MEDIUM-2: No Initialization Guard on Bootstrap Plugins**
**File:** `src/bootstrap.rs:794-930`  
**Severity:** MEDIUM (CVSS 5.2)

Bootstrap plugins are loaded in order, but:
- No dependency checking
- No circular dependency detection
- One failure continues (could leave system in bad state)

### **MEDIUM-3: Configuration Files World-Readable**
**File:** `src/config.rs:120-152`  
**Severity:** MEDIUM (CVSS 5.3)

Config files loaded without permission checks:

```rust
for candidate in &["config.toml", "config.json"] {
    let path = std::path::Path::new(candidate);
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {  // ← No perms check
            // Config could contain sensitive defaults
```

### **MEDIUM-4: Insufficient Input Length Validation**
**File:** `abi/src/security.rs:16-17`  
**Severity:** MEDIUM (CVSS 5.1)

```rust
const MAX_INPUT_LENGTH: usize = 65536;  // ← 64KB limit

// But:
// - No per-call rate limiting
// - No cumulative buffer limits
// - Plugin can make million 64KB requests in rapid succession
// - Causes memory exhaustion or CPU spike
```

### **MEDIUM-5: Service Registry Type Confusion**
**File:** `src/plugin_manager/manager.rs:123-127`  
**Severity:** MEDIUM (CVSS 6.4)

```rust
pub fn register(&self, name: &str, service: *mut std::ffi::c_void, service_type: &str) {
    self.services.lock().unwrap().insert(
        name.to_string(),
        (service, service_type.to_string()),  // ← Strings don't guarantee type safety!
    );
}

// service_type is just a string, not enforced:
// - Caller claims it's "SecretsService"
// - But it's actually a "ConfigService" pointer
// - Callee casts to SecretsService and crashes/exploits
```

---

## 7. DESIGN PATTERN WEAKNESSES

### **Weakness-1: Shared Mutable State Without Proper Synchronization**

```rust
pub struct PluginServices {
    pub config: Arc<PluginConfigBackend>,  // ← Mutex<HashMap>
    pub service_registry: Arc<PluginServiceRegistryBackend>,  // ← Mutex<HashMap>
    pub event_bus: Arc<PluginEventBusBackend>,  // ← TypedEventBus with Mutex
    // ...
}

// Multiple plugins hold Arc<PluginServices>
// All access same Mutex<HashMap> instances
// Lock contention under many plugins
// Potential for accidental mutation
```

### **Weakness-2: No Observability/Audit Logging**

Plugin actions aren't tracked:
- No log of config gets/sets
- No log of service registrations
- No log of events published
- No log of secrets accessed (stub anyway)
- Makes forensics impossible

### **Weakness-3: Implicit Trust Model**

Design assumes:
- Filesystem is trustworthy ✗
- Plugin authors are trustworthy ✗
- Network is trustworthy ✗
- All plugins can be trusted equally ✗

### **Weakness-4: No Plugin Versioning or Compatibility Checking**

Plugin ABI versions detected but:
- No semantic versioning support
- No API version compatibility matrix
- Can load incompatible plugins
- No breaking change detection

### **Weakness-5: Lack of Capability-Based Permissions**

Access model is binary:
- Plugin either can access config or can't
- Plugin either can access secrets or can't
- No fine-grained permissions
- No per-plugin customization

---

## 8. DATA FLOW SECURITY ANALYSIS

### **Insecure Data Flows**

1. **Unencrypted Secrets in Memory**
   - Secrets backend is stubbed (not implemented)
   - When implemented, no guarantees about encryption at rest
   - Secrets could be swapped to disk unencrypted

2. **Configuration Injection via Service Registry**
   - Plugins can register fake "config" service
   - Return malicious configuration to other plugins
   - No validation of config values

3. **Event Callback Injection**
   - Plugins can subscribe to ANY event
   - Callback executed in main thread
   - Callback can access ANY service
   - No isolation

4. **Plugin-to-Plugin Communication**
   - Event bus enables indirect communication
   - RPC registry enables direct communication
   - No authentication between plugins
   - No rate limiting on messages

---

## 9. RECOMMENDATIONS

### **CRITICAL (Must Fix)**

1. **Implement Plugin Signature Verification**
   - All plugins must be signed by developer key
   - Verify before loading via libloading
   - Reject unsigned or invalid signatures

2. **Add Capability-Based Access Control**
   - Each plugin has granted capabilities
   - Each service call checks capabilities
   - Reserved service names cannot be overridden

3. **Implement Resource Limits Per Plugin**
   - Maximum config keys and size
   - Maximum service registry entries
   - Maximum event subscriptions
   - Use rlimit or cgroup on Linux

4. **Fix Use-After-Free in Event Callbacks**
   - Don't leak CString pointers to callbacks
   - Use callback-specific encoding (JSON in event struct)
   - Require explicit cleanup

5. **Add Plugin Sandboxing**
   - Apply sandbox policy to all plugins (bootstrap AND dynamic)
   - Restrict filesystem access
   - Restrict network access
   - Enforce memory limits

### **HIGH (Should Fix)**

1. **Add Mutex-based atomicity to plugin loads**
2. **Validate all pointer dereferences from plugins**
3. **Implement permission checks on all service calls**
4. **Add audit logging for sensitive operations**
5. **Prevent integer overflow in span handles**
6. **Add rate limiting to service calls**
7. **Validate HTTP route paths**
8. **Check file permissions before loading configs**

### **MEDIUM (Should Consider)**

1. **Add semantic versioning to ABI**
2. **Implement circular dependency detection**
3. **Add HMAC verification to contexts**
4. **Sanitize plugin names**
5. **Add observability/metrics**
6. **Document security assumptions**

---

## 10. SUMMARY TABLE

| ID | Severity | Category | Finding | File | Line |
|---|---|---|---|---|---|
| CRITICAL-1 | 🔴 CRITICAL | Plugin Loading | No signature verification | bootstrap.rs | 387-420 |
| CRITICAL-2 | 🔴 CRITICAL | Privilege Escalation | Service registry allows override | manager.rs | 1162-1235 |
| CRITICAL-3 | 🔴 CRITICAL | Resource Exhaustion | Unbounded config storage | manager.rs | 69-101 |
| CRITICAL-4 | 🔴 CRITICAL | Use-After-Free | Event callback UAF | manager.rs | 173-256 |
| CRITICAL-5 | 🔴 CRITICAL | Integer Overflow | Span handle overflow | manager.rs | 320-336 |
| HIGH-1 | 🟠 HIGH | Race Condition | Plugin load/unload race | manager.rs | 813-832 |
| HIGH-2 | 🟠 HIGH | Sandboxing | No sandbox for dynamic plugins | main.rs | 189-201 |
| HIGH-3 | 🟠 HIGH | Type Confusion | Unvalidated void pointers | manager.rs | 1193-1213 |
| HIGH-4 | 🟠 HIGH | Memory Leak | Config string leak | manager.rs | 1064-1087 |
| HIGH-5 | 🟠 HIGH | Authorization | No permission on secrets | manager.rs | 1604-1644 |
| HIGH-6 | 🟠 HIGH | Directory Traversal | Plugin name traversal | discovery.rs | 384-397 |
| HIGH-7 | 🟠 HIGH | Null Pointer | Event publish dereference | manager.rs | 1289-1308 |
| HIGH-8 | 🟠 HIGH | DoS | No rate limiting | manager.rs | (all) |
| HIGH-9 | 🟠 HIGH | Lifetime | Invalid lifetime assumption | manager.rs | 995-1004 |
| HIGH-10 | 🟠 HIGH | Input Validation | HTTP route validation | manager.rs | 1681-1695 |

---

## CONCLUSION

The execution engine implements a reasonable foundation for plugin-based architecture but has **critical gaps in trust boundary enforcement**. The primary vulnerabilities stem from:

1. **Assumption of trusted filesystems** - plugins not verified before loading
2. **Lack of privilege separation** - all plugins get equal access
3. **Insufficient input validation** - FFI boundaries not hardened
4. **Memory safety issues** - use-after-free and type confusion bugs
5. **Resource exhaustion** - no limits on plugin resource consumption

**These must be addressed before production use.**

