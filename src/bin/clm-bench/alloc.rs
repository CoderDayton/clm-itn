use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc::new();

#[cfg(test)]
thread_local! {
    static THREAD_COUNT: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
pub fn thread_allocations() -> u64 {
    THREAD_COUNT.with(|c| c.get())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_and_realloc_each_count_once_dealloc_does_not() {
        let a = CountingAlloc::new();
        let layout = Layout::from_size_align(16, 8).unwrap();
        // SAFETY: layout is non-zero-sized; ptr comes from `a` with that layout
        // and is freed once with the layout realloc returned it under.
        unsafe {
            let p = a.alloc(layout);
            assert!(!p.is_null());
            assert_eq!(a.count(), 1);
            let p = a.realloc(p, layout, 64);
            assert!(!p.is_null());
            assert_eq!(a.count(), 2);
            a.dealloc(p, Layout::from_size_align(64, 8).unwrap());
        }
        assert_eq!(a.count(), 2);
    }

    #[test]
    fn global_count_rises_across_vec_with_capacity() {
        let before = allocations();
        let v: Vec<u64> = Vec::with_capacity(16);
        std::hint::black_box(&v);
        assert!(allocations() > before);
    }
}
