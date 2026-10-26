use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};

use crate::engine::Normalizer;
use crate::engine::span::fold_left;
use crate::tables::{Tables, WORDS_FIRST_WORDS};
use crate::types::{
    CLASS_UNKNOWN, MAX_TOKENS, ROLE_DIGIT, ROLE_HUNDRED, ROLE_MSCALE, ROLE_OH, ROLE_ORDINAL,
    ROLE_SCALE, ROLE_TEEN, ROLE_TENS, Setting, TokenInfo,
};

const LINE_OUT: usize = 512;
const LEAD_PUNCT: &[char] = &['"', '\'', '(', '[', '{', '\u{201C}', '\u{2018}', '\u{00AB}'];
const TRAIL_PUNCT: &[char] = &[
    '.', ',', ';', ':', '!', '?', '"', '\'', ')', ']', '}', '\u{201D}', '\u{2019}', '\u{00BB}',
    '\u{2026}', '\u{2013}', '\u{2014}',
];

const fn ascii_mask(set: &[char]) -> u128 {
    let mut mask = 0;
    let mut i = 0;
    while i < set.len() {
        if set[i].is_ascii() {
            mask |= 1 << set[i] as u32;
        }
        i += 1;
    }
    mask
}

const LEAD_ASCII: u128 = ascii_mask(LEAD_PUNCT);
const TRAIL_ASCII: u128 = ascii_mask(TRAIL_PUNCT);

/// Whether byte `b` is ASCII and not in `mask`; such a byte ends any trim.
#[inline(always)]
fn plain_ascii(b: u8, mask: u128) -> bool {
    b.is_ascii() && mask >> b & 1 == 0
}

#[inline(always)]
fn lead_punct(c: char) -> bool {
    match u8::try_from(c) {
        Ok(b) if b.is_ascii() => !plain_ascii(b, LEAD_ASCII),
        _ => LEAD_PUNCT.contains(&c),
    }
}

#[inline(always)]
fn trail_punct(c: char) -> bool {
    match u8::try_from(c) {
        Ok(b) if b.is_ascii() => !plain_ascii(b, TRAIL_ASCII),
        _ => TRAIL_PUNCT.contains(&c),
    }
}

#[derive(Clone, Copy)]
struct Word<'a> {
    lead: &'a str,
    core: &'a str,
    trail: &'a str,
    /// What the word prints when kept as spoken; empty prints nothing.
    said: &'a str,
    /// Lexicon entry of `core`, handed to the engine so it is looked up once.
    info: TokenInfo,
    /// `W_*` bits.
    flags: u16,
    /// See `kind`.
    kind: u8,
}

const W_NUMERIC: u16 = 1;
const W_VALUE: u16 = 1 << 1;
const W_FILLER: u16 = 1 << 2;
const W_DIGITS: u16 = 1 << 3;
/// "one" to "nine", "zero" or "oh".
const W_DIGIT: u16 = 1 << 4;
const W_ORDINAL: u16 = 1 << 5;
/// A segment ends after this word.
const W_CUT: u16 = 1 << 6;
/// A lone ordinal starting here is written as digits.
const W_DATED: u16 = 1 << 7;
/// Printed as spoken, never formatted.
const W_KEEP: u16 = 1 << 8;

impl Word<'_> {
    #[inline(always)]
    fn has(&self, bit: u16) -> bool {
        self.flags & bit != 0
    }
    #[inline(always)]
    fn numeric(&self) -> bool {
        self.has(W_NUMERIC)
    }
    #[inline(always)]
    fn value(&self) -> bool {
        self.has(W_VALUE)
    }
    #[inline(always)]
    fn filler(&self) -> bool {
        self.has(W_FILLER)
    }
    #[inline(always)]
    fn all_digits(&self) -> bool {
        self.has(W_DIGITS)
    }
    #[inline(always)]
    fn digit(&self) -> bool {
        self.has(W_DIGIT)
    }
    #[inline(always)]
    fn ordinal(&self) -> bool {
        self.has(W_ORDINAL)
    }
    #[inline(always)]
    fn cut(&self) -> bool {
        self.has(W_CUT)
    }
    #[inline(always)]
    fn dated(&self) -> bool {
        self.has(W_DATED)
    }
    #[inline(always)]
    fn keep(&self) -> bool {
        self.has(W_KEEP)
    }
}

impl<'a> Word<'a> {
    /// Respells `core` in other letter case. The lexicon ignores case, so
    /// `info` stays the entry of the new spelling.
    fn recase(&mut self, core: &'a str) {
        debug_assert!(
            self.core.eq_ignore_ascii_case(core),
            "recase changed letters"
        );
        self.core = core;
    }
}

#[inline(always)]
fn bit(on: bool, bit: u16) -> u16 {
    u16::from(on) * bit
}

fn tag_class<'a>(
    t: &Tables,
    info: TokenInfo,
    lead: &'a str,
    core: &'a str,
    trail: &'a str,
) -> Word<'a> {
    let full = t.role_of[info.class as usize];
    let role = full & 0x0F;
    let b = bare(core);
    Word {
        lead,
        core,
        trail,
        said: core,
        info,
        flags: bit(full != 0, W_NUMERIC)
            | bit(
                role != 0 && (t.suffix_roles | t.prefix_roles) >> role & 1 == 0,
                W_VALUE,
            )
            | bit(FILLERS.iter().any(|f| f.eq_ignore_ascii_case(b)), W_FILLER)
            | bit(
                !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()),
                W_DIGITS,
            )
            | bit(role == ROLE_DIGIT || role == ROLE_OH, W_DIGIT)
            | bit(role == ROLE_ORDINAL, W_ORDINAL),
        kind: kind(b),
    }
}

fn tag<'a>(t: &Tables, lead: &'a str, core: &'a str, trail: &'a str) -> Word<'a> {
    tag_class(t, t.lexicon.lookup(core.as_bytes()), lead, core, trail)
}

fn split_word<'a>(token: &'a str, t: &Tables) -> Word<'a> {
    let b = token.as_bytes();
    if b.first().is_some_and(|&x| plain_ascii(x, LEAD_ASCII))
        && b.last().is_some_and(|&x| plain_ascii(x, TRAIL_ASCII))
    {
        return tag(t, "", token, "");
    }
    let rest = token.trim_start_matches(lead_punct);
    let start = token.len() - rest.len();
    let end = start + rest.trim_end_matches(trail_punct).len();
    if (start == 0 && end == token.len()) || start == end {
        return tag(t, "", token, "");
    }
    let info = t.lexicon.lookup(&token.as_bytes()[start..end]);
    if info.class == CLASS_UNKNOWN {
        return tag(t, "", token, "");
    }
    tag_class(t, info, &token[..start], &token[start..end], &token[end..])
}

fn meridiem(letter: &str, next: &str) -> Option<(&'static str, usize)> {
    let m = next.trim_end_matches(trail_punct);
    (letter.eq_ignore_ascii_case("a") && m.eq_ignore_ascii_case("m")).then_some(("am", m.len()))
}

