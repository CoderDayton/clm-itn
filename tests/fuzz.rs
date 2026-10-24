use std::panic::{AssertUnwindSafe, catch_unwind};

use clm_itn::engine::Engine;
use clm_itn::types::{CELL_ABSTAINED, CorrectionError, FormatError, MAX_TOKENS, SpanReceipt};

const FORMAT_WINDOWS: usize = 200_000;
const RESOLVE_CASES: usize = 50_000;
const MAX_OUT: usize = 256;
const SEED: u64 = 0x5EED_C1A3_0000_0001;

const WORDS: &str = "zero oh one two three four five six seven eight nine ten eleven twelve \
    thirteen fifteen nineteen twenty thirty forty fifty ninety hundred thousand million first \
    second third fifth twelfth twentieth seventieth point at am pm a m p the room flight number \
    code call me meeting xylo and dollars o'clock";

const TARGETS: [&[u8]; 14] = [
    b"5:30",
    b"530",
    b"5.30",
    b"5th",
    b"70th",
    b"402",
    b"4 0 2",
    b"3.14",
    b"123",
    b"seventy",
    b"",
    b"\xff\xfe",
    b"05:30 p.m.",
    b"1,000",
];

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn engine() -> Engine<'static> {
    Engine::new(clm_itn::english())
}

fn token(r: &mut XorShift, words: &[&str]) -> Vec<u8> {
    let word = |r: &mut XorShift| words[r.below(words.len())].as_bytes().to_vec();
    let ascii = |r: &mut XorShift, n: usize| -> Vec<u8> {
        (0..n).map(|_| b'a' + r.below(26) as u8).collect()
    };
    match r.below(12) {
        0..=4 => word(r),
        5 => word(r)
            .into_iter()
            .map(|b| {
                if r.below(2) == 0 {
                    b.to_ascii_uppercase()
                } else {
                    b
                }
            })
            .collect(),
        6 => (0..1 + r.below(20))
            .map(|_| 0x20 + r.below(95) as u8)
            .collect(),
        7 => (0..1 + r.below(20)).map(|_| r.next() as u8).collect(),
        8 => {
            let s: String = (0..1 + r.below(6))
                .map(|_| ['é', 'ß', '中', '🦀', 'Ω', '\u{0}'][r.below(6)])
                .collect();
            s.into_bytes()
        }
        9 => Vec::new(),
        10 => {
            let n = [15, 16, 40][r.below(3)];
            ascii(r, n)
        }
        _ => [word(r), word(r)].concat(),
    }
}

fn window(r: &mut XorShift, words: &[&str]) -> Vec<Vec<u8>> {
    let n = if r.below(20) == 0 {
        MAX_TOKENS + 1 + r.below(4)
    } else {
        r.below(MAX_TOKENS + 1)
    };
    (0..n).map(|_| token(r, words)).collect()
}

type Run = (
    Result<clm_itn::engine::Formatted, FormatError>,
    Vec<u8>,
    [SpanReceipt; MAX_TOKENS],
);

fn run_format(e: &Engine, toks: &[&[u8]], out_len: usize) -> Run {
    let mut out = [0u8; MAX_OUT];
    let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
    let r = e.format(toks, &mut out[..out_len], &mut receipts);
    let bytes = match r {
        Ok(f) => out[..f.len.min(out_len)].to_vec(),
        Err(_) => Vec::new(),
    };
    (r, bytes, receipts)
}

