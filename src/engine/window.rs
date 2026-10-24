use crate::builder::english::{
    ROLE_DIGIT, ROLE_HUNDRED, ROLE_MSCALE, ROLE_SCALE, ROLE_TEEN, ROLE_TENS,
};
use crate::engine::Engine;
use crate::tables::{Tables, WORDS_FIRST_WORDS};
use crate::types::{CLASS_UNKNOWN, MAX_TOKENS, SpanReceipt};

const LINE_OUT: usize = 512;
const LEAD_PUNCT: &[char] = &['"', '\'', '(', '[', '{', '\u{201C}', '\u{2018}', '\u{00AB}'];
const TRAIL_PUNCT: &[char] = &[
    '.', ',', ';', ':', '!', '?', '"', '\'', ')', ']', '}', '\u{201D}', '\u{2019}', '\u{00BB}',
    '\u{2026}', '\u{2013}', '\u{2014}',
];

fn trail_punct(c: char) -> bool {
    TRAIL_PUNCT.contains(&c)
}

struct Word<'a> {
    lead: &'a str,
    core: &'a str,
    trail: &'a str,
}

fn split_word<'a>(token: &'a str, known: impl Fn(&[u8]) -> bool) -> Word<'a> {
    let rest = token.trim_start_matches(LEAD_PUNCT);
    let start = token.len() - rest.len();
    let end = start + rest.trim_end_matches(trail_punct).len();
    if (start == 0 && end == token.len()) || start == end || !known(&token.as_bytes()[start..end]) {
        return Word {
            lead: "",
            core: token,
            trail: "",
        };
    }
    Word {
        lead: &token[..start],
        core: &token[start..end],
        trail: &token[end..],
    }
}

fn meridiem(letter: &str, next: &str) -> Option<(&'static str, usize)> {
    let m = next.trim_end_matches(trail_punct);
    (letter.eq_ignore_ascii_case("a") && m.eq_ignore_ascii_case("m")).then_some(("am", m.len()))
}

fn join_meridiem<'a>(words: Vec<Word<'a>>, numeric: impl Fn(&str) -> bool) -> Vec<Word<'a>> {
    let mut out: Vec<Word<'a>> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let w = &words[i];
        let after_number = out
            .last()
            .is_some_and(|p| p.trail.is_empty() && numeric(p.core));
        let joined = (after_number && w.lead.is_empty() && w.trail.is_empty())
            .then(|| words.get(i + 1))
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
                out.push(Word {
                    lead: "",
                    core,
                    trail,
                });
                i += 2;
            }
            None => {
                out.push(Word {
                    lead: w.lead,
                    core: w.core,
                    trail: w.trail,
                });
                i += 1;
            }
        }
    }
    out
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

fn join_zone<'a>(words: Vec<Word<'a>>, numeric: impl Fn(&str) -> bool) -> Vec<Word<'a>> {
    let mut out: Vec<Word<'a>> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let after_time = out.last().is_some_and(|p| {
            p.trail.is_empty()
                && (p.core == "am" || p.core == "pm" || p.core == "m" || numeric(p.core))
        });
        let joined = (after_time && i + 3 <= words.len())
            .then(|| &words[i..i + 3])
            .filter(|w| {
                w.iter().all(|x| x.lead.is_empty())
                    && w[0].trail.is_empty()
                    && w[1].trail.is_empty()
            })
            .and_then(|w| zone([w[0].core, w[1].core, w[2].core]).map(|(z, len)| (z, &w[2], len)));
        match joined {
            Some((core, last, len)) => {
                let trail = if last.trail.is_empty() {
                    &last.core[len..]
                } else {
                    last.trail
                };
                out.push(Word {
                    lead: "",
                    core,
                    trail,
                });
                i += 3;
            }
            None => {
                let w = &words[i];
                out.push(Word {
                    lead: w.lead,
                    core: w.core,
                    trail: w.trail,
                });
                i += 1;
            }
        }
    }
    out
}

