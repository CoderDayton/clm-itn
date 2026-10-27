use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};

use crate::engine::Normalizer;
use crate::engine::span::fold_left;
use crate::tables::{Tables, WORDS_FIRST_WORDS};
use crate::types::{
    CLASS_UNKNOWN, CanonicalId, MAX_TOKENS, ROLE_AND, ROLE_DIGIT, ROLE_HUNDRED, ROLE_MSCALE,
    ROLE_OH, ROLE_ORDINAL, ROLE_SCALE, ROLE_TEEN, ROLE_TENS, Setting, TokenInfo, sense,
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
    /// The whitespace before the word as written; empty for a word a rewrite made.
    gap: &'a str,
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
/// "trillion" or above: read as "billion", so never folded with lower groups.
const W_BIG: u16 = 1 << 9;
/// "thousand" and above.
const W_SCALE: u16 = 1 << 10;

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
        gap: "",
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
            | bit(role == ROLE_ORDINAL, W_ORDINAL)
            | bit(role == ROLE_MSCALE && scale_exp(b) >= 12, W_BIG)
            | bit(role == ROLE_SCALE || role == ROLE_MSCALE, W_SCALE),
        kind: kind(b),
    }
}

/// The lexicon entry of `core`. Scales past billion read as "billion", which
/// prints the spoken word ("5 trillion").
fn lookup(t: &Tables, core: &str) -> TokenInfo {
    let info = t.lexicon.lookup(core.as_bytes());
    if info.class == CLASS_UNKNOWN && scale_exp(core) >= 12 {
        t.lexicon.lookup(b"billion")
    } else {
        info
    }
}

fn tag<'a>(t: &Tables, lead: &'a str, core: &'a str, trail: &'a str) -> Word<'a> {
    tag_class(t, lookup(t, core), lead, core, trail)
}

