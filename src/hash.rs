const K1: u64 = 0x9E37_79B9_7F4A_7C15;
const K2: u64 = 0xC2B2_AE3D_27D4_EB4F;

#[inline(always)]
pub fn bucket_pair(lo: u64, hi: u64, seed: u64, buckets: usize) -> (usize, usize) {
    debug_assert!(buckets.is_power_of_two() && buckets >= 2);
    let a = (lo ^ seed).wrapping_mul(K1).rotate_left(31);
    let b = (hi ^ seed.rotate_left(32)).wrapping_mul(K2);
    let mut h = (a ^ b).wrapping_mul(K1);
    h ^= h >> 32;
    let b1 = (h as usize) & (buckets - 1);
    let mut b2 = ((h >> 32) as usize) & (buckets - 1);
    if b2 == b1 {
        b2 ^= 1;
    }
    (b1, b2)
}

pub fn next_seed(state: &mut u64) -> u64 {
    *state = state.wrapping_add(K1);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_are_distinct_and_in_range() {
        let mut s = 1u64;
        for buckets in [2usize, 64, 1024] {
            for _ in 0..10_000 {
                let (lo, hi, seed) = (next_seed(&mut s), next_seed(&mut s), next_seed(&mut s));
                let (b1, b2) = bucket_pair(lo, hi, seed, buckets);
                assert!(b1 < buckets && b2 < buckets);
                assert_ne!(b1, b2);
            }
        }
    }

    #[test]
    fn seed_changes_placement() {
        let moved = (0..1000u64)
            .filter(|&k| bucket_pair(k, 0, 1, 1024) != bucket_pair(k, 0, 2, 1024))
            .count();
        assert!(moved > 900, "only {moved} of 1000 keys moved");
    }

    #[test]
    fn spreads_sequential_keys() {
        let mut hits = [0u32; 64];
        for k in 1..=6400u64 {
            hits[bucket_pair(k, 0, 7, 64).0] += 1;
        }
        assert!(hits.iter().all(|&n| (50..=150).contains(&n)), "{hits:?}");
    }
}
