use alloc::vec::Vec;

use super::super::{Word, bare, joined, open, tag};
use super::Rules;
use super::shared::{Spoken, hour, is};
use crate::tables::Tables;

pub struct Spanish;

const DOLLARS: &[&str] = &["dólar", "dólares", "dolar", "dolares"];

const SPOKEN: Spoken = Spoken {
    and: "y",
    and_after_hundred: false,
    clock: &["la", "las"],
    digits: [
        "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve",
    ],
    of: &["de"],
    adjectives: &[
        (
            DOLLARS,
            &[
                "estadounidense",
                "estadounidenses",
                "americano",
                "americanos",
            ],
            "usd",
        ),
        (DOLLARS, &["canadiense", "canadienses"], "cad"),
        (&["peso", "pesos"], &["mexicano", "mexicanos"], "mxn"),
    ],
};

impl Rules for Spanish {
    const DECIMAL: char = ',';
    const GROUPED: bool = false;
    const WHOLE_HUNDREDS: bool = true;
    const CENTS_AFTER_CURRENCY: bool = true;
    const CARDINAL_DAYS: bool = true;
    const PERCENT_WORD_IS_NUMBER: bool = true;

    fn hold(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
        before_hour(t, words, i).or_else(|| after_hour(t, words, i))
    }

    fn meridiem(first: &str, second: &str) -> Option<(&'static str, usize)> {
        let m = bare(second);
        if !m.eq_ignore_ascii_case("eme") {
            return None;
        }
        let written = match first {
            f if f.eq_ignore_ascii_case("a") => "am",
            f if f.eq_ignore_ascii_case("pe") => "pm",
            _ => return None,
        };
        Some((written, m.len()))
    }

    fn rewrite<'a>(t: &'a Tables, line: &'a str, mut words: Vec<Word<'a>>) -> Vec<Word<'a>> {
        thousand_millions(t, line, &mut words);
        super::shared::rewrite(t, line, words, &SPOKEN, |_, _| None)
    }
}

fn thousand_millions<'a>(t: &'a Tables, line: &'a str, words: &mut Vec<Word<'a>>) {
    let mut i = 1;
    while i < words.len() {
        let (w, m) = (words[i - 1], words[i]);
        let span = (is(&w, &["mil"]) && is(&m, &["millones"]) && open(&w, &m))
            .then(|| joined(line, w.core, m.core))
            .flatten();
        let Some(span) = span else {
            i += 1;
            continue;
        };
        let counted = i >= 2 && words[i - 2].numeric();
        let more = words.get(i + 1).is_some_and(Word::numeric);
        if !counted && more {
            words[i - 1] = Word {
                gap: w.gap,
                said: w.said,
                ..tag(t, w.lead, "un", "")
            };
            words[i] = Word {
                said: m.said,
                ..tag(t, "", "millardos", m.trail)
            };
            i += 1;
            continue;
        }
        words[i - 1] = Word {
            gap: w.gap,
            core: span,
            said: span,
            ..tag(t, w.lead, "millardos", m.trail)
        };
        words.remove(i);
    }
}

const MINUTES: &[&str] = &["cinco", "diez", "veinte", "veinticinco", "cuarto", "media"];

fn before_hour(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
    let (m, h) = (i.checked_sub(1)?, i + 2);
    let ok = h < words.len()
        && is(&words[i], &["para"])
        && is(&words[i + 1], &["la", "las"])
        && hour(t, &words[h])
        && is(&words[m], MINUTES)
        && (m..h).all(|k| open(&words[k], &words[k + 1]));
    if !ok {
        return None;
    }
    let from = match m.checked_sub(1) {
        Some(a) if is(&words[m], &["cuarto"]) && is(&words[a], &["un"]) => a,
        _ => m,
    };
    Some((from, h))
}

fn after_hour(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
    let (c, m) = (i + 1, i + 2);
    if m >= words.len() || !hour(t, &words[i]) {
        return None;
    }
    if !open(&words[i], &words[c]) || !open(&words[c], &words[m]) {
        return None;
    }
    let fraction = is(&words[m], &["media", "cuarto"]) && is(&words[c], &["y", "menos"]);
    let to = is(&words[c], &["menos"])
        && is(&words[m], MINUTES)
        && i.checked_sub(1)
            .is_some_and(|a| is(&words[a], &["la", "las"]));
    (fraction || to).then_some((i, m))
}
