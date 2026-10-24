use crate::types::{CLASS_BOUNDARY, CLASS_UNKNOWN, ClassId, MAX_TOKENS, ROLE_NONE, Role};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: u8,
    pub len: u8,
    pub core: u8,
    pub lead: u8,
    pub key: u32,
    pub left: ClassId,
    pub right: ClassId,
    pub dropped: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Affixes {
    pub suffix: u16,
    pub prefix: u16,
    pub starts: [u64; 4],
    pub per_stop: [u64; 4],
    pub per_units: [u64; 4],
    pub pair_only: [u64; 4],
    pub connectors: u16,
    pub binds: u16,
}

pub const MAX_SUFFIX: usize = 2;

#[inline(always)]
fn has(mask: u16, role: u32) -> bool {
    (mask >> (role & 0x0F)) & 1 == 1
}

#[inline(always)]
fn starts(bits: &[u64; 4], class: ClassId) -> bool {
    (bits[(class >> 6) as usize & 3] >> (class & 63)) & 1 == 1
}

pub fn segment(
    classes: &[ClassId],
    role_of: &[Role; 256],
    affixes: Affixes,
    known: impl Fn(u32) -> bool,
    out: &mut [Span; MAX_TOKENS],
) -> usize {
    let n = classes.len().min(MAX_TOKENS);
    let classes = &classes[..n];
    let role = |i: usize| role_of[classes[i] as usize];
    let connector = |i: usize| {
        i > 0
            && has(affixes.connectors, u32::from(role(i)))
            && !has(affixes.binds, u32::from(role(i - 1)))
    };
    let mut count = 0;
    let mut i = 0;
    while i < n {
        if role(i) == ROLE_NONE {
            i += 1;
            continue;
        }
        let mut start = i;
        while i < n && role(i) != ROLE_NONE && (i == start || !starts(&affixes.starts, classes[i]))
        {
            i += 1;
        }
        while start + 1 < i && has(affixes.connectors, u32::from(role(start))) {
            start += 1;
        }
        let mut end = i;
        while end > start + 1 && connector(end - 1) {
            end -= 1;
        }
        let mut s = start;
        loop {
            let span = piece(classes, role_of, affixes, s, end);
            let inner = (s + 1..end - 1).filter(|&c| connector(c));
            let cut = if inner.clone().next().is_none() || known(span.key) {
                None
            } else {
                let mut cuts = inner.clone().rev();
                cuts.find(|&c| known(piece(classes, role_of, affixes, s, c).key))
                    .or_else(|| inner.clone().next())
            };
            match cut {
                Some(c) => {
                    out[count] = piece(classes, role_of, affixes, s, c);
                    count += 1;
                    s = c + 1;
                }
                None => {
                    out[count] = span;
                    count += 1;
                    break;
                }
            }
        }
    }
    count
}

