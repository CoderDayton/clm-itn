use alloc::vec::Vec;

use super::super::{Word, joined, open, tag};
use super::Rules;
use super::shared::{Spoken, hour, is};
use crate::tables::Tables;
use crate::types::MAX_KEY_LEN;

pub struct German;

const SPOKEN: Spoken = Spoken {
    and: "und",
    and_after_hundred: true,
    clock: &["um"],
    digits: [
        "null", "eins", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun",
    ],
    of: &[],
    adjectives: &[],
};

const UNITS: [&str; 9] = [
    "ein", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun",
];
const TENS: [&str; 8] = [
    "zwanzig", "dreißig", "vierzig", "fünfzig", "sechzig", "siebzig", "achtzig", "neunzig",
];

const fn fit(long: &'static str, short: &'static str) -> &'static str {
    if long.len() > MAX_KEY_LEN {
        short
    } else {
        long
    }
}

macro_rules! row {
    ($u:literal) => {
        [
            fit(concat!($u, "undzwanzig"), concat!($u, "zwanzig")),
            fit(concat!($u, "unddreißig"), concat!($u, "dreißig")),
            fit(concat!($u, "undvierzig"), concat!($u, "vierzig")),
            fit(concat!($u, "undfünfzig"), concat!($u, "fünfzig")),
            fit(concat!($u, "undsechzig"), concat!($u, "sechzig")),
            fit(concat!($u, "undsiebzig"), concat!($u, "siebzig")),
            fit(concat!($u, "undachtzig"), concat!($u, "achtzig")),
            fit(concat!($u, "undneunzig"), concat!($u, "neunzig")),
        ]
    };
}

const COMPOUNDS: [[&str; 8]; 9] = [
    row!("ein"),
    row!("zwei"),
    row!("drei"),
    row!("vier"),
    row!("fünf"),
    row!("sechs"),
    row!("sieben"),
    row!("acht"),
    row!("neun"),
];

const HUNDREDS: [&str; 9] = [
    "einhundert",
    "zweihundert",
    "dreihundert",
    "vierhundert",
    "fünfhundert",
    "sechshundert",
    "siebenhundert",
    "achthundert",
    "neunhundert",
];

fn position(list: &[&str], word: &str) -> Option<usize> {
    list.iter().position(|w| w.eq_ignore_ascii_case(word))
}

