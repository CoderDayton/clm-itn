//! A language's tables as bytes: [`Tables::to_bytes`] and
//! [`Tables::from_bytes`] move them between processes, so a language can ship
//! as a file instead of being compiled in.
//!
//! The layout is little-endian, field by field, behind a magic number and
//! version, and ends with an FNV-1a checksum. It is the same on every target.
//! The loader checks the layout and the checksum, not the meaning of the
//! tables; a file from an unknown source can make lookups panic or give
//! wrong output, never read out of bounds.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::array::from_fn;

use crate::tables::candidate::{Candidate, CandidateTable, MAX_LITERAL_LEN, MAX_LITERALS, MAX_OPS};
use crate::tables::context::{ContextLut, LUT_SIZE};
use crate::tables::lexicon::{BUCKETS, Bucket, Key, Lexicon, MIN_BUCKETS, SLOTS, SLOTS_PER_BUCKET};
use crate::tables::shape::{SHAPE_BUCKETS, SHAPE_SLOTS, ShapeBucket, ShapeTable};
use crate::tables::{Aligned, MAX_CANONICAL, MAX_ZONES, Tables, Vocab, WORDS_FIRST_WORDS};
use crate::types::{
    CELL_COUNT, DOMAIN_COUNT, MAX_IDIOM_LEN, MAX_IDIOMS, MAX_LEFT_FOLDS, SHAPE_COUNT, name,
};

const MAGIC: [u8; 8] = *b"CLMTBL\x00\x01";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Not a table image, or one from another version.
    Magic,
    /// Shorter than its layout.
    Truncated,
    /// Longer than its layout.
    Trailing,
    /// The checksum does not match the content.
    Checksum,
    /// A count is out of range.
    Invalid,
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

struct Out(Vec<u8>);

impl Out {
    fn u8s<'a>(&mut self, v: impl IntoIterator<Item = &'a u8>) {
        self.0.extend(v);
    }
    fn u16s<'a>(&mut self, v: impl IntoIterator<Item = &'a u16>) {
        v.into_iter()
            .for_each(|x| self.0.extend_from_slice(&x.to_le_bytes()));
    }
    fn u32s<'a>(&mut self, v: impl IntoIterator<Item = &'a u32>) {
        v.into_iter()
            .for_each(|x| self.0.extend_from_slice(&x.to_le_bytes()));
    }
    fn u64s<'a>(&mut self, v: impl IntoIterator<Item = &'a u64>) {
        v.into_iter()
            .for_each(|x| self.0.extend_from_slice(&x.to_le_bytes()));
    }
}

struct In<'a> {
    bytes: &'a [u8],
}

impl<'a> In<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], LoadError> {
        let (head, rest) = self.bytes.split_at_checked(n).ok_or(LoadError::Truncated)?;
        self.bytes = rest;
        Ok(head)
    }
    fn u8s<const N: usize>(&mut self) -> Result<[u8; N], LoadError> {
        let b = self.take(N)?;
        Ok(from_fn(|i| b[i]))
    }
    fn u16s<const N: usize>(&mut self) -> Result<[u16; N], LoadError> {
        let b = self.take(2 * N)?;
        Ok(from_fn(|i| u16::from_le_bytes([b[2 * i], b[2 * i + 1]])))
    }
    fn u32s<const N: usize>(&mut self) -> Result<[u32; N], LoadError> {
        let b = self.take(4 * N)?;
        Ok(from_fn(|i| {
            u32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]])
        }))
    }
    fn u64s<const N: usize>(&mut self) -> Result<[u64; N], LoadError> {
        let b = self.take(8 * N)?;
        Ok(from_fn(|i| {
            let mut w = [0u8; 8];
            w.copy_from_slice(&b[8 * i..8 * i + 8]);
            u64::from_le_bytes(w)
        }))
    }
    fn lut(&mut self) -> Result<[u8; LUT_SIZE], LoadError> {
        let mut raw = [0u8; LUT_SIZE];
        raw.copy_from_slice(self.take(LUT_SIZE)?);
        Ok(raw)
    }
    fn u64(&mut self) -> Result<u64, LoadError> {
        Ok(self.u64s::<1>()?[0])
    }
    fn rows<const R: usize, const C: usize>(&mut self) -> Result<[[u8; C]; R], LoadError> {
        let b = self.take(R * C)?;
        Ok(from_fn(|r| from_fn(|c| b[r * C + c])))
    }
    fn idioms(&mut self) -> Result<[[u16; MAX_IDIOM_LEN]; MAX_IDIOMS], LoadError> {
        let flat = self.u16s::<{ MAX_IDIOMS * MAX_IDIOM_LEN }>()?;
        Ok(from_fn(|r| from_fn(|c| flat[r * MAX_IDIOM_LEN + c])))
    }
}

