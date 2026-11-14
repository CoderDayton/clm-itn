use crate::types::{CandidateId, ClassId, DomainId, TAU};

pub const LUT_SIZE: usize = 256 * 256;

#[inline(always)]
pub const fn pack_entry(domain: DomainId, confidence: u8) -> u8 {
    (domain & 0x0F) | (confidence << 4)
}

#[inline(always)]
pub const fn unpack_entry(entry: u8) -> (DomainId, u8) {
    (entry & 0x0F, entry >> 4)
}

#[inline(always)]
pub const fn abstains(confidence: u8) -> bool {
    confidence < TAU
}

#[inline(always)]
pub const fn select(abstain: bool, from_cell: CandidateId, default: CandidateId) -> CandidateId {
    let mask = 0u8.wrapping_sub(abstain as u8);
    (default & mask) | (from_cell & !mask)
}

#[repr(C, align(64))]
#[derive(Clone, PartialEq, Eq)]
pub struct ContextLut {
    table: [u8; LUT_SIZE],
}

impl Default for ContextLut {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextLut {
    pub const fn new() -> Self {
        Self::from_raw([0; LUT_SIZE])
    }

    pub const fn from_raw(table: [u8; LUT_SIZE]) -> Self {
        Self { table }
    }

    pub fn as_raw(&self) -> &[u8; LUT_SIZE] {
        &self.table
    }

    pub fn as_raw_mut(&mut self) -> &mut [u8; LUT_SIZE] {
        &mut self.table
    }

    pub fn set(&mut self, left: ClassId, right: ClassId, entry: u8) {
        self.table[index(left, right)] = entry;
    }

    #[inline(always)]
    pub fn lookup(&self, left: ClassId, right: ClassId) -> (DomainId, u8) {
        unpack_entry(self.table[index(left, right)])
    }
}

#[inline(always)]
fn index(left: ClassId, right: ClassId) -> usize {
    ((left as usize) << 8) | right as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DOMAIN_CARDINAL, DOMAIN_TIME};

    #[test]
    fn entry_packs_domain_low_confidence_high() {
        assert_eq!(pack_entry(DOMAIN_TIME, 9), 0x91);
        assert_eq!(unpack_entry(0x91), (DOMAIN_TIME, 9));
    }

    #[test]
    fn lookup_indexes_left_major() {
        let mut lut = ContextLut::new();
        lut.set(7, 200, pack_entry(DOMAIN_TIME, 12));
        assert_eq!(lut.lookup(7, 200), (DOMAIN_TIME, 12));
        assert_eq!(lut.lookup(200, 7), (DOMAIN_CARDINAL, 0));
    }

    #[test]
    fn extreme_classes_do_not_overflow() {
        let mut lut = ContextLut::new();
        lut.set(255, 255, pack_entry(DOMAIN_TIME, 15));
        assert_eq!(lut.lookup(255, 255), (DOMAIN_TIME, 15));
    }

    #[test]
    fn from_raw_round_trips_and_is_cache_line_aligned() {
        let mut lut = ContextLut::new();
        lut.set(3, 4, pack_entry(DOMAIN_TIME, 5));
        assert!(ContextLut::from_raw(*lut.as_raw()) == lut);
        assert_eq!(core::mem::align_of::<ContextLut>(), 64);
    }

    #[test]
    fn abstain_below_tau_only() {
        assert!(abstains(0));
        assert!(abstains(TAU - 1));
        assert!(!abstains(TAU));
        assert!(!abstains(15));
    }

    #[test]
    fn select_picks_default_on_abstain_and_cell_otherwise() {
        assert_eq!(select(true, 5, 9), 9);
        assert_eq!(select(false, 5, 9), 5);
    }
}