fn piece(
    classes: &[ClassId],
    role_of: &[Role; 256],
    affixes: Affixes,
    start: usize,
    i: usize,
) -> Span {
    let n = classes.len();
    let mut key = 0u32;
    for (k, &c) in classes[start..i].iter().enumerate() {
        key |= u32::from(role_of[c as usize] & 0x0F) << (4 * k);
    }
    let mut left = if start == 0 {
        CLASS_BOUNDARY
    } else {
        classes[start - 1]
    };
    let mut right = if i == n { CLASS_BOUNDARY } else { classes[i] };
    let mut core = i - start;
    let mut len = core;
    let mut lead = 0;
    for _ in 0..MAX_SUFFIX {
        if core >= 2 && has(affixes.suffix, key >> (4 * (core - 1))) {
            core -= 1;
            key &= !(0x0F << (4 * core));
            right = classes[start + core];
        }
    }
    let first = classes[start + core.min(len - 1)];
    let suffixes = len - core;
    let per = i < n && starts(&affixes.per_stop, classes[i]);
    let drop = suffixes > 0
        && ((per && starts(&affixes.per_units, first))
            || (suffixes == 1 && starts(&affixes.pair_only, first)));
    if drop {
        len = core;
        right = CLASS_UNKNOWN;
    }
    if core >= 2 && has(affixes.prefix, key) {
        core -= 1;
        key >>= 4;
        left = classes[start];
        lead = 1;
    }
    Span {
        start: start as u8,
        len: len as u8,
        core: core as u8,
        lead,
        key,
        left,
        right,
        dropped: drop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::shape::shape_key;

    const DIGIT: ClassId = 2;
    const TENS: ClassId = 3;
    const WORD: ClassId = 9;
    const DOLLAR: ClassId = 10;
    const MINUS: ClassId = 11;
    const SQUARE: ClassId = 12;
    const MONTH: ClassId = 13;
    const PER: ClassId = 14;
    const DEGREE: ClassId = 15;
    const AND: ClassId = 16;
    const ROLE_AND: Role = 7;
    const ROLE_HUNDRED: Role = 4;
    const HUNDRED: ClassId = 17;
    const ROLE_DOLLAR: Role = 11;
    const ROLE_MINUS: Role = 14;

    fn roles() -> [Role; 256] {
        let mut r = [0u8; 256];
        r[DIGIT as usize] = 1;
        r[TENS as usize] = 3;
        r[DOLLAR as usize] = ROLE_DOLLAR;
        r[MINUS as usize] = ROLE_MINUS;
        r[SQUARE as usize] = ROLE_DOLLAR;
        r[MONTH as usize] = ROLE_MINUS;
        r[DEGREE as usize] = ROLE_DOLLAR;
        r[AND as usize] = ROLE_AND;
        r[HUNDRED as usize] = ROLE_HUNDRED;
        r
    }

    fn run_known(classes: &[ClassId], known: impl Fn(u32) -> bool) -> Vec<Span> {
        let mut out = [Span::default(); MAX_TOKENS];
        let affixes = Affixes {
            suffix: 1 << ROLE_DOLLAR,
            prefix: 1 << ROLE_MINUS,
            starts: [1 << MONTH, 0, 0, 0],
            per_stop: [1 << PER, 0, 0, 0],
            per_units: [1 << SQUARE, 0, 0, 0],
            pair_only: [1 << DEGREE, 0, 0, 0],
            connectors: 1 << ROLE_AND,
            binds: 1 << ROLE_HUNDRED,
        };
        let n = segment(classes, &roles(), affixes, known, &mut out);
        out[..n].to_vec()
    }

    fn run(classes: &[ClassId]) -> Vec<Span> {
        run_known(classes, |_| true)
    }

    #[test]
    fn suffix_affix_leaves_the_key_and_becomes_right_context() {
        let spans = run(&[WORD, DIGIT, TENS, DOLLAR, WORD]);
        assert_eq!(
            spans,
            vec![Span {
                start: 1,
                len: 3,
                core: 2,
                lead: 0,
                key: 0x31,
                left: WORD,
                right: DOLLAR,
                dropped: false
            }]
        );
    }

    #[test]
    fn prefix_affix_leaves_the_key_and_becomes_left_context() {
        let s = run(&[MINUS, DIGIT, TENS])[0];
        assert_eq!((s.start, s.len, s.core, s.key), (0, 3, 2, 0x31));
        assert_eq!((s.left, s.right), (MINUS, CLASS_BOUNDARY));
    }

    #[test]
    fn both_affixes_split_and_short_spans_stay_whole() {
        let s = run(&[MINUS, DIGIT, TENS, DOLLAR])[0];
        assert_eq!(
            (s.len, s.core, s.key, s.left, s.right),
            (4, 2, 0x31, MINUS, DOLLAR)
        );
        let s = run(&[MINUS, DIGIT, DOLLAR])[0];
        assert_eq!(
            (s.len, s.core, s.key, s.left, s.right),
            (3, 1, 0x1, MINUS, DOLLAR)
        );
        let s = run(&[MINUS, DOLLAR])[0];
        assert_eq!((s.len, s.core, s.key), (2, 1, ROLE_MINUS as u32));
        let s = run(&[WORD, DOLLAR])[0];
        assert_eq!(
            (s.len, s.core, s.key, s.right),
            (1, 1, ROLE_DOLLAR as u32, CLASS_BOUNDARY)
        );
    }

    #[test]
    fn up_to_two_trailing_suffix_tokens_leave_the_key() {
        let s = run(&[WORD, DIGIT, TENS, SQUARE, DOLLAR, WORD])[0];
        assert_eq!(
            (s.start, s.len, s.core, s.key, s.left, s.right),
            (1, 4, 2, 0x31, WORD, SQUARE)
        );
        let s = run(&[DIGIT, SQUARE, DOLLAR])[0];
        assert_eq!((s.len, s.core, s.key, s.right), (3, 1, 0x1, SQUARE));
        let s = run(&[DIGIT, DOLLAR, SQUARE, DOLLAR])[0];
        assert_eq!((s.len, s.core, s.right), (4, 2, SQUARE));
        let s = run(&[MINUS, DIGIT, SQUARE, DOLLAR])[0];
        assert_eq!((s.len, s.core, s.left, s.right), (4, 1, MINUS, SQUARE));
        let s = run(&[SQUARE, DOLLAR])[0];
        assert_eq!((s.len, s.core, s.right), (2, 1, DOLLAR));
    }

    #[test]
    fn a_start_class_begins_a_new_span_and_splits_as_a_prefix() {
        let spans = run(&[DIGIT, MONTH, DIGIT, TENS]);
        assert_eq!(spans.len(), 2);
        assert_eq!(
            (spans[0].start, spans[0].len, spans[0].right),
            (0, 1, MONTH)
        );
        let s = spans[1];
        assert_eq!((s.start, s.len, s.core, s.lead, s.key), (1, 3, 2, 1, 0x31));
        assert_eq!(s.left, MONTH);
        let s = run(&[MINUS, DIGIT, MINUS, DIGIT])[0];
        assert_eq!(s.len, 4);
    }

    #[test]
    fn a_unit_before_per_and_a_lone_pair_only_suffix_leave_the_span() {
        let s = run(&[DIGIT, SQUARE, PER, WORD])[0];
        assert_eq!((s.len, s.core, s.key, s.right), (1, 1, 0x1, CLASS_UNKNOWN));
        assert!(s.dropped);
        assert!(!run(&[DIGIT, SQUARE])[0].dropped);
        let s = run(&[DIGIT, DOLLAR, PER, WORD])[0];
        assert_eq!((s.len, s.core, s.right), (2, 1, DOLLAR));
        let s = run(&[DIGIT, DEGREE, WORD])[0];
        assert_eq!((s.len, s.core, s.right), (1, 1, CLASS_UNKNOWN));
        let s = run(&[DIGIT, DEGREE, SQUARE])[0];
        assert_eq!((s.len, s.core, s.right), (3, 1, DEGREE));
    }

    #[test]
    fn a_leading_connector_leaves_the_span() {
        let s = run(&[AND, DIGIT, WORD]);
        assert_eq!(s.len(), 1);
        assert_eq!(
            (s[0].start, s[0].len, s[0].key, s[0].left),
            (1, 1, 0x1, AND)
        );
        let s = run(&[WORD, AND, DIGIT, TENS]);
        assert_eq!((s[0].start, s[0].len, s[0].left), (2, 2, AND));
        let s = run(&[AND]);
        assert_eq!((s.len(), s[0].len), (1, 1));
    }

    #[test]
    fn a_trailing_connector_leaves_the_span_and_an_unknown_run_splits_at_one() {
        let s = run(&[DIGIT, TENS, AND, WORD]);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].len, s[0].key, s[0].right), (2, 0x31, AND));
        let whole = 0x171;
        let s = run_known(&[DIGIT, AND, DIGIT], |k| k == whole);
        assert_eq!((s.len(), s[0].len), (1, 3));
        let s = run_known(&[DIGIT, AND, DIGIT], |k| k == 0x1);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].start, s[0].len, s[0].right), (0, 1, AND));
        assert_eq!((s[1].start, s[1].len, s[1].left), (2, 1, AND));
        let s = run_known(&[DIGIT, AND, DIGIT, AND, TENS], |k| k == 0x171 || k == 0x3);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].len, s[1].start, s[1].len), (3, 4, 1));
        let s = run_known(&[DIGIT, HUNDRED, AND, TENS], |k| k == 0x41);
        assert_eq!((s.len(), s[0].len), (1, 4));
        let s = run(&[DIGIT, HUNDRED, AND, WORD]);
        assert_eq!((s.len(), s[0].len), (1, 3));
    }

    #[test]
    fn span_key_matches_shape_key_of_its_roles() {
        let spans = run(&[WORD, DIGIT, TENS, DIGIT]);
        assert_eq!(spans[0].key, shape_key(&[1, 3, 1]));
    }

    #[test]
    fn single_span_between_words_reads_neighbor_classes() {
        let spans = run(&[WORD, DIGIT, TENS, CLASS_UNKNOWN]);
        assert_eq!(
            spans,
            vec![Span {
                start: 1,
                len: 2,
                core: 2,
                lead: 0,
                key: 0x31,
                left: WORD,
                right: CLASS_UNKNOWN,
                dropped: false
            }]
        );
    }

    #[test]
    fn spans_at_window_edges_see_boundary() {
        let spans = run(&[DIGIT, WORD, TENS]);
        assert_eq!(spans.len(), 2);
        assert_eq!((spans[0].left, spans[0].right), (CLASS_BOUNDARY, WORD));
        assert_eq!((spans[1].left, spans[1].right), (WORD, CLASS_BOUNDARY));
    }

    #[test]
    fn no_numeric_tokens_gives_no_spans() {
        assert!(run(&[WORD, CLASS_UNKNOWN, WORD]).is_empty());
        assert!(run(&[]).is_empty());
    }

    #[test]
    fn full_window_is_one_span() {
        let spans = run(&[DIGIT; 8]);
        assert_eq!(spans.len(), 1);
        assert_eq!(
            (spans[0].start, spans[0].len, spans[0].key),
            (0, 8, 0x1111_1111)
        );
    }
}
