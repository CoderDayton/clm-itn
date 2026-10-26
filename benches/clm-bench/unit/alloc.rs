use crate::alloc::*;
use std::alloc::{GlobalAlloc, Layout};

pub fn thread_allocations() -> u64 {
    THREAD_COUNT.with(|c| c.get())
}

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
fn gate_fails_on_slow_p99_or_extra_allocations() {
    let calls = 1_000_000;
    assert!(crate::gate_holds(829.0, 1_010_000, calls));
    assert!(!crate::gate_holds(1001.0, 1_010_000, calls));
    assert!(!crate::gate_holds(829.0, 2_000_000, calls));
}

#[test]
fn global_count_rises_across_vec_with_capacity() {
    let before = allocations();
    let v: Vec<u64> = Vec::with_capacity(16);
    std::hint::black_box(&v);
    assert!(allocations() > before);
}
