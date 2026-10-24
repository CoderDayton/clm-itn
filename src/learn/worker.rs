use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::learn::cell::Cells;
use crate::learn::ring::Consumer;
use crate::types::CELL_COUNT;

const SPIN_ROUNDS: u32 = 64;
const PARK_TIMEOUT: Duration = Duration::from_millis(1);

fn check_h(h: u8) {
    assert!((1..=15).contains(&h), "h must be in 1..=15, got {h}");
}

pub fn drain(consumer: &mut Consumer, cells: &Cells, h: u8) -> usize {
    check_h(h);
    let mut applied = 0;
    while let Some(ev) = consumer.pop() {
        if (ev.cell as usize) < CELL_COUNT {
            cells.apply(ev.cell, ev.candidate, h);
            applied += 1;
        }
    }
    applied
}

pub struct Worker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(mut consumer: Consumer, cells: Arc<Cells>, h: u8) -> Worker {
        check_h(h);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut idle = 0u32;
            loop {
                if drain(&mut consumer, &cells, h) > 0 {
                    idle = 0;
                    continue;
                }
                if flag.load(Ordering::Acquire) {
                    drain(&mut consumer, &cells, h);
                    return;
                }
                if idle < SPIN_ROUNDS {
                    idle += 1;
                    std::hint::spin_loop();
                } else {
                    thread::park_timeout(PARK_TIMEOUT);
                }
            }
        });
        Worker {
            stop,
            thread: Some(thread),
        }
    }

    pub fn wake(&self) {
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if let Some(t) = self.thread.take() {
            self.stop.store(true, Ordering::Release);
            t.thread().unpark();
            if t.join().is_err() && !thread::panicking() {
                panic!("feedback worker panicked");
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learn::cell::{pack, unpack};
    use crate::learn::ring::{Producer, channel};
    use crate::types::{
        CELL_ABSTAINED, CandidateId, CellIndex, CorrectionEvent, DOMAIN_CARDINAL, DOMAIN_TIME,
        cell_index,
    };
    use std::time::{Duration, Instant};

    const A: CandidateId = 10;
    const B: CandidateId = 20;
    const C: CandidateId = 30;
    const D: CandidateId = 40;
    const START: [CandidateId; 4] = [A, B, C, D];
    const H: u8 = 4;

    fn ev(cell: CellIndex, candidate: CandidateId) -> CorrectionEvent {
        CorrectionEvent {
            cell,
            candidate,
            _pad: 0,
        }
    }

    fn push_all(p: &mut Producer, events: &[CorrectionEvent]) {
        for &e in events {
            assert!(p.push(e));
        }
    }

    #[test]
    fn drain_applies_h_corrections_in_order_and_promotes_once() {
        let cell = cell_index(DOMAIN_TIME, 7);
        let cells = Cells::shared();
        cells.store(cell, pack(START, [0; 4]));
        let (mut p, mut c) = channel(64);

        push_all(&mut p, &vec![ev(cell, B); H as usize - 1]);
        assert_eq!(drain(&mut c, &cells, H), H as usize - 1);
        assert_eq!(unpack(cells.load(cell)), (START, [0, H - 1, 0, 0]));

        push_all(&mut p, &[ev(cell, B)]);
        assert_eq!(drain(&mut c, &cells, H), 1);
        assert_eq!(unpack(cells.load(cell)), ([B, A, C, D], [0; 4]));

        push_all(&mut p, &vec![ev(cell, B); 2 * H as usize]);
        assert_eq!(drain(&mut c, &cells, H), 2 * H as usize);
        assert_eq!(unpack(cells.load(cell)), ([B, A, C, D], [0; 4]));
        assert_eq!(drain(&mut c, &cells, H), 0);
    }

    #[test]
    fn events_for_other_cells_leave_target_untouched() {
        let target = cell_index(DOMAIN_TIME, 7);
        let same_domain = cell_index(DOMAIN_TIME, 8);
        let other_domain = cell_index(DOMAIN_CARDINAL, 7);
        let cells = Cells::shared();
        let word = pack(START, [0, 2, 0, 0]);
        cells.store(target, word);
        cells.store(same_domain, pack(START, [0; 4]));
        cells.store(other_domain, pack(START, [0; 4]));
        let (mut p, mut c) = channel(64);

        let mut events = Vec::new();
        for _ in 0..H {
            events.push(ev(same_domain, C));
            events.push(ev(other_domain, D));
        }
        push_all(&mut p, &events);
        assert_eq!(drain(&mut c, &cells, H), events.len());

        assert_eq!(cells.load(target), word);
        assert_eq!(unpack(cells.load(same_domain)).0, [A, C, B, D]);
        assert_eq!(unpack(cells.load(other_domain)).0, [A, B, D, C]);
    }

    #[test]
    fn out_of_range_cell_is_skipped_and_not_counted() {
        let cells = Cells::shared();
        let (mut p, mut c) = channel(8);
        let past_end = crate::types::CELL_COUNT as CellIndex;
        push_all(&mut p, &[ev(CELL_ABSTAINED, B), ev(past_end, B)]);
        assert_eq!(drain(&mut c, &cells, H), 0);
        assert_eq!(c.pop(), None);
    }

    #[test]
    #[should_panic]
    fn drain_rejects_h_zero() {
        let (_p, mut c) = channel(2);
        drain(&mut c, &Cells::shared(), 0);
    }

    #[test]
    #[should_panic]
    fn drain_rejects_h_sixteen() {
        let (_p, mut c) = channel(2);
        drain(&mut c, &Cells::shared(), 16);
    }

    #[test]
    #[should_panic]
    fn spawn_rejects_h_out_of_range() {
        let (_p, c) = channel(2);
        let _ = Worker::spawn(c, Cells::shared(), 16);
    }

    struct XorShift(u64);

    impl XorShift {
        fn below(&mut self, n: u64) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x % n
        }
    }

    #[test]
    fn worker_matches_single_threaded_drain() {
        let targets = [
            cell_index(DOMAIN_TIME, 1),
            cell_index(DOMAIN_TIME, 2),
            cell_index(DOMAIN_CARDINAL, 1),
        ];
        let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
        let events: Vec<CorrectionEvent> = (0..10_000)
            .map(|_| {
                let cell = targets[rng.below(3) as usize];
                let cand = if rng.below(3) == 0 {
                    START[rng.below(4) as usize]
                } else {
                    C
                };
                ev(cell, cand)
            })
            .collect();

        let fresh = || {
            let cells = Cells::shared();
            for &t in &targets {
                cells.store(t, pack(START, [0; 4]));
            }
            cells
        };

        let expected = fresh();
        let (mut p, mut c) = channel(16_384);
        push_all(&mut p, &events);
        assert_eq!(drain(&mut c, &expected, H), events.len());

        let cells = Arc::new(fresh());
        let (mut p, c) = channel(256);
        let worker = Worker::spawn(c, Arc::clone(&cells), H);
        for &e in &events {
            while !p.push(e) {
                worker.wake();
                std::hint::spin_loop();
            }
            worker.wake();
        }
        worker.stop();

        for &t in &targets {
            assert_eq!(cells.load(t), expected.load(t), "cell {t:#x}");
        }
        assert_ne!(
            unpack(expected.load(targets[0])).0,
            START,
            "sequence must promote"
        );
    }

    #[test]
    fn stop_on_empty_ring_returns_promptly() {
        let (_p, c) = channel(8);
        let worker = Worker::spawn(c, Cells::shared(), H);
        std::thread::sleep(Duration::from_millis(20));
        let t = Instant::now();
        worker.stop();
        let elapsed = t.elapsed();
        assert!(elapsed < Duration::from_millis(100), "{elapsed:?}");
    }
}
