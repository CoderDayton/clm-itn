use clm_itn::{Normalizer, format_line};

use crate::lines::Case;
use crate::score::{DIGITS_LEGEND, Failure, accuracy, digits_match};

pub const MAX_LINE_FAILURES: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Score {
    pub total: usize,
    pub exact: usize,
    pub digits: usize,
    pub no_digit_gold: usize,
    pub overformat: usize,
}

#[derive(Debug, Default)]
pub struct LineTally {
    pub all: Score,
    pub firm: Score,
    pub tags: Vec<(String, Score)>,
    pub failures: Vec<Failure>,
    pub all_failures: bool,
}

pub fn tag_matches(case_tag: &str, wanted: &str) -> bool {
    case_tag == wanted || case_tag.strip_suffix('!') == Some(wanted)
}

fn has_digit(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit())
}

impl Score {
    fn add(&mut self, got: &str, want: &str) {
        self.total += 1;
        self.exact += usize::from(got == want);
        self.digits += usize::from(digits_match(got, want));
        if !has_digit(want) {
            self.no_digit_gold += 1;
            self.overformat += usize::from(has_digit(got));
        }
    }
}

impl LineTally {
    /// Scores `got` against the case's written form; a case without one is skipped.
    pub fn record(&mut self, case: &Case, got: &str) {
        let Some(want) = case.written.as_deref() else {
            return;
        };
        let got = got.trim();
        self.all.add(got, want);
        if !case.debatable() {
            self.firm.add(got, want);
        }
        match self.tags.iter_mut().find(|(t, _)| *t == case.tag) {
            Some((_, s)) => s.add(got, want),
            None => {
                let mut s = Score::default();
                s.add(got, want);
                self.tags.push((case.tag.clone(), s));
            }
        }
        if got != want && (self.all_failures || self.failures.len() < MAX_LINE_FAILURES) {
            self.failures.push(Failure {
                input: case.spoken.clone(),
                got: got.to_string(),
                want: want.to_string(),
            });
        }
    }
}

pub fn score(engine: &Normalizer, cases: &[&Case], all_failures: bool) -> LineTally {
    let mut tally = LineTally {
        all_failures,
        ..LineTally::default()
    };
    let mut got = String::new();
    for c in cases.iter().filter(|c| c.written.is_some()) {
        got.clear();
        format_line(engine, &c.spoken, &mut got);
        tally.record(c, &got);
    }
    tally
}

pub fn print(t: &LineTally, malformed: usize) {
    let a = &t.all;
    if a.total == 0 {
        println!("accuracy:       no written column, timing only");
        return;
    }
    println!(
        "accuracy:       {} right, {} wrong of {} lines with a written form ({:.1}% exact)",
        a.exact,
        a.total - a.exact,
        a.total,
        accuracy(a.exact, a.total)
    );
    println!(
        "{:<22} {:>5} {:>8} {:>8} {:>16}",
        "tag", "lines", "exact", "digits", "over-format"
    );
    let row = |label: &str, s: &Score| {
        println!(
            "{label:<22} {:>5} {:>7.1}% {:>7.1}% {:>5.1}% ({}/{})",
            s.total,
            accuracy(s.exact, s.total),
            accuracy(s.digits, s.total),
            accuracy(s.overformat, s.no_digit_gold),
            s.overformat,
            s.no_digit_gold
        );
    };
    let mut tags: Vec<&(String, Score)> = t.tags.iter().collect();
    tags.sort_by(|a, b| a.0.cmp(&b.0));
    for (tag, s) in tags {
        row(tag, s);
    }
    println!("{}", "-".repeat(66));
    row("all lines", &t.all);
    row("firm (no \"!\" tags)", &t.firm);
    println!("over-format: output has digits although the gold has none (rate over those lines)");
    println!("{DIGITS_LEGEND}");
    println!("malformed lines skipped: {malformed}");
    if !t.failures.is_empty() {
        let scope = if t.all_failures { "all" } else { "first" };
        println!("failures ({scope} {}):", t.failures.len());
        for f in &t.failures {
            println!(
                "  {:?}\n    got  {:?}\n    want {:?}",
                f.input, f.got, f.want
            );
        }
    }
}