fn join_meridiem(words: &mut Vec<Word<'_>>, t: &Tables) {
    let mut put = 0;
    let mut i = 0;
    while i < words.len() {
        let after_number = put > 0 && words[put - 1].trail.is_empty() && words[put - 1].numeric();
        let w = &words[i];
        let joined = (after_number && w.lead.is_empty() && w.trail.is_empty())
            .then(|| words.get(i + 1))
            .flatten()
            .filter(|n| n.lead.is_empty())
            .and_then(|n| meridiem(w.core, n.core).map(|(j, len)| (j, n.core, n.trail, len)));
        match joined {
            Some((core, next_core, next_trail, len)) => {
                let trail = if next_trail.is_empty() {
                    &next_core[len..]
                } else {
                    next_trail
                };
                words[put] = tag(t, "", core, trail);
                i += 2;
            }
            None => {
                if put != i {
                    words[put] = words[i];
                }
                i += 1;
            }
        }
        put += 1;
    }
    words.truncate(put);
}

const ZONES: [&str; 11] = [
    "EST", "EDT", "CST", "CDT", "MST", "MDT", "PST", "PDT", "GMT", "UTC", "BST",
];

fn zone(letters: [&str; 3]) -> Option<(&'static str, usize)> {
    let last = letters[2].trim_end_matches(trail_punct);
    let parts = [letters[0], letters[1], last];
    if parts.iter().any(|p| p.len() != 1) {
        return None;
    }
    ZONES
        .iter()
        .find(|z| {
            z.bytes()
                .zip(parts)
                .all(|(c, p)| p.as_bytes()[0].eq_ignore_ascii_case(&c))
        })
        .map(|z| (*z, last.len()))
}

fn join_zone(words: &mut Vec<Word<'_>>, t: &Tables) {
    let mut put = 0;
    let mut i = 0;
    while i < words.len() {
        let after_time = put > 0 && {
            let p = &words[put - 1];
            p.trail.is_empty() && (p.core == "am" || p.core == "pm" || p.core == "m" || p.numeric())
        };
        let joined = (after_time && i + 3 <= words.len())
            .then(|| &words[i..i + 3])
            .filter(|w| {
                w.iter().all(|x| x.lead.is_empty())
                    && w[0].trail.is_empty()
                    && w[1].trail.is_empty()
            })
            .and_then(|w| {
                zone([w[0].core, w[1].core, w[2].core])
                    .map(|(z, len)| (z, w[2].core, w[2].trail, len))
            });
        match joined {
            Some((core, last_core, last_trail, len)) => {
                let trail = if last_trail.is_empty() {
                    &last_core[len..]
                } else {
                    last_trail
                };
                words[put] = tag(t, "", core, trail);
                i += 3;
            }
            None => {
                if put != i {
                    words[put] = words[i];
                }
                i += 1;
            }
        }
        put += 1;
    }
    words.truncate(put);
}

fn drop_twelve(words: &mut Vec<Word<'_>>) {
    let noon = |w: &Word| {
        let b = bare(w.core);
        w.lead.is_empty() && (b.eq_ignore_ascii_case("noon") || b.eq_ignore_ascii_case("midnight"))
    };
    let mut put = 0;
    for i in 0..words.len() {
        let w = &words[i];
        let twelve = w.core.eq_ignore_ascii_case("twelve") && w.trail.is_empty();
        if !(twelve && words.get(i + 1).is_some_and(noon)) {
            if put != i {
                words[put] = words[i];
            }
            put += 1;
        }
    }
    words.truncate(put);
}

fn push_trail(out: &mut String, trail: &str) {
    let trail = if out.ends_with('.') {
        trail.strip_prefix('.').unwrap_or(trail)
    } else {
        trail
    };
    out.push_str(trail);
}

const COUNTING_ROLES: u16 = 1 << ROLE_DIGIT
    | 1 << ROLE_TEEN
    | 1 << ROLE_TENS
    | 1 << ROLE_HUNDRED
    | 1 << ROLE_SCALE
    | 1 << ROLE_MSCALE;

const FILLERS: [&str; 6] = ["uh", "um", "er", "ah", "uhm", "hmm"];

fn bare(core: &str) -> &str {
    match core.as_bytes().last() {
        Some(&b) if plain_ascii(b, TRAIL_ASCII) => core,
        _ => core.trim_end_matches(trail_punct),
    }
}

/// Ranges of words that stay as spoken. Most lines have none, so this
/// rarely allocates.
fn guarded(words: &[Word], counting: impl Fn(&str) -> bool) -> Vec<(usize, usize)> {
    let n = words.len();
    let num = |i: usize| words[i].value();
    let filler = |i: usize| words[i].filler();
    let digits = |i: usize| words[i].all_digits();
    let plural = |i: usize| {
        let b = bare(words[i].core);
        let stem = b.strip_suffix("'s").or_else(|| b.strip_suffix('s'));
        !num(i) && stem.is_some_and(|s| !s.is_empty() && counting(s))
    };
    let mut marks = Vec::new();
    let mut mark = |from: usize, to: usize| marks.push((from, to));
    let run_left = |mut i: usize, ok: &dyn Fn(usize) -> bool| {
        while i > 0 && ok(i - 1) {
            i -= 1;
        }
        i
    };
    let run_right = |mut i: usize, ok: &dyn Fn(usize) -> bool| {
        while i + 1 < n && ok(i + 1) {
            i += 1;
        }
        i
    };
    let num_or_filler = |i: usize| num(i) || filler(i);
    for i in 0..n {
        if filler(i) {
            let j = (0..i).rev().find(|&j| !filler(j));
            let k = (i + 1..n).find(|&k| !filler(k));
            // "twenty uh twenty five" repeats its start, so the speaker
            // restarted: the words before the filler stay and the rest
            // formats. "twenty uh five" stays words.
            if let (Some(j), Some(k)) = (j, k)
                && num(j)
                && num(k)
            {
                let from = run_left(j, &num_or_filler);
                let restart = words[from].core.eq_ignore_ascii_case(words[k].core);
                let to = if restart {
                    k - 1
                } else {
                    run_right(k, &num_or_filler)
                };
                mark(from, to);
            }
        } else if i > 0 && num(i - 1) && (plural(i) || digits(i)) {
            mark(run_left(i - 1, &num), i);
        }
        if digits(i) && i + 1 < n && num(i + 1) {
            mark(i, run_right(i + 1, &num));
        }
        if i > 0
            && i + 1 < n
            && num(i + 1)
            && !num(i)
            && connector_start(words[i].core)
            && let Some((from, to)) = relative_time(words, i)
        {
            mark(from, to);
        }
        if num(i)
            && let Some((from, to)) = word_sense(words, i)
        {
            mark(from, to);
        }
        if let Some(end) = mixed_fraction(words, i) {
            let j = if i > 0 && !num(i - 1) && words[i - 1].numeric() {
                i - 1
            } else {
                i
            };
            if j > 0 && num(j - 1) {
                mark(run_left(j - 1, &num), end);
            }
        }
    }
    marks
}

const FRACTION_WORDS: [&str; 14] = [
    "half",
    "quarter",
    "third",
    "thirds",
    "quarters",
    "halves",
    "fourths",
    "fifths",
    "sixths",
    "sevenths",
    "eighths",
    "ninths",
    "tenths",
    "sixteenths",
];

