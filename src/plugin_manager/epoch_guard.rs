// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Epoch-Based Memory Reclamation for Plugin Hot-Reload
//!
//! This module provides safe hot-reload for plugins by using epoch-based memory
//! reclamation (via crossbeam-epoch). This solves the "dangling VTable" problem:
//!
//! ## The Problem
//!
//! During hot-reload, a plugin's shared library is unloaded and replaced with a
//! new version. However, in-flight requests may still hold function pointers
//! (VTable) to the old plugin's code. If the old library is unloaded while
//! requests are still executing, those requests will segfault.
//!
//! ## The Solution
//!
//! Epoch-based reclamation ensures that:
//! 1. All plugin access goes through an epoch guard (pin)
//! 2. When a plugin is replaced, the old version is deferred for destruction
//! 3. The old version is only destroyed after all guards have been released
//!
//! ## Usage
//!
//! ```ignore
//! // Load a plugin
//! let guarded = EpochGuardedPlugin::new(loader);
//!
//! // Access the plugin (returns a Guard that keeps the plugin alive)
//! {
//!     let guard = guarded.access();
//!     guard.plugin().init(context)?;
//!     // guard dropped here, allowing eventual reclamation
//! }
//!
//! // Replace with new version (old version deferred for destruction)
//! guarded.replace(new_loader);
//! ```

use crossbeam_epoch::{self as epoch, Atomic, Guard, Owned, Shared};
use std::ptr::NonNull;
use std::sync::atomic::Ordering;
use tracing::{debug, trace};

use skylet_abi::AbiV2PluginLoader;

/// An epoch-guarded plugin loader that ensures safe hot-reload.
///
/// This wrapper provides:
/// - Safe concurrent access via epoch guards
/// - Deferred destruction when replacing plugins
/// - No use-after-free for in-flight requests
pub struct EpochGuardedPlugin {
    /// The plugin loader stored in epoch-protected memory
    inner: Atomic<AbiV2PluginLoader>,
    /// Plugin name for logging
    name: String,
}

impl EpochGuardedPlugin {
    /// Create a new epoch-guarded plugin from a loader.
    pub fn new(name: impl Into<String>, loader: AbiV2PluginLoader) -> Self {
        let name = name.into();
        debug!(plugin = %name, "Creating epoch-guarded plugin");
        Self {
            inner: Atomic::new(loader),
            name,
        }
    }

    /// Access the plugin with an epoch guard.
    ///
    /// The returned guard ensures the plugin remains valid for the duration
    /// of the guard's lifetime. This is safe even during hot-reload.
    ///
    /// # Returns
    ///
    /// A `PluginGuard` that provides safe access to the plugin loader.
    /// Returns `None` if the plugin has been unloaded.
    pub fn access(&self) -> Option<PluginGuard<'_>> {
        let guard = epoch::pin();
        let shared = self.inner.load(Ordering::Acquire, &guard);

