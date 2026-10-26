use crate::tiers::*;
use clm_itn::types::{MAX_TOKENS, SpanReceipt};

fn ramp(n: u32) -> Vec<u32> {
    (1..=n).collect()
}

struct Unavailable;

impl StageSource for Unavailable {
    fn profile(
        &self,
        _tokens: &[&[u8]],
        _out: &mut [u8],
        _receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Option<[u64; 4]> {
        None
    }

    fn unavailable(&self) -> Option<&'static str> {
        Some("format_profiled not available yet (engine hook pending)")
    }
}

#[test]
fn percentiles_scale_ticks_to_ns_per_stage() {
    let r = ramp(1000);
    let s = stage_stats([&r, &r, &r, &r], 0.25);
    assert_eq!(s[0].p50_ns, 125.0);
    assert_eq!(s[0].p99_ns, 247.5);
    assert_eq!(s[0].samples, 1000);
    assert_eq!(s.map(|x| x.name), STAGES);
    assert_eq!(s.map(|x| x.budget_ns), BUDGET_NS);
}

#[test]
fn verdict_is_p99_at_or_under_budget() {
    let r = ramp(1000);
    let s = stage_stats([&r, &r, &r, &r], 0.25);
    assert_eq!(s.map(|x| x.pass), [true, false, false, true]);

    let at = vec![1000u32; 100];
    let s = stage_stats([&at, &at, &at, &at], 0.25);
    assert!(s[0].pass, "p99 equal to the budget passes");
    assert!(!s[1].pass);
}

#[test]
fn empty_stage_has_no_samples_and_fails() {
    let r = ramp(10);
    let s = stage_stats([&r, &[], &r, &r], 1.0);
    assert_eq!(s[1].samples, 0);
    assert!(!s[1].pass);
    assert!(s[0].pass);
}

#[test]
fn report_lists_each_stage_with_budget_and_verdict() {
    let r = ramp(1000);
    let s = stage_stats([&r, &r, &r, &r], 0.25);
    let text = format_report(&s, None);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].contains("stage") && lines[0].contains("budget"));
    assert!(lines[1].starts_with("tier1 lookup"));
    assert!(lines[1].contains("125.0") && lines[1].contains("247.5"));
    assert!(lines[1].contains("250") && lines[1].ends_with("PASS"));
    assert!(lines[2].ends_with("WARN over budget"));
    assert!(lines[4].starts_with("output") && lines[4].ends_with("PASS"));
    assert!(text.contains("end-to-end p99 is the gate"));
}

#[test]
fn report_explains_missing_source() {
    let s = stage_stats([&[], &[], &[], &[]], 1.0);
    let text = format_report(&s, Some("format_profiled not available"));
    assert!(text.contains("format_profiled not available"));
    assert!(text.contains("no samples"));
}

struct Fake {
    script: [u64; 4],
}

impl StageSource for Fake {
    fn profile(
        &self,
        tokens: &[&[u8]],
        _out: &mut [u8],
        _receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Option<[u64; 4]> {
        (!tokens.is_empty()).then_some(self.script)
    }
}

#[test]
fn collect_subtracts_overhead_sorts_and_counts_skips() {
    struct Alt;
    impl StageSource for Alt {
        fn profile(
            &self,
            tokens: &[&[u8]],
            _out: &mut [u8],
            _receipts: &mut [SpanReceipt; MAX_TOKENS],
        ) -> Option<[u64; 4]> {
            let k = tokens.len() as u64;
            (k % 2 == 1).then_some([10 + k, 5, 3, 100])
        }
    }
    let corpus = crate::corpus::Corpus::generate(200, 3);
    let c = collect(&Alt, &corpus, 4);
    let kept = c.stages[0].len();
    assert_eq!(kept + c.skipped, 200);
    assert!(kept > 0 && c.skipped > 0);
    assert!(c.stages.iter().all(|s| s.len() == kept));
    assert!(c.stages[0].is_sorted());
    assert!(c.stages[0].iter().all(|&t| t >= 7 && t % 2 == 1));
    assert!(c.stages[1].iter().all(|&t| t == 1));
    assert!(c.stages[2].iter().all(|&t| t == 0), "saturates at 0");
    assert!(c.stages[3].iter().all(|&t| t == 96));
}

#[test]
fn engine_source_profiles_every_ok_window_and_matches_format() {
    let engine = clm_itn::engine::Normalizer::new(clm_itn::english());
    let src = source(&engine);
    assert_eq!(src.unavailable(), None);
    let corpus = crate::corpus::Corpus::generate(500, 7);
    let mut buf: [&[u8]; MAX_TOKENS] = [&[]; MAX_TOKENS];
    let (mut a, mut b) = ([0u8; crate::OUT_BYTES], [0u8; crate::OUT_BYTES]);
    let mut rb = [SpanReceipt::default(); MAX_TOKENS];
    for i in 0..corpus.len() {
        let k = corpus.utterance(i, &mut buf);
        let want = engine.format(&buf[..k], &mut a);
        let got = src.profile(&buf[..k], &mut b, &mut rb);
        assert_eq!(got.is_some(), want.is_ok(), "utterance {i}");
        if let Ok(f) = want {
            assert_eq!(a[..f.len], b[..f.len], "utterance {i}");
        }
    }
    let c = collect(&src, &corpus, 0);
    assert!(c.skipped < corpus.len() / 10, "skipped {}", c.skipped);
    assert!(c.stages.iter().all(|s| s.len() == corpus.len() - c.skipped));
}

#[test]
fn unavailable_source_yields_nothing_and_says_why() {
    let corpus = crate::corpus::Corpus::generate(50, 1);
    let c = collect(&Unavailable, &corpus, 0);
    assert_eq!(c.skipped, 50);
    assert!(c.stages.iter().all(Vec::is_empty));
    assert!(Unavailable.unavailable().is_some());
}

#[test]
fn fake_source_feeds_stats() {
    let f = Fake {
        script: [400, 300, 200, 100],
    };
    let mut out = [0u8; 16];
    let mut rc = [SpanReceipt::default(); MAX_TOKENS];
    let mut cols: [Vec<u32>; 4] = Default::default();
    for toks in [&[b"a".as_slice()][..], &[], &[b"b".as_slice()]] {
        if let Some(d) = f.profile(toks, &mut out, &mut rc) {
            for (c, v) in cols.iter_mut().zip(d) {
                c.push(v as u32);
            }
        }
    }
    let s = stage_stats([&cols[0], &cols[1], &cols[2], &cols[3]], 0.5);
    assert_eq!(s.map(|x| x.samples), [2; 4]);
    assert_eq!(s.map(|x| x.p99_ns), [200.0, 150.0, 100.0, 50.0]);
    assert_eq!(s.map(|x| x.pass), [true; 4]);
}
