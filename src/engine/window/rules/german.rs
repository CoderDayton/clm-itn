use alloc::vec::Vec;
use std::sync::OnceLock;

use alloc::string::String;
use morph_fst::{Lexer, Mode, WordOutput};

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
    adjectives: &[],
};

const GRAMMAR: &str = include_str!("german.morph");

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

fn strip<'s>(text: &'s str, prefix: &str) -> Option<&'s str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

fn position(list: &[&str], word: &str) -> Option<usize> {
    list.iter().position(|w| w.eq_ignore_ascii_case(word))
}

fn below_100(text: &str) -> Option<&'static str> {
    UNITS.iter().enumerate().find_map(|(u, unit)| {
        let tens = strip(strip(text, unit)?, "und")?;
        Some(COMPOUNDS[u][position(&TENS, tens)?])
    })
}

fn key(piece: &str) -> &str {
    match piece.len() > MAX_KEY_LEN {
        true => below_100(piece).unwrap_or(piece),
        false => piece,
    }
}

pub(crate) fn lexer() -> Option<&'static Lexer> {
    static LEXER: OnceLock<Option<Lexer>> = OnceLock::new();
    LEXER
        .get_or_init(|| {
            let built = Lexer::builder().spec(GRAMMAR).build();
            built.ok().map(|l| l.mode(Mode::Strict))
        })
        .as_ref()
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
        let busy = |w: &Word| compound(w.core) || is(w, &["und", "hundert"]);
        let words = if words.iter().any(|w| busy(w) || w.core.len() > MAX_KEY_LEN) {
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
        if !split(t, w, &mut out) {
            let alias = key(w.core);
            out.push(match core::ptr::eq(alias, w.core) {
                true => w,
                false => Word {
                    gap: w.gap,
                    said: w.said,
                    ..tag(t, w.lead, alias, w.trail)
                },
            });
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

fn compound(core: &str) -> bool {
    core.len() > 7
        && core
            .as_bytes()
            .windows(7)
            .any(|w| w.eq_ignore_ascii_case(b"hundert") || w.eq_ignore_ascii_case(b"tausend"))
}

fn split<'a>(t: &'a Tables, w: Word<'a>, out: &mut Vec<Word<'a>>) -> bool {
    if !compound(w.core) {
        return false;
    }
    let Some(lexer) = lexer() else {
        return false;
    };
    let lower: String = w.core.to_lowercase();
    let WordOutput::Analyzed { best, all } = lexer.lex(&lower, Mode::Strict) else {
        return false;
    };
    if all.len() != 1 || lower.len() != w.core.len() {
        return false;
    }
    let mut pieces: Vec<(usize, usize)> = Vec::with_capacity(8);
    let mut run: Option<(usize, bool)> = None;
    let mut at = 0;
    for p in &best {
        let end = p.end;
        match p.tag.as_str() {
            "HUN" => match run.take() {
                Some((s, true)) => pieces.push((s, end)),
                Some((s, false)) => pieces.extend([(s, at), (at, end)]),
                None => pieces.push((at, end)),
            },
            "THO" => {
                if let Some((s, _)) = run.take() {
                    pieces.push((s, at));
                }
                pieces.push((at, end));
            }
            "AND" if run.is_none() => pieces.push((at, end)),
            tag => {
                run = Some(run.map_or((at, tag == "D"), |(s, _)| (s, false)));
            }
        }
        at = end;
    }
    if let Some((s, _)) = run {
        pieces.push((s, at));
    }
    if at != w.core.len() || pieces.len() < 2 {
        return false;
    }
    let start = out.len();
    let last = pieces.len() - 1;
    for (k, &(a, b)) in pieces.iter().enumerate() {
        let Some(piece) = w.core.get(a..b) else {
            out.truncate(start);
            return false;
        };
        let word = tag(t, "", key(piece), if k == last { w.trail } else { "" });
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
    fn the_grammar_builds() {
        assert!(lexer().is_some());
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