        if shared.is_null() {
            trace!(plugin = %self.name, "Plugin access failed: already unloaded");
            None
        } else {
            trace!(plugin = %self.name, "Plugin access granted with epoch guard");
            // SAFETY: We just checked that shared is not null, and the guard
            // keeps the memory valid for as long as PluginGuard exists
            let ptr = unsafe { NonNull::new_unchecked(shared.as_raw() as *mut _) };
            Some(PluginGuard {
                ptr,
                _guard: guard,
                name: &self.name,
            })
        }
    }

    /// Replace the plugin with a new version.
    ///
    /// The old plugin is deferred for destruction until all epoch guards
    /// referencing it have been released. This ensures safe hot-reload.
    ///
    /// # Arguments
    ///
    /// * `new_loader` - The new plugin loader to use
    ///
    /// # Safety
    ///
    /// This is safe because:
    /// - The old plugin is not dropped immediately
    /// - It's deferred until all guards are released
    /// - crossbeam-epoch handles the synchronization
    #[allow(dead_code)] // Will be used when hot-reload is fully implemented
    pub fn replace(&self, new_loader: AbiV2PluginLoader) {
        let guard = epoch::pin();

        debug!(plugin = %self.name, "Replacing plugin with new version");

        // Atomically swap the old plugin with the new one
        let old = self
            .inner
            .swap(Owned::new(new_loader), Ordering::AcqRel, &guard);

        // Defer destruction of the old plugin until all guards are released
        if !old.is_null() {
            debug!(plugin = %self.name, "Deferring destruction of old plugin version");
            // SAFETY: We own the old plugin (it was stored in our Atomic)
            // and we're deferring its destruction properly
            unsafe {
                guard.defer_destroy(old);
            }
        }

        // Flush to help advance the epoch
        guard.flush();
    }

    /// Unload the plugin entirely.
    ///
    /// Like `replace`, the plugin is deferred for destruction until all
    /// epoch guards have been released.
    ///
    /// # Returns
    ///
    /// `true` if a plugin was unloaded, `false` if already unloaded.
    pub fn unload(&self) -> bool {
        let guard = epoch::pin();

        debug!(plugin = %self.name, "Unloading plugin");

        // Swap with null to indicate unloaded
        let old = self.inner.swap(Shared::null(), Ordering::AcqRel, &guard);

        if !old.is_null() {
            debug!(plugin = %self.name, "Deferring destruction of unloaded plugin");
            // SAFETY: We own the old plugin
            unsafe {
                guard.defer_destroy(old);
            }
            guard.flush();
            true
        } else {
            debug!(plugin = %self.name, "Plugin already unloaded");
            false
        }
    }

    /// Check if the plugin is currently loaded.
    #[allow(dead_code)] // Will be used when hot-reload is fully implemented
    pub fn is_loaded(&self) -> bool {
        let guard = epoch::pin();
        !self.inner.load(Ordering::Acquire, &guard).is_null()
    }

    /// Get the plugin name.
    #[allow(dead_code)] // Will be used when hot-reload is fully implemented
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for EpochGuardedPlugin {
    fn drop(&mut self) {
        debug!(plugin = %self.name, "Dropping EpochGuardedPlugin");

        // Take ownership of the stored plugin (if any) and drop it
        let guard = epoch::pin();
        let old = self.inner.swap(Shared::null(), Ordering::AcqRel, &guard);

        if !old.is_null() {
            // SAFETY: We're the sole owner at this point (Drop is exclusive)
            unsafe {
                guard.defer_destroy(old);
            }
        }
    }
}

// SAFETY: EpochGuardedPlugin can be safely sent between threads
// because crossbeam_epoch::Atomic handles synchronization internally
unsafe impl Send for EpochGuardedPlugin {}
unsafe impl Sync for EpochGuardedPlugin {}

/// A guard that provides safe access to a plugin.
///
/// This guard ensures the plugin remains valid for its entire lifetime,
/// even if a hot-reload is triggered while the guard is held.
pub struct PluginGuard<'a> {
    /// Raw pointer to the plugin (kept valid by the epoch guard)
    ptr: NonNull<AbiV2PluginLoader>,
    /// The epoch guard that keeps the pointer valid
    _guard: Guard,
    /// Plugin name for logging
    name: &'a str,
}

impl<'a> PluginGuard<'a> {
    /// Get a reference to the plugin loader.
    ///
    /// # Safety
    ///
    /// This is safe because:
    /// - The epoch guard ensures the plugin won't be deallocated
    /// - The pointer is guaranteed non-null (checked in `access()`)
    pub fn plugin(&self) -> &AbiV2PluginLoader {
        // SAFETY: The epoch guard ensures the plugin is valid, and we checked
        // for null in access()
        unsafe { self.ptr.as_ref() }
    }
}

impl Drop for PluginGuard<'_> {
    fn drop(&mut self) {
        trace!(plugin = %self.name, "Releasing plugin guard");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests would require a mock AbiV2PluginLoader or test plugin
    // For now, we document the expected behavior

    #[test]
    fn test_epoch_guard_api_design() {
        // This test verifies the API design compiles correctly
        // Actual functionality testing requires plugin fixtures

        // API design verification:
        // - EpochGuardedPlugin::new(name, loader) creates a guarded plugin
        // - .access() returns Option<PluginGuard<'_>>
        // - .replace(new_loader) swaps atomically with deferred destruction
        // - .unload() removes and defers destruction
        // - .is_loaded() checks current state
        // - PluginGuard::plugin() returns &AbiV2PluginLoader
    }
}
