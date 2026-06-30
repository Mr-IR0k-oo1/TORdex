use std::sync::atomic::{AtomicI64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStats {
    pub allocated_bytes: i64,
    pub deallocated_bytes: i64,
    pub live_allocations: i64,
    pub peak_allocated_bytes: i64,
}

pub trait MemoryManager: Send + Sync {
    fn allocate(&self, size: usize) -> Vec<u8>;
    fn track_allocation(&self, size: usize);
    fn track_deallocation(&self, size: usize);
    fn stats(&self) -> MemoryStats;
}

#[derive(Default)]
pub struct TrackingAllocator {
    allocated: AtomicI64,
    deallocated: AtomicI64,
    live: AtomicI64,
    peak: AtomicI64,
}

impl TrackingAllocator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            allocated: AtomicI64::new(0),
            deallocated: AtomicI64::new(0),
            live: AtomicI64::new(0),
            peak: AtomicI64::new(0),
        }
    }
}

impl MemoryManager for TrackingAllocator {
    fn allocate(&self, size: usize) -> Vec<u8> {
        let size = if size == 0 { 1 } else { size };
        self.allocated.fetch_add(size as i64, Ordering::Relaxed);
        self.live.fetch_add(1, Ordering::Relaxed);
        self.peak.fetch_max(
            self.allocated.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        vec![0u8; size]
    }

    fn track_allocation(&self, size: usize) {
        let size = if size == 0 { 1 } else { size };
        self.allocated.fetch_add(size as i64, Ordering::Relaxed);
        self.live.fetch_add(1, Ordering::Relaxed);
        self.peak.fetch_max(
            self.allocated.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
    }

    fn track_deallocation(&self, size: usize) {
        let size = if size == 0 { 1 } else { size };
        self.deallocated.fetch_add(size as i64, Ordering::Relaxed);
        self.live.fetch_sub(1, Ordering::Relaxed);
    }

    fn stats(&self) -> MemoryStats {
        MemoryStats {
            allocated_bytes: self.allocated.load(Ordering::Relaxed),
            deallocated_bytes: self.deallocated.load(Ordering::Relaxed),
            live_allocations: self.live.load(Ordering::Relaxed),
            peak_allocated_bytes: self.peak.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_returns_buffer() {
        let mgr = TrackingAllocator::new();
        let buf = mgr.allocate(64);
        assert_eq!(buf.len(), 64);
        let stats = mgr.stats();
        assert_eq!(stats.allocated_bytes, 64);
    }

    #[test]
    fn track_lifecycle() {
        let mgr = TrackingAllocator::new();
        mgr.track_allocation(128);
        mgr.track_allocation(256);
        let stats = mgr.stats();
        assert_eq!(stats.allocated_bytes, 384);
        assert_eq!(stats.live_allocations, 2);
        mgr.track_deallocation(128);
        let stats = mgr.stats();
        assert_eq!(stats.live_allocations, 1);
    }

    #[test]
    fn track_zero_size() {
        let mgr = TrackingAllocator::new();
        mgr.track_allocation(0);
        let stats = mgr.stats();
        assert_eq!(stats.live_allocations, 1);
    }

    #[test]
    fn peak_tracking() {
        let mgr = TrackingAllocator::new();
        mgr.track_allocation(100);
        mgr.track_allocation(200);
        mgr.track_deallocation(100);
        let stats = mgr.stats();
        assert!(stats.peak_allocated_bytes >= 300);
    }
}
