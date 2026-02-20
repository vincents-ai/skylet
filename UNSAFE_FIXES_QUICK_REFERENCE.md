# Unsafe Code Quick Reference Guide
## Execution Engine - Critical Fixes Priority Map

### CRITICAL - Fix First (2 hours each)

#### Issue 1: Unbounded List Iteration
- **File:** `abi/src/ffi_safe.rs:413, 595`
- **Fix:** Add `MAX_SERVICES_RETURNED = 10_000` constant
- **Lines to change:** 2-3
- **Test:** `test_list_services_bounded`

```rust
// BEFORE (UNSAFE):
while !(*list_ptr.add(count)).is_null() {
    count += 1;
}

// AFTER (SAFE):
const MAX_SERVICES_RETURNED: usize = 10_000;
while count < MAX_SERVICES_RETURNED && !(*list_ptr.add(count)).is_null() {
    count += 1;
}
if count >= MAX_SERVICES_RETURNED {
    return Err(AbiError::ResourceExhausted(...));
}
```

#### Issue 2: Transmute Bypass
- **File:** `abi/src/abi_loader.rs:166, 176, 186, 193, 200, 207, 215, 223`
- **Fix:** Use Symbol directly, remove transmute_copy
- **Lines to change:** 8
- **Risk:** High - enables arbitrary code execution via function pointer confusion

```rust
// BEFORE (UNSAFE):
let init_fn_sym = unsafe { library.get::<Symbol<PluginInitFnV2>>(...)?};
let init_fn: PluginInitFnV2 = unsafe { std::mem::transmute_copy(&*init_fn_sym) };
let result = (init_fn)(context);

// AFTER (SAFE):
let init_fn_sym = unsafe { library.get::<Symbol<PluginInitFnV2>>(...)?};
let result = (*init_fn_sym)(context);  // Call through Symbol directly
```

#### Issue 3: Pointer Transmute Lifetime Bug
- **File:** `abi/src/security.rs:226`
- **Fix:** Remove transmute, return reference with proper lifetime
- **Lines to change:** 1

```rust
// BEFORE (UNSAFE):
Ok(std::mem::transmute::<&str, &'static str>(s))

// AFTER (SAFE):
Ok(s.to_string())  // Return owned String instead
```

---

### HIGH - Fix Next (3 hours)

#### Issue 4: Unvalidated Slice Creation
- **File:** `abi/src/ffi_safe.rs:827`
- **Fix:** Add pointer validation before from_raw_parts
- **Pattern:** Validate address range, use catch_unwind

```rust
// Add before from_raw_parts:
let body_addr = resp.body as usize;
if body_addr < 0x1000 || body_addr == usize::MAX {
    return Err(AbiError::InvalidRequest("Invalid body pointer"));
}
```

#### Issue 5: Type Confusion in Capabilities
- **File:** `abi/src/security_rfc/capabilities.rs:144, 148, 152`
- **Fix:** Validate inner struct pointers match the type
- **Add:** Null checks on struct fields

```rust
// BEFORE:
unsafe { (*(self.data as *const FilesystemAccess)).required }

// AFTER:
let fs = self.data as *const FilesystemAccess;
if (*fs).uri.is_null() {
    return Err(AbiError::InvalidRequest("Filesystem URI is null"));
}
Ok((*fs).required)
```

#### Issue 6: Header Alignment Not Checked
- **File:** `abi/src/ffi_safe.rs:910`
- **Fix:** Add alignment validation in from_raw

```rust
// Add to from_raw():
let alignment_mask = std::mem::align_of::<HeaderV2>() - 1;
if (headers_ptr as usize) & alignment_mask != 0 {
    return Err(AbiError::InvalidRequest("Headers pointer misaligned"));
}
```

---

### MEDIUM - Fix Soon (2 hours)

#### Issue 7: Vec::from_raw_parts Double Free
- **File:** `abi/src/lib.rs:540`, `src/plugin_manager/manager.rs:1282, 1514`
- **Fix:** Add null check, add size bounds
- **Pattern:** Add MAX size constants

```rust
const MAX_SERVICE_LIST_SIZE: usize = 100_000;

if list.is_null() || count == 0 {
    return;
}
if count > MAX_SERVICE_LIST_SIZE {
    error!("Service list too large: {}", count);
    return;
}
```

#### Issue 8: String Length Not Bounded
- **File:** `src/plugin_manager/manager.rs:1535, 1567, 1587, 1592`
- **Fix:** Add MAX length constants

```rust
const MAX_EVENT_NAME_LEN: usize = 1024;
const MAX_ATTR_VALUE_LEN: usize = 4096;
const MAX_SECRET_KEY_LEN: usize = 256;

if name_len > MAX_EVENT_NAME_LEN {
    warn!("Event name too long: {} > {}", name_len, MAX_EVENT_NAME_LEN);
    return;
}
```

---

## Testing Checklist

```bash
# 1. Build with all checks enabled
RUSTFLAGS="-D warnings" cargo build --release

# 2. Run tests with MIRI
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test

# 3. Verify no unsafe code was added
cargo audit unsafe

# 4. Test each fix individually
cargo test test_list_services_bounded
cargo test test_transmute_removed
cargo test test_header_alignment
```

---

## Constants to Add (Copy-Paste Ready)

```rust
// In abi/src/ffi_safe.rs (after existing MAX constants)
const MAX_SERVICES_RETURNED: usize = 10_000;
const MIN_VALID_ADDRESS: usize = 0x1000;

// In src/plugin_manager/manager.rs
const MAX_SERVICE_LIST_SIZE: usize = 100_000;
const MAX_EVENT_NAME_LEN: usize = 1024;
const MAX_ATTR_KEY_LEN: usize = 256;
const MAX_ATTR_VALUE_LEN: usize = 4096;
const MAX_SECRET_KEY_LEN: usize = 256;
const MAX_SECRET_VALUE_LEN: usize = 1_000_000;
```

---

## Files Needing Changes

1. **abi/src/ffi_safe.rs** - 4 fixes
   - Add bounds to list iteration (2 locations)
   - Add pointer validation to from_raw_parts
   - Add alignment check to headers

2. **abi/src/abi_loader.rs** - 1 fix
   - Remove transmute_copy (8 instances)

3. **abi/src/security.rs** - 1 fix
   - Remove transmute lifetime hack

4. **abi/src/security_rfc/capabilities.rs** - 1 fix
   - Add struct field null checks

5. **src/plugin_manager/manager.rs** - 3 fixes
   - Add bounds to service list free
   - Add length checks to string ops
   - Add size bounds to slice creation

---

## Impact Summary

| Fix | Risk Reduction | Lines | Time |
|-----|----------------|-------|------|
| Bounded iteration | CRITICAL→LOW | 2-3 | 30min |
| Remove transmute_copy | CRITICAL→LOW | 1 | 2h |
| Transmute lifetime | CRITICAL→LOW | 1 | 15min |
| Pointer validation | HIGH→LOW | 5-10 | 1h |
| Type confusion | HIGH→LOW | 3-5 | 1h |
| Header alignment | HIGH→LOW | 3-5 | 30min |
| Vec double-free | MEDIUM→LOW | 2-3 | 30min |
| String bounds | MEDIUM→LOW | 8-10 | 1h |

**Total Estimated Effort:** 12-16 hours
**Total Files Modified:** 5
**Total Unsafe Blocks Fixed:** 15-20
**Exploitable Vulnerabilities Eliminated:** 2-3

