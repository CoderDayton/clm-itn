use alloc::vec::Vec;

use super::super::{Word, bare, joined, open, role, tag};
use super::Rules;
use crate::tables::Tables;
use crate::types::{ROLE_DIGIT, ROLE_HUNDRED, ROLE_OH, ROLE_POINT, ROLE_TEEN, ROLE_TENS};

pub struct Spanish;

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

    fn rewrite<'a>(t: &'a Tables, line: &'a str, words: Vec<Word<'a>>) -> Vec<Word<'a>> {
        let mut out = Vec::with_capacity(words.len() + 4);
        let mut i = 0;
        while i < words.len() {
            let w = words[i];
            if let Some(n) = words.get(i + 1)
                && open(&w, n)
                && let Some(code) = currency_code(&w, n)
            {
                out.push(Word {
                    said: joined(line, w.core, n.core).unwrap_or(w.core),
                    gap: w.gap,
                    ..tag(t, w.lead, code, n.trail)
                });
                i += 2;
                continue;
            }
            out.push(w);
            i += 1;
            let after_number = out.len() > 1 && out[out.len() - 2].value();
            if after_number && role(t, &w) == ROLE_POINT && w.trail.is_empty() {
                i = fraction_digits(t, line, &words, i, &mut out);
            }
        }
        out
    }
}

const ADJECTIVES: &[(&[&str], &[&str], &str)] = &[
    (
        &["dólar", "dólares", "dolar", "dolares"],
        &[
            "estadounidense",
            "estadounidenses",
            "americano",
            "americanos",
        ],
        "usd",
    ),
    (
        &["dólar", "dólares", "dolar", "dolares"],
        &["canadiense", "canadienses"],
        "cad",
    ),
    (&["peso", "pesos"], &["mexicano", "mexicanos"], "mxn"),
];

fn currency_code(w: &Word, n: &Word) -> Option<&'static str> {
    ADJECTIVES
        .iter()
        .find(|(units, adjectives, _)| is(w, units) && is(n, adjectives))
        .map(|&(_, _, code)| code)
}

const DIGITS: [&str; 10] = [
    "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve",
];

fn group(t: &Tables, words: &[Word], from: usize) -> Option<(u32, usize)> {
    let value = |k: usize| t.value_of.get(words[k].info.canonical as usize).copied();
    let joins = |k: usize| k < words.len() && open(&words[k - 1], &words[k]);
    let at = |k: usize| joins(k).then(|| role(t, &words[k]));
    let first = words.get(from)?;
    let mut k = from;
    let mut v = 0;
    if role(t, first) == ROLE_OH {
        return Some((0, from + 1));
    }
    if role(t, first) == ROLE_HUNDRED {
        v = value(k)?;
        k += 1;
        if !matches!(at(k), Some(ROLE_TENS | ROLE_TEEN | ROLE_DIGIT)) {
            return Some((v, k));
        }
    }
    match role(t, &words[k]) {
        ROLE_TENS => {
            v += value(k)?;
            k += 1;
            if joins(k + 1) && is(&words[k], &["y"]) && at(k + 1) == Some(ROLE_DIGIT) {
                v += value(k + 1)?;
                k += 2;
            }
        }
        ROLE_TEEN | ROLE_DIGIT => {
            v += value(k)?;
            k += 1;
        }
        _ if k > from => {}
        _ => return None,
    }
    Some((v, k))
}

fn fraction_digits<'a>(
    t: &'a Tables,
    line: &'a str,
    words: &[Word<'a>],
    from: usize,
    out: &mut Vec<Word<'a>>,
) -> usize {
    let mut groups = Vec::new();
    let mut k = from;
    while k < words.len()
        && open(&words[k - 1], &words[k])
        && let Some((v, end)) = group(t, words, k)
    {
        groups.push((v, k, end));
        k = end;
    }
    if groups.len() < 2 {
        return from;
    }
    for (v, a, b) in groups {
        let (first, last) = (&words[a], &words[b - 1]);
        let digits = alloc::format!("{v}");
        let n = digits.len();
        for (d, c) in digits.bytes().enumerate() {
            let core = DIGITS[usize::from(c - b'0')];
            out.push(Word {
                gap: if d == 0 { first.gap } else { "" },
                said: match d {
                    0 => joined(line, first.core, last.core).unwrap_or(first.core),
                    _ => "",
                },
                ..tag(
                    t,
                    if d == 0 { first.lead } else { "" },
                    core,
                    if d + 1 == n { last.trail } else { "" },
                )
            });
        }
    }
    k
}

fn is(w: &Word, any: &[&str]) -> bool {
    let b = bare(w.core);
    any.iter().any(|a| b.eq_ignore_ascii_case(a))
}

fn hour(t: &Tables, w: &Word) -> bool {
    w.value()
        && t.value_of
            .get(w.info.canonical as usize)
            .is_some_and(|&v| v <= 24)
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
