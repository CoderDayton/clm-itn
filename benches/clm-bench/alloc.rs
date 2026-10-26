use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc::new();

#[cfg(test)]
thread_local! {
    pub static THREAD_COUNT: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

#[inline(always)]
fn bump(count: &AtomicU64) {
    count.fetch_add(1, Ordering::Relaxed);
    #[cfg(test)]
    let _ = THREAD_COUNT.try_with(|c| c.set(c.get() + 1));
}

pub struct CountingAlloc {
    count: AtomicU64,
}

impl CountingAlloc {
    pub const fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
        }
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

// SAFETY: every method forwards to `System` with the caller's arguments unchanged.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bump(&self.count);
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract for `layout`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System` via this allocator with `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        bump(&self.count);
        // SAFETY: `ptr` came from `System` via this allocator with `layout`; the
        // caller upholds `new_size`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

pub fn allocations() -> u64 {
    GLOBAL.count()
}
