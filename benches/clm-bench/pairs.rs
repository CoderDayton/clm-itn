use std::process::ExitCode;

use clm_itn::tables::Tables;
use clm_itn::types::ROLE_NONE;
use clm_itn::{Normalizer, Setting};

use crate::score::{DIGITS_LEGEND, WordRole, accuracy, is_partial};

pub const OUT_OF_SCOPE: [&str; 2] = ["WHITELIST", "ELECTRONIC"];
pub const MAX_MISSES: usize = 3;
pub const PLAIN: &str = "PLAIN";
const PARTIAL_LEGEND: &str = "partial: output has digits and keeps an input number/unit word the gold lacks; lower is better";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Total {
    InScope,
    All,
    Balanced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub class: String,
    pub spoken: String,
    pub written: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Miss {
    pub spoken: String,
    pub got: String,
    pub want: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassScore {
    pub class: String,
    pub total: usize,
    pub exact: usize,
    pub relaxed: usize,
    pub digits: usize,
    pub partial: usize,
    pub partial_examples: Vec<Miss>,
    pub misses: Vec<Miss>,
}

#[derive(Debug, Default)]
pub struct PairTally {
    pub classes: Vec<ClassScore>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub line: String,
    pub items: Vec<(std::ops::Range<usize>, Span)>,
}

pub fn word_role(t: &Tables, w: &str) -> WordRole {
    let c = t.lexicon.lookup(w.as_bytes()).class as usize;
    let start = (t.starts[c / 64] >> (c % 64)) & 1 == 1;
    let role = t.role_of[c];
    let affix = role < 16 && ((t.suffix_roles | t.prefix_roles) >> role) & 1 == 1;
    match (role != ROLE_NONE && !start, affix) {
        (false, _) => WordRole::Plain,
        (true, true) => WordRole::Affix,
        (true, false) => WordRole::Number,
    }
}

pub fn number_word(t: &Tables, w: &str) -> bool {
    word_role(t, w) == WordRole::Number
}

fn spoken_tokens(line: &str) -> Vec<&str> {
    line.split(' ')
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .collect()
}

fn parse_span(rest: &str, tokens: &[&str]) -> Option<(Span, std::ops::Range<usize>)> {
    let mut f = rest.splitn(4, ' ');
    let start: usize = f.next()?.parse().ok()?;
    let len: usize = f.next()?.parse().ok()?;
    let class = f.next()?;
    let written = f.next().filter(|w| !w.is_empty())?;
    if len == 0 || class.is_empty() || start.checked_add(len)? > tokens.len() {
        return None;
    }
    let words: Vec<&str> = tokens[start..start + len]
        .iter()
        .copied()
        .filter(|t| !t.is_empty())
        .collect();
    let span = Span {
        class: class.to_string(),
        spoken: words.join(" "),
        written: written.to_string(),
    };
    Some((span, start..start + len))
}

fn push_plain_runs(
    tokens: &[&str],
    covered: &[bool],
    is_number: &impl Fn(&str) -> bool,
    out: &mut Vec<(std::ops::Range<usize>, Span)>,
) {
    let mut i = 0;
    while i < tokens.len() {
        if covered[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < tokens.len() && !covered[i] {
            i += 1;
        }
        let words: Vec<&str> = tokens[start..i]
            .iter()
            .copied()
            .filter(|t| !t.is_empty())
            .collect();
        if words.iter().any(|w| is_number(w)) {
            let text = words.join(" ");
            let span = Span {
                class: PLAIN.to_string(),
                spoken: text.clone(),
                written: text,
            };
            out.push((start..i, span));
        }
    }
}

struct Open<'a> {
    line: &'a str,
    toks: Vec<&'a str>,
    covered: Vec<bool>,
    items: Vec<(std::ops::Range<usize>, Span)>,
}

fn close(open: Open<'_>, is_number: &impl Fn(&str) -> bool) -> Record {
    let mut items = open.items;
    push_plain_runs(&open.toks, &open.covered, is_number, &mut items);
    Record {
        line: open.line.to_string(),
        items,
    }
}

pub fn parse_records(text: &str, is_number: impl Fn(&str) -> bool) -> (Vec<Record>, usize) {
    let mut records = Vec::new();
    let mut bad = 0;
    let mut open: Option<Open> = None;
    for line in text.lines() {
        if line.is_empty() {
            if let Some(o) = open.take() {
                records.push(close(o, &is_number));
            }
        } else if let Some(s) = line.strip_prefix("S ") {
            if let Some(o) = open.take() {
                records.push(close(o, &is_number));
            }
            let toks = spoken_tokens(s);
            open = Some(Open {
                line: s,
                covered: vec![false; toks.len()],
                toks,
                items: Vec::new(),
            });
        } else if let (Some(rest), Some(o)) = (line.strip_prefix("T "), open.as_mut()) {
            match parse_span(rest, &o.toks) {
                Some((s, range)) => {
                    o.covered[range.clone()].fill(true);
                    o.items.push((range, s));
                }
                None => bad += 1,
            }
        } else {
            bad += 1;
        }
    }
    if let Some(o) = open {
        records.push(close(o, &is_number));
    }
    (records, bad)
}

fn anchors(raw: &[&str], out: &[&str]) -> Vec<Option<usize>> {
    let lower = |v: &[&str]| v.iter().map(|t| t.to_lowercase()).collect::<Vec<_>>();
    let (raw_l, out_l) = (lower(raw), lower(out));
    let eq = |i: usize, j: usize| raw_l[i] == out_l[j];
    let (n, m) = (raw.len(), out.len());
    let mut lcs = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if eq(i, j) {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut map = vec![None; n];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if eq(i, j) && lcs[i][j] == lcs[i + 1][j + 1] + 1 {
            map[i] = Some(j);
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    map
}

fn gap_output(map: &[Option<usize>], out_len: usize, i: usize) -> (usize, std::ops::Range<usize>) {
    let mut s = i;
    while s > 0 && map[s - 1].is_none() {
        s -= 1;
    }
    let mut e = i;
    while e < map.len() && map[e].is_none() {
        e += 1;
    }
    let lo = s.checked_sub(1).and_then(|k| map[k]).map_or(0, |j| j + 1);
    let hi = map.get(e).copied().flatten().unwrap_or(out_len);
    (e, lo..hi.max(lo))
}

pub fn span_output(raw: &[&str], out: &[&str], range: std::ops::Range<usize>) -> String {
    let map = anchors(raw, out);
    let mut pieces: Vec<&str> = Vec::new();
    let mut i = range.start;
    while i < range.end.min(raw.len()) {
        match map[i] {
            Some(j) => {
                let t = out[j].trim_matches(|c: char| !c.is_alphanumeric());
                if !t.is_empty() {
                    pieces.push(t);
                }
                i += 1;
            }
            None => {
                let (end, gap) = gap_output(&map, out.len(), i);
                pieces.extend_from_slice(&out[gap]);
                i = end;
            }
        }
    }
    let joined = pieces.join(" ");
    let edge = |c: char| !c.is_alphanumeric();
    let first = raw.get(range.start).copied().unwrap_or("");
    let last = raw.get(range.end.saturating_sub(1)).copied().unwrap_or("");
    let lead = &first[..first.len() - first.trim_start_matches(edge).len()];
    let trail = &last[last.trim_end_matches(edge).len()..];
    let s = joined.strip_prefix(lead).unwrap_or(&joined);
    s.strip_suffix(trail).unwrap_or(s).to_string()
}

pub fn relaxed(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .flat_map(char::to_lowercase)
        .collect()
}

impl PairTally {
    pub fn record(&mut self, span: &Span, got: &str) {
        let idx = match self.classes.iter().position(|c| c.class == span.class) {
            Some(i) => i,
            None => {
                self.classes.push(ClassScore {
                    class: span.class.clone(),
                    ..ClassScore::default()
                });
                self.classes.len() - 1
            }
        };
        let c = &mut self.classes[idx];
        c.total += 1;
        c.exact += usize::from(got.trim() == span.written);
        let ok = relaxed(got) == relaxed(&span.written);
        c.relaxed += usize::from(ok);
        c.digits += usize::from(crate::score::value_match(&span.class, got, &span.written));
        if !ok && c.misses.len() < MAX_MISSES {
            c.misses.push(Miss {
                spoken: span.spoken.clone(),
                got: got.trim().to_string(),
                want: span.written.clone(),
            });
        }
    }

    pub fn record_partial(&mut self, span: &Span, got: &str) {
        let Some(c) = self.classes.iter_mut().find(|c| c.class == span.class) else {
            return;
        };
        c.partial += 1;
        if c.partial_examples.len() < MAX_MISSES {
            c.partial_examples.push(Miss {
                spoken: span.spoken.clone(),
                got: got.trim().to_string(),
                want: span.written.clone(),
            });
        }
    }

    fn in_total(c: &ClassScore, scope: Total) -> bool {
        let out = OUT_OF_SCOPE.contains(&c.class.as_str());
        let plain = c.class == PLAIN;
        match scope {
            Total::InScope => !out && !plain,
            Total::All => !plain,
            Total::Balanced => !out,
        }
    }

    pub fn partials(&self, scope: Total) -> usize {
        self.classes
            .iter()
            .filter(|c| Self::in_total(c, scope))
            .map(|c| c.partial)
            .sum()
    }

    pub fn totals(&self, scope: Total) -> (usize, usize, usize, usize) {
        self.classes
            .iter()
            .filter(|c| Self::in_total(c, scope))
            .fold((0, 0, 0, 0), |(t, e, r, d), c| {
                (t + c.total, e + c.exact, r + c.relaxed, d + c.digits)
            })
    }
}

pub fn run(path: &str, isolated: bool) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let tables = clm_itn::tables::english();
    let (records, malformed) = parse_records(&text, |w| number_word(tables, w));
    let engine = Normalizer::english().with(Setting::SingleDigits);
    let mut tally = PairTally::default();
    let mut sentence = String::new();
    let mut got = String::new();
    for r in &records {
        sentence.clear();
        if !isolated {
            engine.normalize_into(&r.line, &mut sentence);
        }
        let raw: Vec<&str> = r.line.split(' ').collect();
        let out: Vec<&str> = sentence.split_whitespace().collect();
        for (range, s) in &r.items {
            got.clear();
            if isolated {
                engine.normalize_into(&s.spoken, &mut got);
            } else {
                got.push_str(&span_output(&raw, &out, range.clone()));
            }
            tally.record(s, &got);
            if is_partial(&got, &s.written, |w| word_role(tables, w)) {
                tally.record_partial(s, &got);
            }
        }
    }
    let mode = if isolated {
        "isolated (each span formatted alone)"
    } else {
        "in context (whole sentence formatted, spans aligned)"
    };
    println!("scoring: {mode}");
    report(path, &tally, malformed);
    ExitCode::SUCCESS
}

fn print_misses(label: &str, misses: &[Miss]) {
    println!("\n{label} (first {}):", misses.len());
    for m in misses {
        println!(
            "  {:?}\n    got  {:?}\n    want {:?}",
            m.spoken, m.got, m.want
        );
    }
}

fn report(path: &str, tally: &PairTally, malformed: usize) {
    let mut classes: Vec<&ClassScore> = tally.classes.iter().collect();
    classes.sort_by(|a, b| b.total.cmp(&a.total).then(a.class.cmp(&b.class)));
    println!("pairs file: {path}");
    println!(
        "{:<12} {:>6} {:>8} {:>8} {:>8} {:>12}",
        "class", "spans", "exact", "relaxed", "digits", "partial"
    );
    for c in &classes {
        let scope = if OUT_OF_SCOPE.contains(&c.class.as_str()) {
            "  (out of scope)"
        } else {
            ""
        };
        println!(
            "{:<12} {:>6} {:>7.1}% {:>7.1}% {:>7.1}% {:>5} {:>5.1}%{scope}",
            c.class,
            c.total,
            accuracy(c.exact, c.total),
            accuracy(c.relaxed, c.total),
            accuracy(c.digits, c.total),
            c.partial,
            accuracy(c.partial, c.total)
        );
    }
    println!("{}", "-".repeat(60));
    for (label, scope) in [
        ("in scope", Total::InScope),
        ("all classes", Total::All),
        ("balanced", Total::Balanced),
    ] {
        let (t, e, r, d) = tally.totals(scope);
        let p = tally.partials(scope);
        println!(
            "{label:<12} {t:>6} {:>7.1}% {:>7.1}% {:>7.1}% {p:>5} {:>5.1}%",
            accuracy(e, t),
            accuracy(r, t),
            accuracy(d, t),
            accuracy(p, t)
        );
    }
    println!("out of scope: {}", OUT_OF_SCOPE.join(", "));
    println!(
        "{PLAIN}: non-span runs with a number word, correct when kept as words; only in balanced"
    );
    println!("balanced = in scope + {PLAIN}");
    println!("relaxed ignores case, whitespace and commas");
    println!("{DIGITS_LEGEND}");
    println!("{PARTIAL_LEGEND}");
    println!("malformed lines skipped: {malformed}");
    for c in classes.iter().filter(|c| !c.partial_examples.is_empty()) {
        print_misses(&format!("{} partial", c.class), &c.partial_examples);
    }
    for c in classes.iter().filter(|c| !c.misses.is_empty()) {
        print_misses(&format!("{} misses", c.class), &c.misses);
    }
}