fn drop_twelve(words: Vec<Word<'_>>) -> Vec<Word<'_>> {
    let noon = |w: &Word| {
        let b = bare(w.core);
        w.lead.is_empty() && (b.eq_ignore_ascii_case("noon") || b.eq_ignore_ascii_case("midnight"))
    };
    let mut out = Vec::with_capacity(words.len());
    for (i, w) in words.iter().enumerate() {
        let twelve = w.core.eq_ignore_ascii_case("twelve") && w.trail.is_empty();
        if !(twelve && words.get(i + 1).is_some_and(noon)) {
            out.push(Word {
                lead: w.lead,
                core: w.core,
                trail: w.trail,
            });
        }
    }
    out
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
    core.trim_end_matches(trail_punct)
}

fn guarded(
    words: &[Word],
    value: impl Fn(&str) -> bool,
    counting: impl Fn(&str) -> bool,
    affix: impl Fn(&str) -> bool,
) -> Vec<bool> {
    let n = words.len();
    let num = |i: usize| value(words[i].core);
    let filler = |i: usize| {
        FILLERS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(bare(words[i].core)))
    };
    let digits = |i: usize| {
        let b = bare(words[i].core);
        !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit())
    };
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
        if let Some(end) = mixed_fraction(words, i) {
            let j = if i > 0 && !num(i - 1) && affix(words[i - 1].core) {
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

pub fn format_line(engine: &Engine, line: &str, out: &mut String) {
    let t = engine.tables();
    let known = |core: &[u8]| t.lexicon.lookup(core).class != CLASS_UNKNOWN;
    let numeric = |core: &str| t.role_of[t.lexicon.lookup(core.as_bytes()).class as usize] != 0;
    let value = |core: &str| {
        let role = t.role_of[t.lexicon.lookup(core.as_bytes()).class as usize] & 0x0F;
        role != 0 && (t.suffix_roles | t.prefix_roles) >> role & 1 == 0
    };
    let words: Vec<Word> = line
        .split_whitespace()
        .map(|tok| split_word(tok, known))
        .collect();
    let words = join_meridiem(words, numeric);
    let words = join_zone(words, numeric);
    let words = drop_twelve(words);
    let counting = |core: &str| {
        let role = t.role_of[t.lexicon.lookup(core.as_bytes()).class as usize] & 0x0F;
        COUNTING_ROLES >> role & 1 == 1
    };
    let affix = |core: &str| numeric(core) && !value(core);
    let keep = guarded(&words, value, counting, affix);
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
                let raw: Vec<&str> = words[start..=i].iter().map(|w| w.core).collect();
                out.push_str(&raw.join(" "));
            } else {
                format_segment(engine, &words[start..=i], out);
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

fn render(engine: &Engine, bytes: &[&[u8]], w: &Window) -> Option<String> {
    if w.raw {
        return None;
    }
    let mut buf = [0u8; LINE_OUT];
    let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
    let done = engine
        .format(&bytes[w.start..w.end], &mut buf, &mut receipts)
        .ok()?;
    core::str::from_utf8(&buf[..done.len])
        .ok()
        .map(str::to_owned)
}

fn render_run(engine: &Engine, bytes: &[&[u8]], words: &[Word], run: &[Window]) -> Option<String> {
    let mut pieces = Vec::with_capacity(run.len());
    for (i, w) in run.iter().enumerate() {
        let digits = piece_digits(&render(engine, bytes, w)?)?;
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

fn format_segment(engine: &Engine, words: &[Word], out: &mut String) {
    let t = engine.tables();
    if let [only] = words
        && words_first(t, only.core)
    {
        out.push_str(only.core);
        return;
    }
    let bytes: Vec<&[u8]> = words.iter().map(|w| w.core.as_bytes()).collect();
    let numeric = |i: usize| t.role_of[t.lexicon.lookup(bytes[i]).class as usize] != 0;
    let ws = windows_scaled(bytes.len(), numeric, |i| scale_exp(words[i].core) != 0);
    let mut i = 0;
    while i < ws.len() {
        let mut j = i + 1;
        while j < ws.len() && ws[j].join {
            j += 1;
        }
        if i > 0 {
            out.push(' ');
        }
        let formatted = if j - i > 1 {
            render_run(engine, &bytes, words, &ws[i..j])
        } else {
            render(engine, &bytes, &ws[i])
        };
        match formatted {
            Some(text) => out.push_str(&text),
            None => {
                let raw: Vec<&str> = words[ws[i].start..ws[j - 1].end]
                    .iter()
                    .map(|w| w.core)
                    .collect();
                out.push_str(&raw.join(" "));
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
    let run_end = |mut i: usize| {
        while i < n && numeric(i) {
            i += 1;
        }
        i
    };
    let mut out = Vec::new();
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
                end = cut;
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn cut_moves_before_a_number_that_crosses_the_limit() {
        let mut numeric = [false; 10];
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
