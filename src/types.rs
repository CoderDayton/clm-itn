pub const MAX_TOKENS: usize = 8;
pub const MAX_KEY_LEN: usize = 15;

pub type ClassId = u8;
pub const CLASS_BOUNDARY: ClassId = 0;
pub const CLASS_UNKNOWN: ClassId = 1;

pub type CanonicalId = u16;

pub type Role = u8;
pub const ROLE_NONE: Role = 0;

pub type ShapeId = u16;
pub const SHAPE_UNKNOWN: ShapeId = 0;
pub const SHAPE_COUNT: usize = 2048;

pub type DomainId = u8;
pub const DOMAIN_CARDINAL: DomainId = 0;
pub const DOMAIN_TIME: DomainId = 1;
pub const DOMAIN_IDENTIFIER: DomainId = 2;
pub const DOMAIN_ORDINAL: DomainId = 3;
pub const DOMAIN_DATE: DomainId = 4;
pub const DOMAIN_PLAIN: DomainId = 5;
pub const DOMAIN_MONEY: DomainId = 6;
pub const DOMAIN_CENTS: DomainId = 7;
pub const DOMAIN_PERCENT: DomainId = 8;
pub const DOMAIN_NEGATIVE: DomainId = 9;
pub const DOMAIN_MEASURE: DomainId = 10;
pub const DOMAIN_MEASURE_POWER: DomainId = 11;
pub const DOMAIN_MEASURE_PREFIX: DomainId = 12;
pub const DOMAIN_COUNT: usize = 16;

pub type CandidateId = u8;
pub const CANDIDATE_NONE: CandidateId = 0;

pub type CellIndex = u16;
pub const CELL_COUNT: usize = 32768;
pub const CELL_ABSTAINED: CellIndex = 0xFFFF;

pub const TAU: u8 = 3;
pub const DEFAULT_H: u8 = 4;

#[inline(always)]
pub const fn cell_index(domain: DomainId, shape: ShapeId) -> CellIndex {
    (((domain & 0x0F) as u16) << 11) | (shape & 0x7FF)
}

#[inline(always)]
pub const fn cell_domain(cell: CellIndex) -> DomainId {
    ((cell >> 11) & 0x0F) as u8
}

#[inline(always)]
pub const fn cell_shape(cell: CellIndex) -> ShapeId {
    cell & 0x7FF
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenInfo {
    pub canonical: CanonicalId,
    pub class: ClassId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpanReceipt {
    pub cell: CellIndex,
    pub start: u8,
    pub len: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CorrectionEvent {
    pub cell: CellIndex,
    pub candidate: CandidateId,
    pub _pad: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatError {
    Overflow,
    TooManyTokens,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorrectionError {
    OutOfSet,
    Abstained,
    Dropped,
    InvalidReceipt,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_index_packs_domain_high_shape_low() {
        assert_eq!(cell_index(DOMAIN_TIME, 0x31), 0x0831);
        assert_eq!(cell_index(0x0F, 0x7FF), 0x7FFF);
    }

    #[test]
    fn cell_index_masks_domain_to_four_bits() {
        assert_eq!(cell_index(0x1F, 0x00), 0x7800);
    }

    #[test]
    fn cell_index_masks_shape_to_eleven_bits() {
        assert_eq!(cell_index(DOMAIN_TIME, 0xFFF), 0x0FFF);
    }

    #[test]
    fn every_cell_fits_cell_count() {
        assert_eq!(cell_index(0x0F, 0x7FF) as usize, CELL_COUNT - 1);
        assert_eq!(SHAPE_COUNT, 2048);
    }

    #[test]
    fn cell_parts_round_trip() {
        let cell = cell_index(DOMAIN_ORDINAL, 7);
        assert_eq!(cell_domain(cell), DOMAIN_ORDINAL);
        assert_eq!(cell_shape(cell), 7);
    }

    #[test]
    fn abstained_cell_is_outside_cell_range() {
        assert!(CELL_ABSTAINED as usize >= CELL_COUNT);
    }

    #[test]
    fn correction_event_is_four_bytes() {
        assert_eq!(core::mem::size_of::<CorrectionEvent>(), 4);
    }
}
