pub const MAX_FAILURES: usize = 5;
pub const MAX_PARTIAL_EXAMPLES: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    pub spoken: String,
    pub written: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub input: String,
    pub got: String,
    pub want: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryScore {
    pub name: String,
    pub total: usize,
    pub correct: usize,
    pub digits: usize,
    pub partial: usize,
    pub partial_examples: Vec<Failure>,
    pub failures: Vec<Failure>,
}

#[derive(Debug, Default)]
pub struct Tally {
    pub categories: Vec<CategoryScore>,
}

pub fn parse_cases(text: &str) -> (Vec<Case>, usize) {
    let mut cases = Vec::new();
    let mut bad = 0;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        match line.split_once('~') {
            Some((s, w)) if !s.trim().is_empty() => cases.push(Case {
                spoken: s.trim().to_string(),
                written: w.trim().to_string(),
            }),
            _ => bad += 1,
        }
    }
    (cases, bad)
}

pub fn category_of(file_name: &str) -> Option<&str> {
    file_name
        .strip_prefix("test_cases_")?
        .strip_suffix(".txt")
        .filter(|c| !c.is_empty())
}

impl Tally {
    pub fn record(&mut self, category: &str, input: &str, got: &str, want: &str) {
        let idx = match self.categories.iter().position(|c| c.name == category) {
            Some(i) => i,
            None => {
                self.categories.push(CategoryScore {
                    name: category.to_string(),
                    ..CategoryScore::default()
                });
                self.categories.len() - 1
            }
        };
        let c = &mut self.categories[idx];
        c.total += 1;
        c.digits += usize::from(value_match(category, got, want));
        if got == want {
            c.correct += 1;
        } else if c.failures.len() < MAX_FAILURES {
            c.failures.push(Failure {
                input: input.to_string(),
                got: got.to_string(),
                want: want.to_string(),
            });
        }
    }

    pub fn record_partial(&mut self, category: &str, input: &str, got: &str, want: &str) {
        let Some(c) = self.categories.iter_mut().find(|c| c.name == category) else {
            return;
        };
        c.partial += 1;
        if c.partial_examples.len() < MAX_PARTIAL_EXAMPLES {
            c.partial_examples.push(Failure {
                input: input.to_string(),
                got: got.to_string(),
                want: want.to_string(),
            });
        }
    }

    pub fn overall_partial(&self) -> usize {
        self.categories.iter().map(|c| c.partial).sum()
    }

    pub fn overall_digits(&self) -> usize {
        self.categories.iter().map(|c| c.digits).sum()
    }

    pub fn overall(&self) -> (usize, usize) {
        self.categories
            .iter()
            .fold((0, 0), |(c, t), s| (c + s.correct, t + s.total))
    }
}

pub fn digit_runs(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut runs = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            cur.push(c);
            continue;
        }
        let thousands =
            c == ',' && !cur.is_empty() && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if !thousands && !cur.is_empty() {
            runs.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs
}

fn words(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| c.is_whitespace() || c == '-')
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordRole {
    Plain,
    Number,
    Affix,
}

