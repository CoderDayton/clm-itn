use clm_itn::engine::Internals;
use clm_itn::types::{MAX_TOKENS, SpanReceipt};

pub const STAGES: [&str; 4] = ["tier1 lookup", "tier2 + spans", "tier3 cells", "output"];
pub const BUDGET_NS: [f64; 4] = [250.0, 200.0, 150.0, 350.0];

pub trait StageSource {
    fn profile(
        &self,
        tokens: &[&[u8]],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Option<[u64; 4]>;

    fn unavailable(&self) -> Option<&'static str> {
        None
    }
}

impl StageSource for clm_itn::engine::Normalizer<'_> {
    #[inline]
    fn profile(
        &self,
        tokens: &[&[u8]],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Option<[u64; 4]> {
        let (r, d) = self.format_profiled(tokens, out, receipts, &mut crate::clock::start);
        r.ok().map(|_| d)
    }
}

impl<S: StageSource + ?Sized> StageSource for &S {
    #[inline]
    fn profile(
        &self,
        tokens: &[&[u8]],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Option<[u64; 4]> {
        (**self).profile(tokens, out, receipts)
    }

    fn unavailable(&self) -> Option<&'static str> {
        (**self).unavailable()
    }
}

pub fn source<'e, 't>(
    engine: &'e clm_itn::engine::Normalizer<'t>,
) -> &'e clm_itn::engine::Normalizer<'t> {
    engine
}

pub struct Collected {
    pub stages: [Vec<u32>; 4],
    pub skipped: usize,
}

pub fn collect(src: &impl StageSource, corpus: &crate::corpus::Corpus, overhead: u64) -> Collected {
    let mut buf: [&[u8]; MAX_TOKENS] = [&[]; MAX_TOKENS];
    let mut out = [0u8; crate::OUT_BYTES];
    let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
    let mut stages: [Vec<u32>; 4] = Default::default();
    let mut skipped = 0;
    for i in 0..corpus.len() {
        let k = corpus.utterance(i, &mut buf);
        match src.profile(std::hint::black_box(&buf[..k]), &mut out, &mut receipts) {
            Some(d) => {
                for (s, t) in stages.iter_mut().zip(d) {
                    s.push(crate::replay::net_ticks(0, t, overhead));
                }
            }
            None => skipped += 1,
        }
    }
    for s in &mut stages {
        s.sort_unstable();
    }
    Collected { stages, skipped }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageStat {
    pub name: &'static str,
    pub samples: usize,
    pub p50_ns: f64,
    pub p99_ns: f64,
    pub budget_ns: f64,
    pub pass: bool,
}

pub fn stage_stats(sorted: [&[u32]; 4], ns_per_tick: f64) -> [StageStat; 4] {
    core::array::from_fn(|i| {
        let ticks = sorted[i];
        let ns = |p: f64| match ticks.is_empty() {
            true => 0.0,
            false => {
                let t = crate::replay::percentile(ticks, p) as u64;
                crate::clock::ticks_to_ns(t, ns_per_tick)
            }
        };
        let p99_ns = ns(0.99);
        StageStat {
            name: STAGES[i],
            samples: ticks.len(),
            p50_ns: ns(0.50),
            p99_ns,
            budget_ns: BUDGET_NS[i],
            pass: !ticks.is_empty() && p99_ns <= BUDGET_NS[i],
        }
    })
}

pub fn format_report(stats: &[StageStat; 4], note: Option<&str>) -> String {
    let mut s = format!(
        "{:<14} {:>8} {:>8} {:>8}  verdict\n",
        "stage", "p50 ns", "p99 ns", "budget"
    );
    for st in stats {
        let row = if st.samples == 0 {
            format!(
                "{:<14} {:>8} {:>8} {:>8.0}  no samples",
                st.name, "-", "-", st.budget_ns
            )
        } else {
            let verdict = if st.pass { "PASS" } else { "WARN over budget" };
            format!(
                "{:<14} {:>8.1} {:>8.1} {:>8.0}  {verdict}",
                st.name, st.p50_ns, st.p99_ns, st.budget_ns
            )
        };
        s.push_str(&row);
        s.push('\n');
    }
    s.push_str("stage budgets are warnings; the end-to-end p99 is the gate\n");
    if let Some(note) = note {
        s.push_str(&format!("note: {note}\n"));
    }
    s
}
