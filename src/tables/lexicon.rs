use crate::tables::Aligned;
use crate::types::{CLASS_UNKNOWN, MAX_KEY_LEN, MAX_TOKENS, TokenInfo};

/// Bucket capacity. A build starts at the fewest buckets, a power of two from
/// `MIN_BUCKETS`, that fill to at most 90% (capped at `BUCKETS`), and doubles
/// until every key places. Lookups touch only the buckets in use.
pub const BUCKETS: usize = 1024;
pub const MIN_BUCKETS: usize = 2;
pub const SLOTS_PER_BUCKET: usize = 4;
pub const SLOTS: usize = BUCKETS * SLOTS_PER_BUCKET;

pub type Key = [u8; 16];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildError {
    InvalidLength {
        index: usize,
    },
    /// The boundary class is no word's class. An unknown-class entry is a word
    /// with no part in the number tables that still has a canonical ID.
    ReservedClass {
        index: usize,
    },
    Duplicate {
        index: usize,
    },
    TooManyEntries,
    NoSeed,
}

#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bucket {
    keys: [Key; SLOTS_PER_BUCKET],
}

impl Bucket {
    pub const EMPTY: Bucket = Bucket {
        keys: [[0; 16]; SLOTS_PER_BUCKET],
    };

    pub const fn from_keys(keys: [Key; SLOTS_PER_BUCKET]) -> Self {
        Self { keys }
    }