const SMALL: [&str; 9] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];
const ONE_AFTER_WORD: [&str; 5] = ["no", "which", "this", "that", "the"];
const ONE_BEFORE_WORD: [&str; 9] = [
    "of", "more", "another", "at", "way", "thing", "sec", "every", "piece",
];
const GROUP: [&str; 3] = ["us", "them", "you"];
const SIX_OF_ONE: &[&str] = &["six", "of", "one"];
const CLOUD_NINE: &[&str] = &["cloud", "nine"];
const TWO_HEADS: &[&str] = &["two", "heads", "are", "better", "than", "one"];
const ELEVENTH_HOUR: &[&str] = &["eleventh", "hour"];
const ONE_OFF: &[&str] = &["one", "off"];
const TWENTY_TWENTY: &[&str] = &["twenty", "twenty", "vision"];

/// The idioms `me` is part of, with its place in each.
fn idioms_for(me: &str) -> &'static [(&'static [&'static str], usize)] {
    let eq = |s: &str| me.eq_ignore_ascii_case(s);
    match me.len() {
        3 if eq("one") => &[(SIX_OF_ONE, 2), (TWO_HEADS, 5), (ONE_OFF, 0)],
        3 if eq("six") => &[(SIX_OF_ONE, 0)],
        3 if eq("two") => &[(TWO_HEADS, 0)],
        4 if eq("nine") => &[(CLOUD_NINE, 1)],
        6 if eq("twenty") => &[(TWENTY_TWENTY, 0), (TWENTY_TWENTY, 1)],
        8 if eq("eleventh") => &[(ELEVENTH_HOUR, 0)],
        _ => &[],
    }
}

/// A lone "one" to "nine" used as a word, not a count ("no one", "one of
/// them", "the two of us", "two thirds", "cloud nine"). Returns the span that
/// stays as spoken. Context never crosses punctuation.
fn word_sense(words: &[Word], i: usize) -> Option<(usize, usize)> {
    let eq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let any = |a: &str, set: &[&str]| set.iter().any(|s| eq(a, s));
    let me = words[i].core;
    let small = (3..=5).contains(&me.len()) && any(me, &SMALL);
    let idioms = idioms_for(me);
    if !small && idioms.is_empty() {
        return None;
    }
    let at = |d: isize| -> Option<&Word> {
        let k = i.checked_add_signed(d).filter(|&k| k < words.len())?;
        let (lo, hi) = (k.min(i), k.max(i));
        let open = words[lo..hi].iter().all(|w| w.trail.is_empty())
            && words[lo + 1..=hi].iter().all(|w| w.lead.is_empty());
        open.then(|| &words[k])
    };
    let text = |d: isize| at(d).map_or("", |w| bare(w.core));
    let numeric = |d: isize| at(d).is_some_and(|w| w.numeric());
    if numeric(-1) {
        return None;
    }
    for &(idiom, p) in idioms {
        let start = -(p as isize);
        if idiom.iter().zip(start..).all(|(w, d)| eq(text(d), w)) {
            return Some((i - p, i - p + idiom.len() - 1));
        }
    }
    if !small {
        return None;
    }
    let (prev, next) = (text(-1), text(1));
    let one = eq(me, "one");
    if one && eq(next, "by") && eq(text(2), "one") {
        return Some((i, i + 2));
    }
    let pronoun = one
        && ((any(prev, &ONE_AFTER_WORD) && !numeric(1))
            || (any(next, &ONE_BEFORE_WORD) && !numeric(2))
            || (eq(next, "day") && at(-1).is_none())
            || (eq(prev, "in") && eq(next, "piece"))
            || (eq(next, "in") && eq(text(2), "a") && eq(text(3), "million")));
    let group = eq(next, "of") && any(text(2), &GROUP);
    (pronoun || group || any(next, &FRACTION_WORDS)).then_some((i, i))
}

const RELATIVE: [&str; 4] = ["to", "till", "past", "after"];

/// Whether `core` starts like a [`RELATIVE`] word; skips most `relative_time` calls.
fn connector_start(core: &str) -> bool {
    match core.as_bytes() {
        [a, b, ..] => matches!(
            [a | 0x20, b | 0x20],
            [b't', b'o'] | [b't', b'i'] | [b'p', b'a'] | [b'a', b'f']
        ),
        _ => false,
    }
}
const HOURS: [&str; 12] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
    "twelve",
];
const TIME_CUES: [&str; 9] = [
    "at", "is", "it's", "was", "by", "until", "till", "around", "about",
];

/// A relative time such as "ten to four" or "half past six", with the
/// connector at `i`. It reads as a clock time or as a range or score, so the
/// span from the minute word to the hour stays as spoken. It counts only
/// after a time cue ("at", "it's") or as a whole clause, and not when the
/// hour is followed by a number word such as "pm" or a unit.
#[inline(never)]
fn relative_time(words: &[Word], i: usize) -> Option<(usize, usize)> {
    let is = |k: usize, set: &[&str]| {
        let b = bare(words[k].core);
        set.iter().any(|s| s.eq_ignore_ascii_case(b))
    };
    let plain = |k: usize| words[k].lead.is_empty() && words[k].trail.is_empty();
    let h = i + 1;
    if i == 0 || h >= words.len() || !words[h].value() || !plain(i) || !is(i, &RELATIVE) {
        return None;
    }
    if !words[h].lead.is_empty() || !is(h, &HOURS) {
        return None;
    }
    if h + 1 < words.len() && words[h].trail.is_empty() && words[h + 1].numeric() {
        return None;
    }
    let m = i - 1;
    if !words[m].trail.is_empty() {
        return None;
    }
    let from = if m > 0 && is(m - 1, &["twenty"]) && is(m, &["five"]) && plain(m - 1) {
        m - 1
    } else if is(m, &["five", "ten", "twenty", "quarter", "half"]) {
        m
    } else {
        return None;
    };
    if from > 0 && words[from - 1].value() && words[from - 1].trail.is_empty() {
        return None;
    }
    let c = if from > 0 && is(from, &["quarter"]) && is(from - 1, &["a"]) && plain(from - 1) {
        from - 1
    } else {
        from
    };
    let opens = c == 0 || !words[c - 1].trail.is_empty() || !words[c].lead.is_empty();
    let closes = h + 1 == words.len() || !words[h].trail.is_empty();
    let cue = !opens && is(c - 1, &TIME_CUES);
    (cue || (opens && closes)).then_some((from, h))
}

#[inline(always)]
fn mixed_fraction(words: &[Word], i: usize) -> Option<usize> {
    let at = |k: usize| words.get(k).map(|w| bare(w.core));
    if !at(i)?.eq_ignore_ascii_case("and") {
        return None;
    }
    let article = at(i + 1)?;
    let fraction = at(i + 2)?;
    let count = |a: &str| SMALL.iter().any(|s| s.eq_ignore_ascii_case(a));
    ((article.eq_ignore_ascii_case("a") || article.eq_ignore_ascii_case("an") || count(article))
        && FRACTION_WORDS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(fraction)))
    .then_some(i + 2)
}

/// Whether nothing but a space sits between `a` and `b`.
fn open(a: &Word, b: &Word) -> bool {
    a.trail.is_empty() && b.lead.is_empty()
}

fn is(w: &Word, s: &str) -> bool {
    bare(w.core).eq_ignore_ascii_case(s)
}

const SCALE_WORDS: [&str; 4] = ["hundred", "thousand", "million", "billion"];

