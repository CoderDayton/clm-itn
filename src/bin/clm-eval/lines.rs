use crate::eval::{Failure, digits_match};

pub const MAX_LINE_FAILURES: usize = 10;
pub const UNTAGGED: &str = "(untagged)";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineCase {
    pub spoken: String,
    pub written: String,
    pub tag: String,
}

impl LineCase {
    pub fn debatable(&self) -> bool {
        self.tag.ends_with('!')
    }
}

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

pub fn parse_lines(text: &str) -> (Vec<LineCase>, usize) {
    let mut cases = Vec::new();
    let mut bad = 0;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.splitn(3, '\t');
        let spoken = cols.next().unwrap_or("").trim();
        let written = cols.next().unwrap_or("").trim();
        let tag = cols.next().map(str::trim).filter(|t| !t.is_empty());
        if spoken.is_empty() || written.is_empty() {
            bad += 1;
            continue;
        }
        cases.push(LineCase {
            spoken: spoken.to_string(),
            written: written.to_string(),
            tag: tag.unwrap_or(UNTAGGED).to_string(),
        });
    }
    (cases, bad)
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
    pub fn record(&mut self, case: &LineCase, got: &str) {
        let got = got.trim();
        let want = case.written.as_str();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn case(s: &str, w: &str, t: &str) -> LineCase {
        LineCase {
            spoken: s.into(),
            written: w.into(),
            tag: t.into(),
        }
    }

    #[test]
    fn parses_tagged_and_untagged_lines_skipping_blanks_and_comments() {
        let text = "# header comment\n\
                    set an alarm for seven a m\tset an alarm for 7 a.m.\ttime\r\n\
                    \n\
                    i have one dog\ti have one dog\n\
                    it was ok\tit was ok\tkeep!\n";
        let (cases, bad) = parse_lines(text);
        assert_eq!(bad, 0);
        assert_eq!(
            cases,
            vec![
                case(
                    "set an alarm for seven a m",
                    "set an alarm for 7 a.m.",
                    "time"
                ),
                case("i have one dog", "i have one dog", UNTAGGED),
                case("it was ok", "it was ok", "keep!"),
            ]
        );
        assert!(cases[2].debatable());
        assert!(!cases[0].debatable());
    }

    #[test]
    fn counts_lines_without_a_tab_or_with_empty_sides_as_malformed() {
        let (cases, bad) = parse_lines("no tab here\n\tonly written\nonly spoken\t\nok\tok\n");
        assert_eq!(cases, vec![case("ok", "ok", UNTAGGED)]);
        assert_eq!(bad, 3);
    }

    #[test]
    fn an_empty_tag_column_is_untagged() {
        let (cases, _) = parse_lines("a\tb\t \n");
        assert_eq!(cases, vec![case("a", "b", UNTAGGED)]);
    }

    #[test]
    fn tally_scores_exact_digits_and_overformat() {
        let mut t = LineTally::default();
        t.record(
            &case("call me at five", "call me at 5", "time"),
            "call me at 5",
        );
        t.record(
            &case("i paid one thousand", "i paid 1,000", "money"),
            "i paid 1000",
        );
        t.record(&case("one of them", "one of them", "keep"), "1 of them");
        t.record(
            &case("the first time", "the first time", "keep"),
            "the first time",
        );
        t.record(&case("two dogs", "2 dogs", "count!"), "two dogs");
        let a = t.all;
        assert_eq!((a.total, a.exact, a.digits), (5, 2, 3));
        assert_eq!((a.overformat, a.no_digit_gold), (1, 2));
        assert_eq!((t.firm.total, t.firm.exact), (4, 2));
        assert_eq!(t.failures.len(), 3);
        assert_eq!(t.failures[1].input, "one of them");
    }

    #[test]
    fn tally_keeps_scores_per_tag_in_first_seen_order() {
        let mut t = LineTally::default();
        t.record(&case("one of them", "one of them", "keep"), "1 of them");
        t.record(&case("five pm", "5 p.m.", "time"), "5 p.m.");
        t.record(&case("the one", "the one", "keep"), "the one");
        let names: Vec<&str> = t.tags.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["keep", "time"]);
        let keep = t.tags[0].1;
        assert_eq!((keep.total, keep.exact, keep.overformat), (2, 1, 1));
    }

    #[test]
    fn a_tag_filter_matches_the_base_tag_and_its_debatable_form() {
        assert!(tag_matches("time", "time"));
        assert!(tag_matches("time!", "time"));
        assert!(tag_matches("time!", "time!"));
        assert!(!tag_matches("time", "time!"));
        assert!(!tag_matches("timer", "time"));
        assert!(!tag_matches("trap_time", "time"));
    }

    #[test]
    fn all_failures_lifts_the_cap() {
        let mut t = LineTally {
            all_failures: true,
            ..LineTally::default()
        };
        for i in 0..15 {
            t.record(&case(&i.to_string(), "want", "a"), "got");
        }
        assert_eq!(t.failures.len(), 15);
    }

    #[test]
    fn trims_output_and_keeps_at_most_ten_failures() {
        let mut t = LineTally::default();
        t.record(&case("x", "5", "a"), " 5 ");
        assert_eq!(t.all.exact, 1);
        for i in 0..15 {
            t.record(&case(&i.to_string(), "want", "a"), "got");
        }
        assert_eq!(t.failures.len(), MAX_LINE_FAILURES);
        assert_eq!(t.failures[9].input, "9");
    }
}
