use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use clm_itn::engine::Engine;
use clm_itn::learn::cell::Cells;
use clm_itn::types::{MAX_TOKENS, SpanReceipt};

struct Counting;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
    static BYTES: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            let _ = BYTES.try_with(|b| b.set(b.get() + layout.size()));
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn counted<R>(f: impl FnOnce() -> R) -> (R, usize, usize) {
    COUNT.with(|c| c.set(0));
    BYTES.with(|b| b.set(0));
    ARMED.with(|a| a.set(true));
    let r = f();
    ARMED.with(|a| a.set(false));
    (r, COUNT.with(Cell::get), BYTES.with(Cell::get))
}

#[test]
fn engine_on_static_tables_allocates_only_its_cells() {
    let (engine, count, bytes) = counted(|| Engine::new(clm_itn::english()));
    assert_eq!(count, 1, "Engine::new made {count} allocations");
    assert!(
        bytes >= size_of::<Cells>() && bytes < size_of::<Cells>() + 128,
        "{bytes} bytes for Arc<Cells> of {}",
        size_of::<Cells>()
    );

    let tokens: [&[u8]; 4] = [b"at", b"five", b"thirty", b"pm"];
    let mut out = [0u8; 64];
    let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
    let (done, count, _) = counted(|| engine.format(&tokens, &mut out, &mut receipts));
    assert_eq!(count, 0, "format allocated");
    let done = done.expect("format");
    assert_eq!(&out[..done.len], b"at 5:30 pm");
}

#[test]
fn english_is_a_static_with_no_runtime_build() {
    let (t, count, _) = counted(clm_itn::english);
    assert_eq!(count, 0);
    assert!(core::ptr::eq(t, clm_itn::english()));
}
