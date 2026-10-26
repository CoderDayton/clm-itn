use std::panic::{AssertUnwindSafe, catch_unwind};

use clm_itn::engine::{Internals, Normalizer};
use clm_itn::types::{FormatError, MAX_TOKENS, SpanReceipt};

const FORMAT_WINDOWS: usize = 200_000;
const MAX_OUT: usize = 256;
const SEED: u64 = 0x5EED_C1A3_0000_0001;

const WORDS: &str = "zero oh one two three four five six seven eight nine ten eleven twelve \
    thirteen fifteen nineteen twenty thirty forty fifty ninety hundred thousand million first \
    second third fifth twelfth twentieth seventieth point at am pm a m p the room flight number \
    code call me meeting xylo and dollars o'clock";

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

fn engine() -> Normalizer<'static> {
    Normalizer::new(clm_itn::tables::english())
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

fn run_format(e: &Normalizer, toks: &[&[u8]], out_len: usize) -> Run {
    let mut out = [0u8; MAX_OUT];
    let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
    let r = e.format_receipts(toks, &mut out[..out_len], &mut receipts);
    let bytes = match r {
        Ok(f) => out[..f.len.min(out_len)].to_vec(),
        Err(_) => Vec::new(),
    };
    (r, bytes, receipts)
}

fn check_format(e: &Normalizer, toks: &[&[u8]], out_len: usize) -> Result<(), String> {
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

const LINES: usize = 50_000;
const MAX_LINE_TOKENS: usize = 40;
const GLUE: [&str; 9] = [" ", " ", " ", "  ", "\t", ", ", ". ", "-", "? "];

fn line(r: &mut XorShift, words: &[&str]) -> String {
    let n = r.below(MAX_LINE_TOKENS + 1);
    let mut s = String::new();
    for _ in 0..n {
        s.push_str(&String::from_utf8_lossy(&token(r, words)));
        s.push_str(GLUE[r.below(GLUE.len())]);
    }
    s
}

#[test]
fn normalize_survives_random_lines() {
    let e = clm_itn::Normalizer::english();
    let words: Vec<&str> = WORDS.split_whitespace().collect();
    let mut r = XorShift(SEED);
    for i in 0..LINES {
        let text = line(&mut r, &words);
        let got = catch_unwind(AssertUnwindSafe(|| e.normalize(&text)))
            .unwrap_or_else(|_| panic!("case {i}: normalize panicked on {text:?}"));
        if text.trim().is_empty() {
            assert_eq!(got, text, "case {i}");
        }
        let breaks = |s: &str| s.matches(['\t', '\n', '\r']).collect::<String>();
        assert_eq!(breaks(&got), breaks(&text), "case {i}: {text:?}");
        let lead = |s: &str| s.len() - s.trim_start().len();
        assert_eq!(lead(&got), lead(&text), "case {i}: {text:?}");
        assert!(
            got.ends_with(&text[text.trim_end().len()..]),
            "case {i}: {text:?}"
        );
    }
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
        let mut out = [0u8; MAX_OUT];
        match e.format(&toks, &mut out[..out_len]) {
            Ok(_) => ok += 1,
            Err(FormatError::Overflow) => overflow += 1,
            Err(FormatError::TooManyTokens) => too_many += 1,
            Err(other) => panic!("case {i}: unexpected {other:?}"),
        }
    }
    eprintln!(
        "format fuzz: {FORMAT_WINDOWS} windows, ok={ok} overflow={overflow} too_many={too_many}"
    );
    assert!(ok > 0 && overflow > 0 && too_many > 0);
}