#[inline(always)]
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
    let info = lookup(t, &token[start..end]);
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
                let gap = words[i].gap;
                words[put] = Word {
                    gap,
                    ..tag(t, "", core, trail)
                };
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
                let gap = words[i].gap;
                words[put] = Word {
                    gap,
                    ..tag(t, "", core, trail)
                };
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
        if twelve && words.get(i + 1).is_some_and(noon) {
            words[i + 1].gap = words[i].gap;
            continue;
        }
        if put != i {
            words[put] = words[i];
        }
        put += 1;
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
fn guarded(t: &Tables, words: &[Word], counting: impl Fn(&str) -> bool) -> Vec<(usize, usize)> {
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
            && let Some((from, to)) = relative_time(t, words, i)
        {
            mark(from, to);
        }
        if num(i)
            && let Some((from, to)) = word_sense(t, words, i)
        {
            mark(from, to);
        }
        if let Some(end) = mixed_fraction(t, words, i) {
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

/// Canonical ID of `w` without trailing punctuation, which the sense rules
/// read: an unknown "us." reads as "us".
fn bare_id(t: &Tables, w: &Word) -> CanonicalId {
    let b = bare(w.core);
    if b.len() == w.core.len() {
        w.info.canonical
    } else {
        lookup(t, b).canonical
    }
}

fn senses(t: &Tables, w: &Word) -> u32 {
    t.sense[bare_id(t, w) as usize]
}

/// The phrase in `t.idioms` that `words[i]` is part of, as a word range.
fn idiom(
    t: &Tables,
    words: &[Word],
    i: usize,
    id: impl Fn(isize) -> CanonicalId,
) -> Option<(usize, usize)> {
    let me = words[i].info.canonical;
    for row in &t.idioms {
        let len = row.iter().position(|&c| c == 0).unwrap_or(row.len());
        for p in (0..len).filter(|&p| row[p] == me) {
            let start = -(p as isize);
            if row[..len].iter().zip(start..).all(|(&c, d)| id(d) == c) {
                return Some((i - p, i - p + len - 1));
            }
        }
    }
    None
}

/// A lone "one" to "nine" used as a word, not a count ("no one", "one of
/// them", "the two of us", "two thirds", "cloud nine"). Returns the span that
/// stays as spoken. Context never crosses punctuation.
fn word_sense(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
    let me = t.sense[words[i].info.canonical as usize];
    if me & (sense::COUNT | sense::IDIOM) == 0 {
        return None;
    }
    let at = |d: isize| -> Option<&Word> {
        let k = i.checked_add_signed(d).filter(|&k| k < words.len())?;
        let (lo, hi) = (k.min(i), k.max(i));
        let open = words[lo..hi].iter().all(|w| w.trail.is_empty())
            && words[lo + 1..=hi].iter().all(|w| w.lead.is_empty());
        open.then(|| &words[k])
    };
    let id = |d: isize| at(d).map_or(0, |w| bare_id(t, w));
    let is = |d: isize, bit: u32| t.sense[id(d) as usize] & bit != 0;
    let numeric = |d: isize| at(d).is_some_and(|w| w.numeric());
    if numeric(-1) {
        return None;
    }
    if me & sense::IDIOM != 0
        && let Some(span) = idiom(t, words, i, id)
    {
        return Some(span);
    }
    if me & sense::COUNT == 0 {
        return None;
    }
    let one = me & sense::ONE != 0;
    if one && is(1, sense::BY) && is(2, sense::ONE) {
        return Some((i, i + 2));
    }
    // A cue word before it ("step one.", "number one is") makes it a label.
    let plain_prev = at(-1).is_none_or(|w| w.info.class == CLASS_UNKNOWN);
    let (prev, next) = (t.sense[id(-1) as usize], t.sense[id(1) as usize]);
    let pronoun = one
        && ((prev & sense::ONE_AFTER != 0 && (!numeric(1) || next & sense::AND != 0))
            || (plain_prev && (at(1).is_none() || next & sense::ONE_CLAUSE != 0))
            || (next & sense::ONE_BEFORE != 0 && !numeric(2))
            || (next & sense::DAY != 0 && at(-1).is_none())
            || (prev & sense::IN != 0 && next & sense::PIECE != 0)
            || (next & sense::IN != 0 && is(2, sense::A) && is(3, sense::MILLION)));
    let group = next & sense::OF != 0 && is(2, sense::GROUP);
    (pronoun || group || next & sense::FRACTION != 0).then_some((i, i))
}

/// Whether `core` starts like a `sense::RELATIVE` word; skips most
/// `relative_time` calls.
fn connector_start(core: &str) -> bool {
    match core.as_bytes() {
        [a, b, ..] => matches!(
            [a | 0x20, b | 0x20],
            [b't', b'o'] | [b't', b'i'] | [b'p', b'a'] | [b'a', b'f']
        ),
        _ => false,
    }
}
/// A relative time such as "ten to four" or "half past six", with the
/// connector at `i`. It reads as a clock time or as a range or score, so the
/// span from the minute word to the hour stays as spoken. It counts only
/// after a time cue ("at", "it's") or as a whole clause, and not when the
/// hour is followed by a number word such as "pm" or a unit.
#[inline(never)]
fn relative_time(t: &Tables, words: &[Word], i: usize) -> Option<(usize, usize)> {
    let is = |k: usize, bit: u32| senses(t, &words[k]) & bit != 0;
    let plain = |k: usize| words[k].lead.is_empty() && words[k].trail.is_empty();
    let h = i + 1;
    if i == 0 || h >= words.len() || !words[h].value() || !plain(i) || !is(i, sense::RELATIVE) {
        return None;
    }
    if !words[h].lead.is_empty() || !is(h, sense::HOUR) {
        return None;
    }
    if h + 1 < words.len() && words[h].trail.is_empty() && words[h + 1].numeric() {
        return None;
    }
    let m = i - 1;
    if !words[m].trail.is_empty() {
        return None;
    }
    let from = if m > 0 && is(m - 1, sense::TWENTY) && is(m, sense::FIVE) && plain(m - 1) {
        m - 1
    } else if is(m, sense::MINUTE) {
        m
    } else {
        return None;
    };
    if from > 0 && words[from - 1].value() && words[from - 1].trail.is_empty() {
        return None;
    }
    let c = if from > 0 && is(from, sense::QUARTER) && is(from - 1, sense::A) && plain(from - 1) {
        from - 1
    } else {
        from
    };
    let opens = c == 0 || !words[c - 1].trail.is_empty() || !words[c].lead.is_empty();
    let closes = h + 1 == words.len() || !words[h].trail.is_empty();
    let cue = !opens && is(c - 1, sense::TIME_CUE);
    (cue || (opens && closes)).then_some((from, h))
}

#[inline(always)]
fn mixed_fraction(t: &Tables, words: &[Word], i: usize) -> Option<usize> {
    // "and" is a lexicon word, so its punctuation is already split off.
    if t.sense[words.get(i)?.info.canonical as usize] & sense::AND == 0 {
        return None;
    }
    let at = |k: usize| words.get(k).map(|w| senses(t, w));
    let article = at(i + 1)?;
    let fraction = at(i + 2)?;
    (article & (sense::A | sense::AN | sense::COUNT) != 0 && fraction & sense::FRACTION != 0)
        .then_some(i + 2)
}

/// Whether nothing but a space sits between `a` and `b`.
fn open(a: &Word, b: &Word) -> bool {
    a.trail.is_empty() && b.lead.is_empty()
}

fn is(w: &Word, s: &str) -> bool {
    bare(w.core).eq_ignore_ascii_case(s)
}

const SCALE_WORDS: [&str; 7] = [
    "hundred",
    "thousand",
    "million",
    "billion",
    "trillion",
    "quadrillion",
    "quintillion",
];

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
            K_HUNDRED
                if prev.is_some_and(|p| is(p, "twenty"))
                    && next.is_some_and(|n| is(n, "hours")) =>
            {
                F_EXPAND
            }
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
                    gap: w.gap,
                    ..joined
                });
                i += 2;
            }
            Some(n)
                if is(&w, "twenty")
                    && is(&n, "hundred")
                    && after.is_some_and(|a| is(&a, "hours")) =>
            {
                push_two_thousand(&mut out, t, &w, &n);
                i += 2;
            }
            Some(n) if n.digit() && (is(&w, "double") || is(&w, "triple")) => {
                let copies = if is(&w, "double") { 1 } else { 2 };
                for k in 0..copies {
                    out.push(Word {
                        gap: if k == 0 { w.gap } else { "" },
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
                    gap: w.gap,
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
                    gap: w.gap,
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
                    gap: w.gap,
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

/// "twenty hundred" on the 24-hour clock is 2000: reads as "two thousand".
fn push_two_thousand<'a>(
    out: &mut Vec<Word<'a>>,
    t: &Tables,
    twenty: &Word<'a>,
    hundred: &Word<'a>,
) {
    out.push(Word {
        said: twenty.core,
        gap: twenty.gap,
        ..tag(t, twenty.lead, "two", "")
    });
    out.push(Word {
        said: hundred.core,
        gap: hundred.gap,
        ..tag(t, "", "thousand", hundred.trail)
    });
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
            out.push(Word {
                said: w.core,
                gap: w.gap,
                ..oh
            });
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
    // "twenty one hundred" is 2100 on the 24-hour clock, but any other tens
    // word before a digit is a larger count.
    let clock = h >= 2
        && is(&segment[h - 2], "twenty")
        && matches!(bare(segment[h - 1].core), "one" | "two" | "three")
        && !(h >= 3 && segment[h - 3].value());
    let lead = h >= 2 && segment[h - 2].value() && !clock;
    tail_ok
        && x.is_some_and(|x| x.value() && (!x.digit() || clock))
        && !lead
        && (!last.trail.is_empty() || next.is_none_or(|n| is(n, "hours")))
}

/// Drops the comma from the last "1,900" in `out[body..]`.
fn ungroup_last(out: &mut String, body: usize) {
    let text = &out[body..];
    let text = text.strip_suffix(" hours").unwrap_or(text);
    let end = body + text.len();
    let last = out[body..end].rfind(char::is_whitespace).map_or(body, |k| {
        body + k + out[body + k..].chars().next().map_or(1, char::len_utf8)
    });
    let b = &out.as_bytes()[last..end];
    if b.len() == 5 && b[1] == b',' && b.iter().all(|&c| c == b',' || c.is_ascii_digit()) {
        out.remove(last + 1);
    }
}

/// "192 dot 168 dot 1" -> "192.168.1".
fn join_dots(out: &mut String, begin: usize) {
    let text = out.split_off(begin);
    let toks = gapped(&text);
    let digit = |c: char| c.is_ascii_digit();
    let glue = |k: usize| {
        toks[k].1 == "dot"
            && k > 0
            && k + 1 < toks.len()
            && toks[k - 1].1.ends_with(digit)
            && toks[k + 1].1.starts_with(digit)
    };
    for (k, &(gap, tok)) in toks.iter().enumerate() {
        if glue(k) {
            out.push('.');
            continue;
        }
        if k == 0 || !glue(k - 1) {
            out.push_str(gap);
        }
        out.push_str(tok);
    }
    out.push_str(&text[text.trim_end().len()..]);
}

/// `text` cut at whitespace: each word with the whitespace before it.
fn gapped(text: &str) -> Vec<(&str, &str)> {
    let base = text.as_ptr().addr();
    let mut end = 0;
    text.split_whitespace()
        .map(|tok| {
            let at = tok.as_ptr().addr() - base;
            let gap = &text[end..at];
            end = at + tok.len();
            (gap, tok)
        })
        .collect()
}

/// Joins a lone letter to the number before or after it: "4 k" -> "4k",
/// "gate b 12" -> "b12". "a" never joins, "i" joins only before a number,
/// and a letter in a spelled run ("d e 5") stays apart.
fn join_letters(out: &mut String, begin: usize) {
    let text = out.split_off(begin);
    let toks = gapped(&text);
    let letter = |k: usize| {
        let core = toks.get(k)?.1.trim_end_matches(trail_punct);
        (core.len() == 1 && core.as_bytes()[0].is_ascii_alphabetic())
            .then(|| core.as_bytes()[0].to_ascii_lowercase())
    };
    let lone = |k: usize| {
        letter(k).is_some() && (k == 0 || letter(k - 1).is_none()) && letter(k + 1).is_none()
    };
    let digit = |c: char| c.is_ascii_digit();
    let joins = |k: usize| {
        let (prev, tok) = (toks[k - 1].1, toks[k].1);
        let after_number =
            prev.ends_with(digit) && lone(k) && letter(k).is_some_and(|l| l != b'a' && l != b'i');
        let before_number = prev.len() == 1
            && lone(k - 1)
            && letter(k - 1).is_some_and(|l| l != b'a')
            && tok.starts_with(digit);
        after_number || before_number
    };
    out.reserve(text.len());
    for (k, &(gap, tok)) in toks.iter().enumerate() {
        if k == 0 || !joins(k) {
            out.push_str(gap);
        }
        out.push_str(tok);
    }
    out.push_str(&text[text.trim_end().len()..]);
}

/// Tabs and line breaks print as written, and no number or context crosses them.
fn is_break(c: char) -> bool {
    matches!(c, '\t'..='\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Words a long line is formatted in at a time, so its word list stays in cache.
const BATCH_WORDS: usize = 256;
/// Words read before a batch's first word and after its cut: more than any
/// line rule reads from a word.
const CUT_BEHIND: usize = 16;
const CUT_AHEAD: usize = 16;

pub fn format_line(engine: &Normalizer, line: &str, out: &mut String) {
    format_line_in(engine, line, out, BATCH_WORDS);
}

/// [`format_line`] in batches of at least `batch` words, cut only where the
/// output is the same as formatting the line whole. Returns the most words
/// formatted at once.
pub fn format_line_in(engine: &Normalizer, line: &str, out: &mut String, batch: usize) -> usize {
    let spaces = line.bytes().filter(|&b| b == b' ').count();
    if spaces < batch {
        return format_short(engine, line, out, spaces);
    }
    let first = batch.saturating_add(CUT_BEHIND + CUT_AHEAD);
    let mut words = Vec::with_capacity(first.saturating_add(3));
    let mut raw = Vec::with_capacity(first.saturating_add(3));
    let mut rest = line;
    let (mut most, mut limit) = (0, first);
    // Where the words in `words` start in `rest`, where the last one ends,
    // and the first word of this batch not yet written.
    let (mut end_at, mut resume, mut ragged) = (0, None, false);
    loop {
        let (end, r, stop) = tokenize::<true>(rest, engine.tables, &mut words, end_at, limit);
        ragged |= r;
        most = most.max(words.len());
        if let Stop::Batch = stop {
            let this = Batch {
                ragged,
                resume,
                size: batch,
            };
            let step = format_batch(engine, rest, &mut words, &mut raw, this, out);
            match step {
                Some(next) => {
                    rest = &rest[next.at..];
                    end_at = end - next.at;
                    resume = Some(next.resume);
                    ragged = words.iter().skip(1).any(|w| w.gap != " ");
                }
                // No cut: read on, and try again with twice the words.
                None => {
                    end_at = end;
                    limit = words.len().saturating_mul(2);
                }
            }
            continue;
        }
        let cut = match stop {
            Stop::Break(k) => Some(k),
            _ => None,
        };
        let piece = cut.map_or(rest, |k| &rest[..k]);
        words = format_piece(engine, piece, words, end, ragged, resume, out);
        let Some(k) = cut else { return most };
        let n = rest[k..].find(|c| !is_break(c)).unwrap_or(rest.len() - k);
        out.push_str(&rest[k..k + n]);
        rest = &rest[k + n..];
        words.clear();
        (end_at, resume, ragged, limit) = (0, None, false, first);
    }
}

/// A line too short to batch, formatted piece by piece between its breaks.
fn format_short(engine: &Normalizer, line: &str, out: &mut String, spaces: usize) -> usize {
    let mut words = Vec::with_capacity(spaces + 3);
    let mut rest = line;
    let mut most = 0;
    loop {
        let (end, ragged, stop) = tokenize::<false>(rest, engine.tables, &mut words, 0, 0);
        most = most.max(words.len());
        let cut = match stop {
            Stop::Break(k) => Some(k),
            _ => None,
        };
        let piece = cut.map_or(rest, |k| &rest[..k]);
        words = format_piece(engine, piece, words, end, ragged, None, out);
        let Some(k) = cut else { return most };
        let n = rest[k..].find(|c| !is_break(c)).unwrap_or(rest.len() - k);
        out.push_str(&rest[k..k + n]);
        rest = &rest[k + n..];
        words.clear();
    }
}

/// Where `tokenize` stopped.
enum Stop {
    End,
    /// A tab or line break at this byte.
    Break(usize),
    /// The asked number of words.
    Batch,
}

/// A word a line rule can start from: a number word, digit or filler, or
/// "am", "pm", "m", "may" or "march", whose rules need no number. Other words
/// change only within 5 words of one.
fn anchor(w: &Word) -> bool {
    w.numeric()
        || w.filler()
        || w.kind == K_MONTH_VERB
        || w.core.bytes().any(|b| b.is_ascii_digit())
        || ["am", "pm", "m"].iter().any(|m| is(w, m))
}

/// Reads words from `line` after byte `from` up to its first tab or line
/// break, each with the whitespace before it. `BATCHED` stops early once
/// `words` holds `limit` words. Returns the end of the last word, whether any
/// gap read is other than one space, and why it stopped.
fn tokenize<'a, const BATCHED: bool>(
    line: &'a str,
    t: &Tables,
    words: &mut Vec<Word<'a>>,
    from: usize,
    limit: usize,
) -> (usize, bool, Stop) {
    let base = line.as_ptr().addr();
    let (mut end, mut ragged) = (from, false);
    for tok in line[from..].split_whitespace() {
        let at = tok.as_ptr().addr() - base;
        let gap = &line[end..at];
        let usual = gap.len() == usize::from(end != 0) && (end == 0 || gap.as_bytes()[0] == b' ');
        if !usual {
            if let Some(k) = gap.find(is_break) {
                return (end, ragged, Stop::Break(end + k));
            }
            ragged |= end != 0;
        }
        let mut w = split_word(tok, t);
        w.gap = gap;
        end = at + tok.len();
        match hyphenated(&w, t) {
            Some((a, b)) => words.extend([a, b]),
            None => words.push(w),
        }
        if BATCHED && words.len() >= limit {
            return (end, ragged, Stop::Batch);
        }
    }
    match line[end..].find(is_break) {
        Some(k) => (end, ragged, Stop::Break(end + k)),
        None => (end, ragged, Stop::End),
    }
}

/// Formats a line with no tabs or line breaks, given its words, and returns
/// the word list for reuse. Words before `resume` are already written.
fn format_piece<'a>(
    engine: &Normalizer,
    line: &'a str,
    words: Vec<Word<'a>>,
    end: usize,
    ragged: bool,
    resume: Option<usize>,
    out: &mut String,
) -> Vec<Word<'a>> {
    let begin = out.len();
    if words.is_empty() {
        out.push_str(line);
        return words;
    }
    let (words, f, piece) = prepare(engine, line, words, ragged);
    let from = resume.map_or(0, |at| resume_at(line, &words, at));
    emit(engine, &words, from..words.len(), f, piece, begin, out);
    out.push_str(&line[end..]);
    words
}

/// The first prepared word at or after address `at` in `line`.
fn resume_at(line: &str, words: &[Word], at: usize) -> usize {
    let (lo, hi) = (line.as_ptr().addr(), line.as_ptr().addr() + line.len());
    let inside = |w: &Word| (lo..=hi).contains(&w.gap.as_ptr().addr());
    words
        .iter()
        .position(|w| inside(w) && w.gap.as_ptr().addr() >= at)
        .unwrap_or(words.len())
}

/// Where the next batch of a long line starts.
struct Next {
    /// The byte of the line where its first word, read only for context, starts.
    at: usize,
    /// The address of the first word it writes.
    resume: usize,
}

/// Whether a word's gap lies in `line`; a word a rewrite made has none.
fn in_line(line: &str, w: &Word) -> bool {
    let (lo, at) = (line.as_ptr().addr(), w.gap.as_ptr().addr());
    (lo..=lo + line.len()).contains(&at)
}

/// What `format_batch` is told about a batch.
#[derive(Clone, Copy)]
struct Batch {
    /// Some gap between its words is not one space.
    ragged: bool,
    /// The address of the first word not yet written.
    resume: Option<usize>,
    /// The words it writes at least.
    size: usize,
}

/// Writes a batch of `words` up to its cut and leaves in `words` the tail the
/// next batch starts with, or writes nothing and leaves them all when there
/// is no cut. `raw` is scratch.
#[inline(never)]
fn format_batch<'a>(
    engine: &Normalizer,
    line: &'a str,
    words: &mut Vec<Word<'a>>,
    raw: &mut Vec<Word<'a>>,
    Batch {
        ragged,
        resume,
        size,
    }: Batch,
    out: &mut String,
) -> Option<Next> {
    let begin = out.len();
    raw.clear();
    raw.extend_from_slice(words);
    let (mut prepared, f, piece) = prepare(engine, line, core::mem::take(words), ragged);
    let skip = resume.map_or(0, |at| resume_at(line, &prepared, at));
    let least = skip + (size / 2).max(1);
    let found = batch_cut(engine.tables, &prepared, piece, skip, least).and_then(|c| {
        let at = prepared[c].gap.as_ptr().addr();
        let r = raw.iter().rposition(|w| w.gap.as_ptr().addr() == at)?;
        Some((c, at, r))
    });
    let Some((c, resume, r)) = found else {
        prepared.clear();
        prepared.extend_from_slice(raw);
        *words = prepared;
        return None;
    };
    emit(engine, &prepared, skip..c, f, piece, begin, out);
    let mut behind = r.saturating_sub(CUT_BEHIND);
    while behind > 0 && !in_line(line, &raw[behind]) {
        behind -= 1;
    }
    let at = raw[behind].gap.as_ptr().addr() - line.as_ptr().addr();
    prepared.clear();
    prepared.extend_from_slice(&raw[behind..]);
    *words = prepared;
    Some(Next { at, resume })
}

/// Whether a segment ends after `words[i]`, which is not the last word.
fn ends_segment(words: &[Word], i: usize) -> bool {
    !words[i].trail.is_empty()
        || !words[i + 1].lead.is_empty()
        || words[i].keep() != words[i + 1].keep()
        || words[i].cut()
}

/// Whether a batch can end before `words[c]`. Runs of number words, fillers
/// and digits are the only chains of rules that reach past `CUT_BEHIND`
/// words, and `c` is none of them. It is not a letter or "dot", which the
/// output joins to a neighbor. Inside a segment, no "hundred" is near, since
/// the comma of "1,900" depends on what ends its segment.
fn seam_ok(words: &[Word], c: usize, boundary: bool) -> bool {
    let w = &words[c];
    !anchor(w)
        && !matches!(w.kind, K_A | K_LETTER | K_DOT)
        && !w.gap.is_empty()
        && (boundary
            || !words[c.saturating_sub(5)..c]
                .iter()
                .any(|w| w.kind == K_HUNDRED))
}

/// The latest word in `least..=n - CUT_AHEAD` a batch of `words` can end
/// before. It starts a window, so the next batch plans its windows from the
/// same start, and the words on both sides of it read as in the whole line.
/// A segment cut in two keeps at least two words a side, since a one-word
/// segment formats differently.
fn batch_cut(t: &Tables, words: &[Word], piece: Piece, skip: usize, least: usize) -> Option<usize> {
    let n = words.len();
    let most = n.checked_sub(CUT_AHEAD)?;
    if most < least {
        return None;
    }
    let mut windows = WindowBuf::new();
    let mut e = n;
    while e > skip {
        let mut s = e - 1;
        while s > skip && !ends_segment(words, s - 1) {
            s -= 1;
        }
        if s <= most {
            plan(t, piece, &words[s..e], &mut windows);
            let found = windows
                .as_slice()
                .iter()
                .rev()
                .filter(|w| !w.join)
                .map(|w| s + w.start)
                .find(|&c| {
                    (least..=most).contains(&c)
                        && (c == s || (c - s >= 2 && c + 1 < e))
                        && seam_ok(words, c, c == s)
                });
            if found.is_some() {
                return found;
            }
        }
        if s <= least {
            return None;
        }
        e = s;
    }
    None
}

/// Runs the line passes on a line's words.
#[inline(always)]
fn prepare<'a>(
    engine: &Normalizer,
    line: &'a str,
    mut words: Vec<Word<'a>>,
    ragged: bool,
) -> (Vec<Word<'a>>, u8, Piece<'a>) {
    let t = engine.tables;
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
        let role = t.role_of[lookup(t, core).class as usize] & 0x0F;
        COUNTING_ROLES >> role & 1 == 1
    };
    for (from, to) in guarded(t, &words, counting) {
        for w in &mut words[from..=to] {
            w.flags |= W_KEEP;
        }
    }
    if words.len() > MAX_TOKENS {
        elide_and(&mut words, engine, line, MAX_TOKENS);
    }
    let piece = Piece {
        text: line,
        plain: !ragged,
        seen: words.iter().fold(0, |a, w| a | w.flags),
    };
    (words, f, piece)
}