const F_EXPAND: u8 = 1;
const F_PHONE: u8 = 2;
const F_CUTS: u8 = 4;
const F_DOTS: u8 = 8;
const F_LETTERS: u8 = 16;
const F_VERBS: u8 = 32;

const K_A: u8 = 1;
const K_LETTER: u8 = 2;
const K_OF: u8 = 3;
const K_PER: u8 = 4;
const K_DOT: u8 = 5;
const K_DOUBLE: u8 = 6;
const K_HUNDRED: u8 = 7;
const K_STREET: u8 = 8;
/// "may" or "march", which can be verbs.
const K_MONTH_VERB: u8 = 9;
/// "1" to "12" written as digits.
const K_CLOCK_DIGITS: u8 = 10;
const K_HUNDREDS: u8 = 11;

const HOUR_WORDS: [&str; 12] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
    "twelve",
];
const TEEN_WORDS: [&str; 9] = [
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const CENTURIES: [&str; 9] = [
    "1100s", "1200s", "1300s", "1400s", "1500s", "1600s", "1700s", "1800s", "1900s",
];
/// Words after which "may" and "march" are verbs.
const VERB_SUBJECTS: [&str; 16] = [
    "i", "you", "we", "they", "he", "she", "it", "who", "to", "will", "can", "could", "might",
    "would", "should", "must",
];

/// `core` read as "1" to "12".
fn clock_digits(core: &str) -> Option<usize> {
    let v: usize = core.parse().ok()?;
    (!core.starts_with('0') && (1..=12).contains(&v)).then_some(v)
}

/// The few words the line passes look for, found once per word.
fn kind(core: &str) -> u8 {
    let eq = |s: &str| core.eq_ignore_ascii_case(s);
    let any = |set: &[&str]| set.iter().any(|s| eq(s));
    let c = core.as_bytes();
    let Some(&first) = c.first() else {
        return 0;
    };
    match (c.len(), first.to_ascii_lowercase()) {
        (1, b'a') => K_A,
        (1, l) if l.is_ascii_alphabetic() => K_LETTER,
        (1 | 2, b'0'..=b'9') if clock_digits(core).is_some() => K_CLOCK_DIGITS,
        (3, b'm') if eq("may") => K_MONTH_VERB,
        (5, b'm') if eq("march") => K_MONTH_VERB,
        (8, b'h') if eq("hundreds") => K_HUNDREDS,
        (2, b'o') if eq("of") => K_OF,
        (3, b'p') if eq("per") => K_PER,
        (3, b'd') if eq("dot") => K_DOT,
        (4, b'r' | b'l') if any(&["road", "lane"]) => K_STREET,
        (5, b'd' | b'c' | b'p') if any(&["drive", "court", "place"]) => K_STREET,
        (6, b'd' | b't') if any(&["double", "triple"]) => K_DOUBLE,
        (6, b's' | b'a') if any(&["street", "avenue"]) => K_STREET,
        (7, b'h') if eq("hundred") => K_HUNDRED,
        (7, b'p' | b'h') if any(&["parkway", "highway"]) => K_STREET,
        (9, b'b') if eq("boulevard") => K_STREET,
        _ => 0,
    }
}

/// Which of the rare line passes can apply, so a plain line pays for none of
/// them. Each bit is a superset of its pass.
fn features(words: &[Word]) -> u8 {
    let mut f = 0;
    let number = |w: Option<&Word>| w.is_some_and(|w| w.numeric() || w.all_digits());
    for (k, w) in words.iter().enumerate().filter(|(_, w)| w.kind != 0) {
        let prev = k.checked_sub(1).map(|j| &words[j]);
        let next = words.get(k + 1);
        let digit = |w: Option<&Word>| w.is_some_and(|w| w.digit());
        f |= match w.kind {
            K_A if next.is_some_and(|n| SCALE_WORDS.iter().any(|s| is(n, s)) || unit(n)) => {
                F_EXPAND
            }
            K_CLOCK_DIGITS if next.is_some_and(minutes) => F_EXPAND,
            K_HUNDREDS if prev.is_some_and(|p| p.value()) => F_EXPAND,
            K_MONTH_VERB if prev.is_some_and(|p| VERB_SUBJECTS.iter().any(|s| is(p, s))) => F_VERBS,
            K_LETTER if number(prev) || number(next) => F_LETTERS,
            K_OF if prev.is_some_and(|p| p.ordinal()) => F_CUTS,
            K_PER | K_DOUBLE => F_EXPAND,
            K_DOT => F_DOTS,
            K_HUNDRED if digit(prev) && digit(next) => F_PHONE,
            K_STREET if prev.is_some_and(|p| p.value()) || k >= 2 && words[k - 2].value() => F_CUTS,
            _ => 0,
        };
    }
    f
}

/// A unit or currency word. "a hundred percent" is an idiom, so not percent.
fn unit(w: &Word) -> bool {
    w.numeric() && !w.value() && !is(w, "percent")
}

/// A word that can be the minutes of "5 thirty": not a digit or a scale.
fn minutes(w: &Word) -> bool {
    w.value()
        && (!w.digit() || is(w, "oh"))
        && !is(w, "and")
        && !SCALE_WORDS.iter().any(|s| is(w, s))
}

/// "we march five miles", "it may first be noticed": after a subject or a
/// modal, "may" and "march" are verbs. "may first" and "may one day" are
/// adverbs, not dates.
fn verb_months(words: &mut [Word]) {
    for k in 1..words.len() {
        let w = words[k];
        let verb = w.kind == K_MONTH_VERB
            && open(&words[k - 1], &w)
            && VERB_SUBJECTS.iter().any(|s| is(&words[k - 1], s));
        if !verb {
            continue;
        }
        words[k].flags |= W_KEEP;
        if is(&w, "may")
            && let Some(n) = words.get_mut(k + 1)
            && w.trail.is_empty()
            && n.lead.is_empty()
            && (is(n, "first") || is(n, "one"))
        {
            n.flags |= W_KEEP;
        }
    }
}

/// Rewrites spoken forms the tables do not read: "twelve per cent",
/// "double oh seven", "a hundred and twenty" (but not "a hundred people"),
/// "a dollar fifty", "5 thirty" and "eighteen hundreds".
fn expand<'a>(words: Vec<Word<'a>>, t: &Tables, percent: bool) -> Vec<Word<'a>> {
    let mut out: Vec<Word<'a>> = Vec::with_capacity(words.len() + 2);
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let at = |k: usize| words.get(k).filter(|x| open(&words[k - 1], x)).copied();
        let next = at(i + 1);
        let after = next.and(at(i + 2));
        let small_scale = next.is_some_and(|n| is(&n, "hundred") || is(&n, "thousand"));
        let counts_on = after.is_some_and(|a| {
            (a.value() && !SCALE_WORDS.iter().any(|s| is(&a, s)))
                || (is(&a, "and") && at(i + 3).is_some_and(|b| b.value()))
                || (small_scale && unit(&a))
        });
        let priced = next.is_some_and(|n| unit(&n)) && after.is_some_and(|a| a.value());
        let century = next
            .filter(|n| n.kind == K_HUNDREDS)
            .and(TEEN_WORDS.iter().position(|s| is(&w, s)));
        let after_number = out.last().is_some_and(|p| p.value() && p.trail.is_empty());
        match next {
            Some(n) if percent && is(&w, "per") && is(&n, "cent") && after_number => {
                let joined = tag(t, w.lead, "percent", n.trail);
                out.push(Word {
                    said: "per cent",
                    ..joined
                });
                i += 2;
            }
            Some(n) if n.digit() && (is(&w, "double") || is(&w, "triple")) => {
                let copies = if is(&w, "double") { 1 } else { 2 };
                for k in 0..copies {
                    out.push(Word {
                        lead: if k == 0 { w.lead } else { "" },
                        said: if k == 0 { w.core } else { "" },
                        trail: "",
                        ..n
                    });
                }
                i += 1;
            }
            Some(n) if century.is_some() => {
                let c = CENTURIES[century.unwrap_or_default()];
                out.push(Word {
                    said: c,
                    ..tag(t, w.lead, c, n.trail)
                });
                i += 2;
            }
            Some(n)
                if w.kind == K_CLOCK_DIGITS
                    && let Some(h) = clock_digits(w.core)
                    && w.trail.is_empty()
                    && minutes(&n) =>
            {
                let hour = HOUR_WORDS[h - 1];
                out.push(Word {
                    said: w.core,
                    ..tag(t, w.lead, hour, w.trail)
                });
                i += 1;
            }
            Some(n)
                if is(&w, "a")
                    && ((SCALE_WORDS.iter().any(|s| is(&n, s)) && counts_on) || priced) =>
            {
                let one = tag(t, w.lead, "one", w.trail);
                out.push(Word {
                    said: w.core,
                    ..one
                });
                i += 1;
            }
            _ => {
                out.push(w);
                i += 1;
            }
        }
    }
    out
}

