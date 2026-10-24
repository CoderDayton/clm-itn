use core::sync::atomic::{AtomicU64, Ordering};

use crate::types::{CANDIDATE_NONE, CELL_COUNT, CandidateId, CellIndex};

const MAX_H: u8 = 15;

#[inline]
pub fn pack(ranks: [CandidateId; 4], streaks: [u8; 4]) -> u64 {
    let mut word = 0u64;
    for i in 0..4 {
        word |= (ranks[i] as u64) << (8 * i);
        word |= ((streaks[i] & 0x0F) as u64) << (32 + 4 * i);
    }
    word
}

#[inline]
pub fn unpack(word: u64) -> ([CandidateId; 4], [u8; 4]) {
    let mut ranks = [0; 4];
    let mut streaks = [0; 4];
    for i in 0..4 {
        ranks[i] = (word >> (8 * i)) as u8;
        streaks[i] = ((word >> (32 + 4 * i)) & 0x0F) as u8;
    }
    (ranks, streaks)
}

#[inline(always)]
pub fn rank0(word: u64) -> CandidateId {
    (word & 0xFF) as u8
}

pub fn apply_correction(word: u64, candidate: CandidateId, h: u8) -> u64 {
    if candidate == CANDIDATE_NONE {
        return word;
    }
    let (mut ranks, streaks) = unpack(word);
    let Some(r) = ranks.iter().position(|&c| c == candidate) else {
        return word;
    };
    let mut next = [0u8; 4];
    if r > 0 {
        let streak = (streaks[r] + 1).min(MAX_H);
        if streak >= h.clamp(1, MAX_H) {
            ranks.swap(r, r - 1);
        } else {
            next[r] = streak;
        }
    }
    pack(ranks, next)
}

#[repr(C, align(64))]
pub struct Cells {
    words: [AtomicU64; CELL_COUNT],
}

impl Cells {
    pub const fn new() -> Self {
        Self {
            words: [const { AtomicU64::new(0) }; CELL_COUNT],
        }
    }

    pub fn shared() -> std::sync::Arc<Cells> {
        let c = std::sync::Arc::<Cells>::new_zeroed();
        // SAFETY: Cells is an array of AtomicU64, for which all-zero bytes are a valid value.
        unsafe { c.assume_init() }
    }

