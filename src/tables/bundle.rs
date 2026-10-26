use core::ops::{Deref, DerefMut};

use crate::tables::candidate::CandidateTable;
use crate::tables::context::ContextLut;
use crate::tables::lexicon::{self, Lexicon};
use crate::tables::shape::{self, ShapeTable};
use crate::types::{
    CELL_COUNT, CandidateId, ClassId, DOMAIN_COUNT, MAX_LEFT_FOLDS, Role, SHAPE_COUNT, SettingBits,
};

pub const MAX_CANONICAL: usize = 4096;
pub const WORDS_FIRST_WORDS: usize = MAX_CANONICAL / 64;

#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aligned<T>(pub T);

impl<T> Deref for Aligned<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Aligned<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

#[derive(PartialEq, Eq)]
pub struct Tables {
    pub lexicon: Lexicon,
    pub role_of: Aligned<[Role; 256]>,
    pub value_of: Aligned<[u32; MAX_CANONICAL]>,
    pub shapes: ShapeTable,
    pub shape_len: Aligned<[u8; SHAPE_COUNT]>,
    pub context: ContextLut,
    pub candidates: CandidateTable,
    pub cardinal_default: Aligned<[CandidateId; SHAPE_COUNT]>,
    pub initial_cells: Aligned<[u64; CELL_COUNT]>,
    pub affix_len: Aligned<[u8; DOMAIN_COUNT]>,
    pub words_first: Aligned<[u64; WORDS_FIRST_WORDS]>,
    pub plural: Aligned<[u64; WORDS_FIRST_WORDS]>,
    pub starts: Aligned<[u64; 4]>,
    pub per_stop: Aligned<[u64; 4]>,
    pub per_units: Aligned<[u64; 4]>,
    pub pair_only: Aligned<[u64; 4]>,
    /// The settings a word class belongs to. A token whose class has a
    /// switched-off setting is read as an unknown word and kept as spoken.
    pub class_settings: Aligned<[SettingBits; 256]>,
    /// The settings a domain belongs to. A cell in a domain with a
    /// switched-off setting uses its rank-1 candidate.
    pub domain_settings: Aligned<[SettingBits; DOMAIN_COUNT]>,
    pub suffix_roles: u16,
    pub prefix_roles: u16,
    pub connector_roles: u16,
    pub bind_roles: u16,
    pub words_first_roles: u16,
    /// Roles a span never starts with, such as "am" in "i am five".
    pub lead_skip_roles: u16,
    /// `[a, b, c]`: a span right after words of classes `a` and `b` takes `c`
    /// as its left context ("it is" acts like "at"). The context table reads
    /// `a` and `b` anywhere else as unknown. A zero row does nothing.
    pub left_folds: [[ClassId; 3]; MAX_LEFT_FOLDS],
}

impl Tables {
    pub const EMPTY: Tables = Tables {
        lexicon: Lexicon::from_raw(
            [lexicon::Bucket::EMPTY; lexicon::BUCKETS],
            [0; lexicon::SLOTS],
            0,
        ),
        role_of: Aligned([0; 256]),
        value_of: Aligned([0; MAX_CANONICAL]),
        shapes: ShapeTable::from_raw([shape::ShapeBucket::EMPTY; shape::SHAPE_BUCKETS], 0),
        shape_len: Aligned([0; SHAPE_COUNT]),
        context: ContextLut::new(),
        candidates: CandidateTable::new(),
        cardinal_default: Aligned([0; SHAPE_COUNT]),
        initial_cells: Aligned([0; CELL_COUNT]),
        affix_len: Aligned([0; DOMAIN_COUNT]),
        words_first: Aligned([0; WORDS_FIRST_WORDS]),
        plural: Aligned([0; WORDS_FIRST_WORDS]),
        starts: Aligned([0; 4]),
        per_stop: Aligned([0; 4]),
        per_units: Aligned([0; 4]),
        pair_only: Aligned([0; 4]),
        class_settings: Aligned([0; 256]),
        domain_settings: Aligned([0; DOMAIN_COUNT]),
        suffix_roles: 0,
        prefix_roles: 0,
        connector_roles: 0,
        bind_roles: 0,
        words_first_roles: 0,
        lead_skip_roles: 0,
        left_folds: [[0; 3]; MAX_LEFT_FOLDS],
    };

    #[cfg(not(clm_no_std))]
    pub fn boxed_empty() -> Box<Tables> {
        let t = Box::<Tables>::new_zeroed();
        // SAFETY: every field of Tables is an integer or an array of integers, so all-zero
        // bytes are a valid value, and it is the value of Tables::EMPTY.
        unsafe { t.assume_init() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of_val, offset_of};

    #[test]
    fn boxed_empty_equals_empty_without_a_stack_copy() {
        static EMPTY: Tables = Tables::EMPTY;
        assert!(*Tables::boxed_empty() == EMPTY);
    }

    #[test]
    fn every_table_starts_on_a_cache_line() {
        assert_eq!(core::mem::align_of::<Tables>(), 64);
        let offsets = [
            offset_of!(Tables, lexicon),
            offset_of!(Tables, role_of),
            offset_of!(Tables, value_of),
            offset_of!(Tables, shapes),
            offset_of!(Tables, shape_len),
            offset_of!(Tables, context),
            offset_of!(Tables, candidates),
            offset_of!(Tables, cardinal_default),
            offset_of!(Tables, initial_cells),
            offset_of!(Tables, affix_len),
            offset_of!(Tables, words_first),
            offset_of!(Tables, plural),
            offset_of!(Tables, starts),
            offset_of!(Tables, per_stop),
            offset_of!(Tables, per_units),
            offset_of!(Tables, pair_only),
            offset_of!(Tables, class_settings),
            offset_of!(Tables, domain_settings),
        ];
        for (i, off) in offsets.into_iter().enumerate() {
            assert_eq!(off % 64, 0, "field {i} at offset {off}");
        }
        let t = crate::tables::english::english();
        let addrs = [
            core::ptr::from_ref(&t.lexicon).addr(),
            core::ptr::from_ref(&t.role_of).addr(),
            core::ptr::from_ref(&t.value_of).addr(),
            core::ptr::from_ref(&t.shapes).addr(),
            core::ptr::from_ref(&t.shape_len).addr(),
            core::ptr::from_ref(&t.context).addr(),
            core::ptr::from_ref(&t.candidates).addr(),
            core::ptr::from_ref(&t.cardinal_default).addr(),
            core::ptr::from_ref(&t.initial_cells).addr(),
            t.lexicon.buckets().as_ptr().addr(),
            t.lexicon.payload().as_ptr().addr(),
        ];
        for (i, a) in addrs.into_iter().enumerate() {
            assert_eq!(a % 64, 0, "table {i} at {a:#x}");
        }
        assert_eq!(align_of_val(t.lexicon.payload()), 64);
    }
}
