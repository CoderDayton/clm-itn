#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub input: String,
    pub got: String,
    pub want: String,
}

pub const DIGITS_LEGEND: &str = "digits: same sequence of digit runs (thousands commas removed); letters, case and punctuation ignored; time classes: same parsed hour, minute and am/pm (24-hour aware)";

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

pub fn value_runs(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut runs = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            cur.push(c);
            continue;
        }
        let inside = matches!(c, ',' | '.')
            && !cur.is_empty()
            && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if !inside && !cur.is_empty() {
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
