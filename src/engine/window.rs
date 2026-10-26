use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};

use crate::engine::Normalizer;
use crate::tables::{Tables, WORDS_FIRST_WORDS};
use crate::types::{
    CLASS_UNKNOWN, ClassId, MAX_TOKENS, ROLE_DIGIT, ROLE_HUNDRED, ROLE_MSCALE, ROLE_SCALE,
    ROLE_TEEN, ROLE_TENS, Setting,
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
    numeric: bool,
    value: bool,
    filler: bool,
    digits: bool,
}

fn tag_class<'a>(
    t: &Tables,
    class: ClassId,
    lead: &'a str,
    core: &'a str,
    trail: &'a str,
) -> Word<'a> {
    let full = t.role_of[class as usize];
    let role = full & 0x0F;
    let b = bare(core);
    Word {
        lead,
        core,
        trail,
        numeric: full != 0,
        value: role != 0 && (t.suffix_roles | t.prefix_roles) >> role & 1 == 0,
        filler: FILLERS.iter().any(|f| f.eq_ignore_ascii_case(b)),
        digits: !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()),
    }
}

fn tag<'a>(t: &Tables, lead: &'a str, core: &'a str, trail: &'a str) -> Word<'a> {
    tag_class(
        t,
        t.lexicon.lookup(core.as_bytes()).class,
        lead,
        core,
        trail,
    )
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
    let class = t.lexicon.lookup(&token.as_bytes()[start..end]).class;
    if class == CLASS_UNKNOWN {
        return tag(t, "", token, "");
    }
    tag_class(t, class, &token[..start], &token[start..end], &token[end..])
}

fn meridiem(letter: &str, next: &str) -> Option<(&'static str, usize)> {
    let m = next.trim_end_matches(trail_punct);
    (letter.eq_ignore_ascii_case("a") && m.eq_ignore_ascii_case("m")).then_some(("am", m.len()))
}