fn check_format(e: &Engine, toks: &[&[u8]], out_len: usize) -> Result<(), String> {
    let n = toks.len();
    let (first, bytes, receipts) = run_format(e, toks, out_len);
    match first {
        Ok(f) => {
            if f.len > out_len {
                return Err(format!("len {} > out.len() {out_len}", f.len));
            }
            if f.spans > n {
                return Err(format!("{} spans for {n} tokens", f.spans));
            }
            for (k, rc) in receipts[..f.spans].iter().enumerate() {
                let (s, l) = (rc.start as usize, rc.len as usize);
                if l == 0 || s + l > n {
                    return Err(format!("receipt {k} {rc:?} outside window of {n}"));
                }
            }
        }
        Err(FormatError::TooManyTokens) if n > MAX_TOKENS => {}
        Err(FormatError::Overflow) if n <= MAX_TOKENS => {}
        Err(err) => return Err(format!("unexpected {err:?} for {n} tokens")),
    }
    let (second, bytes2, receipts2) = run_format(e, toks, out_len);
    let spans = first.map_or(0, |f| f.spans);
    if second != first || bytes2 != bytes || receipts2[..spans] != receipts[..spans] {
        return Err(format!("nondeterministic: {first:?} vs {second:?}"));
    }
    Ok(())
}

#[test]
fn format_survives_random_windows() {
    let e = engine();
    let words: Vec<&str> = WORDS.split_whitespace().collect();
    let mut r = XorShift(SEED);
    let (mut ok, mut overflow, mut too_many) = (0usize, 0usize, 0usize);
    for i in 0..FORMAT_WINDOWS {
        let owned = window(&mut r, &words);
        let toks: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
        let out_len = r.below(MAX_OUT + 1);
        let res = catch_unwind(AssertUnwindSafe(|| check_format(&e, &toks, out_len)));
        match res {
            Ok(Ok(())) => {}
            Ok(Err(msg)) => panic!("case {i}: {msg}; out_len={out_len} tokens={owned:?}"),
            Err(_) => panic!("case {i}: format panicked; out_len={out_len} tokens={owned:?}"),
        }
        let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
        let mut out = [0u8; MAX_OUT];
        match e.format(&toks, &mut out[..out_len], &mut receipts) {
            Ok(_) => ok += 1,
            Err(FormatError::Overflow) => overflow += 1,
            Err(FormatError::TooManyTokens) => too_many += 1,
        }
    }
    eprintln!(
        "format fuzz: {FORMAT_WINDOWS} windows, ok={ok} overflow={overflow} too_many={too_many}"
    );
    assert!(ok > 0 && overflow > 0 && too_many > 0);
}

#[test]
fn resolve_survives_random_receipts_and_targets() {
    let e = engine();
    let words: Vec<&str> = WORDS.split_whitespace().collect();
    let mut r = XorShift(SEED ^ 0xC0FFEE);
    let mut counts = [0usize; 5];
    for i in 0..RESOLVE_CASES {
        let mut owned = window(&mut r, &words);
        owned.truncate(MAX_TOKENS);
        let toks: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();

        let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
        let mut out = [0u8; MAX_OUT];
        let real = match e.format(&toks, &mut out, &mut receipts) {
            Ok(f) if f.spans > 0 => Some(receipts[r.below(f.spans)]),
            _ => None,
        };
        let receipt = match (real, r.below(3)) {
            (Some(rc), 0 | 1) => rc,
            _ => SpanReceipt {
                cell: match r.below(4) {
                    0 => CELL_ABSTAINED,
                    1 => r.below(4096) as u16,
                    _ => r.next() as u16,
                },
                start: r.next() as u8 % 12,
                len: r.next() as u8 % 12,
            },
        };
        let target: Vec<u8> = if r.below(4) == 0 {
            (0..r.below(40)).map(|_| r.next() as u8).collect()
        } else {
            TARGETS[r.below(TARGETS.len())].to_vec()
        };
        let span_toks: &[&[u8]] = if r.below(2) == 0 {
            &toks
        } else {
            let s = (receipt.start as usize).min(toks.len());
            let end = (s + receipt.len as usize).min(toks.len());
            &toks[s..end]
        };

        let res = catch_unwind(AssertUnwindSafe(|| e.resolve(receipt, span_toks, &target)));
        match res {
            Ok(Ok(ev)) => {
                assert_eq!(
                    ev.cell, receipt.cell,
                    "case {i}: event cell differs from receipt"
                );
                counts[0] += 1;
            }
            Ok(Err(CorrectionError::OutOfSet)) => counts[1] += 1,
            Ok(Err(CorrectionError::Abstained)) => counts[2] += 1,
            Ok(Err(CorrectionError::InvalidReceipt)) => counts[3] += 1,
            Ok(Err(CorrectionError::Dropped)) => counts[4] += 1,
            Err(_) => panic!(
                "case {i}: resolve panicked; receipt={receipt:?} tokens={span_toks:?} target={target:?}"
            ),
        }
    }
    eprintln!(
        "resolve fuzz: {RESOLVE_CASES} cases, ok={} out_of_set={} abstained={} invalid={} dropped={}",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );
}

