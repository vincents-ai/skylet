use std::alloc::{GlobalAlloc, Layout};
use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::memory::MemoryTracker;

pub struct PluginArena {
    total_capacity: usize,
    used: AtomicUsize,
    allocations: AtomicUsize,
    allocations_map: std::sync::Mutex<HashMap<usize, usize>>,
    tracker: MemoryTracker,
}

impl PluginArena {
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            total_capacity: capacity_bytes,
            used: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
            allocations_map: std::sync::Mutex::new(HashMap::new()),
            tracker: MemoryTracker::new(),
        }
    }

    pub fn alloc(&self, layout: Layout) -> Result<NonNull<u8>, std::alloc::AllocError> {
        let size = layout.size();

        if self.used.load(Ordering::Relaxed) + size > self.total_capacity {
            return Err(std::alloc::AllocError);
        }

        let ptr = std::alloc::alloc(layout);
        let non_null = NonNull::new(ptr).ok_or(std::alloc::AllocError)?;

        self.used.fetch_add(size, Ordering::Relaxed);
        self.allocations.fetch_add(1, Ordering::Relaxed);
        self.tracker.track_allocation(size);

        if let Ok(mut map) = self.allocations_map.lock() {
            map.insert(ptr as usize, size);
        }

        Ok(non_null)
    }

    pub fn dealloc(&self, ptr: NonNull<u8>, layout: Layout) {
        let size = layout.size();

        std::alloc::dealloc(ptr.as_ptr(), layout);

        self.used.fetch_sub(size, Ordering::Relaxed);
        self.tracker.track_deallocation(size);

        if let Ok(mut map) = self.allocations_map.lock() {
            map.remove(&(ptr.as_ptr() as usize));
        }
    }

    pub fn used_bytes(&self) -> usize {
        self.used.load(Ordering::Relaxed)
    }

    pub fn available_bytes(&self) -> usize {
        self.total_capacity
            .saturating_sub(self.used.load(Ordering::Relaxed))
    }

    pub fn capacity_bytes(&self) -> usize {
        self.total_capacity
    }

    pub fn utilization(&self) -> f64 {
        let used = self.used.load(Ordering::Relaxed) as f64;
        let capacity = self.total_capacity as f64;
        if capacity == 0.0 {
            0.0
        } else {
            used / capacity
        }
    }

    pub fn allocation_count(&self) -> usize {
        self.allocations.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.used.store(0, Ordering::Relaxed);
        self.allocations.store(0, Ordering::Relaxed);
        if let Ok(mut map) = self.allocations_map.lock() {
            map.clear();
        }
        self.tracker.reset();
    }

    pub fn memory_stats(&self) -> crate::memory::MemoryStats {
        crate::memory::MemoryStats::from(&self.tracker)
    }
}

impl Default for PluginArena {
    fn default() -> Self {
        Self::new(1024 * 1024) // 1MB default
    }
}

pub struct PluginArenaAllocator {
    arena: std::sync::Arc<PluginArena>,
}

impl PluginArenaAllocator {
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            arena: std::sync::Arc::new(PluginArena::new(capacity_bytes)),
        }
    }

    pub fn arena(&self) -> &PluginArena {
        &self.arena
    }
}

unsafe impl GlobalAlloc for PluginArenaAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match self.arena.alloc(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => std::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(non_null) = NonNull::new(ptr) {
            self.arena.dealloc(non_null, layout);
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());

        if let Some(non_null) = NonNull::new(ptr) {
            let new_ptr = self.alloc(new_layout);
            if !new_ptr.is_null() {
                std::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
            new_ptr
        } else {
            self.alloc(new_layout)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Layout;

    #[test]
    fn test_arena_alloc() {
        let arena = PluginArena::new(1024);

        let layout = Layout::new::<u64>();
        let ptr = arena.alloc(layout);
        assert!(ptr.is_ok());

        if let Ok(p) = ptr {
            arena.dealloc(p, layout);
        }

        assert_eq!(arena.allocation_count(), 1);
    }

    #[test]
    fn test_arena_capacity_exceeded() {
        let arena = PluginArena::new(8);

        let layout = Layout::new::<[u8; 16]>();
        let result = arena.alloc(layout);
        assert!(result.is_err());
    }

    #[test]
    fn test_arena_utilization() {
        let arena = PluginArena::new(1000);
        assert!(arena.utilization() < 0.01);

        let layout = Layout::new::<[u8; 500]>();
        if let Ok(ptr) = arena.alloc(layout) {
            assert!(arena.utilization() > 0.4);
            arena.dealloc(ptr, layout);
        }
    }

    #[test]
    fn test_arena_reset() {
        let arena = PluginArena::new(1000);

        let layout = Layout::new::<[u8; 100]>();
        if let Ok(ptr) = arena.alloc(layout) {
            arena.dealloc(ptr, layout);
        }

        arena.reset();

        assert_eq!(arena.used_bytes(), 0);
        assert_eq!(arena.allocation_count(), 0);
    }
}