const MIN_PHONE_DIGITS: usize = 7;

/// "one eight hundred two five five ..." in a run of seven or more digit
/// words: "hundred" stands for two zeros.
fn phone_hundreds<'a>(words: Vec<Word<'a>>, t: &Tables) -> Vec<Word<'a>> {
    let n = words.len();
    let hundred = |k: usize| {
        is(&words[k], "hundred")
            && k > 0
            && k + 1 < n
            && words[k - 1].digit()
            && words[k + 1].digit()
            && open(&words[k - 1], &words[k])
            && open(&words[k], &words[k + 1])
    };
    let mut zeros = vec![false; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j < n
            && (words[j].digit() || hundred(j))
            && (j == i || open(&words[j - 1], &words[j]))
        {
            j += 1;
        }
        let digits = (i..j).filter(|&k| words[k].digit()).count();
        if digits >= MIN_PHONE_DIGITS {
            (i..j)
                .filter(|&k| !words[k].digit())
                .for_each(|k| zeros[k] = true);
        }
        i = j.max(i + 1);
    }
    let mut out = Vec::with_capacity(n + 2);
    for (w, z) in words.into_iter().zip(zeros) {
        if z {
            let oh = tag(t, "", "oh", "");
            out.push(Word { said: w.core, ..oh });
            out.push(Word { said: "", ..oh });
        } else {
            out.push(w);
        }
    }
    out
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Where a segment must end, and which lone ordinals are written as digits.
/// A cut keeps a word from being context for the number next to it: "at"
/// before a street number is not a time cue. "the fourth of july" is a date
/// even though "the fourth" alone stays a word, so its month is capitalized.
fn cuts(words: &mut [Word]) {
    let n = words.len();
    let linked = |words: &[Word], k: usize| k + 1 < n && open(&words[k], &words[k + 1]);
    for i in 0..n {
        let run_start = i == 0 || !words[i - 1].value() || !linked(words, i - 1);
        if words[i].value() && run_start {
            let mut j = i;
            while j + 1 < n && words[j + 1].value() && linked(words, j) {
                j += 1;
            }
            let street = (j + 1..(j + 3).min(n))
                .take_while(|&k| linked(words, k - 1))
                .any(|k| words[k].kind == K_STREET);
            if street && i > 0 {
                words[i - 1].flags |= W_CUT;
            }
        }
        let month = (i + 2 < n && words[i].ordinal() && is(&words[i + 1], "of"))
            .then(|| MONTHS.iter().copied().find(|m| is(&words[i + 2], m)))
            .flatten()
            .filter(|_| linked(words, i) && linked(words, i + 1));
        if let Some(m) = month {
            let in_date = words.get(i + 3).is_some_and(|w| w.value()) && linked(words, i + 2);
            words[i + 2].said = m;
            if !in_date {
                words[i + 2].recase(m);
            }
            if run_start {
                words[i].flags |= W_DATED | W_CUT;
                if i > 0 {
                    words[i - 1].flags |= W_CUT;
                }
            }
        }
    }
}

/// "nineteen hundred" closing a clause or before "hours" is a year or a
/// clock time, so it takes no comma; before a noun it is a count.
fn bare_hundreds(segment: &[Word], next: Option<&Word>) -> bool {
    if let [rest @ .., hours] = segment
        && hours.core.len() == 5
        && is(hours, "hours")
        && rest.last().is_some_and(|h| h.trail.is_empty())
    {
        return bare_hundreds(rest, None);
    }
    let Some(last) = segment.last() else {
        return false;
    };
    // "nineteen hundred" or "nineteen hundred and five".
    let n = segment.len();
    let Some(h) = (n.saturating_sub(4)..n)
        .rev()
        .find(|&k| segment[k].kind == K_HUNDRED)
    else {
        return false;
    };
    let tail = &segment[h + 1..];
    let tail_ok = tail.is_empty()
        || (tail.len() >= 2 && is(&tail[0], "and") && tail[1..].iter().all(|w| w.value()));
    let x = h.checked_sub(1).map(|k| &segment[k]);
    let lead = h >= 2 && segment[h - 2].value();
    tail_ok
        && x.is_some_and(|x| x.value() && !x.digit())
        && !lead
        && (!last.trail.is_empty() || next.is_none_or(|n| is(n, "hours")))
}

/// Drops the comma from the last "1,900" in `out[body..]`.
fn ungroup_last(out: &mut String, body: usize) {
    let text = &out[body..];
    let text = text.strip_suffix(" hours").unwrap_or(text);
    let end = body + text.len();
    let last = out[body..end].rfind(' ').map_or(body, |k| body + k + 1);
    let b = &out.as_bytes()[last..end];
    if b.len() == 5 && b[1] == b',' && b.iter().all(|&c| c == b',' || c.is_ascii_digit()) {
        out.remove(last + 1);
    }
}

/// "192 dot 168 dot 1" -> "192.168.1".
fn join_dots(out: &mut String, begin: usize) {
    let text = out.split_off(begin);
    let toks: Vec<&str> = text.split(' ').collect();
    let digit = |c: char| c.is_ascii_digit();
    let glue = |k: usize| {
        toks[k] == "dot"
            && k > 0
            && k + 1 < toks.len()
            && toks[k - 1].ends_with(digit)
            && toks[k + 1].starts_with(digit)
    };
    for (k, tok) in toks.iter().enumerate() {
        if glue(k) {
            out.push('.');
            continue;
        }
        if k > 0 && !glue(k - 1) {
            out.push(' ');
        }
        out.push_str(tok);
    }
}

