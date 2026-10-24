use std::sync::Arc;

use crate::engine::Engine;
use crate::learn::ring::{Producer, channel};
use crate::learn::worker::Worker;
use crate::types::{CandidateId, CorrectionError, SpanReceipt};

pub struct Corrector {
    producer: Producer,
    worker: Worker,
}

impl Corrector {
    pub fn start(engine: &Engine, capacity: usize, h: u8) -> Corrector {
        let (producer, consumer) = channel(capacity);
        let worker = Worker::spawn(consumer, Arc::clone(engine.cells()), h);
        Corrector { producer, worker }
    }

    pub fn correct(
        &mut self,
        engine: &Engine,
        receipt: SpanReceipt,
        tokens: &[&[u8]],
        target: &[u8],
    ) -> Result<CandidateId, CorrectionError> {
        let result = engine.correct(&mut self.producer, receipt, tokens, target);
        if result.is_ok() {
            self.worker.wake();
        }
        result
    }

    pub fn dropped(&self) -> u64 {
        self.producer.dropped()
    }

    pub fn stop(self) {
        self.worker.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DEFAULT_H, MAX_TOKENS};
    use std::time::{Duration, Instant};

    fn run(engine: &Engine, text: &str) -> (String, SpanReceipt) {
        let tokens: Vec<&[u8]> = text.split(' ').map(str::as_bytes).collect();
        let mut out = [0u8; 128];
        let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
        let done = engine
            .format(&tokens, &mut out, &mut receipts)
            .expect("format");
        (
            String::from_utf8(out[..done.len].to_vec()).expect("utf8"),
            receipts[0],
        )
    }

    fn wait_for(engine: &Engine, text: &str, want: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if run(engine, text).0 == want {
                return true;
            }
            std::thread::yield_now();
        }
        false
    }

    #[test]
    fn corrections_reach_the_engine_without_manual_drain() {
        let engine = Engine::new(crate::english());
        let mut corrector = Corrector::start(&engine, 64, DEFAULT_H);
        let tokens: [&[u8]; 2] = [b"at", b"five"];
        for _ in 0..DEFAULT_H {
            let receipt = run(&engine, "at five").1;
            assert!(
                corrector
                    .correct(&engine, receipt, &tokens, b"five")
                    .is_ok()
            );
        }
        assert!(wait_for(&engine, "at five", "at five"));
        assert_eq!(corrector.dropped(), 0);
        corrector.stop();
    }

    #[test]
    fn out_of_set_is_reported_and_not_queued() {
        let engine = Engine::new(crate::english());
        let mut corrector = Corrector::start(&engine, 8, DEFAULT_H);
        let tokens: [&[u8]; 2] = [b"at", b"five"];
        let receipt = run(&engine, "at five").1;
        assert_eq!(
            corrector.correct(&engine, receipt, &tokens, b"V"),
            Err(CorrectionError::OutOfSet)
        );
        corrector.stop();
        assert_eq!(run(&engine, "at five").0, "at 5");
    }

    #[test]
    fn stop_applies_everything_already_queued() {
        let engine = Engine::new(crate::english());
        let mut corrector = Corrector::start(&engine, 64, DEFAULT_H);
        let tokens: [&[u8]; 2] = [b"at", b"five"];
        for _ in 0..DEFAULT_H {
            let receipt = run(&engine, "at five").1;
            corrector
                .correct(&engine, receipt, &tokens, b"five")
                .expect("in set");
        }
        corrector.stop();
        assert_eq!(run(&engine, "at five").0, "at five");
    }
}
