use crate::types::CorrectionEvent;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

#[repr(align(128))]
struct Padded<T>(T);

struct Shared {
    slots: Box<[UnsafeCell<CorrectionEvent>]>,
    mask: usize,
    head: Padded<AtomicUsize>,
    tail: Padded<AtomicUsize>,
    dropped: AtomicU64,
}

// SAFETY: a slot is written only by the single producer while it lies outside
// [head, tail), and read only by the single consumer while inside it; the
// Release/Acquire pairs on head and tail order those accesses.
unsafe impl Sync for Shared {}

pub struct Producer {
    shared: Arc<Shared>,
    tail: usize,
    head_cache: usize,
}

pub struct Consumer {
    shared: Arc<Shared>,
    head: usize,
    tail_cache: usize,
}

pub fn channel(capacity: usize) -> (Producer, Consumer) {
    assert!(
        capacity >= 2 && capacity.is_power_of_two(),
        "ring capacity must be a power of two >= 2, got {capacity}"
    );
    let slots = (0..capacity)
        .map(|_| UnsafeCell::new(CorrectionEvent::default()))
        .collect();
    let shared = Arc::new(Shared {
        slots,
        mask: capacity - 1,
        head: Padded(AtomicUsize::new(0)),
        tail: Padded(AtomicUsize::new(0)),
        dropped: AtomicU64::new(0),
    });
    (
        Producer {
            shared: Arc::clone(&shared),
            tail: 0,
            head_cache: 0,
        },
        Consumer {
            shared,
            head: 0,
            tail_cache: 0,
        },
    )
}

impl Producer {
    #[inline]
    pub fn push(&mut self, ev: CorrectionEvent) -> bool {
        let s = &*self.shared;
        let cap = s.mask + 1;
        if self.tail.wrapping_sub(self.head_cache) == cap {
            self.head_cache = s.head.0.load(Ordering::Acquire);
            if self.tail.wrapping_sub(self.head_cache) == cap {
                s.dropped.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }
        let slot = &s.slots[self.tail & s.mask];
        // SAFETY: tail - head < cap, so this slot is outside [head, tail) and the
        // consumer does not read it until the Release store below publishes it.
        unsafe { *slot.get() = ev };
        self.tail = self.tail.wrapping_add(1);
        s.tail.0.store(self.tail, Ordering::Release);
        true
    }

    pub fn dropped(&self) -> u64 {
        self.shared.dropped.load(Ordering::Relaxed)
    }
}

impl Consumer {
    #[inline]
    pub fn pop(&mut self) -> Option<CorrectionEvent> {
        let s = &*self.shared;
        if self.head == self.tail_cache {
            self.tail_cache = s.tail.0.load(Ordering::Acquire);
            if self.head == self.tail_cache {
                return None;
            }
        }
        let slot = &s.slots[self.head & s.mask];
        // SAFETY: head < tail, so the producer published this slot (Acquire above)
        // and does not rewrite it until the Release store below frees it.
        let ev = unsafe { *slot.get() };
        self.head = self.head.wrapping_add(1);
        s.head.0.store(self.head, Ordering::Release);
        Some(ev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CellIndex;

    #[cfg(not(miri))]
    const STRESS_EVENTS: u32 = 1_000_000;
    #[cfg(miri)]
    const STRESS_EVENTS: u32 = 2_000;

    fn ev(seq: u32) -> CorrectionEvent {
        CorrectionEvent {
            cell: (seq >> 8) as CellIndex,
            candidate: seq as u8,
            _pad: 0,
        }
    }

    fn seq(ev: CorrectionEvent) -> u32 {
        ((ev.cell as u32) << 8) | ev.candidate as u32
    }

    #[test]
    fn fifo_order_single_thread() {
        let (mut p, mut c) = channel(8);
        for i in 0..5 {
            assert!(p.push(ev(i)));
        }
        for i in 0..5 {
            assert_eq!(c.pop(), Some(ev(i)));
        }
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn empty_pop_is_none() {
        let (_p, mut c) = channel(4);
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn full_ring_rejects_and_counts_drops() {
        let (mut p, mut c) = channel(4);
        for i in 0..4 {
            assert!(p.push(ev(i)));
        }
        assert!(!p.push(ev(100)));
        assert!(!p.push(ev(101)));
        assert_eq!(p.dropped(), 2);
        for i in 0..4 {
            assert_eq!(c.pop(), Some(ev(i)));
        }
        assert_eq!(c.pop(), None);
        assert!(p.push(ev(5)));
        assert_eq!(p.dropped(), 2);
    }

    #[test]
    fn wraparound_many_capacities() {
        for shift in 1..=8 {
            let cap = 1usize << shift;
            let (mut p, mut c) = channel(cap);
            let mut next_push = 0u32;
            let mut next_pop = 0u32;
            for round in 0..(cap * 5) {
                let burst = round % cap + 1;
                for _ in 0..burst {
                    assert!(p.push(ev(next_push)));
                    next_push += 1;
                }
                for _ in 0..burst {
                    assert_eq!(c.pop(), Some(ev(next_pop)));
                    next_pop += 1;
                }
                assert_eq!(c.pop(), None);
            }
            assert_eq!(p.dropped(), 0);
        }
    }

    #[test]
    #[should_panic]
    fn capacity_not_power_of_two_panics() {
        let _ = channel(6);
    }

    #[test]
    #[should_panic]
    fn capacity_one_panics() {
        let _ = channel(1);
    }

    #[test]
    #[should_panic]
    fn capacity_zero_panics() {
        let _ = channel(0);
    }

    #[test]
    fn producer_consumer_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Producer>();
        assert_send::<Consumer>();
    }

    #[test]
    fn cross_thread_stress_preserves_order() {
        let (mut p, mut c) = channel(1024);
        let producer = std::thread::spawn(move || {
            let mut accepted = 0u64;
            for i in 0..STRESS_EVENTS {
                if p.push(ev(i)) {
                    accepted += 1;
                }
            }
            (accepted, p.dropped())
        });

        let mut popped = 0u64;
        let mut last: Option<u32> = None;
        loop {
            match c.pop() {
                Some(e) => {
                    let s = seq(e);
                    if let Some(prev) = last {
                        assert!(s > prev, "order broken: {s} after {prev}");
                    }
                    last = Some(s);
                    popped += 1;
                }
                None if producer.is_finished() => {
                    while let Some(e) = c.pop() {
                        let s = seq(e);
                        assert!(last.is_none_or(|prev| s > prev));
                        last = Some(s);
                        popped += 1;
                    }
                    break;
                }
                None => std::hint::spin_loop(),
            }
        }

        let (accepted, dropped) = producer.join().unwrap();
        assert_eq!(popped, accepted);
        assert_eq!(popped + dropped, STRESS_EVENTS as u64);
    }
}