impl Tables {
    /// The tables as a self-checking byte image; see the module notes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut o = Out(Vec::with_capacity(1 << 20));
        o.u8s(&MAGIC);
        let lx = &self.lexicon;
        for bucket in lx.buckets() {
            bucket.keys().iter().for_each(|k| o.u8s(k));
        }
        o.u32s(&lx.payload()[..]);
        o.u64s(&[lx.seed(), lx.buckets_used() as u64]);
        o.u8s(&self.role_of[..]);
        o.u32s(&self.value_of[..]);
        for b in self.shapes.buckets() {
            o.u32s(b.keys());
            o.u16s(b.ids());
        }
        o.u64s(&[self.shapes.seed()]);
        o.u8s(&self.shape_len[..]);
        o.u8s(self.context.as_raw());
        let c = &self.candidates;
        for row in c.all_records() {
            for r in row {
                o.u8s(&[r.span_len, r.op_count, r._pad[0], r._pad[1]]);
                o.u32s(&r.ops);
            }
        }
        c.literals().iter().for_each(|l| o.u8s(l));
        o.u8s(c.literal_lens());
        o.u64s(&[c.literal_count() as u64]);
        o.u8s(&self.cardinal_default[..]);
        o.u64s(&self.initial_cells[..]);
        o.u8s(&self.affix_len[..]);
        o.u64s(&self.words_first[..]);
        o.u64s(&self.plural[..]);
        o.u32s(&self.sense[..]);
        o.u8s(&self.kind_of[..]);
        o.u8s(&self.aux_of[..]);
        for bits in [
            &self.starts,
            &self.per_stop,
            &self.per_units,
            &self.pair_only,
            &self.lone_after,
        ] {
            o.u64s(&bits[..]);
        }
        o.u16s(&self.class_settings[..]);
        o.u16s(&self.domain_settings[..]);
        o.u16s(&[
            self.suffix_roles,
            self.prefix_roles,
            self.connector_roles,
            self.bind_roles,
            self.words_first_roles,
            self.lead_skip_roles,
        ]);
        self.left_folds.iter().for_each(|r| o.u8s(r));
        self.idioms.iter().for_each(|r| o.u16s(r));
        let v = &self.vocab;
        v.names.iter().for_each(|t| o.u8s(t));
        o.u16s(&v.name_id);
        v.months.iter().for_each(|t| o.u8s(t));
        v.hours.iter().for_each(|t| o.u8s(t));
        v.centuries.iter().for_each(|t| o.u8s(t));
        v.zones.iter().for_each(|z| o.u8s(z));
        o.u32s(&[v.not_after_number, v.not_before_number]);
        let sum = fnv(&o.0);
        o.u64s(&[sum]);
        o.0
    }

    /// Tables from the image [`Tables::to_bytes`] wrote.
    pub fn from_bytes(bytes: &[u8]) -> Result<Box<Tables>, LoadError> {
        if bytes.get(..MAGIC.len()) != Some(&MAGIC[..]) {
            return Err(LoadError::Magic);
        }
        let body_len =
            (bytes.len().checked_sub(MAGIC.len() + 8)).ok_or(LoadError::Truncated)? + MAGIC.len();
        let (body, tail) = bytes.split_at(body_len);
        let mut stored = [0u8; 8];
        stored.copy_from_slice(tail);
        if u64::from_le_bytes(stored) != fnv(body) {
            return Err(LoadError::Checksum);
        }
        let mut i = In {
            bytes: &body[MAGIC.len()..],
        };
        let mut t = Tables::boxed_empty();
        let mut buckets = [Bucket::EMPTY; BUCKETS];
        for b in &mut buckets {
            let mut keys: [Key; SLOTS_PER_BUCKET] = [[0; 16]; SLOTS_PER_BUCKET];
            for k in &mut keys {
                *k = i.u8s()?;
            }
            *b = Bucket::from_keys(keys);
        }
        let payload = i.u32s::<SLOTS>()?;
        let [seed, used] = i.u64s::<2>()?;
        let used = usize::try_from(used).map_err(|_| LoadError::Invalid)?;
        if !used.is_power_of_two() || !(MIN_BUCKETS..=BUCKETS).contains(&used) {
            return Err(LoadError::Invalid);
        }
        t.lexicon = Lexicon::from_raw(buckets, payload, seed, used);
        t.role_of = Aligned(i.u8s()?);
        t.value_of = Aligned(i.u32s::<MAX_CANONICAL>()?);
        let mut shapes = [ShapeBucket::EMPTY; SHAPE_BUCKETS];
        for b in &mut shapes {
            *b = ShapeBucket::from_raw(i.u32s::<SHAPE_SLOTS>()?, i.u16s::<SHAPE_SLOTS>()?);
        }
        t.shapes = ShapeTable::from_raw(shapes, i.u64()?);
        t.shape_len = Aligned(i.u8s::<SHAPE_COUNT>()?);
        t.context = ContextLut::from_raw(i.lut()?);
        let mut records = CandidateTable::new().all_records().to_owned();
        for row in &mut records {
            for r in row.iter_mut() {
                let head = i.u8s::<4>()?;
                *r = Candidate {
                    span_len: head[0],
                    op_count: head[1],
                    _pad: [head[2], head[3]],
                    ops: i.u32s::<MAX_OPS>()?,
                };
            }
        }
        let literals = i.rows::<MAX_LITERALS, MAX_LITERAL_LEN>()?;
        let lens = i.u8s::<MAX_LITERALS>()?;
        let count = usize::try_from(i.u64()?).map_err(|_| LoadError::Invalid)?;
        if count > MAX_LITERALS {
            return Err(LoadError::Invalid);
        }
        t.candidates = CandidateTable::from_raw(records, literals, lens, count);
        t.cardinal_default = Aligned(i.u8s::<SHAPE_COUNT>()?);
        t.initial_cells = Aligned(i.u64s::<CELL_COUNT>()?);
        t.affix_len = Aligned(i.u8s::<DOMAIN_COUNT>()?);
        t.words_first = Aligned(i.u64s::<WORDS_FIRST_WORDS>()?);
        t.plural = Aligned(i.u64s::<WORDS_FIRST_WORDS>()?);
        t.sense = Aligned(i.u32s::<MAX_CANONICAL>()?);
        t.kind_of = Aligned(i.u8s::<MAX_CANONICAL>()?);
        t.aux_of = Aligned(i.u8s::<MAX_CANONICAL>()?);
        t.starts = Aligned(i.u64s::<4>()?);
        t.per_stop = Aligned(i.u64s::<4>()?);
        t.per_units = Aligned(i.u64s::<4>()?);
        t.pair_only = Aligned(i.u64s::<4>()?);
        t.lone_after = Aligned(i.u64s::<4>()?);
        t.class_settings = Aligned(i.u16s::<256>()?);
        t.domain_settings = Aligned(i.u16s::<DOMAIN_COUNT>()?);
        let [a, b, c, d, e, f] = i.u16s::<6>()?;
        (t.suffix_roles, t.prefix_roles, t.connector_roles) = (a, b, c);
        (t.bind_roles, t.words_first_roles, t.lead_skip_roles) = (d, e, f);
        t.left_folds = i.rows::<MAX_LEFT_FOLDS, 3>()?;
        t.idioms = i.idioms()?;
        let mut v = Vocab::EMPTY;
        v.names = i.rows::<{ name::COUNT }, 16>()?;
        v.name_id = i.u16s()?;
        v.months = i.rows()?;
        v.hours = i.rows()?;
        v.centuries = i.rows()?;
        v.zones = i.rows::<MAX_ZONES, 3>()?;
        let [after, before] = i.u32s::<2>()?;
        (v.not_after_number, v.not_before_number) = (after, before);
        t.vocab = v;
        if !i.bytes.is_empty() {
            return Err(LoadError::Trailing);
        }
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Internals, Normalizer};

    fn image() -> Vec<u8> {
        crate::tables::english().to_bytes()
    }

    #[test]
    fn the_english_image_loads_to_equal_tables() {
        let loaded = Tables::from_bytes(&image()).expect("load");
        assert!(*loaded == *crate::tables::english());
    }

    #[test]
    fn a_loaded_image_formats_like_the_compiled_tables() {
        let loaded = Tables::from_bytes(&image()).expect("load");
        let engine = Normalizer::new(&loaded);
        assert_eq!(
            engine.normalize("meet at five thirty pm on march third"),
            "meet at 5:30 pm on March 3rd"
        );
    }

    #[test]
    fn bad_images_are_refused() {
        let good = image();
        assert_eq!(Tables::from_bytes(&[]).err(), Some(LoadError::Magic));
        let mut other = good.clone();
        other[7] = 9;
        assert_eq!(Tables::from_bytes(&other).err(), Some(LoadError::Magic));
        assert_eq!(
            Tables::from_bytes(&good[..good.len() - 1]).err(),
            Some(LoadError::Checksum)
        );
        let mut flipped = good.clone();
        flipped[good.len() / 2] ^= 1;
        assert_eq!(
            Tables::from_bytes(&flipped).err(),
            Some(LoadError::Checksum)
        );
        let mut longer = good[..good.len() - 8].to_vec();
        longer.push(0);
        let sum = fnv(&longer);
        longer.extend_from_slice(&sum.to_le_bytes());
        assert_eq!(Tables::from_bytes(&longer).err(), Some(LoadError::Trailing));
        let mut shorter = good[..good.len() - 9].to_vec();
        let sum = fnv(&shorter);
        shorter.extend_from_slice(&sum.to_le_bytes());
        assert_eq!(
            Tables::from_bytes(&shorter).err(),
            Some(LoadError::Truncated)
        );
        assert_eq!(Tables::from_bytes(&MAGIC).err(), Some(LoadError::Truncated));
    }
}
