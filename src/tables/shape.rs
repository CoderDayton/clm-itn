use crate::hash::{bucket_pair, next_seed};
use crate::types::{Role, SHAPE_UNKNOWN, ShapeId};

pub fn shape_key(roles: &[Role]) -> u32 {
    roles
        .iter()
        .enumerate()
        .fold(0, |key, (i, &r)| key | (((r & 0x0F) as u32) << (4 * i)))
}

pub const SHAPE_BUCKETS: usize = 512;
pub const SHAPE_SLOTS: usize = 8;
const MAX_KICKS: usize = 512;
const MAX_SEED_TRIES: usize = 4096;

#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShapeBucket {
    keys: [u32; SHAPE_SLOTS],
    ids: [ShapeId; SHAPE_SLOTS],
}

impl ShapeBucket {
    pub const EMPTY: ShapeBucket = ShapeBucket {
        keys: [0; SHAPE_SLOTS],
        ids: [0; SHAPE_SLOTS],
    };

    pub const fn from_raw(keys: [u32; SHAPE_SLOTS], ids: [ShapeId; SHAPE_SLOTS]) -> Self {
        Self { keys, ids }
    }

    pub fn keys(&self) -> &[u32; SHAPE_SLOTS] {
        &self.keys
    }

    pub fn ids(&self) -> &[ShapeId; SHAPE_SLOTS] {
        &self.ids
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ShapeBuildError {
    ReservedId,
    ZeroKey,
    DuplicateKey(u32),
    Full,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ShapeTable {
    buckets: [ShapeBucket; SHAPE_BUCKETS],
    seed: u64,
}

impl ShapeTable {
    pub const fn from_raw(buckets: [ShapeBucket; SHAPE_BUCKETS], seed: u64) -> Self {
        Self { buckets, seed }
    }

    pub fn buckets(&self) -> &[ShapeBucket; SHAPE_BUCKETS] {
        &self.buckets
    }

    pub fn buckets_mut(&mut self) -> &mut [ShapeBucket; SHAPE_BUCKETS] {
        &mut self.buckets
    }

    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed;
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn build(entries: &[(u32, ShapeId)]) -> Result<Self, ShapeBuildError> {
        for (i, &(key, id)) in entries.iter().enumerate() {
            if id == SHAPE_UNKNOWN {
                return Err(ShapeBuildError::ReservedId);
            }
            if key == 0 {
                return Err(ShapeBuildError::ZeroKey);
            }
            if entries[..i].iter().any(|&(k, _)| k == key) {
                return Err(ShapeBuildError::DuplicateKey(key));
            }
        }
        let mut state = 0u64;
        for _ in 0..MAX_SEED_TRIES {
            let seed = next_seed(&mut state);
            if let Some(buckets) = try_fill(entries, seed) {
                return Ok(Self { buckets, seed });
            }
        }
        Err(ShapeBuildError::Full)
    }

    #[inline(always)]
    pub fn lookup(&self, key: u32) -> ShapeId {
        let (b1, b2) = bucket_pair(key as u64, 0, self.seed, SHAPE_BUCKETS);
        find(&self.buckets[b1], key) | find(&self.buckets[b2], key)
    }
}

#[inline(always)]
fn find(bucket: &ShapeBucket, key: u32) -> ShapeId {
    let mut id = 0;
    for s in 0..SHAPE_SLOTS {
        id |= bucket.ids[s] & 0u16.wrapping_sub((bucket.keys[s] == key) as u16);
    }
    id
}

fn try_fill(entries: &[(u32, ShapeId)], seed: u64) -> Option<[ShapeBucket; SHAPE_BUCKETS]> {
    let mut buckets = [ShapeBucket::EMPTY; SHAPE_BUCKETS];
    for &(key, id) in entries {
        let (mut key, mut id) = (key, id);
        let mut b = bucket_pair(key as u64, 0, seed, SHAPE_BUCKETS).0;
        let mut placed = false;
        for kick in 0..MAX_KICKS {
            let (b1, b2) = bucket_pair(key as u64, 0, seed, SHAPE_BUCKETS);
            for cand in [b1, b2] {
                if let Some(s) = buckets[cand].keys.iter().position(|&k| k == 0) {
                    buckets[cand].keys[s] = key;
                    buckets[cand].ids[s] = id;
                    placed = true;
                    break;
                }
            }
            if placed {
                break;
            }
            b = if b == b1 { b2 } else { b1 };
            let s = kick % SHAPE_SLOTS;
            core::mem::swap(&mut buckets[b].keys[s], &mut key);
            core::mem::swap(&mut buckets[b].ids[s], &mut id);
        }
        if !placed {
            return None;
        }
    }
    Some(buckets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_key_packs_roles_low_nibble_first() {
        assert_eq!(shape_key(&[1, 3]), 0x31);
        assert_eq!(shape_key(&[1; 8]), 0x1111_1111);
    }

    #[test]
    fn shape_table_finds_every_built_key_and_misses_others() {
        let max = crate::types::SHAPE_COUNT as u32 - 1;
        let entries: Vec<(u32, ShapeId)> = (1..=max).map(|i| (i * 7 + 1, i as ShapeId)).collect();
        let table = ShapeTable::build(&entries).expect("build");
        for &(key, id) in &entries {
            assert_eq!(table.lookup(key), id);
        }
        assert_eq!(table.lookup(0xDEAD_BEEF), SHAPE_UNKNOWN);
    }

    #[test]
    fn shape_table_from_raw_round_trips() {
        let table = ShapeTable::build(&[(0x31, 1), (0x1, 2)]).expect("build");
        let copy = ShapeTable::from_raw(*table.buckets(), table.seed());
        assert!(copy == table);
        assert_eq!(copy.lookup(0x31), 1);
    }

    #[test]
    fn shape_table_rejects_reserved_and_duplicate_entries() {
        assert!(ShapeTable::build(&[(0x31, SHAPE_UNKNOWN)]).is_err());
        assert!(ShapeTable::build(&[(0, 1)]).is_err());
        assert!(ShapeTable::build(&[(0x31, 1), (0x31, 2)]).is_err());
    }
}
