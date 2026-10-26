use crate::types::CandidateId;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_and_unpack_round_trip() {
        let word = pack([1, 2, 3, 4], [5, 6, 7, 8]);
        assert_eq!(unpack(word), ([1, 2, 3, 4], [5, 6, 7, 8]));
    }

    #[test]
    fn streaks_keep_four_bits() {
        assert_eq!(unpack(pack([0; 4], [0x1F, 0, 0, 0])).1, [0x0F, 0, 0, 0]);
    }
}
