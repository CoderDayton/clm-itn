use clm_itn::types::MAX_TOKENS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Category {
    Filler,
    Cardinal,
    Time,
    Identifier,
    Decimal,
    Ordinal,
}

const UNITS: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];
const TEENS: [&str; 10] = [
    "ten",
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
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const ORD_UNITS: [&str; 10] = [
    "", "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
];
const ORD_TEENS: [&str; 10] = [
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
];
const ORD_TENS: [&str; 10] = [
    "",
    "",
    "twentieth",
    "thirtieth",
    "fortieth",
    "fiftieth",
    "sixtieth",
    "seventieth",
    "eightieth",
    "ninetieth",
];
const ID_PREFIXES: [&str; 4] = ["room", "flight", "number", "code"];
const FILLERS: [&str; 20] = [
    "call", "me", "meeting", "xylo", "please", "set", "an", "alarm", "for", "tomorrow", "book",
    "remind", "to", "zorp", "and", "is", "quux", "send", "it", "now",
];

struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }

    fn below(&mut self, n: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x % n as u64) as usize
    }

    fn pick<'a>(&mut self, words: &[&'a str]) -> &'a str {
        words[self.below(words.len())]
    }
}

#[derive(Default)]
struct Phrase {
    words: [&'static str; MAX_TOKENS],
    len: usize,
}

impl Phrase {
    fn push(&mut self, w: &'static str) {
        self.words[self.len] = w;
        self.len += 1;
    }

    fn below_100(&mut self, n: usize) {
        match n {
            0..=9 => self.push(UNITS[n]),
            10..=19 => self.push(TEENS[n - 10]),
            _ => {
                self.push(TENS[n / 10]);
                if !n.is_multiple_of(10) {
                    self.push(UNITS[n % 10]);
                }
            }
        }
    }

    fn cardinal(&mut self, r: &mut XorShift) {
        match r.below(3) {
            0 => self.below_100(1 + r.below(99)),
            1 => {
                self.push(UNITS[1 + r.below(9)]);
                self.push("hundred");
                if r.below(4) != 0 {
                    self.below_100(1 + r.below(99));
                }
            }
            _ => {
                self.push(UNITS[1 + r.below(9)]);
                self.push("thousand");
                match r.below(3) {
                    0 => {}
                    1 => self.push(UNITS[1 + r.below(9)]),
                    _ => {
                        self.push(UNITS[1 + r.below(9)]);
                        self.push("hundred");
                        self.below_100(1 + r.below(99));
                    }
                }
            }
        }
    }

    fn time(&mut self, r: &mut XorShift) {
        if r.below(2) == 0 {
            self.push("at");
        }
        self.below_100(1 + r.below(12));
        match r.below(4) {
            0 => {
                self.push("oh");
                self.push(UNITS[1 + r.below(9)]);
            }
            1 => self.push(TEENS[r.below(10)]),
            2 => self.below_100(20 + r.below(40)),
            _ => {}
        }
        match r.below(3) {
            0 => self.push("am"),
            1 => self.push("pm"),
            _ => {}
        }
    }

    fn identifier(&mut self, r: &mut XorShift) {
        self.push(r.pick(&ID_PREFIXES));
        for _ in 0..2 + r.below(3) {
            match r.below(10) {
                0 => self.push(if r.below(2) == 0 { "oh" } else { "zero" }),
                d => self.push(UNITS[d]),
            }
        }
    }

    fn decimal(&mut self, r: &mut XorShift) {
        self.below_100(r.below(100));
        self.push("point");
        for _ in 0..1 + r.below(3) {
            self.push(UNITS[r.below(10)]);
        }
    }

    fn ordinal(&mut self, r: &mut XorShift) {
        self.push("the");
        let n = 1 + r.below(99);
        match n {
            1..=9 => self.push(ORD_UNITS[n]),
            10..=19 => self.push(ORD_TEENS[n - 10]),
            _ if n.is_multiple_of(10) => self.push(ORD_TENS[n / 10]),
            _ => {
                self.push(TENS[n / 10]);
                self.push(ORD_UNITS[n % 10]);
            }
        }
    }
}

pub struct Corpus {
    arena: Vec<u8>,
    tokens: Vec<(u32, u8)>,
    utts: Vec<(u32, u8)>,
    cats: Vec<Category>,
}

impl Corpus {
    pub fn generate(n: usize, seed: u64) -> Corpus {
        let mut r = XorShift::new(seed);
        let mut c = Corpus {
            arena: Vec::with_capacity(n * 32),
            tokens: Vec::with_capacity(n * 5),
            utts: Vec::with_capacity(n),
            cats: Vec::with_capacity(n),
        };
        for _ in 0..n {
            let mut p = Phrase::default();
            let cat = if r.below(2) == 0 {
                Category::Filler
            } else {
                match r.below(5) {
                    0 => {
                        p.cardinal(&mut r);
                        Category::Cardinal
                    }
                    1 => {
                        p.time(&mut r);
                        Category::Time
                    }
                    2 => {
                        p.identifier(&mut r);
                        Category::Identifier
                    }
                    3 => {
                        p.decimal(&mut r);
                        Category::Decimal
                    }
                    _ => {
                        p.ordinal(&mut r);
                        Category::Ordinal
                    }
                }
            };
            let min = p.len.max(1);
            let total = min + r.below(MAX_TOKENS - min + 1);
            let fill = total - p.len;
            let before = r.below(fill + 1);
            let first = c.tokens.len() as u32;
            for _ in 0..before {
                c.push_token(r.pick(&FILLERS));
            }
            for &w in &p.words[..p.len] {
                c.push_token(w);
            }
            for _ in before..fill {
                c.push_token(r.pick(&FILLERS));
            }
            c.utts.push((first, total as u8));
            c.cats.push(cat);
        }
        c
    }

    fn push_token(&mut self, w: &str) {
        self.tokens.push((self.arena.len() as u32, w.len() as u8));
        self.arena.extend_from_slice(w.as_bytes());
    }

    pub fn len(&self) -> usize {
        self.utts.len()
    }

    pub fn category(&self, i: usize) -> Category {
        self.cats[i]
    }

    pub fn utterance<'a>(&'a self, i: usize, buf: &mut [&'a [u8]; MAX_TOKENS]) -> usize {
        let (first, count) = self.utts[i];
        let toks = &self.tokens[first as usize..first as usize + count as usize];
        for (slot, &(start, len)) in buf.iter_mut().zip(toks) {
            *slot = &self.arena[start as usize..start as usize + len as usize];
        }
        count as usize
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::thread_allocations;

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
}
