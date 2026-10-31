//! A counting global allocator, to measure the heap a load takes.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Wraps the system allocator and counts the live and the peak bytes.
///
/// Install it in a binary with `#[global_allocator]`, read it with
/// [`stats`](Self::stats) and restart the peak with
/// [`reset_peak`](Self::reset_peak).
#[derive(Debug)]
pub struct CountingAlloc {
    live: AtomicUsize,
    peak: AtomicUsize,
}

/// A reading of the counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapStats {
    /// Bytes allocated now.
    pub live: usize,
    /// The most bytes that were allocated at once since the last reset.
    pub peak: usize,
}

impl CountingAlloc {
    /// A counter with nothing allocated yet.
    pub const fn new() -> Self {
        Self {
            live: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }

    /// The live and peak bytes.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            live: self.live.load(Ordering::Relaxed),
            peak: self.peak.load(Ordering::Relaxed),
        }
    }

    /// Makes the peak the live bytes again.
    pub fn reset_peak(&self) {
        self.peak
            .store(self.live.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    fn grew(&self, by: usize) {
        let live = self.live.fetch_add(by, Ordering::Relaxed) + by;
        self.peak.fetch_max(live, Ordering::Relaxed);
    }

    fn shrank(&self, by: usize) {
        self.live.fetch_sub(by, Ordering::Relaxed);
    }
}

impl Default for CountingAlloc {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: every call is forwarded unchanged to the system allocator; the
// counters only observe the sizes.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            self.grew(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            self.grew(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        self.shrank(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            if new_size >= layout.size() {
                self.grew(new_size - layout.size());
            } else {
                self.shrank(layout.size() - new_size);
            }
        }
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_and_peak_follow_allocations_and_reallocations() {
        let counter = CountingAlloc::new();
        let layout = Layout::from_size_align(1000, 8).unwrap();
        // SAFETY: the layouts are valid and every pointer is freed with the
        // layout it was last allocated with.
        unsafe {
            let ptr = counter.alloc(layout);
            assert_eq!(
                counter.stats(),
                HeapStats {
                    live: 1000,
                    peak: 1000
                }
            );
            let ptr = counter.realloc(ptr, layout, 4000);
            assert_eq!(
                counter.stats(),
                HeapStats {
                    live: 4000,
                    peak: 4000
                }
            );
            let ptr = counter.realloc(ptr, Layout::from_size_align(4000, 8).unwrap(), 500);
            assert_eq!(
                counter.stats(),
                HeapStats {
                    live: 500,
                    peak: 4000
                }
            );
            counter.reset_peak();
            assert_eq!(
                counter.stats(),
                HeapStats {
                    live: 500,
                    peak: 500
                }
            );
            counter.dealloc(ptr, Layout::from_size_align(500, 8).unwrap());
        }
        assert_eq!(counter.stats().live, 0);
    }
}