/// Writes the prepared words `from..to` of a line, which started at `begin`
/// in `out`. A segment starts at `from` and ends at `to`.
#[inline(always)]
fn emit(
    engine: &Normalizer,
    all: &[Word],
    span: core::ops::Range<usize>,
    f: u8,
    piece: Piece,
    begin: usize,
    out: &mut String,
) {
    let words = &all[..span.end];
    let mut scratch = Scratch {
        windows: WindowBuf::new(),
        buf: [0u8; LINE_OUT],
    };
    let mut start = span.start;
    for i in span.start..words.len() {
        let last = i + 1 == words.len() || ends_segment(words, i);
        if !last {
            continue;
        }
        out.push_str(if start > 0 {
            sep(words[start].gap)
        } else {
            words[0].gap
        });
        out.push_str(words[start].lead);
        let body = out.len();
        let segment = &words[start..=i];
        if words[i].keep() || (start == i && counting_list(all, i)) {
            push_raw(out, segment);
        } else {
            format_segment(
                engine,
                segment,
                piece,
                words[start].dated(),
                out,
                &mut scratch,
            );
            if bare_hundreds(segment, all.get(i + 1)) {
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

/// The whitespace to print before a word; a word a rewrite made gets one space.
fn sep(gap: &str) -> &str {
    if gap.is_empty() { " " } else { gap }
}

fn role(t: &Tables, w: &Word) -> u8 {
    t.role_of[w.info.class as usize] & 0x0F
}

/// A word a cardinal group can start with: a digit, teen or tens word.
fn starts_group(t: &Tables, w: &Word) -> bool {
    matches!(role(t, w), ROLE_DIGIT | ROLE_TEEN | ROLE_TENS | ROLE_OH)
}

/// `line` from the start of `a` to the end of `b`, when both lie in it.
fn joined<'a>(line: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let base = line.as_ptr().addr();
    let from = a.as_ptr().addr().checked_sub(base)?;
    let to = (b.as_ptr().addr() + b.len()).checked_sub(base)?;
    line.get(from..to)
}

/// In a number run longer than `min_len` words, drops the "and" of "hundred
/// and five" or "thousand and five", so the run splits at its scales. The
/// "and" stays in the spoken text. A range ("one hundred and two hundred")
/// keeps it.
fn elide_and<'a>(words: &mut Vec<Word<'a>>, engine: &Normalizer, line: &'a str, min_len: usize) {
    let t = engine.tables;
    let linked = |a: &Word, b: &Word| {
        open(a, b) && a.numeric() && b.numeric() && !a.cut() && !a.keep() && !b.keep()
    };
    let mut i = 0;
    while i < words.len() {
        let mut j = i + 1;
        while j < words.len() && linked(&words[j - 1], &words[j]) {
            j += 1;
        }
        let mut k = i + 1;
        while j - i > min_len && k + 1 < j {
            let prev = words[k - 1];
            let said = (role(t, &words[k]) == ROLE_AND
                && core::ptr::eq(prev.said, prev.core)
                && final_and(engine, &words[k + 1..j], &prev))
            .then(|| joined(line, prev.said, words[k].core))
            .flatten();
            match said {
                Some(said) => {
                    words[k - 1].said = said;
                    words.remove(k);
                    j -= 1;
                }
                None => k += 1,
            }
        }
        i = j;
    }
}

/// Whether an "and" after `prev` joins the group in `after` to it: `prev` is
/// "hundred" or a scale, and the group is smaller than it.
fn final_and(engine: &Normalizer, after: &[Word], prev: &Word) -> bool {
    let t = engine.tables;
    let p = if role(t, prev) == ROLE_HUNDRED {
        2
    } else {
        scale_exp(prev.core)
    };
    let mut group = 0u64;
    let mut len = 0;
    for w in after {
        match role(t, w) {
            ROLE_DIGIT | ROLE_TEEN | ROLE_TENS | ROLE_OH => {
                group += u64::from(engine.value(w.info.canonical));
            }
            ROLE_HUNDRED => group = group.max(1) * 100,
            _ => break,
        }
        len += 1;
    }
    let next_scale = after.get(len).map_or(0, |w| scale_exp(w.core));
    p > 0 && len > 0 && group < 10u64.pow(p) && (p == 2 || next_scale < p)
}

/// The value of the number `words` start with, folded the way the engine
/// folds it. `None` when the words are not one number ("five six").
fn lead_value(engine: &Normalizer, words: &[Word]) -> Option<u128> {
    let t = engine.tables;
    let (mut total, mut cur, mut any) = (0u128, 0u128, false);
    let add = |cur: u128, v: u128| {
        let p = 10u128.pow(v.checked_ilog10().unwrap_or(0) + 1);
        cur.is_multiple_of(p).then_some(cur + v)
    };
    for w in words {
        let v = u128::from(engine.value(w.info.canonical));
        let exp = scale_exp(w.core);
        match role(t, w) {
            ROLE_DIGIT | ROLE_TEEN | ROLE_TENS | ROLE_OH => cur = add(cur, v)?,
            ROLE_HUNDRED => cur = cur.max(1) * 100,
            ROLE_AND if any => continue,
            ROLE_ORDINAL => {
                match v {
                    100 => cur = cur.max(1) * 100,
                    1000.. => {
                        total += cur.max(1) * v;
                        cur = 0;
                    }
                    _ => cur = add(cur, v)?,
                }
                return Some(total + cur);
            }
            _ if exp > 0 => {
                total += cur.max(1) * 10u128.pow(exp);
                cur = 0;
            }
            _ => break,
        }
        any = true;
    }
    any.then_some(total + cur)
}

/// A scale word right after a number in rendered text: its exponent and the
/// bytes it spans with its space.
fn scale_after(rest: &str) -> Option<(u32, usize)> {
    let word = rest.strip_prefix(' ')?;
    let end = word
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(word.len());
    let exp = scale_exp(&word[..end]);
    (exp > 0).then_some((exp, 1 + end))
}

/// Writes the whole number into the last piece of a split run, whose text
/// carries a unit, currency or suffix ("$342 and 16 cents"). The piece's first
/// number must be the value its words say, or the run stays as words.
fn splice(
    engine: &Normalizer,
    piece: &[Word],
    text: &str,
    higher: &mut Vec<(String, u32)>,
) -> Option<String> {
    let a = text.find(|c: char| c.is_ascii_digit())?;
    let len = text[a..]
        .find(|c: char| !(c.is_ascii_digit() || c == ','))
        .unwrap_or(text.len() - a);
    let mut b = a + len;
    let mut digits: String = text[a..b].chars().filter(|&c| c != ',').collect();
    let rest = &text[b..];
    if rest.starts_with(':') {
        return None;
    }
    match rest.strip_prefix('.') {
        Some(frac) if frac.starts_with(|c: char| c.is_ascii_digit()) => {
            let n = frac
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(frac.len());
            if scale_after(&frac[n..]).is_some() {
                return None;
            }
        }
        _ => {
            if let Some((exp, n)) = scale_after(rest) {
                digits.push_str(&"0".repeat(exp as usize));
                b += n;
            }
        }
    }
    let want = lead_value(engine, piece)?.to_string();
    if digits.trim_start_matches('0') != want.trim_start_matches('0') {
        return None;
    }
    higher.push((want, 0));
    let whole = group_thousands(&stitch(higher)?);
    Some(format!("{}{whole}{}", &text[..a], &text[b..]))
}

/// A window the tables could not read, whose one number run has several
/// groups ("one million seven hundred eighty one thousand nineteen"): reads the
/// run one group at a time and writes the window. False when that fails too.
fn push_regrouped<'a>(
    out: &mut String,
    engine: &Normalizer,
    span: &[Word<'a>],
    piece: Piece<'a>,
    buf: &mut [u8; LINE_OUT],
) -> bool {
    let t = engine.tables;
    let Some(a) = span.iter().position(Word::numeric) else {
        return false;
    };
    let b = a + span[a..].iter().take_while(|w| w.numeric()).count();
    let group_after = |i: usize| {
        span[i].has(W_SCALE)
            && span[i + 1..b]
                .iter()
                .find(|w| role(t, w) != ROLE_AND)
                .is_some_and(|w| starts_group(t, w))
    };
    if !(a..b - 1).any(group_after) || span[b..].iter().any(Word::numeric) {
        return false;
    }
    let mut run = span[a..b].to_vec();
    elide_and(&mut run, engine, piece.text, 0);
    let group_next = |i: usize| run.get(i + 1).is_some_and(|w| starts_group(t, w));
    let scale = |i: usize| scale_exp(run[i].core) != 0 && group_next(i);
    let pieces = split_run(0, run.len(), &scale, &|_| false);
    let Some(text) = pieces
        .filter(|p| p.len() > 1)
        .and_then(|p| render_run(engine, &run, &p, buf))
        .filter(|text| !as_spoken(text, &run))
    else {
        return false;
    };
    push_raw(out, &span[..a]);
    if a > 0 {
        out.push_str(sep(run[0].gap));
    }
    push_aligned(out, &text, &run, piece.plain);
    if b < span.len() {
        out.push_str(sep(span[b].gap));
        push_raw(out, &span[b..]);
    }
    true
}

/// Writes a window's rendering, keeping the whitespace written before each
/// word it kept. Words it rewrote are joined by one space.
fn push_aligned(out: &mut String, text: &str, span: &[Word], plain: bool) {
    if plain || span.iter().skip(1).all(|w| sep(w.gap) == " ") {
        out.push_str(text);
        return;
    }
    let toks: Vec<&str> = text.split(' ').collect();
    let (m, n) = (toks.len(), span.len());
    let at = |i: usize, j: usize| i * (n + 1) + j;
    let mut lcs = vec![0u16; (m + 1) * (n + 1)];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            lcs[at(i, j)] = if toks[i] == span[j].core {
                lcs[at(i + 1, j + 1)] + 1
            } else {
                lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
            };
        }
    }
    let mut matched = vec![None; m];
    let (mut i, mut j) = (0, 0);
    while i < m && j < n {
        if toks[i] == span[j].core && lcs[at(i, j)] == lcs[at(i + 1, j + 1)] + 1 {
            matched[i] = Some(j);
            i += 1;
            j += 1;
        } else if lcs[at(i + 1, j)] >= lcs[at(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let (mut next, mut after_match) = (0, true);
    for (k, tok) in toks.iter().enumerate() {
        if k > 0 {
            out.push_str(match matched[k] {
                Some(j) => sep(span[j].gap),
                None if after_match => span.get(next).map_or(" ", |w| sep(w.gap)),
                None => " ",
            });
        }
        out.push_str(tok);
        after_match = matched[k].is_some();
        if let Some(j) = matched[k] {
            next = j + 1;
        }
    }
}

/// "twenty-five" -> "twenty" "five", printing "twenty-five" if kept as words.
#[inline(always)]
fn hyphenated<'a>(w: &Word<'a>, t: &Tables) -> Option<(Word<'a>, Word<'a>)> {
    if w.numeric() || w.core.len() < 7 {
        return None;
    }
    // An unknown word keeps its punctuation in `core`: "(twenty-one)," splits
    // into the lead, "twenty-one" and the trail.
    let body = w.core;
    let from = if w.lead.is_empty() {
        body.len() - body.trim_start_matches(lead_punct).len()
    } else {
        0
    };
    let to = if w.trail.is_empty() {
        body.trim_end_matches(trail_punct).len().max(from)
    } else {
        body.len()
    };
    let (lead, inner, trail) = (
        if from > 0 { &body[..from] } else { w.lead },
        &body[from..to],
        if w.trail.is_empty() {
            &body[to..]
        } else {
            w.trail
        },
    );
    let (a, b) = inner.split_once('-')?;
    let (a, b) = (tag(t, lead, a, ""), tag(t, "", b, trail));
    let a = Word {
        said: inner,
        gap: w.gap,
        ..a
    };
    (a.value() && b.value()).then_some((a, Word { said: "", ..b }))
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
    const SCALES: [(&str, u32); 6] = [
        ("thousand", 3),
        ("million", 6),
        ("billion", 9),
        ("trillion", 12),
        ("quadrillion", 15),
        ("quintillion", 18),
    ];
    if !matches!(word.len(), 7 | 8 | 11) {
        return 0;
    }
    SCALES
        .iter()
        .find(|(s, _)| s.eq_ignore_ascii_case(word))
        .map_or(0, |&(_, e)| e)
}

struct Scratch {
    windows: WindowBuf,
    buf: [u8; LINE_OUT],
}

fn push_raw(out: &mut String, words: &[Word]) {
    let mut first = true;
    for w in words.iter().filter(|w| !w.said.is_empty()) {
        if !first {
            out.push_str(sep(w.gap));
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
    let (last, higher) = run.split_last()?;
    for w in higher {
        let digits = piece_digits(render(engine, words, w, buf)?)?;
        pieces.push((digits, scale_exp(words[w.end - 1].core)));
    }
    let text = render(engine, words, last, buf)?;
    match piece_digits(text) {
        Some(digits) => {
            pieces.push((digits, 0));
            stitch(&pieces).map(|n| group_thousands(&n))
        }
        None => splice(engine, &words[last.start..last.end], text, &mut pieces),
    }
}

/// Whether the engine kept a rewritten span as its words, so it must print
/// as said ("a hundred", not "one hundred").
fn as_spoken(text: &str, span: &[Word]) -> bool {
    span.iter().any(|w| !core::ptr::eq(w.said, w.core)) && same_words(text, span)
}

/// Whether the engine wrote the span's words back unread.
fn same_words(text: &str, span: &[Word]) -> bool {
    let len = span.iter().map(|w| w.core.len() + 1).sum::<usize>();
    text.len() + 1 == len && text.split(' ').eq(span.iter().map(|w| w.core))
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
    let c = lookup(t, core).canonical as usize;
    (t.words_first[(c >> 6) % WORDS_FIRST_WORDS] >> (c & 63)) & 1 == 1
}

fn digit_word(engine: &Normalizer, core: &str) -> bool {
    let t = engine.tables;
    let info = lookup(t, core);
    engine.is_on(Setting::SingleDigits)
        && engine.is_digit_word(info.canonical, t.role_of[info.class as usize] & 0x0F)
}

/// A line cut at its tabs and line breaks, with facts taken once.
#[derive(Clone, Copy)]
struct Piece<'a> {
    text: &'a str,
    /// Every gap between words is one space.
    plain: bool,
    /// The `W_*` bits found on any word.
    seen: u16,
}

/// Plans a segment's windows.
#[inline(always)]
fn plan(t: &Tables, piece: Piece, words: &[Word], windows: &mut WindowBuf) {
    let group_next = |i: usize| words.get(i + 1).is_some_and(|w| starts_group(t, w));
    windows_scaled_into(
        windows,
        words.len(),
        |i| words[i].numeric(),
        |i| scale_exp(words[i].core) != 0 && group_next(i),
        |i| piece.seen & W_BIG != 0 && words[i].has(W_BIG) && group_next(i),
        |i| words[i].digit(),
        |i| {
            let (a, b) = (words[i - 1].info.class, words[i].info.class);
            fold_left(&t.left_folds, a, b) != b
        },
    );
}

/// `dated` writes a lone ordinal as digits ("the 4th of July").
fn format_segment<'a>(
    engine: &Normalizer,
    words: &[Word<'a>],
    piece: Piece<'a>,
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
    plan(t, piece, words, windows);
    let ws = windows.as_slice();
    let mut i = 0;
    while i < ws.len() {
        let mut j = i + 1;
        while j < ws.len() && ws[j].join {
            j += 1;
        }
        if i > 0 {
            out.push_str(sep(words[ws[i].start].gap));
        }
        let span = &words[ws[i].start..ws[j - 1].end];
        if j - i > 1 {
            match render_run(engine, words, &ws[i..j], buf) {
                Some(text) if !as_spoken(&text, span) => {
                    push_aligned(out, &text, span, piece.plain);
                }
                _ => push_raw(out, span),
            }
        } else {
            let scaled =
                piece.seen & W_SCALE != 0 && span.iter().rev().skip(1).any(|w| w.has(W_SCALE));
            let read = match render(engine, words, &ws[i], buf) {
                Some(text) if !as_spoken(text, span) && !(scaled && same_words(text, span)) => {
                    push_aligned(out, text, span, piece.plain);
                    true
                }
                _ => false,
            };
            if !read && !(scaled && push_regrouped(out, engine, span, piece, buf)) {
                push_raw(out, span);
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

/// Splits a number run after every group scale, so each piece holds one
/// scale ("105 billion"), the form the tables read. A digit string splits
/// where its pieces fit a window.
fn split_run(
    start: usize,
    end: usize,
    group_scale: &impl Fn(usize) -> bool,
    digit: &impl Fn(usize) -> bool,
) -> Option<Vec<Window>> {
    let digits = (start..end).all(digit);
    let mut pieces = Vec::new();
    let mut pos = start;
    loop {
        let rest = end - pos;
        let cut = if digits {
            if rest <= MAX_TOKENS {
                break;
            }
            let limit = if rest - MAX_TOKENS < MIN_DIGIT_PIECE {
                rest - MIN_DIGIT_PIECE
            } else {
                MAX_TOKENS
            };
            (pos..pos + limit).rev().find(|&j| digit(j))?
        } else {
            match (pos..end - 1).find(|&j| group_scale(j)) {
                Some(j) if j - pos < MAX_TOKENS => j,
                None if rest <= MAX_TOKENS => break,
                _ => return None,
            }
        };
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
        Some((d, scale)) => match scale_exp(scale) {
            0 => return None,
            e => (d, e as usize),
        },
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
    windows_scaled_into(
        &mut out,
        n,
        numeric,
        group_scale,
        |_| false,
        |_| false,
        |_| false,
    );
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

/// Windows over words with no number between them, which print as spoken.
fn push_plain(out: &mut WindowBuf, mut start: usize, end: usize) {
    while start < end {
        let to = (start + MAX_TOKENS).min(end);
        out.push(Window {
            start,
            end: to,
            raw: true,
            join: false,
        });
        start = to;
    }
}

#[inline(always)]
fn windows_scaled_into(
    out: &mut WindowBuf,
    n: usize,
    numeric: impl Fn(usize) -> bool,
    group_scale: impl Fn(usize) -> bool,
    must_cut: impl Fn(usize) -> bool,
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
        if !numeric(start) {
            // The next run's window starts at its context word, or at a folded
            // pair ("it is"). Words before that print as spoken in windows of
            // their own, so how many precede the run never moves its edge.
            let ctx = (start..n).find(|&j| numeric(j)).map_or(n, |a| {
                let c = a - 1;
                if c > start && folded(c) { c - 1 } else { c }
            });
            if ctx > start {
                push_plain(out, start, ctx);
                start = ctx;
                continue;
            }
        }
        let split =
            |from: usize, to: usize| to - from > MAX_TOKENS || (from..to - 1).any(&must_cut);
        if numeric(start) {
            let r = run_end(start);
            if split(start, r) {
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
                if split(j, r) && j > start {
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
    fn plain_words_before_a_number_never_change_how_it_reads() {
        let engine = Normalizer::english();
        for tail in [
            "am works. first! x",
            "the score is two and then first place",
        ] {
            let want = engine.normalize(tail);
            for k in 1..=12 {
                let prefix = "zq ".repeat(k);
                let got = engine.normalize(&format!("{prefix}{tail}"));
                assert_eq!(
                    got.strip_prefix(&prefix),
                    Some(want.as_str()),
                    "{k} words before"
                );
            }
        }
    }

    #[test]
    fn long_lines_format_in_bounded_batches_with_the_same_output() {
        let engine = Normalizer::english();
        let plain = "we talked about the plans for the trip, what everybody wanted to bring along, where we would stay, then we went home to rest.";
        let numbers = "at two thirty pm we need about twenty five chairs, one hundred and twenty people, and room four oh two by May third.";
        let line = [plain, numbers, plain, plain, numbers].repeat(40).join(" ");
        let mut whole = String::new();
        format_line_in(&engine, &line, &mut whole, usize::MAX);
        for batch in [1, 16, 64, 300] {
            let mut out = String::new();
            let most = format_line_in(&engine, &line, &mut out, batch);
            assert_eq!(out, whole, "batch {batch}");
            assert!(most <= batch + 150, "batch {batch}: {most} words at once");
        }
    }

    fn check(cases: &[(&str, &str)]) {
        let engine = Normalizer::english();
        for (said, want) in cases {
            assert_eq!(engine.normalize(said), *want, "{said}");
        }
    }

    #[test]
    fn military_hundreds_keep_their_zeros() {
        check(&[
            ("oh eight hundred hours", "0800 hours"),
            ("zero six hundred", "0600"),
            ("meet at oh nine hundred", "meet at 0900"),
            ("twenty one hundred hours", "2100 hours"),
            ("twenty hundred hours", "2000 hours"),
            ("twenty three hundred", "2300"),
            ("sixteen hundred hours", "1600 hours"),
            ("twenty one hundred people", "2,100 people"),
        ]);
    }

    #[test]
    fn twenty_four_hour_times_read_as_clock_times() {
        check(&[
            ("oh seven thirty", "07:30"),
            ("oh seven fifteen hours", "07:15 hours"),
            ("at oh seven thirty", "at 07:30"),
            ("oh nine oh five", "09:05"),
            ("at twenty three forty five", "at 23:45"),
            ("twenty three forty five hours", "23:45 hours"),
            ("fifteen thirty hours", "15:30 hours"),
            ("at twenty two oh five", "at 22:05"),
        ]);
    }

    #[test]
    fn twenty_four_hour_forms_leave_other_readings_alone() {
        check(&[
            ("twenty twenty", "2020"),
            ("in twenty twenty five", "in 2025"),
            ("twenty five hours", "25 hours"),
            ("three hours", "3 hours"),
            ("it took twenty three hours", "it took 23 hours"),
        ]);
    }

    #[test]
    fn ordinals_after_hundreds_take_an_and() {
        let engine = Normalizer::english();
        for (said, want) in [
            ("one hundred and first", "101st"),
            ("two hundred and third", "203rd"),
            ("one hundred and twenty first", "121st"),
            ("a hundred and second", "102nd"),
            ("the one hundred and fifth place", "the 105th place"),
            ("one thousand and first", "1,001st"),
            ("one thousand one hundred and first", "1,101st"),
        ] {
            assert_eq!(engine.normalize(said), want, "{said}");
        }
    }

    #[test]
    fn hyphenated_numbers_read_before_punctuation() {
        let engine = Normalizer::english();
        for (said, want) in [
            ("She is twenty-one.", "She is 21."),
            ("twenty-one, twenty-two, twenty-three", "21, 22, 23"),
            ("(twenty-one) dogs", "(21) dogs"),
            ("count to twenty-one, then thirty", "count to 21, then 30"),
            ("ninety-nine?", "99?"),
        ] {
            assert_eq!(engine.normalize(said), want, "{said}");
        }
    }

    #[test]
    fn dense_long_lines_format_in_bounded_batches_with_the_same_output() {
        let engine = Normalizer::english();
        let dense = "we paid twenty five dollars, then thirty people came at five thirty pm. it was one hundred and twenty degrees on May third, so room four oh two filled by six and the other one left at ten to four.";
        let line = [dense].repeat(60).join(" ");
        let mut whole = String::new();
        format_line_in(&engine, &line, &mut whole, usize::MAX);
        for batch in [16, 64, 300] {
            let mut out = String::new();
            let most = format_line_in(&engine, &line, &mut out, batch);
            assert_eq!(out, whole, "batch {batch}");
            assert!(most <= batch + 80, "batch {batch}: {most} words at once");
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
        assert_eq!(split(&numeric), vec![raw(0, 5), fmt(5, 10)]);
    }

    #[test]
    fn a_run_window_starts_at_the_word_before_it_and_holds_runs_nearby() {
        let mut numeric = [false; 10];
        numeric[4] = true;
        numeric[6..9].fill(true);
        assert_eq!(split(&numeric), vec![raw(0, 3), fmt(3, 10)]);
    }

    #[test]
    fn cut_between_words_stays_at_the_limit() {
        assert_eq!(split(&[false; 10]), vec![raw(0, 8), raw(8, 10)]);
    }

    #[test]
    fn number_of_exactly_eight_tokens_is_formatted() {
        assert_eq!(split(&[true; 8]), vec![fmt(0, 8)]);
    }

    #[test]
    fn number_longer_than_a_window_is_left_raw_whole() {
        let mut numeric = [false; 13];
        numeric[2..12].fill(true);
        assert_eq!(
            split(&numeric),
            vec![raw(0, 1), fmt(1, 2), raw(2, 12), raw(12, 13)]
        );
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
    fn long_run_splits_after_every_group_scale() {
        let scales = [1, 6, 9];
        let ws = windows_scaled(14, |_| true, |i| scales.contains(&i));
        assert_eq!(ws, vec![fmt(0, 2), join(2, 7), join(7, 10), join(10, 14)]);
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