    #[inline(always)]
    pub fn load(&self, cell: CellIndex) -> u64 {
        self.words[cell as usize].load(Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn store(&self, cell: CellIndex, word: u64) {
        self.words[cell as usize].store(word, Ordering::Release);
    }

    pub fn apply(&self, cell: CellIndex, candidate: CandidateId, h: u8) -> bool {
        let word = self.load(cell);
        let next = apply_correction(word, candidate, h);
        if next == word {
            return false;
        }
        self.store(cell, next);
        true
    }
}

impl Default for Cells {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CANDIDATE_NONE;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Model {
        ranks: [CandidateId; 4],
        streaks: [u8; 4],
    }

    impl Model {
        fn new(ranks: [CandidateId; 4]) -> Self {
            Self {
                ranks,
                streaks: [0; 4],
            }
        }

        fn correct(&mut self, c: CandidateId, h: u8) -> bool {
            if c == CANDIDATE_NONE {
                return false;
            }
            let Some(r) = self.ranks.iter().position(|&x| x == c) else {
                return false;
            };
            if r == 0 {
                self.streaks = [0; 4];
                return false;
            }
            let streak = self.streaks[r] + 1;
            self.streaks = [0; 4];
            if streak >= h {
                self.ranks.swap(r, r - 1);
                true
            } else {
                self.streaks[r] = streak;
                false
            }
        }
    }

    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    const A: CandidateId = 10;
    const B: CandidateId = 20;
    const C: CandidateId = 30;
    const D: CandidateId = 40;
    const START: [CandidateId; 4] = [A, B, C, D];

    fn run(start: [CandidateId; 4], stream: &[CandidateId], h: u8) -> usize {
        let mut word = pack(start, [0; 4]);
        let mut model = Model::new(start);
        let mut swaps = 0;
        for (i, &c) in stream.iter().enumerate() {
            let before = unpack(word).0;
            word = apply_correction(word, c, h);
            let swapped = model.correct(c, h);
            let (ranks, streaks) = unpack(word);
            assert_eq!(
                (ranks, streaks),
                (model.ranks, model.streaks),
                "h={h} event {i} candidate {c}"
            );
            assert_eq!(word >> 48, 0);
            assert_eq!(streaks[0], 0);
            if ranks != before {
                swaps += 1;
            }
            assert_eq!(ranks != before, swapped, "h={h} event {i}");
        }
        swaps
    }

    fn alternating(x: CandidateId, y: CandidateId, n: usize) -> Vec<CandidateId> {
        (0..n).map(|i| if i % 2 == 0 { x } else { y }).collect()
    }

    fn ratio(x: CandidateId, y: CandidateId, nx: usize, ny: usize, n: usize) -> Vec<CandidateId> {
        let period = nx + ny;
        (0..n)
            .map(|i| if i % period < nx { x } else { y })
            .collect()
    }

    #[test]
    fn pack_uses_spec_bit_layout() {
        let word = pack([1, 2, 3, 4], [0, 5, 6, 7]);
        assert_eq!(word, 0x0000_7650_0403_0201);
        assert_eq!(rank0(word), 1);
    }

    #[test]
    fn pack_masks_streaks_to_four_bits() {
        let word = pack([1, 2, 3, 4], [0, 0x1F, 0, 0]);
        assert_eq!(unpack(word).1, [0, 0x0F, 0, 0]);
        assert_eq!(word >> 48, 0);
    }

    #[test]
    fn unpack_round_trips_pack() {
        let mut rng = XorShift(0x1234_5678_9ABC_DEF1);
        for _ in 0..1000 {
            let v = rng.next();
            let ranks = [v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8];
            let streaks = [
                (v >> 32) as u8 & 0xF,
                (v >> 36) as u8 & 0xF,
                (v >> 40) as u8 & 0xF,
                (v >> 44) as u8 & 0xF,
            ];
            assert_eq!(unpack(pack(ranks, streaks)), (ranks, streaks));
        }
    }

    #[test]
    fn none_or_absent_candidate_leaves_word_unchanged() {
        let word = pack([A, B, CANDIDATE_NONE, CANDIDATE_NONE], [0, 2, 0, 0]);
        for h in 1..=15 {
            assert_eq!(apply_correction(word, CANDIDATE_NONE, h), word);
            assert_eq!(apply_correction(word, 99, h), word);
        }
    }

    #[test]
    fn rank0_correction_clears_all_counters() {
        let word = pack(START, [0, 2, 0, 0]);
        assert_eq!(apply_correction(word, A, 4), pack(START, [0; 4]));
    }

    #[test]
    fn lower_rank_correction_increments_and_clears_others() {
        let word = pack(START, [0, 2, 0, 0]);
        assert_eq!(apply_correction(word, C, 4), pack(START, [0, 0, 1, 0]));
    }

    #[test]
    fn reaching_h_swaps_with_rank_above_and_clears() {
        let word = pack(START, [0, 0, 3, 0]);
        assert_eq!(apply_correction(word, C, 4), pack([A, C, B, D], [0; 4]));
    }

    #[test]
    fn invariant1_swaps_match_reference_model() {
        for h in 1..=15u8 {
            let hu = h as usize;
            let n = 8 * hu + 7;
            let streams = [
                alternating(A, B, n),
                alternating(B, C, n),
                ratio(A, B, 3, 1, n),
                ratio(A, B, 1, 3, n),
                ratio(C, D, 3, 1, n),
                vec![B; hu],
                vec![B; hu - 1],
                vec![D; 3 * hu],
            ];
            for s in &streams {
                run(START, s, h);
            }
        }
    }

    #[test]
    fn invariant2_n_corrections_cause_at_most_n_over_h_swaps() {
        let mut rng = XorShift(0xDEAD_BEEF_CAFE_F00D);
        for h in 1..=15u8 {
            for n in [1usize, 2, 7, 15, 16, 31, 100] {
                let streams = [
                    alternating(A, B, n),
                    ratio(A, B, 3, 1, n),
                    ratio(A, B, 1, 3, n),
                    vec![D; n],
                    (0..n).map(|_| START[rng.below(4) as usize]).collect(),
                ];
                for s in &streams {
                    assert!(run(START, s, h) <= n / h as usize, "h={h} n={n}");
                }
            }
        }
    }

    fn non_rank0_corrections(start: [CandidateId; 4], stream: &[CandidateId], h: u8) -> usize {
        let mut word = pack(start, [0; 4]);
        let mut count = 0;
        for &c in stream {
            count += usize::from(rank0(word) != c);
            word = apply_correction(word, c, h);
        }
        count
    }

    #[test]
    fn invariant3_strict_alternation_never_swaps_for_h_at_least_2() {
        for h in 2..=15u8 {
            for (x, y) in [(A, B), (B, A), (B, C), (C, D)] {
                assert_eq!(run(START, &alternating(x, y, 40), h), 0, "h={h} {x}/{y}");
            }
        }
    }

    #[test]
    fn invariant3_h1_swaps_every_non_rank0_correction() {
        for (x, y) in [(A, B), (B, A), (B, C), (C, D)] {
            let s = alternating(x, y, 40);
            let expected = non_rank0_corrections(START, &s, 1);
            assert_eq!(run(START, &s, 1), expected, "{x}/{y}");
            assert!(expected >= 39, "{x}/{y}");
        }
    }

    #[test]
    fn invariant4_h_consecutive_swap_once_h_minus_one_none() {
        for h in 1..=15u8 {
            let hu = h as usize;
            for c in [B, C, D] {
                assert_eq!(run(START, &vec![c; hu], h), 1, "h={h} c={c}");
                assert_eq!(run(START, &vec![c; hu - 1], h), 0, "h={h} c={c}");
            }
        }
    }

    #[test]
    fn randomized_stream_matches_model() {
        let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
        for _ in 0..200 {
            let h = 1 + rng.below(15) as u8;
            let valid = 2 + rng.below(3) as usize;
            let mut start = [CANDIDATE_NONE; 4];
            for (i, slot) in start.iter_mut().take(valid).enumerate() {
                *slot = 1 + i as u8 * 7;
            }
            let pool = [start[0], start[1], start[2], start[3], CANDIDATE_NONE, 200];
            let stream: Vec<CandidateId> = (0..500)
                .map(|_| pool[rng.below(pool.len() as u64) as usize])
                .collect();
            let n = stream.len();
            assert!(run(start, &stream, h) <= n / h as usize);
            let word = stream
                .iter()
                .fold(pack(start, [0; 4]), |w, &c| apply_correction(w, c, h));
            assert_ne!(rank0(word), CANDIDATE_NONE);
        }
    }

    #[test]
    fn cells_block_is_cache_line_aligned() {
        assert_eq!(core::mem::align_of::<Cells>(), 64);
        assert_eq!(core::mem::size_of::<Cells>(), CELL_COUNT * 8);
    }

    #[test]
    fn cells_apply_stores_only_when_changed() {
        let cells = Cells::shared();
        let cell = crate::types::cell_index(crate::types::DOMAIN_TIME, 7);
        let word = pack(START, [0; 4]);
        cells.store(cell, word);
        assert!(!cells.apply(cell, A, 4));
        assert_eq!(cells.load(cell), word);
        assert!(!cells.apply(cell, 99, 4));
        assert!(cells.apply(cell, B, 4));
        assert_eq!(cells.load(cell), pack(START, [0, 1, 0, 0]));
        assert_eq!(cells.load(cell + 1), 0);
    }
}
