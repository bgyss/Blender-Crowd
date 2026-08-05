//! Allocation accounting for the memory metric.
//!
//! Reports *peak allocated bytes*, not resident set size. A counting allocator
//! avoids platform-specific RSS APIs and is itself deterministic, at the cost
//! of excluding allocator overhead and static data. Stating which number is
//! being reported matters more than reporting the larger one.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

pub struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            // Track the net change so a shrink is not counted as growth.
            let live = if new_size >= layout.size() {
                LIVE.fetch_add(new_size - layout.size(), Ordering::Relaxed)
                    + (new_size - layout.size())
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed)
                    - (layout.size() - new_size)
            };
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        new_ptr
    }
}

pub fn peak_bytes() -> usize {
    PEAK.load(Ordering::Relaxed)
}

// Not yet called from the report path; kept as a diagnostic escape hatch
// alongside `peak_bytes`. Allowed rather than deleted per the brief's
// verbatim allocator code.
#[allow(dead_code)]
pub fn live_bytes() -> usize {
    LIVE.load(Ordering::Relaxed)
}

/// Drop the high-water mark to the current live total.
///
/// Called immediately before a measured run so setup allocations do not count
/// against the simulation's memory figure.
pub fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_tracks_the_largest_live_allocation() {
        reset_peak();
        let before = peak_bytes();
        let big: Vec<u8> = vec![0; 4 * 1024 * 1024];
        let after_alloc = peak_bytes();
        drop(big);
        let after_drop = peak_bytes();
        assert!(after_alloc >= before + 4 * 1024 * 1024);
        assert_eq!(after_drop, after_alloc, "peak must not fall when freed");
    }

    #[test]
    fn reset_peak_lowers_the_high_water_mark() {
        let big: Vec<u8> = vec![0; 2 * 1024 * 1024];
        let high = peak_bytes();
        drop(big);
        reset_peak();
        assert!(peak_bytes() < high);
    }
}