const DIGIT_KEYS: [&str; 10] = [
    "null", "eins", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun",
];
const TEEN_KEYS: [&str; 10] = [
    "zehn",
    "elf",
    "zwölf",
    "dreizehn",
    "vierzehn",
    "fünfzehn",
    "sechzehn",
    "siebzehn",
    "achtzehn",
    "neunzehn",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Piece {
    Digit(u8),
    One,
    Teen(u8),
    Tens(u8),
    Hundred,
    Thousand,
    And,
}

const PIECES: &[(&str, Piece)] = &[
    ("eins", Piece::One),
    ("ein", Piece::Digit(1)),
    ("zwei", Piece::Digit(2)),
    ("zwo", Piece::Digit(2)),
    ("drei", Piece::Digit(3)),
    ("vier", Piece::Digit(4)),
    ("fünf", Piece::Digit(5)),
    ("sechs", Piece::Digit(6)),
    ("sieben", Piece::Digit(7)),
    ("acht", Piece::Digit(8)),
    ("neun", Piece::Digit(9)),
    ("zehn", Piece::Teen(10)),
    ("elf", Piece::Teen(11)),
    ("zwölf", Piece::Teen(12)),
    ("dreizehn", Piece::Teen(13)),
    ("vierzehn", Piece::Teen(14)),
    ("fünfzehn", Piece::Teen(15)),
    ("sechzehn", Piece::Teen(16)),
    ("siebzehn", Piece::Teen(17)),
    ("achtzehn", Piece::Teen(18)),
    ("neunzehn", Piece::Teen(19)),
    ("zwanzig", Piece::Tens(2)),
    ("dreißig", Piece::Tens(3)),
    ("dreissig", Piece::Tens(3)),
    ("vierzig", Piece::Tens(4)),
    ("fünfzig", Piece::Tens(5)),
    ("sechzig", Piece::Tens(6)),
    ("siebzig", Piece::Tens(7)),
    ("achtzig", Piece::Tens(8)),
    ("neunzig", Piece::Tens(9)),
    ("hundert", Piece::Hundred),
    ("tausend", Piece::Thousand),
    ("und", Piece::And),
];

const MAX_PIECES: usize = 16;
const MAX_KEYS: usize = 8;

struct Keys {
    keys: [&'static str; MAX_KEYS],
    len: usize,
}

impl Keys {
    fn push(&mut self, key: &'static str) -> Option<()> {
        *self.keys.get_mut(self.len)? = key;
        self.len += 1;
        Some(())
    }

    fn as_slice(&self) -> &[&'static str] {
        &self.keys[..self.len]
    }
}

fn lex(text: &str) -> Option<([Piece; MAX_PIECES], usize)> {
    let bytes = text.as_bytes();
    let mut out = [Piece::And; MAX_PIECES];
    let (mut n, mut at) = (0, 0);
    while at < bytes.len() {
        let rest = &bytes[at..];
        let (word, piece) = PIECES
            .iter()
            .filter(|(w, _)| {
                rest.get(..w.len())
                    .is_some_and(|h| h.eq_ignore_ascii_case(w.as_bytes()))
            })
            .max_by_key(|(w, _)| w.len())?;
        *out.get_mut(n)? = *piece;
        n += 1;
        at += word.len();
    }
    Some((out, n))
}

enum Head {
    None,
    Hundreds(u8),
    Year(u8),
}

struct Group {
    head: Head,
    below: u8,
    ein: bool,
}

fn group(p: &[Piece], i: &mut usize) -> Option<Group> {
    let at = |k: usize| p.get(k).copied();
    let mut j = *i;
    let head = match (at(j), at(j + 1)) {
        (Some(Piece::Digit(d)), Some(Piece::Hundred)) => {
            j += 2;
            Head::Hundreds(d)
        }
        (Some(Piece::Teen(t)), Some(Piece::Hundred)) if t > 10 => {
            j += 2;
            Head::Year(t)
        }
        (Some(Piece::Hundred), _) => {
            j += 1;
            Head::Hundreds(0)
        }
        _ => Head::None,
    };
    let and = !matches!(head, Head::None) && at(j) == Some(Piece::And);
    if and {
        j += 1;
    }
    *i = j;
    let (below, ein, used) = match (at(j), at(j + 1), at(j + 2)) {
        (Some(Piece::Digit(d)), Some(Piece::And), Some(Piece::Tens(t))) => (t * 10 + d, false, 3),
        (Some(Piece::One), ..) => (1, false, 1),
        (Some(Piece::Digit(d)), ..) => (d, d == 1, 1),
        (Some(Piece::Teen(t)), ..) => (t, false, 1),
        (Some(Piece::Tens(t)), ..) => (t * 10, false, 1),
        _ => (0, false, 0),
    };
    *i += used;
    if (matches!(head, Head::None) || and) && below == 0 {
        return None;
    }
    Some(Group { head, below, ein })
}

fn below_key(v: u8, last: bool) -> &'static str {
    match v {
        1 if !last => "ein",
        0..=9 => DIGIT_KEYS[v as usize],
        10..=19 => TEEN_KEYS[v as usize - 10],
        _ if v.is_multiple_of(10) => TENS[v as usize / 10 - 2],
        _ => COMPOUNDS[v as usize % 10 - 1][v as usize / 10 - 2],
    }
}

fn emit(g: &Group, last: bool, keys: &mut Keys) -> Option<()> {
    match g.head {
        Head::None => {}
        Head::Hundreds(0) => keys.push("hundert")?,
        Head::Hundreds(d) => keys.push(HUNDREDS[d as usize - 1])?,
        Head::Year(t) => {
            keys.push(TEEN_KEYS[t as usize - 10])?;
            keys.push("hundert")?;
        }
    }
    if g.below > 0 {
        keys.push(below_key(g.below, last))?;
    }
    Some(())
}

fn compound(text: &str) -> Option<Keys> {
    let (pieces, n) = lex(text)?;
    let p = &pieces[..n];
    let mut i = 0;
    let first = match p.first() {
        Some(Piece::Thousand) => None,
        _ => Some(group(p, &mut i)?),
    };
    let thousand = p.get(i) == Some(&Piece::Thousand);
    let mut second = None;
    if thousand {
        i += 1;
        let and = p.get(i) == Some(&Piece::And);
        i += usize::from(and);
        if i < n {
            let g = group(p, &mut i)?;
            if and && !matches!(g.head, Head::None) {
                return None;
            }
            second = Some(g);
        } else if and {
            return None;
        }
    }
    if i != n {
        return None;
    }
    let year = |g: &Option<Group>| {
        matches!(
            g,
            Some(Group {
                head: Head::Year(_),
                ..
            })
        )
    };
    let one_last = |g: &Option<Group>| g.as_ref().is_some_and(|g| g.ein);
    let eins_before = first.as_ref().is_some_and(|g| g.below == 1 && !g.ein) && thousand;
    if (thousand && (year(&first) || year(&second)))
        || eins_before
        || (!thousand && one_last(&first))
        || one_last(&second)
    {
        return None;
    }
    let mut keys = Keys {
        keys: [""; MAX_KEYS],
        len: 0,
    };
    if let Some(g) = &first {
        emit(g, !thousand, &mut keys)?;
    }
    if thousand {
        keys.push("tausend")?;
    }
    if let Some(g) = &second {
        emit(g, true, &mut keys)?;
    }
    Some(keys)
}

impl Rules for German {
    const DECIMAL: char = ',';
    const GROUPED: bool = false;
    const WHOLE_HUNDREDS: bool = true;
    const CENTS_AFTER_CURRENCY: bool = true;
    const CARDINAL_DAYS: bool = false;
    const PERCENT_WORD_IS_NUMBER: bool = true;

    fn hold(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
        relative(t, words, i).or_else(|| fraction_of_hour(t, words, i))
    }

    fn rewrite<'a>(t: &'a Tables, line: &'a str, words: Vec<Word<'a>>) -> Vec<Word<'a>> {
        let busy =
            |w: &Word| is(w, &["und", "hundert"]) || (!w.numeric() && compound(w.core).is_some());
        let words = if words.iter().any(busy) {
            numbers(t, line, &words)
        } else {
            words
        };
        super::shared::rewrite(t, line, words, &SPOKEN, |_, _| None)
    }
}