/// Joins a lone letter to the number before or after it: "4 k" -> "4k",
/// "gate b 12" -> "b12". "a" never joins, "i" joins only before a number,
/// and a letter in a spelled run ("d e 5") stays apart.
fn join_letters(out: &mut String, begin: usize) {
    let text = out.split_off(begin);
    let toks: Vec<&str> = text.split(' ').collect();
    let letter = |k: usize| {
        let core = toks.get(k)?.trim_end_matches(trail_punct);
        (core.len() == 1 && core.as_bytes()[0].is_ascii_alphabetic())
            .then(|| core.as_bytes()[0].to_ascii_lowercase())
    };
    let lone = |k: usize| {
        letter(k).is_some() && (k == 0 || letter(k - 1).is_none()) && letter(k + 1).is_none()
    };
    let digit = |c: char| c.is_ascii_digit();
    let joins = |k: usize| {
        let (prev, tok) = (toks[k - 1], toks[k]);
        let after_number =
            prev.ends_with(digit) && lone(k) && letter(k).is_some_and(|l| l != b'a' && l != b'i');
        let before_number = prev.len() == 1
            && lone(k - 1)
            && letter(k - 1).is_some_and(|l| l != b'a')
            && tok.starts_with(digit);
        after_number || before_number
    };
    out.reserve(text.len());
    for (k, tok) in toks.iter().enumerate() {
        if k > 0 && !joins(k) {
            out.push(' ');
        }
        out.push_str(tok);
    }
}

pub fn format_line(engine: &Normalizer, line: &str, out: &mut String) {
    let t = engine.tables;
    let begin = out.len();
    let spaces = line.bytes().filter(|&b| b == b' ').count();
    let mut words: Vec<Word> = Vec::with_capacity(spaces + 3);
    for tok in line.split_whitespace() {
        let w = split_word(tok, t);
        match hyphenated(&w, t) {
            Some((a, b)) => words.extend([a, b]),
            None => words.push(w),
        }
    }
    if engine.is_on(Setting::Letters) {
        join_meridiem(&mut words, t);
        join_zone(&mut words, t);
    }
    if engine.is_on(Setting::Noon) {
        drop_twelve(&mut words);
    }
    let mut f = features(&words);
    if f & F_VERBS != 0 {
        verb_months(&mut words);
    }
    if f & F_EXPAND != 0 {
        words = expand(words, t, engine.is_on(Setting::Percent));
        f = features(&words);
    }
    if f & F_PHONE != 0 {
        words = phone_hundreds(words, t);
    }
    if f & F_CUTS != 0 {
        cuts(&mut words);
    }
    let counting = |core: &str| {
        let role = t.role_of[t.lexicon.lookup(core.as_bytes()).class as usize] & 0x0F;
        COUNTING_ROLES >> role & 1 == 1
    };
    for (from, to) in guarded(&words, counting) {
        for w in &mut words[from..=to] {
            w.flags |= W_KEEP;
        }
    }
    let mut scratch = Scratch {
        windows: WindowBuf::new(),
        buf: [0u8; LINE_OUT],
    };
    let mut start = 0;
    for i in 0..words.len() {
        let last = i + 1 == words.len()
            || !words[i].trail.is_empty()
            || !words[i + 1].lead.is_empty()
            || words[i].keep() != words[i + 1].keep()
            || words[i].cut();
        if !last {
            continue;
        }
        if start > 0 {
            out.push(' ');
        }
        out.push_str(words[start].lead);
        let body = out.len();
        let segment = &words[start..=i];
        if words[i].keep() || (start == i && counting_list(&words, i)) {
            push_raw(out, segment);
        } else {
            format_segment(engine, segment, words[start].dated(), out, &mut scratch);
            if bare_hundreds(segment, words.get(i + 1)) {
                ungroup_last(out, body);
            }
        }
        push_trail(out, words[i].trail);
        start = i + 1;
    }
    if f & F_DOTS != 0 {
        join_dots(out, begin);
    }
    if f & F_LETTERS != 0 && engine.is_on(Setting::Letters) {
        join_letters(out, begin);
    }
}

/// "twenty-five" -> "twenty" "five", printing "twenty-five" if kept as words.
fn hyphenated<'a>(w: &Word<'a>, t: &Tables) -> Option<(Word<'a>, Word<'a>)> {
    if w.numeric() || w.core.len() < 7 {
        return None;
    }
    let (a, b) = w.core.split_once('-')?;
    let (a, b) = (tag(t, w.lead, a, ""), tag(t, "", b, w.trail));
    (a.value() && b.value()).then_some((Word { said: w.core, ..a }, Word { said: "", ..b }))
}

/// "One, two, three, go.": lone number words in a row, each closed by
/// punctuation, are a count said aloud and stay words.
fn counting_list(words: &[Word], i: usize) -> bool {
    let n = words.len();
    let lone = |k: usize| {
        words[k].digit()
            && (k == 0 || !words[k - 1].trail.is_empty())
            && (!words[k].trail.is_empty() || k + 1 == n)
    };
    lone(i) && ((i > 0 && lone(i - 1)) || (i + 1 < n && lone(i + 1)))
}

fn scale_exp(word: &str) -> u32 {
    match word {
        "thousand" => 3,
        "million" => 6,
        "billion" => 9,
        "trillion" => 12,
        _ => 0,
    }
}

struct Scratch {
    windows: WindowBuf,
    buf: [u8; LINE_OUT],
}

fn push_raw(out: &mut String, words: &[Word]) {
    let mut first = true;
    for w in words.iter().filter(|w| !w.said.is_empty()) {
        if !first {
            out.push(' ');
        }
        out.push_str(w.said);
        first = false;
    }
}

fn render<'b>(
    engine: &Normalizer,
    words: &[Word],
    w: &Window,
    buf: &'b mut [u8; LINE_OUT],
) -> Option<&'b str> {
    let span = words
        .get(w.start..w.end)
        .filter(|s| !w.raw && s.len() <= MAX_TOKENS)?;
    let mut tokens: [&[u8]; MAX_TOKENS] = [&[]; MAX_TOKENS];
    let mut infos = [TokenInfo::default(); MAX_TOKENS];
    for ((token, info), word) in tokens.iter_mut().zip(&mut infos).zip(span) {
        *token = word.core.as_bytes();
        *info = word.info;
    }
    let done = engine
        .format_infos(&tokens[..span.len()], infos, buf)
        .ok()?;
    core::str::from_utf8(&buf[..done.len]).ok()
}

fn render_run(
    engine: &Normalizer,
    words: &[Word],
    run: &[Window],
    buf: &mut [u8; LINE_OUT],
) -> Option<String> {
    let digit_run = run
        .iter()
        .all(|w| words[w.start..w.end].iter().all(|x| x.digit()));
    if digit_run {
        let mut all = String::new();
        for w in run {
            let text = render(engine, words, w, buf)?;
            if !text.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            all.push_str(text);
        }
        return Some(all);
    }
    let mut pieces = Vec::with_capacity(run.len());
    for (i, w) in run.iter().enumerate() {
        let digits = piece_digits(render(engine, words, w, buf)?)?;
        let k = if i + 1 == run.len() {
            0
        } else {
            scale_exp(words[w.end - 1].core)
        };
        pieces.push((digits, k));
    }
    stitch(&pieces).map(|n| group_thousands(&n))
}