fn join_meridiem(words: &mut Vec<Word<'_>>, t: &Tables) {
    let mut put = 0;
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let after_number = put > 0 && words[put - 1].trail.is_empty() && words[put - 1].numeric;
        let joined = (after_number && w.lead.is_empty() && w.trail.is_empty())
            .then(|| words.get(i + 1).copied())
            .flatten()
            .filter(|n| n.lead.is_empty())
            .and_then(|n| meridiem(w.core, n.core).map(|(j, len)| (j, n, len)));
        match joined {
            Some((core, next, len)) => {
                let trail = if next.trail.is_empty() {
                    &next.core[len..]
                } else {
                    next.trail
                };
                words[put] = tag(t, "", core, trail);
                i += 2;
            }
            None => {
                words[put] = w;
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
            let p = words[put - 1];
            p.trail.is_empty() && (p.core == "am" || p.core == "pm" || p.core == "m" || p.numeric)
        };
        let joined = (after_time && i + 3 <= words.len())
            .then(|| [words[i], words[i + 1], words[i + 2]])
            .filter(|w| {
                w.iter().all(|x| x.lead.is_empty())
                    && w[0].trail.is_empty()
                    && w[1].trail.is_empty()
            })
            .and_then(|w| zone([w[0].core, w[1].core, w[2].core]).map(|(z, len)| (z, w[2], len)));
        match joined {
            Some((core, last, len)) => {
                let trail = if last.trail.is_empty() {
                    &last.core[len..]
                } else {
                    last.trail
                };
                words[put] = tag(t, "", core, trail);
                i += 3;
            }
            None => {
                words[put] = words[i];
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
        let w = words[i];
        let twelve = w.core.eq_ignore_ascii_case("twelve") && w.trail.is_empty();
        if !(twelve && words.get(i + 1).is_some_and(noon)) {
            words[put] = w;
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

fn guarded(words: &[Word], counting: impl Fn(&str) -> bool) -> Vec<bool> {
    let n = words.len();
    let num = |i: usize| words[i].value;
    let filler = |i: usize| words[i].filler;
    let digits = |i: usize| words[i].digits;
    let plural = |i: usize| {
        let b = bare(words[i].core);
        let stem = b.strip_suffix("'s").or_else(|| b.strip_suffix('s'));
        !num(i) && stem.is_some_and(|s| !s.is_empty() && counting(s))
    };
    let mut keep = vec![false; n];
    let mut mark = |from: usize, to: usize| keep[from..=to].fill(true);
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
            if let (Some(j), Some(k)) = (j, k)
                && num(j)
                && num(k)
            {
                mark(run_left(j, &num_or_filler), run_right(k, &num_or_filler));
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
            let j = if i > 0 && !num(i - 1) && words[i - 1].numeric {
                i - 1
            } else {
                i
            };
            if j > 0 && num(j - 1) {
                mark(run_left(j - 1, &num), end);
            }
        }
    }
    keep
}

const FRACTION_WORDS: [&str; 6] = ["half", "quarter", "third", "thirds", "quarters", "halves"];

const SMALL: [&str; 9] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];
const ONE_AFTER_WORD: [&str; 5] = ["no", "which", "this", "that", "the"];
const ONE_BEFORE_WORD: [&str; 7] = ["of", "more", "another", "at", "way", "thing", "sec"];
const GROUP: [&str; 3] = ["us", "them", "you"];
const IDIOMS: [&[&str]; 3] = [
    &["six", "of", "one"],
    &["cloud", "nine"],
    &["two", "heads", "are", "better", "than", "one"],
];

/// A lone "one" to "nine" used as a word, not a count ("no one", "one of
/// them", "the two of us", "two thirds", "cloud nine"). Returns the span that
/// stays as spoken. Context never crosses punctuation.
fn word_sense(words: &[Word], i: usize) -> Option<(usize, usize)> {
    let eq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let any = |a: &str, set: &[&str]| set.iter().any(|s| eq(a, s));
    let me = words[i].core;
    if !(3..=5).contains(&me.len()) || !any(me, &SMALL) {
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
    let numeric = |d: isize| at(d).is_some_and(|w| w.numeric);
    if numeric(-1) {
        return None;
    }
    for idiom in IDIOMS {
        for (p, _) in idiom.iter().enumerate().filter(|(_, w)| eq(me, w)) {
            let start = -(p as isize);
            if idiom.iter().zip(start..).all(|(w, d)| eq(text(d), w)) {
                return Some((i - p, i - p + idiom.len() - 1));
            }
        }
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
    if i == 0 || h >= words.len() || !words[h].value || !plain(i) || !is(i, &RELATIVE) {
        return None;
    }
    if !words[h].lead.is_empty() || !is(h, &HOURS) {
        return None;
    }
    if h + 1 < words.len() && words[h].trail.is_empty() && words[h + 1].numeric {
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
    if from > 0 && words[from - 1].value && words[from - 1].trail.is_empty() {
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
    let article = at(i + 1)?;
    let fraction = at(i + 2)?;
    (at(i)?.eq_ignore_ascii_case("and")
        && (article.eq_ignore_ascii_case("a") || article.eq_ignore_ascii_case("an"))
        && FRACTION_WORDS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(fraction)))
    .then_some(i + 2)
}

pub fn format_line(engine: &Normalizer, line: &str, out: &mut String) {
    let t = engine.tables();
    let mut words: Vec<Word> = Vec::with_capacity(line.split_whitespace().count());
    words.extend(line.split_whitespace().map(|tok| split_word(tok, t)));
    if engine.is_on(Setting::Letters) {
        join_meridiem(&mut words, t);
        join_zone(&mut words, t);
    }
    if engine.is_on(Setting::Noon) {
        drop_twelve(&mut words);
    }
    let counting = |core: &str| {
        let role = t.role_of[t.lexicon.lookup(core.as_bytes()).class as usize] & 0x0F;
        COUNTING_ROLES >> role & 1 == 1
    };
    let keep = guarded(&words, counting);
    let mut scratch = Scratch {
        windows: Vec::new(),
        buf: [0u8; LINE_OUT],
    };
    let mut start = 0;
    for i in 0..words.len() {
        let last = i + 1 == words.len()
            || !words[i].trail.is_empty()
            || !words[i + 1].lead.is_empty()
            || keep[i] != keep[i + 1];
        if last {
            if start > 0 {
                out.push(' ');
            }
            out.push_str(words[start].lead);
            if keep[i] {
                push_raw(out, &words[start..=i]);
            } else {
                format_segment(engine, &words[start..=i], out, &mut scratch);
            }
            push_trail(out, words[i].trail);
            start = i + 1;
        }
    }
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
    windows: Vec<Window>,
    buf: [u8; LINE_OUT],
}

fn push_raw(out: &mut String, words: &[Word]) {
    for (k, w) in words.iter().enumerate() {
        if k > 0 {
            out.push(' ');
        }
        out.push_str(w.core);
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
    for (token, word) in tokens.iter_mut().zip(span) {
        *token = word.core.as_bytes();
    }
    let done = engine.format(&tokens[..span.len()], buf).ok()?;
    core::str::from_utf8(&buf[..done.len]).ok()
}

fn render_run(
    engine: &Normalizer,
    words: &[Word],
    run: &[Window],
    buf: &mut [u8; LINE_OUT],
) -> Option<String> {
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
    stitch(&pieces)
}

fn words_first(t: &Tables, core: &str) -> bool {
    let c = t.lexicon.lookup(core.as_bytes()).canonical as usize;
    (t.words_first[(c >> 6) % WORDS_FIRST_WORDS] >> (c & 63)) & 1 == 1
}

fn digit_word(engine: &Normalizer, core: &str) -> bool {
    let t = engine.tables();
    let info = t.lexicon.lookup(core.as_bytes());
    engine.is_on(Setting::SingleDigits)
        && engine.is_digit_word(info.canonical, t.role_of[info.class as usize] & 0x0F)
}

fn format_segment(engine: &Normalizer, words: &[Word], out: &mut String, scratch: &mut Scratch) {
    let t = engine.tables();
    if let [only] = words
        && words_first(t, only.core)
        && !digit_word(engine, only.core)
    {
        out.push_str(only.core);
        return;
    }
    let Scratch { windows: ws, buf } = scratch;
    windows_scaled_into(
        ws,
        words.len(),
        |i| words[i].numeric,
        |i| scale_exp(words[i].core) != 0,
    );
    let mut i = 0;
    while i < ws.len() {
        let mut j = i + 1;
        while j < ws.len() && ws[j].join {
            j += 1;
        }
        if i > 0 {
            out.push(' ');
        }
        if j - i > 1 {
            match render_run(engine, words, &ws[i..j], buf) {
                Some(text) => out.push_str(&text),
                None => push_raw(out, &words[ws[i].start..ws[j - 1].end]),
            }
        } else {
            match render(engine, words, &ws[i], buf) {
                Some(text) => out.push_str(text),
                None => push_raw(out, &words[ws[i].start..ws[i].end]),
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

fn split_run(
    start: usize,
    end: usize,
    group_scale: &impl Fn(usize) -> bool,
) -> Option<Vec<Window>> {
    let mut pieces = Vec::new();
    let mut pos = start;
    while end - pos > MAX_TOKENS {
        let cut = (pos..pos + MAX_TOKENS).rev().find(|&j| group_scale(j))?;
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
    let (digits, zeros) = match text.split_once(' ') {
        None => (text, 0),
        Some((d, "thousand")) => (d, 3),
        Some((d, "million")) => (d, 6),
        Some((d, "billion")) => (d, 9),
        Some((d, "trillion")) => (d, 12),
        Some(_) => return None,
    };
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

pub fn windows(n: usize, numeric: impl Fn(usize) -> bool) -> Vec<Window> {
    windows_scaled(n, numeric, |_| false)
}

pub fn windows_scaled(
    n: usize,
    numeric: impl Fn(usize) -> bool,
    group_scale: impl Fn(usize) -> bool,
) -> Vec<Window> {
    let mut out = Vec::new();
    windows_scaled_into(&mut out, n, numeric, group_scale);
    out
}

fn windows_scaled_into(
    out: &mut Vec<Window>,
    n: usize,
    numeric: impl Fn(usize) -> bool,
    group_scale: impl Fn(usize) -> bool,
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
                match split_run(start, r, &group_scale) {
                    Some(pieces) => out.extend(pieces),
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
        if end < n && numeric(end - 1) && numeric(end) {
            let mut cut = end - 1;
            while cut > start && numeric(cut - 1) {
                cut -= 1;
            }
            if cut > start {
                // Start the next window on the word before the run, so the run
                // keeps its left context ("room", "at"), unless that word is the
                // right context of a number ending this window.
                end = if cut - 1 > start && !numeric(cut - 2) {
                    cut - 1
                } else {
                    cut
                };
            }
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
