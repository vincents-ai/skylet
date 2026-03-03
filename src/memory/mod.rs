// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicUsize, Ordering};

pub mod arena;
pub mod pool;

pub use arena::PluginArena;
pub use pool::ObjectPool;

use std::sync::Arc;

pub struct MemoryTracker {
    allocations: Arc<AtomicUsize>,
    deallocations: Arc<AtomicUsize>,
    peak_usage: Arc<AtomicUsize>,
    current_usage: Arc<AtomicUsize>,
}

impl MemoryTracker {
    pub fn new() -> Self {
        Self {
            allocations: Arc::new(AtomicUsize::new(0)),
            deallocations: Arc::new(AtomicUsize::new(0)),
            peak_usage: Arc::new(AtomicUsize::new(0)),
            current_usage: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn track_allocation(&self, size: usize) {
        self.allocations.fetch_add(1, Ordering::Relaxed);
        let new_current = self.current_usage.fetch_add(size, Ordering::Relaxed) + size;
        let current_peak = self.peak_usage.load(Ordering::Relaxed);
        if new_current > current_peak {
            self.peak_usage.store(new_current, Ordering::Relaxed);
        }
    }

    pub fn track_deallocation(&self, size: usize) {
        self.deallocations.fetch_add(1, Ordering::Relaxed);
        self.current_usage.fetch_sub(size, Ordering::Relaxed);
    }

    pub fn current_usage(&self) -> usize {
        self.current_usage.load(Ordering::Relaxed)
    }

    pub fn peak_usage(&self) -> usize {
        self.peak_usage.load(Ordering::Relaxed)
    }

    pub fn total_allocations(&self) -> usize {
        self.allocations.load(Ordering::Relaxed)
    }

    pub fn total_deallocations(&self) -> usize {
        self.deallocations.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.allocations.store(0, Ordering::Relaxed);
        self.deallocations.store(0, Ordering::Relaxed);
        self.current_usage.store(0, Ordering::Relaxed);
        self.peak_usage.store(0, Ordering::Relaxed);
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MemoryTracker {
    fn clone(&self) -> Self {
        Self {
            allocations: self.allocations.clone(),
            deallocations: self.deallocations.clone(),
            peak_usage: self.peak_usage.clone(),
            current_usage: self.current_usage.clone(),
        }
    }
}

pub struct MemoryStats {
    pub current_bytes: usize,
    pub peak_bytes: usize,
    pub total_allocations: usize,
    pub total_deallocations: usize,
}

impl From<&MemoryTracker> for MemoryStats {
    fn from(tracker: &MemoryTracker) -> Self {
        Self {
            current_bytes: tracker.current_usage(),
            peak_bytes: tracker.peak_usage(),
            total_allocations: tracker.total_allocations(),
            total_deallocations: tracker.total_deallocations(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tracker() {
        let tracker = MemoryTracker::new();

        tracker.track_allocation(100);
        assert_eq!(tracker.current_usage(), 100);
        assert_eq!(tracker.peak_usage(), 100);

        tracker.track_allocation(200);
        assert_eq!(tracker.current_usage(), 300);
        assert_eq!(tracker.peak_usage(), 300);

        tracker.track_deallocation(100);
        assert_eq!(tracker.current_usage(), 200);
    }

    #[test]
    fn test_memory_stats() {
        let tracker = MemoryTracker::new();
        tracker.track_allocation(500);

        let stats = MemoryStats::from(&tracker);
        assert_eq!(stats.current_bytes, 500);
        assert_eq!(stats.total_allocations, 1);
    }
}