fn numbers<'a>(t: &'a Tables, line: &'a str, words: &[Word<'a>]) -> Vec<Word<'a>> {
    let mut out = Vec::with_capacity(words.len() + 8);
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if let Some((word, used)) = spaced(t, line, words, i) {
            out.push(word);
            i += used;
            continue;
        }
        if w.numeric() || !split(t, w, &mut out) {
            out.push(w);
        }
        i += 1;
    }
    out
}

fn spaced<'a>(
    t: &'a Tables,
    line: &'a str,
    words: &[Word<'a>],
    i: usize,
) -> Option<(Word<'a>, usize)> {
    let a = &words[i];
    let u = position(&UNITS, a.core)?;
    let b = words.get(i + 1).filter(|b| open(a, b))?;
    let (key, last) = if is(b, &["hundert"]) {
        (HUNDREDS[u], b)
    } else {
        let c = words.get(i + 2).filter(|c| is(b, &["und"]) && open(b, c))?;
        (COMPOUNDS[u][position(&TENS, c.core)?], c)
    };
    let used = if core::ptr::eq(last, b) { 2 } else { 3 };
    let word = Word {
        gap: a.gap,
        said: joined(line, a.core, last.core).unwrap_or(a.core),
        ..tag(t, a.lead, key, last.trail)
    };
    Some((word, used))
}

fn split<'a>(t: &'a Tables, w: Word<'a>, out: &mut Vec<Word<'a>>) -> bool {
    let Some(keys) = compound(w.core) else {
        return false;
    };
    let start = out.len();
    let last = keys.len - 1;
    for (k, &key) in keys.as_slice().iter().enumerate() {
        let word = tag(t, "", key, if k == last { w.trail } else { "" });
        if !word.numeric() {
            out.truncate(start);
            return false;
        }
        out.push(match k {
            0 => Word {
                gap: w.gap,
                lead: w.lead,
                said: w.said,
                ..word
            },
            _ => Word { said: "", ..word },
        });
    }
    true
}

fn minutes(t: &Tables, w: &Word) -> bool {
    let value = || t.value_of.get(w.info.canonical as usize).copied();
    is(w, &["viertel"]) || (w.value() && value().is_some_and(|v| (1..30).contains(&v)))
}

fn relative(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
    if !is(&words[i], &["vor", "nach"]) {
        return None;
    }
    let m = i.checked_sub(1).filter(|&m| minutes(t, &words[m]))?;
    let mut h = i + 1;
    if words.get(h).is_some_and(|w| is(w, &["halb"])) {
        h += 1;
    }
    let ok =
        h < words.len() && hour(t, &words[h]) && (m..h).all(|k| open(&words[k], &words[k + 1]));
    ok.then_some((m, h))
}

fn fraction_of_hour(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
    let h = i + 1;
    let ok = is(&words[i], &["halb", "viertel", "dreiviertel"])
        && words
            .get(h)
            .is_some_and(|w| hour(t, w) && open(&words[i], w));
    ok.then_some((i, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compounds_split_into_lexicon_words() {
        let keys = |s: &str| compound(s).map(|k| k.as_slice().to_vec());
        let cases: [(&str, Option<&[&str]>); 12] = [
            (
                "zweihundertundfünfundvierzig",
                Some(&["zweihundert", "fünfundvierzig"]),
            ),
            ("Hundertsieben", Some(&["hundert", "sieben"])),
            ("hunderteins", Some(&["hundert", "eins"])),
            ("eintausend", Some(&["ein", "tausend"])),
            ("tausendundeins", Some(&["tausend", "eins"])),
            ("zwounddreissig", Some(&["zweiunddreißig"])),
            ("siebenundzwanzig", Some(&["siebenzwanzig"])),
            (
                "neunzehnhundertneunzig",
                Some(&["neunzehn", "hundert", "neunzig"]),
            ),
            ("hundertein", None),
            ("einstausend", None),
            ("einer", None),
            ("achtung", None),
        ];
        for (text, want) in cases {
            assert_eq!(keys(text).as_deref(), want, "{text}");
        }
    }

    #[test]
    fn every_key_is_a_german_number() {
        let t = crate::tables::german::german();
        let value = |k: &str| {
            let info = super::super::super::lookup(t, k);
            t.value_of.get(info.canonical as usize).copied()
        };
        for (u, row) in COMPOUNDS.iter().enumerate() {
            for (n, k) in row.iter().enumerate() {
                assert_eq!(value(k), Some((n as u32 + 2) * 10 + u as u32 + 1), "{k}");
            }
        }
        for (u, k) in HUNDREDS.iter().enumerate() {
            assert_eq!(value(k), Some((u as u32 + 1) * 100), "{k}");
        }
    }
}
