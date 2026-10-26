use super::alloc::thread_allocations;
use crate::corpus::*;
use clm_itn::types::MAX_TOKENS;

const N: usize = 10_000;

fn collect(c: &Corpus) -> Vec<Vec<Vec<u8>>> {
    let mut buf: [&[u8]; MAX_TOKENS] = [&[]; MAX_TOKENS];
    (0..c.len())
        .map(|i| {
            let n = c.utterance(i, &mut buf);
            buf[..n].iter().map(|t| t.to_vec()).collect()
        })
        .collect()
}

#[test]
fn same_seed_same_corpus_different_seed_differs() {
    let a = collect(&Corpus::generate(N, 7));
    let b = collect(&Corpus::generate(N, 7));
    let c = collect(&Corpus::generate(N, 8));
    assert_eq!(a.len(), N);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn every_utterance_has_one_to_eight_lowercase_ascii_tokens() {
    for u in collect(&Corpus::generate(N, 1)) {
        assert!((1..=MAX_TOKENS).contains(&u.len()), "{} tokens", u.len());
        for t in u {
            assert!(!t.is_empty());
            assert!(t.iter().all(u8::is_ascii_lowercase), "{t:?}");
        }
    }
}

#[test]
fn mix_contains_each_category_and_about_half_numeric() {
    let c = Corpus::generate(N, 2);
    let mut counts = [0usize; 6];
    for i in 0..c.len() {
        counts[c.category(i) as usize] += 1;
    }
    assert!(counts.iter().all(|&k| k > 0), "{counts:?}");
    let numeric = N - counts[Category::Filler as usize];
    assert!(
        (4_000..=6_000).contains(&numeric),
        "{numeric} numeric of {N}"
    );
}

#[test]
fn categories_carry_their_marker_words() {
    let c = Corpus::generate(N, 3);
    let words = collect(&c);
    for (i, u) in words.iter().enumerate() {
        let has = |w: &[u8]| u.iter().any(|t| t == w);
        match c.category(i) {
            Category::Decimal => assert!(has(b"point"), "{i}"),
            Category::Ordinal => assert!(has(b"the"), "{i}"),
            Category::Identifier => assert!(
                has(b"room") || has(b"flight") || has(b"number") || has(b"code"),
                "{i}"
            ),
            _ => {}
        }
    }
}

#[test]
fn times_occur_with_and_without_meridiem() {
    let c = Corpus::generate(N, 4);
    let words = collect(&c);
    let (mut with, mut without) = (0, 0);
    for (i, u) in words.iter().enumerate() {
        if c.category(i) == Category::Time {
            if u.iter().any(|t| t == b"am" || t == b"pm") {
                with += 1;
            } else {
                without += 1;
            }
        }
    }
    assert!(with > 0 && without > 0, "with={with} without={without}");
}

#[test]
fn utterance_does_not_allocate() {
    let c = Corpus::generate(N, 5);
    let mut buf: [&[u8]; MAX_TOKENS] = [&[]; MAX_TOKENS];
    let control = thread_allocations();
    std::hint::black_box(Vec::<u8>::with_capacity(8));
    assert!(
        thread_allocations() > control,
        "counter must see this thread"
    );

    let before = thread_allocations();
    let mut total = 0;
    for i in 0..c.len() {
        total += c.utterance(i, &mut buf);
    }
    std::hint::black_box(total);
    assert_eq!(thread_allocations(), before);
}
