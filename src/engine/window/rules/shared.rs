use alloc::vec::Vec;

use super::super::{W_CUT, Word, bare, joined, open, role, tag};
use crate::tables::Tables;
use crate::types::{
    ROLE_AND, ROLE_DIGIT, ROLE_HUNDRED, ROLE_MSCALE, ROLE_OH, ROLE_POINT, ROLE_SCALE, ROLE_TEEN,
    ROLE_TENS,
};

pub(super) struct Spoken {
    pub and: &'static str,
    pub and_after_hundred: bool,
    pub clock: &'static [&'static str],
    pub digits: [&'static str; 10],
    pub adjectives: &'static [(
        &'static [&'static str],
        &'static [&'static str],
        &'static str,
    )],
}

pub(super) fn is(w: &Word, any: &[&str]) -> bool {
    let b = bare(w.core);
    any.iter().any(|a| b.eq_ignore_ascii_case(a))
}

pub(super) fn hour(t: &Tables, w: &Word) -> bool {
    w.value()
        && t.value_of
            .get(w.info.canonical as usize)
            .is_some_and(|&v| v <= 24)
}

pub(super) fn rewrite<'a>(
    t: &'a Tables,
    line: &'a str,
    mut words: Vec<Word<'a>>,
    s: &Spoken,
    join: impl Fn(&[Word<'a>], usize) -> Option<Word<'a>>,
) -> Vec<Word<'a>> {
    cut_lists(t, &mut words, s);
    let mut out = Vec::with_capacity(words.len() + 4);
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if let Some(j) = join(&words, i) {
            out.push(j);
            i += 2;
            continue;
        }
        if let Some(n) = words.get(i + 1)
            && open(&w, n)
            && let Some(&(_, _, code)) = s
                .adjectives
                .iter()
                .find(|(units, adjectives, _)| is(&w, units) && is(n, adjectives))
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
            i = fraction_digits(t, line, &words, i, &mut out, s);
        }
    }
    out
}

fn cut_lists(t: &Tables, words: &mut [Word], s: &Spoken) {
    for k in 1..words.len().saturating_sub(1) {
        let (p, c, n) = (&words[k - 1], &words[k], &words[k + 1]);
        if !(is(c, &[s.and]) && p.value() && n.value() && open(p, c) && open(c, n)) {
            continue;
        }
        let clock = hour(t, p) && k >= 2 && is(&words[k - 2], s.clock);
        if clock {
            continue;
        }
        let joins = match role(t, p) {
            ROLE_TENS => role(t, n) == ROLE_DIGIT,
            ROLE_HUNDRED => {
                s.and_after_hundred && matches!(role(t, n), ROLE_TENS | ROLE_TEEN | ROLE_DIGIT)
            }
            ROLE_SCALE | ROLE_MSCALE => words[k + 1..]
                .iter()
                .take_while(|w| w.value() || role(t, w) == ROLE_AND)
                .all(|w| w.exp < p.exp),
            _ => false,
        };
        if !joins {
            words[k - 1].flags |= W_CUT;
            words[k].flags |= W_CUT;
        }
    }
}

fn group(t: &Tables, words: &[Word], from: usize, s: &Spoken) -> Option<(u32, usize)> {
    let value = |k: usize| t.value_of.get(words[k].info.canonical as usize).copied();
    let joins = |k: usize| k < words.len() && open(&words[k - 1], &words[k]);
    let at = |k: usize| joins(k).then(|| role(t, &words[k]));
    let and = |k: usize| joins(k) && is(&words[k], &[s.and]);
    let first = words.get(from)?;
    let mut k = from;
    let mut v = 0;
    if role(t, first) == ROLE_OH {
        return Some((0, from + 1));
    }
    if role(t, first) == ROLE_HUNDRED {
        v = value(k)?;
        k += 1;
        let next = match s.and_after_hundred {
            true if and(k) => k + 1,
            true => return Some((v, k)),
            false => k,
        };
        if !matches!(at(next), Some(ROLE_TENS | ROLE_TEEN | ROLE_DIGIT)) {
            return Some((v, k));
        }
        k = next;
    }
    match role(t, &words[k]) {
        ROLE_TENS => {
            v += value(k)?;
            k += 1;
            if and(k) && at(k + 1) == Some(ROLE_DIGIT) {
                v += value(k + 1)?;
                k += 2;
            } else if at(k) == Some(ROLE_DIGIT) {
                v += value(k)?;
                k += 1;
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
    s: &Spoken,
) -> usize {
    let mut groups = Vec::new();
    let mut k = from;
    while k < words.len()
        && open(&words[k - 1], &words[k])
        && let Some((v, end)) = group(t, words, k, s)
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
            out.push(Word {
                gap: if d == 0 { first.gap } else { "" },
                said: match d {
                    0 => joined(line, first.core, last.core).unwrap_or(first.core),
                    _ => "",
                },
                ..tag(
                    t,
                    if d == 0 { first.lead } else { "" },
                    s.digits[usize::from(c - b'0')],
                    if d + 1 == n { last.trail } else { "" },
                )
            });
        }
    }
    k
}