/// Whether the engine kept a rewritten span as its words, so it must print
/// as said ("a hundred", not "one hundred").
fn as_spoken(text: &str, span: &[Word]) -> bool {
    span.iter().any(|w| !core::ptr::eq(w.said, w.core))
        && text.split(' ').eq(span.iter().map(|w| w.core))
}

fn group_thousands(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn words_first(t: &Tables, core: &str) -> bool {
    let c = t.lexicon.lookup(core.as_bytes()).canonical as usize;
    (t.words_first[(c >> 6) % WORDS_FIRST_WORDS] >> (c & 63)) & 1 == 1
}

fn digit_word(engine: &Normalizer, core: &str) -> bool {
    let t = engine.tables;
    let info = t.lexicon.lookup(core.as_bytes());
    engine.is_on(Setting::SingleDigits)
        && engine.is_digit_word(info.canonical, t.role_of[info.class as usize] & 0x0F)
}

/// `dated` writes a lone ordinal as digits ("the 4th of July").
fn format_segment(
    engine: &Normalizer,
    words: &[Word],
    dated: bool,
    out: &mut String,
    scratch: &mut Scratch,
) {
    let t = engine.tables;
    if let [only] = words
        && !dated
        && words_first(t, only.core)
        && !digit_word(engine, only.core)
    {
        out.push_str(only.said);
        return;
    }
    let Scratch { windows, buf } = scratch;
    windows_scaled_into(
        windows,
        words.len(),
        |i| words[i].numeric(),
        |i| scale_exp(words[i].core) != 0,
        |i| words[i].digit(),
        |i| {
            let class = |w: &Word| t.lexicon.lookup(w.core.as_bytes()).class;
            let (a, b) = (class(&words[i - 1]), class(&words[i]));
            fold_left(&t.left_folds, a, b) != b
        },
    );
    let ws = windows.as_slice();
    let mut i = 0;
    while i < ws.len() {
        let mut j = i + 1;
        while j < ws.len() && ws[j].join {
            j += 1;
        }
        if i > 0 {
            out.push(' ');
        }
        let span = &words[ws[i].start..ws[j - 1].end];
        if j - i > 1 {
            match render_run(engine, words, &ws[i..j], buf) {
                Some(text) if !as_spoken(&text, span) => out.push_str(&text),
                _ => push_raw(out, span),
            }
        } else {
            match render(engine, words, &ws[i], buf) {
                Some(text) if !as_spoken(text, span) => out.push_str(text),
                _ => push_raw(out, span),
            }
        }
        i = j;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub start: usize,
    pub end: usize,
    pub raw: bool,
    pub join: bool,
}

/// The fewest words a piece of a long digit string can have; shorter pieces
/// have no shape.
const MIN_DIGIT_PIECE: usize = 3;

fn split_run(
    start: usize,
    end: usize,
    group_scale: &impl Fn(usize) -> bool,
    digit: &impl Fn(usize) -> bool,
) -> Option<Vec<Window>> {
    let digits = (start..end).all(digit);
    let mut pieces = Vec::new();
    let mut pos = start;
    while end - pos > MAX_TOKENS {
        let rest = end - pos;
        let limit = if digits && rest - MAX_TOKENS < MIN_DIGIT_PIECE {
            rest - MIN_DIGIT_PIECE
        } else {
            MAX_TOKENS
        };
        let cut = (pos..pos + limit)
            .rev()
            .find(|&j| if digits { digit(j) } else { group_scale(j) })?;
        pieces.push(Window {
            start: pos,
            end: cut + 1,
            raw: false,
            join: pos > start,
        });
        pos = cut + 1;
    }
    pieces.push(Window {
        start: pos,
        end,
        raw: false,
        join: true,
    });
    Some(pieces)
}

fn piece_digits(text: &str) -> Option<String> {
    let (grouped, zeros) = match text.split_once(' ') {
        None => (text, 0),
        Some((d, "thousand")) => (d, 3),
        Some((d, "million")) => (d, 6),
        Some((d, "billion")) => (d, 9),
        Some((d, "trillion")) => (d, 12),
        Some(_) => return None,
    };
    let digits: String = grouped.chars().filter(|&c| c != ',').collect();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{digits}{}", "0".repeat(zeros)))
}

fn stitch(pieces: &[(String, u32)]) -> Option<String> {
    let (first, rest) = pieces.split_first()?;
    let mut acc = first.0.clone();
    let mut k = first.1 as usize;
    for (digits, next_k) in rest {
        let next_k = *next_k as usize;
        if (next_k != 0 && next_k >= k) || digits.len() > k || !acc.ends_with(&"0".repeat(k)) {
            return None;
        }
        acc.truncate(acc.len() - k);
        acc.push_str(&"0".repeat(k - digits.len()));
        acc.push_str(digits);
        k = next_k;
    }
    Some(acc)
}

#[cfg(feature = "internals")]
pub fn windows(n: usize, numeric: impl Fn(usize) -> bool) -> Vec<Window> {
    windows_scaled(n, numeric, |_| false)
}

#[cfg(feature = "internals")]
pub fn windows_scaled(
    n: usize,
    numeric: impl Fn(usize) -> bool,
    group_scale: impl Fn(usize) -> bool,
) -> Vec<Window> {
    let mut out = WindowBuf::new();
    windows_scaled_into(&mut out, n, numeric, group_scale, |_| false, |_| false);
    out.as_slice().to_vec()
}

const INLINE_WINDOWS: usize = 16;

/// A segment's windows, on the stack unless there are many.
struct WindowBuf {
    inline: [Window; INLINE_WINDOWS],
    len: usize,
    spill: Vec<Window>,
}

impl WindowBuf {
    fn new() -> Self {
        let empty = Window {
            start: 0,
            end: 0,
            raw: false,
            join: false,
        };
        Self {
            inline: [empty; INLINE_WINDOWS],
            len: 0,
            spill: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.spill.clear();
    }

    fn push(&mut self, w: Window) {
        if self.len < INLINE_WINDOWS {
            self.inline[self.len] = w;
            self.len += 1;
        } else {
            if self.spill.is_empty() {
                self.spill.extend_from_slice(&self.inline);
            }
            self.spill.push(w);
        }
    }

    fn as_slice(&self) -> &[Window] {
        if self.spill.is_empty() {
            &self.inline[..self.len]
        } else {
            &self.spill
        }
    }
}

fn windows_scaled_into(
    out: &mut WindowBuf,
    n: usize,
    numeric: impl Fn(usize) -> bool,
    group_scale: impl Fn(usize) -> bool,
    digit: impl Fn(usize) -> bool,
    folded: impl Fn(usize) -> bool,
) {
    out.clear();
    let run_end = |mut i: usize| {
        while i < n && numeric(i) {
            i += 1;
        }
        i
    };
    let mut start = 0;
    while start < n {
        if numeric(start) {
            let r = run_end(start);
            if r - start > MAX_TOKENS {
                match split_run(start, r, &group_scale, &digit) {
                    Some(pieces) => pieces.into_iter().for_each(|p| out.push(p)),
                    None => out.push(Window {
                        start,
                        end: r,
                        raw: true,
                        join: false,
                    }),
                }
                start = r;
                continue;
            }
        }
        let mut end = (start + MAX_TOKENS).min(n);
        let mut j = start;
        while j < end {
            if numeric(j) {
                let r = run_end(j);
                if r - j > MAX_TOKENS && j > start {
                    end = j;
                    break;
                }
                j = r;
            } else {
                j += 1;
            }
        }
        // The run the edge falls in or right before: inside it, before it
        // ("at | five"), or inside its folded cue ("it | is five").
        let run =
            if end < n && numeric(end) && (numeric(end - 1) || run_end(end) - end <= MAX_TOKENS) {
                let mut cut = end;
                while cut > start && numeric(cut - 1) {
                    cut -= 1;
                }
                Some(cut)
            } else {
                (end + 1 < n && numeric(end + 1) && folded(end)).then_some(end + 1)
            };
        if let Some(cut) = run
            && cut > start
        {
            // Start the next window on the word before the run, so the run
            // keeps its left context ("room", "at"), unless that word is the
            // right context of a number ending this window.
            end = if cut - 1 > start && !numeric(cut - 2) {
                // A folded pair ("it is") is one context word: keep both.
                let pair = cut - 2 > start && folded(cut - 1) && !numeric(cut - 3);
                cut - 1 - usize::from(pair)
            } else {
                cut
            };
        }
        out.push(Window {
            start,
            end,
            raw: false,
            join: false,
        });
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punct_checks_match_the_lists() {
        for c in (0..0x3000).filter_map(char::from_u32) {
            assert_eq!(lead_punct(c), LEAD_PUNCT.contains(&c), "{c:?}");
            assert_eq!(trail_punct(c), TRAIL_PUNCT.contains(&c), "{c:?}");
        }
        assert_eq!(bare("them.\u{201D}"), "them");
        assert_eq!(bare("them"), "them");
    }

    fn split(numeric: &[bool]) -> Vec<Window> {
        windows(numeric.len(), |i| numeric[i])
    }

    fn fmt(start: usize, end: usize) -> Window {
        Window {
            start,
            end,
            raw: false,
            join: false,
        }
    }

    fn raw(start: usize, end: usize) -> Window {
        Window {
            start,
            end,
            raw: true,
            join: false,
        }
    }

    #[test]
    fn empty_input_has_no_windows() {
        assert!(split(&[]).is_empty());
    }

    #[test]
    fn short_line_is_one_window() {
        assert_eq!(split(&[false, true, true]), vec![fmt(0, 3)]);
    }

    #[test]
    fn cut_moves_before_the_word_ahead_of_a_number_that_crosses_the_limit() {
        let mut numeric = [false; 10];
        numeric[6..9].fill(true);
        assert_eq!(split(&numeric), vec![fmt(0, 5), fmt(5, 10)]);
    }

    #[test]
    fn cut_keeps_the_word_after_an_earlier_number_in_its_window() {
        let mut numeric = [false; 10];
        numeric[4] = true;
        numeric[6..9].fill(true);
        assert_eq!(split(&numeric), vec![fmt(0, 6), fmt(6, 10)]);
    }

    #[test]
    fn cut_between_words_stays_at_the_limit() {
        assert_eq!(split(&[false; 10]), vec![fmt(0, 8), fmt(8, 10)]);
    }

    #[test]
    fn number_of_exactly_eight_tokens_is_formatted() {
        assert_eq!(split(&[true; 8]), vec![fmt(0, 8)]);
    }

    #[test]
    fn number_longer_than_a_window_is_left_raw_whole() {
        let mut numeric = [false; 13];
        numeric[2..12].fill(true);
        assert_eq!(split(&numeric), vec![fmt(0, 2), raw(2, 12), fmt(12, 13)]);
    }

    fn join(start: usize, end: usize) -> Window {
        Window {
            start,
            end,
            raw: false,
            join: true,
        }
    }

    #[test]
    fn long_run_splits_after_group_scales() {
        let scales = [1, 6, 9];
        let ws = windows_scaled(14, |_| true, |i| scales.contains(&i));
        assert_eq!(ws, vec![fmt(0, 7), join(7, 14)]);
    }

    #[test]
    fn long_run_takes_the_latest_scale_that_fits() {
        let scales = [4, 9];
        let ws = windows_scaled(15, |_| true, |i| scales.contains(&i));
        assert_eq!(ws, vec![fmt(0, 5), join(5, 10), join(10, 15)]);
    }

    #[test]
    fn long_run_without_a_reachable_scale_stays_raw() {
        let ws = windows_scaled(12, |_| true, |i| i == 10);
        assert_eq!(ws, vec![raw(0, 12)]);
    }

    #[test]
    fn plain_windows_never_split_a_long_run() {
        assert_eq!(windows(12, |_| true), vec![raw(0, 12)]);
    }

    #[test]
    fn piece_digits_accepts_digits_and_digits_with_a_scale_word() {
        assert_eq!(piece_digits("72961").as_deref(), Some("72961"));
        assert_eq!(piece_digits("999 million").as_deref(), Some("999000000"));
        assert_eq!(piece_digits("12 thousand").as_deref(), Some("12000"));
        assert_eq!(piece_digits("9 trillion").as_deref(), Some("9000000000000"));
        assert_eq!(scale_exp("trillion"), 12);
        assert_eq!(piece_digits("5.2 million"), None);
        assert_eq!(piece_digits("$925"), None);
        assert_eq!(piece_digits("one billion"), None);
        assert_eq!(piece_digits(""), None);
    }

    #[test]
    fn stitch_fills_lower_groups_with_zero_padding() {
        assert_eq!(
            stitch(&[("147000".into(), 3), ("451".into(), 0)]).as_deref(),
            Some("147451")
        );
        assert_eq!(
            stitch(&[("1593000000".into(), 6), ("72961".into(), 0)]).as_deref(),
            Some("1593072961")
        );
        assert_eq!(
            stitch(&[
                ("999000000".into(), 6),
                ("999000".into(), 3),
                ("999".into(), 0)
            ])
            .as_deref(),
            Some("999999999")
        );
        assert_eq!(
            stitch(&[("5000".into(), 3), ("7".into(), 0)]).as_deref(),
            Some("5007")
        );
    }

    #[test]
    fn stitch_rejects_pieces_that_do_not_fit() {
        assert_eq!(stitch(&[("147000".into(), 3), ("4510".into(), 0)]), None);
        assert_eq!(stitch(&[("147001".into(), 3), ("451".into(), 0)]), None);
        assert_eq!(
            stitch(&[("5000".into(), 3), ("6000000".into(), 6), ("1".into(), 0)]),
            None
        );
    }

    #[test]
    fn windows_cover_every_token_once_in_order() {
        let mut state = 0x1234_5678_u64;
        for _ in 0..2000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let n = (state % 40) as usize;
            let numeric: Vec<bool> = (0..n).map(|i| (state >> (i % 60)) & 1 == 1).collect();
            let ws = split(&numeric);
            let mut next = 0;
            for w in &ws {
                assert_eq!(w.start, next);
                assert!(w.end > w.start);
                assert!(w.raw || w.end - w.start <= MAX_TOKENS);
                next = w.end;
            }
            assert_eq!(next, n);
        }
    }
}
