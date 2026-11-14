use alloc::vec::Vec;

use super::super::{Word, joined, open};
use super::Rules;
use super::shared::{Spoken, hour, is};
use crate::tables::Tables;
use crate::types::kind;

pub struct Portuguese;

const DOLLARS: &[&str] = &["dólar", "dólares", "dolar", "dolares"];

const SPOKEN: Spoken = Spoken {
    and: "e",
    and_after_hundred: true,
    clock: ARTICLES,
    digits: [
        "zero", "um", "dois", "três", "quatro", "cinco", "seis", "sete", "oito", "nove",
    ],
    of: &["de"],
    adjectives: &[
        (
            DOLLARS,
            &[
                "americano",
                "americanos",
                "estadunidense",
                "estadunidenses",
                "norte-americano",
                "norte-americanos",
            ],
            "usd",
        ),
        (
            DOLLARS,
            &["canadense", "canadenses", "canadiano", "canadianos"],
            "cad",
        ),
        (&["libra", "libras"], &["esterlina", "esterlinas"], "gbp"),
    ],
};

impl Rules for Portuguese {
    const DECIMAL: char = ',';
    const GROUPED: bool = false;
    const WHOLE_HUNDREDS: bool = true;
    const CENTS_AFTER_CURRENCY: bool = true;
    const CARDINAL_DAYS: bool = true;
    const PERCENT_WORD_IS_NUMBER: bool = true;

    fn hold(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
        before_hour(t, words, i)
            .or_else(|| after_hour(t, words, i))
            .or_else(|| after_midday(words, i))
    }

    fn rewrite<'a>(t: &'a Tables, line: &'a str, words: Vec<Word<'a>>) -> Vec<Word<'a>> {
        let scale_and = |words: &[Word<'a>], i: usize| {
            let w = words[i];
            let [e, v, ..] = words.get(i + 1..)? else {
                return None;
            };
            let run = words[i + 2..].iter().take_while(|x| x.numeric()).count();
            let scale_follows = words[i + 2..i + 2 + run]
                .iter()
                .rev()
                .skip(1)
                .any(|x| x.exp > 0);
            let drop = (big_scale(t, &w) || (w.exp >= 6 && scale_follows))
                && open(&w, e)
                && open(e, v)
                && is(e, &["e"])
                && v.value()
                && core::ptr::eq(w.said, w.core);
            drop.then(|| Word {
                said: joined(line, w.core, e.core).unwrap_or(w.core),
                ..w
            })
        };
        super::shared::rewrite(t, line, words, &SPOKEN, scale_and)
    }
}

fn big_scale(t: &Tables, w: &Word) -> bool {
    let own = w.own as usize;
    t.kind_of.get(own) == Some(&kind::SCALE) && t.aux_of.get(own).is_some_and(|&a| a >= 12)
}

fn clock_hour(t: &Tables, w: &Word) -> bool {
    is(w, &["meio", "meia"]) || hour(t, w)
}

const MINUTES: &[&str] = &["cinco", "dez", "quinze", "vinte", "quarto"];
const ARTICLES: &[&str] = &["a", "as", "o", "à", "às"];

fn before_hour(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
    let m = i.checked_sub(1)?;
    if !is(&words[m], MINUTES) {
        return None;
    }
    let next = words.get(i + 1)?;
    let h = if is(&words[i], &["pras", "pro"]) {
        i + 1
    } else if is(&words[i], &["para", "pra"]) && is(next, ARTICLES) {
        i + 2
    } else if is(&words[i], &["para", "pra"]) && is(next, &["uma", "meia", "meio"]) {
        i + 1
    } else {
        return None;
    };
    let ok = h < words.len()
        && clock_hour(t, &words[h])
        && (m..h).all(|k| open(&words[k], &words[k + 1]));
    if !ok {
        return None;
    }
    let lead = |k: usize, w: &[&str]| k.checked_sub(1).filter(|&a| is(&words[a], w));
    let from = if is(&words[m], &["quarto"]) {
        lead(m, &["um"]).unwrap_or(m)
    } else if is(&words[m], &["cinco"])
        && let Some(e) = lead(m, &["e"])
        && let Some(v) = lead(e, &["vinte"])
    {
        v
    } else {
        m
    };
    Some((from, h))
}

fn after_midday(words: &[Word], i: usize) -> Option<(usize, usize)> {
    let from = i.checked_sub(1)?;
    let (c, m) = (i + 1, i + 2);
    let ok = m < words.len()
        && is(&words[from], &["meio", "meia"])
        && is(&words[i], &["dia", "noite"])
        && is(&words[c], &["e"])
        && (from..m).all(|k| open(&words[k], &words[k + 1]))
        && (words[m].value() || is(&words[m], &["meia", "meio"]));
    if !ok {
        return None;
    }
    let mut end = m;
    while end + 2 < words.len()
        && is(&words[end + 1], &["e"])
        && words[end + 2].value()
        && open(&words[end], &words[end + 1])
        && open(&words[end + 1], &words[end + 2])
    {
        end += 2;
    }
    Some((from, end))
}

fn after_hour(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
    let (c, m) = (i + 1, i + 2);
    if m >= words.len() || !clock_hour(t, &words[i]) {
        return None;
    }
    if !open(&words[i], &words[c]) || !open(&words[c], &words[m]) {
        return None;
    }
    let quarter = is(&words[m], &["um"])
        && words
            .get(m + 1)
            .is_some_and(|q| open(&words[m], q) && is(q, &["quarto"]));
    let end = if quarter { m + 1 } else { m };
    let fraction = is(&words[c], &["e", "menos"]) && (quarter || is(&words[m], &["meia", "meio"]));
    let to = is(&words[c], &["menos"])
        && is(&words[m], MINUTES)
        && i.checked_sub(1).is_some_and(|a| is(&words[a], ARTICLES));
    (fraction || to).then_some((i, end))
}