    pub fn keys(&self) -> &[Key; SLOTS_PER_BUCKET] {
        &self.keys
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Lexicon {
    buckets: [Bucket; BUCKETS],
    payload: Aligned<[u32; SLOTS]>,
    seed: u64,
    /// Buckets in use are `MIN_BUCKETS << doublings`, so all-zero tables are valid.
    doublings: u32,
}

const MISS: TokenInfo = TokenInfo {
    canonical: 0,
    class: CLASS_UNKNOWN,
};

#[inline(always)]
pub fn fold_key(token: &[u8]) -> Option<Key> {
    if token.is_empty() || token.len() > MAX_KEY_LEN {
        return None;
    }
    let mut key = [0u8; 16];
    for (dst, &src) in key.iter_mut().zip(token) {
        *dst = src.to_ascii_lowercase();
    }
    key[15] = token.len() as u8;
    Some(key)
}

#[inline(always)]
pub fn bucket_pair(key: &Key, seed: u64, buckets: usize) -> (usize, usize) {
    let v = u128::from_le_bytes(*key);
    crate::hash::bucket_pair(v as u64, (v >> 64) as u64, seed, buckets)
}
#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn match_mask(bucket: &Bucket, key: &Key) -> u32 {
    use core::arch::x86_64::{
        __m128i, _mm_cmpeq_epi8, _mm_load_si128, _mm_loadu_si128, _mm_movemask_epi8,
    };
    let mut mask = 0;
    // SAFETY: SSE2 is baseline on x86_64. `key` is 16 readable bytes; each slot is 16 bytes at a
    // 16-byte-aligned offset of a 64-byte-aligned bucket.
    unsafe {
        let k = _mm_loadu_si128(key.as_ptr().cast::<__m128i>());
        for (i, slot) in bucket.keys.iter().enumerate() {
            let s = _mm_load_si128(slot.as_ptr().cast::<__m128i>());
            let eq = _mm_movemask_epi8(_mm_cmpeq_epi8(k, s));
            mask |= u32::from(eq == 0xFFFF) << i;
        }
    }
    mask
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn match_mask(bucket: &Bucket, key: &Key) -> u32 {
    use core::arch::aarch64::{vceqq_u8, vld1q_u8, vminvq_u8};
    let mut mask = 0;
    // SAFETY: NEON is baseline on aarch64. `key` and each slot are 16 readable bytes.
    unsafe {
        let k = vld1q_u8(key.as_ptr());
        for (i, slot) in bucket.keys.iter().enumerate() {
            let s = vld1q_u8(slot.as_ptr());
            mask |= u32::from(vminvq_u8(vceqq_u8(k, s)) == 0xFF) << i;
        }
    }
    mask
}
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline(always)]
fn match_mask(bucket: &Bucket, key: &Key) -> u32 {
    match_mask_scalar(bucket, key)
}

#[cfg(any(test, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
fn match_mask_scalar(bucket: &Bucket, key: &Key) -> u32 {
    let mut mask = 0;
    for (i, slot) in bucket.keys.iter().enumerate() {
        mask |= u32::from(slot == key) << i;
    }
    mask
}

#[inline(always)]
fn prefetch(bucket: &Bucket) {
    #[cfg(target_arch = "x86_64")]
    {
        use core::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
        // SAFETY: prefetch is a hint and never faults; the pointer is a valid reference.
        unsafe { _mm_prefetch::<_MM_HINT_T0>((bucket as *const Bucket).cast::<i8>()) };
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = bucket;
}

#[inline(always)]
fn decode(payload: u32) -> TokenInfo {
    TokenInfo {
        canonical: payload as u16,
        class: (payload >> 16) as u8,
    }
}

#[cfg(not(clm_no_std))]
mod build {
    use super::{
        BUCKETS, Bucket, BuildError, Key, Lexicon, MIN_BUCKETS, SLOTS, SLOTS_PER_BUCKET,
        bucket_pair, fold_key,
    };
    use crate::types::{CLASS_BOUNDARY, CanonicalId, ClassId};

    const EMPTY_SLOT: u32 = u32::MAX;
    const MAX_KICKS: usize = 1024;
    const MAX_SEEDS: usize = 256;
    const SEED_STATE: u64 = 0x434C_414D_2D49_544E;

    fn place_all(keys: &[Key], seed: u64, buckets: usize, slots: &mut [u32]) -> bool {
        slots.fill(EMPTY_SLOT);
        let mut rng = seed ^ SEED_STATE;
        for start in 0..keys.len() {
            let mut cur = start as u32;
            let mut from = usize::MAX;
            let mut placed = false;
            for _ in 0..MAX_KICKS {
                let (b1, b2) = bucket_pair(&keys[cur as usize], seed, buckets);
                let free = [b1, b2].into_iter().find_map(|b| {
                    let base = b * SLOTS_PER_BUCKET;
                    (base..base + SLOTS_PER_BUCKET).find(|&s| slots[s] == EMPTY_SLOT)
                });
                if let Some(s) = free {
                    slots[s] = cur;
                    placed = true;
                    break;
                }
                let r = crate::hash::next_seed(&mut rng);
                let target = match from {
                    f if f == b1 => b2,
                    f if f == b2 => b1,
                    _ if r & 1 == 0 => b1,
                    _ => b2,
                };
                let s = target * SLOTS_PER_BUCKET + (r >> 1) as usize % SLOTS_PER_BUCKET;
                cur = core::mem::replace(&mut slots[s], cur);
                from = target;
            }
            if !placed {
                return false;
            }
        }
        true
    }

    impl Lexicon {
        pub fn build(entries: &[(&[u8], CanonicalId, ClassId)]) -> Result<Lexicon, BuildError> {
            let mut keys = Vec::with_capacity(entries.len());
            let mut seen = std::collections::HashSet::with_capacity(entries.len());
            for (index, &(token, _, class)) in entries.iter().enumerate() {
                let key = fold_key(token).ok_or(BuildError::InvalidLength { index })?;
                if class == CLASS_BOUNDARY {
                    return Err(BuildError::ReservedClass { index });
                }
                if !seen.insert(key) {
                    return Err(BuildError::Duplicate { index });
                }
                keys.push(key);
            }
            if keys.len() > SLOTS {
                return Err(BuildError::TooManyEntries);
            }

            let fit = (keys.len() * 10).div_ceil(9 * SLOTS_PER_BUCKET);
            let mut used = fit.next_power_of_two().clamp(MIN_BUCKETS, BUCKETS);
            loop {
                if let Some(lex) = place(entries, &keys, used) {
                    return Ok(lex);
                }
                if used == BUCKETS {
                    return Err(BuildError::NoSeed);
                }
                used *= 2;
            }
        }
    }

    /// Places every key in the first `used` buckets under the first seed that fits.
    fn place(
        entries: &[(&[u8], CanonicalId, ClassId)],
        keys: &[Key],
        used: usize,
    ) -> Option<Lexicon> {
        let mut slots = vec![EMPTY_SLOT; used * SLOTS_PER_BUCKET];
        let mut state = SEED_STATE;
        for _ in 0..MAX_SEEDS {
            let seed = crate::hash::next_seed(&mut state);
            if !place_all(keys, seed, used, &mut slots) {
                continue;
            }
            let mut buckets = [Bucket::EMPTY; BUCKETS];
            let mut payload = [0u32; SLOTS];
            for (s, &e) in slots.iter().enumerate() {
                if e == EMPTY_SLOT {
                    continue;
                }
                let (_, canonical, class) = entries[e as usize];
                buckets[s / SLOTS_PER_BUCKET].keys[s % SLOTS_PER_BUCKET] = keys[e as usize];
                payload[s] = u32::from(canonical) | (u32::from(class) << 16);
            }
            return Some(Lexicon::from_raw(buckets, payload, seed, used));
        }
        None
    }
}

impl Lexicon {
    pub const fn from_raw(
        buckets: [Bucket; BUCKETS],
        payload: [u32; SLOTS],
        seed: u64,
        buckets_used: usize,
    ) -> Self {
        assert!(buckets_used.is_power_of_two() && buckets_used >= MIN_BUCKETS);
        assert!(buckets_used <= BUCKETS);
        Self {
            buckets,
            payload: Aligned(payload),
            seed,
            doublings: (buckets_used / MIN_BUCKETS).trailing_zeros(),
        }
    }

    /// Buckets the build filled from; the rest stay empty and unread.
    #[inline(always)]
    pub fn buckets_used(&self) -> usize {
        MIN_BUCKETS << self.doublings
    }

    pub fn buckets(&self) -> &[Bucket; BUCKETS] {
        &self.buckets
    }

    pub fn payload(&self) -> &Aligned<[u32; SLOTS]> {
        &self.payload
    }

    pub fn raw_mut(&mut self) -> (&mut [Bucket; BUCKETS], &mut [u32; SLOTS]) {
        (&mut self.buckets, &mut self.payload.0)
    }

    pub fn set_hash(&mut self, seed: u64, buckets_used: usize) {
        assert!(buckets_used.is_power_of_two() && buckets_used >= MIN_BUCKETS);
        assert!(buckets_used <= BUCKETS);
        self.seed = seed;
        self.doublings = (buckets_used / MIN_BUCKETS).trailing_zeros();
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The key's two buckets. Masking by the capacity changes nothing, but lets
    /// the compiler drop the bounds checks a fixed bucket count avoided.
    #[inline(always)]
    fn pair(&self, key: &Key) -> (usize, usize) {
        let (b1, b2) = bucket_pair(key, self.seed, self.buckets_used());
        (b1 & (BUCKETS - 1), b2 & (BUCKETS - 1))
    }

    #[inline(always)]
    fn resolve(&self, key: &Key, (b1, b2): (usize, usize)) -> TokenInfo {
        let m1 = match_mask(&self.buckets[b1], key);
        let m2 = match_mask(&self.buckets[b2], key);
        let slot = if m1 != 0 {
            b1 * SLOTS_PER_BUCKET + m1.trailing_zeros() as usize
        } else if m2 != 0 {
            b2 * SLOTS_PER_BUCKET + m2.trailing_zeros() as usize
        } else {
            return MISS;
        };
        decode(self.payload[slot])
    }

    #[inline]
    pub fn lookup(&self, token: &[u8]) -> TokenInfo {
        match fold_key(token) {
            Some(key) => self.resolve(&key, self.pair(&key)),
            None => MISS,
        }
    }

    #[inline]
    pub fn lookup_window(&self, tokens: &[&[u8]], out: &mut [TokenInfo; MAX_TOKENS]) {
        let mut keys = [[0u8; 16]; MAX_TOKENS];
        let mut pairs = [(0usize, 0usize); MAX_TOKENS];
        let mut valid = [false; MAX_TOKENS];
        let n = tokens.len().min(MAX_TOKENS);
        for (i, token) in tokens[..n].iter().enumerate() {
            if let Some(key) = fold_key(token) {
                let pair = self.pair(&key);
                prefetch(&self.buckets[pair.0]);
                prefetch(&self.buckets[pair.1]);
                keys[i] = key;
                pairs[i] = pair;
                valid[i] = true;
            }
        }
        for (i, slot) in out[..n].iter_mut().enumerate() {
            *slot = if valid[i] {
                self.resolve(&keys[i], pairs[i])
            } else {
                MISS
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::next_seed;
    use crate::types::{CanonicalId, ClassId};

    fn random_words(n: usize, seed: u64) -> Vec<Vec<u8>> {
        let mut state = seed;
        let mut seen = std::collections::HashSet::new();
        let mut words = Vec::with_capacity(n);
        while words.len() < n {
            let r = next_seed(&mut state);
            let len = 1 + (r % 15) as usize;
            let word: Vec<u8> = (0..len)
                .map(|i| b'a' + ((r >> (4 + 3 * i)) ^ next_seed(&mut state)) as u8 % 26)
                .collect();
            if seen.insert(word.clone()) {
                words.push(word);
            }
        }
        words
    }

    fn build_words(words: &[Vec<u8>]) -> Lexicon {
        let entries: Vec<(&[u8], CanonicalId, ClassId)> = words
            .iter()
            .enumerate()
            .map(|(i, w)| (w.as_slice(), i as u16, 2 + (i % 254) as u8))
            .collect();
        Lexicon::build(&entries).expect("random words build")
    }

    fn expected(i: usize) -> TokenInfo {
        TokenInfo {
            canonical: i as u16,
            class: 2 + (i % 254) as u8,
        }
    }

    #[test]
    fn fold_key_folds_ascii_pads_and_stores_length() {
        let key = fold_key(b"FiVe").expect("key");
        let mut want = [0u8; 16];
        want[..4].copy_from_slice(b"five");
        want[15] = 4;
        assert_eq!(key, want);
    }

    #[test]
    fn fold_key_keeps_non_ascii_letters() {
        let key = fold_key("Ä1-Z".as_bytes()).expect("key");
        assert_eq!(&key[..5], &[0xC3, 0x84, b'1', b'-', b'z']);
        assert_eq!(key[15], 5);
    }

    #[test]
    fn fold_key_rejects_empty_and_over_15_bytes() {
        assert_eq!(fold_key(b""), None);
        assert!(fold_key(&[b'a'; 15]).is_some());
        assert_eq!(fold_key(&[b'a'; 16]), None);
    }

    #[test]
    fn bucket_pair_splits_key_little_endian() {
        let key = fold_key(b"seventeen").expect("key");
        let mut lo = [0u8; 8];
        let mut hi = [0u8; 8];
        lo.copy_from_slice(&key[..8]);
        hi.copy_from_slice(&key[8..]);
        let seed = 0x0123_4567_89AB_CDEF;
        assert_eq!(
            bucket_pair(&key, seed, BUCKETS),
            crate::hash::bucket_pair(
                u64::from_le_bytes(lo),
                u64::from_le_bytes(hi),
                seed,
                BUCKETS
            )
        );
    }

    #[test]
    fn bucket_is_one_cache_line() {
        assert_eq!(core::mem::size_of::<Bucket>(), 64);
        assert_eq!(core::mem::align_of::<Bucket>(), 64);
    }

    #[test]
    fn simd_match_equals_scalar_match_on_random_keys() {
        let mut state = 0xC0FF_EE00_1234_5678;
        for _ in 0..20_000 {
            let mut bucket = Bucket::default();
            let mut key = [0u8; 16];
            for b in key.iter_mut() {
                *b = next_seed(&mut state) as u8;
            }
            for slot in bucket.keys.iter_mut() {
                let r = next_seed(&mut state);
                *slot = match r % 4 {
                    0 => key,
                    1 => {
                        let mut k = key;
                        k[(r >> 8) as usize % 16] ^= 1 << ((r >> 16) % 8);
                        k
                    }
                    2 => [0u8; 16],
                    _ => {
                        let mut k = [0u8; 16];
                        for b in k.iter_mut() {
                            *b = next_seed(&mut state) as u8;
                        }
                        k
                    }
                };
            }
            assert_eq!(match_mask(&bucket, &key), match_mask_scalar(&bucket, &key));
        }
    }

    #[test]
    fn scalar_match_marks_equal_slots() {
        let key = fold_key(b"nine").expect("key");
        let mut bucket = Bucket::default();
        bucket.keys[1] = key;
        bucket.keys[3] = key;
        assert_eq!(match_mask_scalar(&bucket, &key), 0b1010);
        assert_eq!(match_mask(&bucket, &key), 0b1010);
    }

    #[test]
    fn build_and_lookup_finds_every_entry_at_high_load() {
        let words = random_words(3700, 42);
        let lex = build_words(&words);
        for (i, w) in words.iter().enumerate() {
            assert_eq!(lex.lookup(w), expected(i), "word {i}");
        }
    }

    #[test]
    fn build_uses_the_fewest_buckets_that_hold_the_words() {
        for (n, want) in [(1, 2), (10, 4), (295, 128), (1000, 512)] {
            let words = random_words(n, 3);
            let lex = build_words(&words);
            assert_eq!(lex.buckets_used(), want, "{n} words");
            for (i, w) in words.iter().enumerate() {
                assert_eq!(lex.lookup(w), expected(i), "word {i} of {n}");
            }
            let mut out = [MISS; MAX_TOKENS];
            lex.lookup_window(&[&words[0], b"notaword"], &mut out);
            assert_eq!(out[..2], [expected(0), MISS]);
        }
    }

    #[test]
    fn from_raw_round_trips() {
        let lex = Lexicon::build(&[(b"five", 5, 2), (b"six", 6, 3)]).expect("build");
        let copy = Lexicon::from_raw(
            *lex.buckets(),
            **lex.payload(),
            lex.seed(),
            lex.buckets_used(),
        );
        assert!(copy == lex);
        assert_eq!(
            copy.lookup(b"six"),
            TokenInfo {
                canonical: 6,
                class: 3
            }
        );
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let lex = Lexicon::build(&[(b"thirty", 30, 7)]).expect("build");
        let hit = TokenInfo {
            canonical: 30,
            class: 7,
        };
        assert_eq!(lex.lookup(b"THIRTY"), hit);
        assert_eq!(lex.lookup(b"Thirty"), hit);
    }

    #[test]
    fn lookup_miss_returns_unknown() {
        let lex = Lexicon::build(&[(b"five", 5, 2)]).expect("build");
        assert_eq!(lex.lookup(b"fiv"), MISS);
        assert_eq!(lex.lookup(b"fivee"), MISS);
        assert_eq!(lex.lookup(b""), MISS);
        assert_eq!(lex.lookup(b"fivefivefivefive"), MISS);
    }

    #[test]
    fn build_keeps_an_unknown_class_word_with_its_canonical() {
        let lex = Lexicon::build(&[(b"old", 7, CLASS_UNKNOWN), (b"five", 8, 2)]).expect("build");
        assert_eq!(
            lex.lookup(b"OLD"),
            TokenInfo {
                canonical: 7,
                class: CLASS_UNKNOWN
            }
        );
        assert_eq!(lex.lookup(b"older"), MISS);
    }

    #[test]
    fn build_rejects_bad_entries() {
        assert_eq!(
            Lexicon::build(&[(b"ok", 1, 2), (b"sixteen_bytes_xx", 2, 2)]).err(),
            Some(BuildError::InvalidLength { index: 1 })
        );
        assert_eq!(
            Lexicon::build(&[(b"", 1, 2)]).err(),
            Some(BuildError::InvalidLength { index: 0 })
        );
        assert_eq!(
            Lexicon::build(&[(b"zero", 1, 0)]).err(),
            Some(BuildError::ReservedClass { index: 0 })
        );
        assert_eq!(
            Lexicon::build(&[(b"oh", 1, 2), (b"two", 2, 2), (b"OH", 3, 2)]).err(),
            Some(BuildError::Duplicate { index: 2 })
        );
    }

    #[test]
    fn build_rejects_more_entries_than_slots() {
        let words = random_words(SLOTS + 1, 7);
        let entries: Vec<(&[u8], CanonicalId, ClassId)> =
            words.iter().map(|w| (w.as_slice(), 0, 2)).collect();
        assert_eq!(
            Lexicon::build(&entries).err(),
            Some(BuildError::TooManyEntries)
        );
    }

    #[test]
    fn lookup_window_matches_lookup() {
        let words = random_words(500, 9);
        let lex = build_words(&words);
        let long = [b'x'; 20];
        let tokens: [&[u8]; MAX_TOKENS] = [
            &words[0],
            b"NOTAWORDATALL",
            &words[499],
            &long,
            b"",
            &words[250],
            &words[1],
            b"q",
        ];
        let mut out = [TokenInfo::default(); MAX_TOKENS];
        lex.lookup_window(&tokens, &mut out);
        for (i, t) in tokens.iter().enumerate() {
            assert_eq!(out[i], lex.lookup(t), "token {i}");
        }
        assert_eq!(out[0], expected(0));
        assert_eq!(out[3], MISS);
    }

    #[test]
    fn lookup_window_writes_only_the_given_tokens() {
        let lex = Lexicon::build(&[(b"one", 1, 2)]).expect("build");
        let marker = TokenInfo {
            canonical: 0xBEEF,
            class: 9,
        };
        let mut out = [marker; MAX_TOKENS];
        lex.lookup_window(&[b"one", b"two"], &mut out);
        assert_eq!(
            out[0],
            TokenInfo {
                canonical: 1,
                class: 2
            }
        );
        assert_eq!(out[1], MISS);
        assert!(out[2..].iter().all(|&t| t == marker));
    }
}
