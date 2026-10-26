#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub input: String,
    pub got: String,
    pub want: String,
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
    fn accuracy_is_percent_and_zero_total_is_zero() {
        assert_eq!(accuracy(1, 4), 25.0);
        assert_eq!(accuracy(0, 0), 0.0);
    }
}