pub fn is_partial(got: &str, want: &str, role: impl Fn(&str) -> WordRole) -> bool {
    if !got.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let digits_ok = digits_match(got, want);
    let is_number = |w: &str| match role(w) {
        WordRole::Number => true,
        WordRole::Affix => !digits_ok,
        WordRole::Plain => false,
    };
    let kept: Vec<String> = words(want).collect();
    let got: Vec<String> = words(got).collect();
    got.iter().enumerate().any(|(i, w)| {
        let after_digits = i > 0 && got[i - 1].chars().any(|c| c.is_ascii_digit());
        let style_word =
            MONTHS.contains(&w.as_str()) || (after_digits && SCALES.contains(&w.as_str()));
        let letter = w.len() == 1 && w.chars().all(|c| c.is_ascii_alphabetic());
        let meridiem = (w == "a" || w == "p") && got.get(i + 1).is_some_and(|n| n == "m");
        let skipped_letter = letter && !meridiem;
        is_number(w) && !style_word && !skipped_letter && !kept.contains(w)
    })
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

const SCALES: [&str; 6] = [
    "hundred",
    "thousand",
    "million",
    "billion",
    "trillion",
    "quadrillion",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meridiem {
    Am,
    Pm,
}

fn bare(t: &str) -> String {
    t.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn meridiem(toks: &[&str]) -> Option<Meridiem> {
    toks.iter().enumerate().find_map(|(i, t)| {
        let t = bare(t);
        let next_m = toks.get(i + 1).is_some_and(|n| bare(n) == "m");
        match t.as_str() {
            "am" => Some(Meridiem::Am),
            "pm" => Some(Meridiem::Pm),
            "a" if next_m => Some(Meridiem::Am),
            "p" if next_m => Some(Meridiem::Pm),
            _ => None,
        }
    })
}

fn clock(core: &str) -> Option<(u8, u8)> {
    let mut parts = core.split([':', '.']);
    let h = parts.next()?;
    let m = parts.next()?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(h) || h.len() > 2 || !digits(m) || m.len() != 2 {
        return None;
    }
    let (h, m): (u8, u8) = (h.parse().ok()?, m.parse().ok()?);
    (h <= 24 && m < 60).then_some((h, m))
}

pub fn time_value(s: &str) -> Option<(u8, u8, Option<Meridiem>)> {
    let toks: Vec<&str> = s.split_whitespace().collect();
    let mer = meridiem(&toks);
    toks.iter().enumerate().find_map(|(i, t)| {
        let core = t.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if let Some((h, m)) = clock(core) {
            return Some((h, m, mer));
        }
        let oclock = toks.get(i + 1).is_some_and(|n| bare(n) == "oclock");
        let hour = core.len() <= 2 && !core.is_empty() && core.bytes().all(|b| b.is_ascii_digit());
        let h: u8 = core.parse().ok().filter(|_| hour)?;
        (h <= 24 && (mer.is_some() || oclock)).then_some((h, 0, mer))
    })
}

fn hour24(h: u8, mer: Option<Meridiem>) -> u8 {
    match mer {
        Some(Meridiem::Pm) if h < 12 => h + 12,
        Some(Meridiem::Am) if h == 12 => 0,
        _ => h,
    }
}

pub fn time_match(got: &str, want: &str) -> bool {
    let (Some((h1, m1, a)), Some((h2, m2, b))) = (time_value(got), time_value(want)) else {
        return false;
    };
    let unambiguous = |h: u8| h == 0 || h >= 13;
    m1 == m2
        && match (a, b) {
            (Some(_), Some(_)) => hour24(h1, a) == hour24(h2, b),
            (Some(_), None) => unambiguous(h2) && hour24(h1, a) == h2,
            (None, Some(_)) => unambiguous(h1) && hour24(h2, b) == h1,
            (None, None) => h1 == h2,
        }
}

pub fn value_match(class: &str, got: &str, want: &str) -> bool {
    if class.to_ascii_lowercase().starts_with("time") {
        time_match(got, want)
    } else {
        digits_match(got, want)
    }
}

pub fn digits_match(got: &str, want: &str) -> bool {
    digit_runs(got) == digit_runs(want)
}

pub fn accuracy(correct: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * correct as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(s: &str, w: &str) -> Case {
        Case {
            spoken: s.into(),
            written: w.into(),
        }
    }

    #[test]
    fn parses_spoken_tilde_written_lines() {
        let (cases, bad) = parse_cases("two p m~02:00 p.m.\nsecond~2nd\n");
        assert_eq!(
            cases,
            vec![case("two p m", "02:00 p.m."), case("second", "2nd")]
        );
        assert_eq!(bad, 0);
    }

    #[test]
    fn handles_crlf_blank_lines_and_missing_trailing_newline() {
        let (cases, bad) = parse_cases("one~1\r\n\r\n\ntwo~2");
        assert_eq!(cases, vec![case("one", "1"), case("two", "2")]);
        assert_eq!(bad, 0);
    }

    #[test]
    fn splits_on_first_tilde_only_and_counts_lines_without_one() {
        let (cases, bad) = parse_cases("a tilde~~\nno separator here\n~empty spoken\n");
        assert_eq!(cases, vec![case("a tilde", "~")]);
        assert_eq!(bad, 2);
    }

    #[test]
    fn category_comes_from_file_name() {
        assert_eq!(category_of("test_cases_cardinal.txt"), Some("cardinal"));
        assert_eq!(
            category_of("test_cases_decimal_cased.txt"),
            Some("decimal_cased")
        );
        assert_eq!(category_of("README.md"), None);
        assert_eq!(category_of("test_cases_.txt"), None);
    }

    #[test]
    fn tally_counts_exact_matches_per_category_in_first_seen_order() {
        let mut t = Tally::default();
        t.record("time", "two p m", "02:00 p.m.", "02:00 p.m.");
        t.record("cardinal", "one", "1", "1");
        t.record("time", "two thirty", "2:30", "02:30");
        t.record("time", "x", "1 ", "1");
        let names: Vec<_> = t.categories.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["time", "cardinal"]);
        assert_eq!((t.categories[0].correct, t.categories[0].total), (1, 3));
        assert_eq!(
            t.categories[0].failures[0],
            Failure {
                input: "two thirty".into(),
                got: "2:30".into(),
                want: "02:30".into()
            }
        );
        assert_eq!(t.overall(), (2, 4));
    }

    #[test]
    fn tally_keeps_at_most_five_failures() {
        let mut t = Tally::default();
        for i in 0..9 {
            t.record("c", &i.to_string(), "got", "want");
        }
        assert_eq!(t.categories[0].total, 9);
        assert_eq!(t.categories[0].failures.len(), MAX_FAILURES);
        assert_eq!(t.categories[0].failures[4].input, "4");
    }

    #[test]
    fn digit_runs_ignore_letters_punctuation_and_thousands_commas() {
        assert_eq!(digit_runs("January 8, 1815"), ["8", "1815"]);
        assert_eq!(digit_runs("jan. 8th 1815"), ["8", "1815"]);
        assert_eq!(digit_runs("$1,250,000.50"), ["1250000", "50"]);
        assert_eq!(digit_runs("10-23-26"), ["10", "23", "26"]);
        assert!(digit_runs("mister smith").is_empty());
        assert_eq!(digit_runs("a, 1"), ["1"]);
    }

    #[test]
    fn digits_match_compares_digit_run_sequences() {
        assert!(digits_match("jan. 8th 1815", "January 8, 1815"));
        assert!(digits_match("1000", "1,000"));
        assert!(digits_match("the 5th of april 1771", "the 5th April 1771"));
        assert!(digits_match("sixties", "sixties"));
        assert!(!digits_match("sixties", "60s"));
        assert!(!digits_match("8 1815", "1815 8"));
        assert!(!digits_match("1 dog", "one dog"));
        assert!(!digits_match("12.7", "127"));
    }

    #[test]
    fn tally_counts_digit_matches() {
        let mut t = Tally::default();
        t.record("date", "x", "january 8 1815", "January 8, 1815");
        t.record("date", "x", "january 8 1816", "January 8, 1815");
        t.record("date", "x", "1815", "1815");
        let c = &t.categories[0];
        assert_eq!((c.total, c.correct, c.digits), (3, 1, 2));
        assert_eq!(t.overall_digits(), 2);
    }

    fn number_or_unit(w: &str) -> WordRole {
        const AFFIX: [&str; 11] = [
            "a", "m", "p", "mi", "per", "hour", "degree", "am", "pm", "g", "t",
        ];
        const NUMBER: [&str; 12] = [
            "nine", "trillion", "quarter", "to", "million", "and", "half", "november", "seven",
            "sixty", "l", "v",
        ];
        if AFFIX.contains(&w) {
            WordRole::Affix
        } else if NUMBER.contains(&w) {
            WordRole::Number
        } else {
            WordRole::Plain
        }
    }

    #[test]
    fn partial_is_digits_plus_a_leftover_input_number_word() {
        let p = |got, want| is_partial(got, want, number_or_unit);
        assert!(p("1140 a m", "11:40 a.m."));
        assert!(p("quarter to 12", "11:45"));
        assert!(p("Nine Trillion 789", "9789000000000"));
        assert!(p("2 and a half", "2 1/2"));
        assert!(p("five to 6 mi", "5-6 mi"));
    }

    #[test]
    fn partial_ignores_affix_words_when_the_digits_are_correct() {
        let p = |got, want| is_partial(got, want, number_or_unit);
        assert!(!p("60 mi per hour", "60 mph"));
        assert!(!p("60 mi, per hour.", "60 mph"));
        assert!(!p("60 degree", "60°"));
        assert!(!p("7 p m", "7 p.m."));
        assert!(p("61 mi per hour", "60 mph"));
    }

    #[test]
    fn partial_excludes_raw_output_clean_output_and_words_the_gold_keeps() {
        let p = |got, want| is_partial(got, want, number_or_unit);
        assert!(!p("nine trillion", "9000000000000"));
        assert!(!p("11:40 a.m.", "11:40 a.m."));
        assert!(!p("5.2 million", "5.2 million"));
        assert!(!p("5.2 Million", "5.2 million"));
        assert!(!p("1 dog", "one dog"));
        assert!(!p("12:00 noon", "12:00 noon"));
    }

    #[test]
    fn partial_ignores_month_names_and_scales_right_after_digits() {
        let p = |got, want| is_partial(got, want, number_or_unit);
        assert!(!p("November 3", "Nov. 3"));
        assert!(!p("10 million", "10,000,000"));
        assert!(!p("$5.2 Million.", "$5,200,000"));
        assert!(p("66 degree-sixty seven degree", "66°-67°"));
        assert!(p("nine million 5", "9000005"));
    }

    #[test]
    fn partial_compares_words_without_inner_punctuation() {
        let p = |got, want| is_partial(got, want, number_or_unit);
        assert!(!p("12:01 am", "12:01 a.m."));
        assert!(!p("5 pm", "5:00 P.M."));
        assert!(p("1140 a m", "11:40 a.m."));
        assert!(p("66 degree-sixty", "66°-60°"));
    }

    #[test]
    fn partial_skips_spelled_letters_except_a_m_and_p_m() {
        let p = |got, want| is_partial(got, want, number_or_unit);
        assert!(!p("l 24", "L24"));
        assert!(!p("1449 v", "1449V"));
        assert!(!p("8:00 g m t", "08:00 gmt"));
        assert!(p("1140 a m", "11:40 a.m."));
        assert!(p("1140 p m", "11:40 p.m."));
    }

    #[test]
    fn tally_counts_partials_and_keeps_three_examples() {
        let mut t = Tally::default();
        for i in 0..5 {
            t.record("time", &i.to_string(), "1 a m", "1 a.m.");
            t.record_partial("time", &i.to_string(), "1 a m", "1 a.m.");
        }
        t.record("cardinal", "x", "1", "1");
        let c = &t.categories[0];
        assert_eq!(c.partial, 5);
        assert_eq!(c.partial_examples.len(), MAX_PARTIAL_EXAMPLES);
        assert_eq!(c.partial_examples[2].input, "2");
        assert_eq!(t.categories[1].partial, 0);
        assert_eq!(t.overall_partial(), 5);
    }

    #[test]
    fn time_value_parses_hour_minute_and_meridiem() {
        use Meridiem::{Am, Pm};
        assert_eq!(time_value("08:06"), Some((8, 6, None)));
        assert_eq!(time_value("8:06"), Some((8, 6, None)));
        assert_eq!(time_value("2 p.m."), Some((2, 0, Some(Pm))));
        assert_eq!(time_value("02:00 p.m."), Some((2, 0, Some(Pm))));
        assert_eq!(time_value("2:00 PM"), Some((2, 0, Some(Pm))));
        assert_eq!(time_value("7 a m e s t"), Some((7, 0, Some(Am))));
        assert_eq!(time_value("10.30 A.M."), Some((10, 30, Some(Am))));
        assert_eq!(time_value("11:59:59 p.m."), Some((11, 59, Some(Pm))));
        assert_eq!(time_value("5 o'clock"), Some((5, 0, None)));
        assert_eq!(time_value("5 oclock"), Some((5, 0, None)));
        assert_eq!(
            time_value("set alarm at 10:50 p.m."),
            Some((10, 50, Some(Pm)))
        );
        assert_eq!(time_value("two thirty"), None);
        assert_eq!(time_value("5"), None);
        assert_eq!(time_value("25:00"), None);
        assert_eq!(time_value("1140 a m"), None);
    }

    #[test]
    fn time_match_normalizes_padding_and_24_hour_clock() {
        assert!(time_match("8:06", "08:06"));
        assert!(time_match("2 p.m.", "02:00 p.m."));
        assert!(time_match("2:00 PM", "14:00"));
        assert!(time_match("14:00 pm", "2 p.m."));
        assert!(time_match("5 o'clock", "5:00"));
        assert!(time_match("8:00 g m t", "08:00 gmt"));
        assert!(time_match("12 am", "00:00"));
        assert!(!time_match("2:00", "02:00 p.m."));
        assert!(!time_match("2:00 a.m.", "02:00 p.m."));
        assert!(!time_match("two thirty", "02:30"));
        assert!(!time_match("x", "y"));
    }

    #[test]
    fn value_match_uses_time_parsing_only_for_time_classes() {
        assert!(value_match("time", "2 p.m.", "02:00 p.m."));
        assert!(value_match("time_cased", "2 P.M.", "02:00 P.M."));
        assert!(value_match("TIME", "8:06", "08:06"));
        assert!(!value_match("cardinal", "2 p.m.", "02:00 p.m."));
        assert!(value_match("cardinal", "1,000", "1000"));
    }

    #[test]
    fn tally_uses_time_value_for_time_digits() {
        let mut t = Tally::default();
        t.record("time", "two p m", "2:00 p.m.", "02:00 p.m.");
        assert_eq!(t.categories[0].digits, 1);
    }

    #[test]
    fn accuracy_is_percent_and_zero_total_is_zero() {
        assert_eq!(accuracy(1, 4), 25.0);
        assert_eq!(accuracy(0, 0), 0.0);
    }
}