const ROUND_TRIP_WINDOWS: usize = 50_000;

fn render_candidate(
    e: &Engine,
    domain: u8,
    cand: u8,
    span: &[&[u8]],
    out: &mut [u8; MAX_OUT],
) -> Option<usize> {
    let t = e.tables();
    if cand == clm_itn::types::CANDIDATE_NONE
        || t.candidates.span_len(domain, cand) as usize != span.len()
    {
        return None;
    }
    let mut values = [0u32; MAX_TOKENS];
    for (v, tok) in values.iter_mut().zip(span) {
        let canonical = t.lexicon.lookup(tok).canonical as usize;
        *v = t.value_of.get(canonical).copied().unwrap_or(0);
    }
    let mut pos = 0;
    t.candidates
        .render(domain, cand, &values[..span.len()], span, out, &mut pos)
        .ok()
        .map(|_| pos)
}

#[test]
fn resolve_accepts_every_rendered_candidate() {
    let e = engine();
    let words: Vec<&str> = WORDS.split_whitespace().collect();
    let mut r = XorShift(SEED ^ 0xACCE_55ED);
    let (mut spans, mut hits) = (0usize, 0usize);
    for i in 0..ROUND_TRIP_WINDOWS {
        let n = 1 + r.below(MAX_TOKENS);
        let toks: Vec<&[u8]> = (0..n)
            .map(|_| words[r.below(words.len())].as_bytes())
            .collect();
        let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
        let mut out = [0u8; MAX_OUT];
        let Ok(f) = e.format(&toks, &mut out, &mut receipts) else {
            continue;
        };
        for rc in &receipts[..f.spans] {
            if rc.cell == CELL_ABSTAINED {
                continue;
            }
            spans += 1;
            let span = &toks[rc.start as usize..(rc.start + rc.len) as usize];
            let (ranks, _) = clm_itn::learn::cell::unpack(e.cells().load(rc.cell));
            let d = clm_itn::types::cell_domain(rc.cell);
            for (rank, &cand) in ranks.iter().enumerate() {
                let mut target = [0u8; MAX_OUT];
                let Some(len) = render_candidate(&e, d, cand, span, &mut target) else {
                    continue;
                };
                let target = &target[..len];
                let ev = e.resolve(*rc, &toks, target).unwrap_or_else(|err| {
                    panic!("case {i}: {err:?} for rank {rank} cand {cand} of {rc:?} in {toks:?}")
                });
                assert_eq!(ev.cell, rc.cell, "case {i}");
                let first = ranks
                    .iter()
                    .position(|&c| {
                        let mut buf = [0u8; MAX_OUT];
                        render_candidate(&e, d, c, span, &mut buf) == Some(len)
                            && &buf[..len] == target
                    })
                    .expect("the rendered candidate itself matches");
                let want = ranks[first];
                assert_eq!(ev.candidate, want, "case {i}: not the first equal render");
                hits += 1;
            }
        }
    }
    eprintln!("round-trip fuzz: {ROUND_TRIP_WINDOWS} windows, {spans} spans, {hits} resolves");
    assert!(hits >= 1_000, "only {hits} round trips; generator too weak");
}
