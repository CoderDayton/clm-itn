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
use crate::tables::lexicon::{BUCKETS, Bucket, Key, Lexicon, MIN_BUCKETS, SLOTS_PER_BUCKET};
use crate::tables::shape::{SHAPE_SLOTS, ShapeBucket, ShapeTable};
use crate::tables::{Aligned, MAX_ZONES, Tables, Vocab, WORDS_FIRST_WORDS};
use crate::types::{DOMAIN_COUNT, MAX_IDIOM_LEN, MAX_IDIOMS, MAX_LEFT_FOLDS, name};

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
    fn fill_u8s(&mut self, out: &mut [u8]) -> Result<(), LoadError> {
        out.copy_from_slice(self.take(out.len())?);
        Ok(())
    }
    fn fill_u32s(&mut self, out: &mut [u32]) -> Result<(), LoadError> {
        let b = self.take(4 * out.len())?;
        for (o, c) in out.iter_mut().zip(b.as_chunks::<4>().0) {
            *o = u32::from_le_bytes(*c);
        }
        Ok(())
    }
    fn fill_u64s(&mut self, out: &mut [u64]) -> Result<(), LoadError> {
        let b = self.take(8 * out.len())?;
        for (o, c) in out.iter_mut().zip(b.as_chunks::<8>().0) {
            *o = u64::from_le_bytes(*c);
        }
        Ok(())
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
        i.lexicon(&mut t.lexicon)?;
        t.role_of = Aligned(i.u8s()?);
        i.fill_u32s(&mut t.value_of[..])?;
        i.shapes(&mut t.shapes)?;
        i.fill_u8s(&mut t.shape_len[..])?;
        i.fill_u8s(t.context.as_raw_mut())?;
        i.candidates(&mut t.candidates)?;
        i.fill_u8s(&mut t.cardinal_default[..])?;
        i.fill_u64s(&mut t.initial_cells[..])?;
        t.affix_len = Aligned(i.u8s::<DOMAIN_COUNT>()?);
        t.words_first = Aligned(i.u64s::<WORDS_FIRST_WORDS>()?);
        t.plural = Aligned(i.u64s::<WORDS_FIRST_WORDS>()?);
        i.fill_u32s(&mut t.sense[..])?;
        i.fill_u8s(&mut t.kind_of[..])?;
        i.fill_u8s(&mut t.aux_of[..])?;
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
        i.vocab(&mut t.vocab)?;
        if !i.bytes.is_empty() {
            return Err(LoadError::Trailing);
        }
        Ok(t)
    }
}

impl In<'_> {
    fn lexicon(&mut self, lx: &mut Lexicon) -> Result<(), LoadError> {
        let (buckets, payload) = lx.raw_mut();
        for b in buckets.iter_mut() {
            let mut keys: [Key; SLOTS_PER_BUCKET] = [[0; 16]; SLOTS_PER_BUCKET];
            for k in &mut keys {
                *k = self.u8s()?;
            }
            *b = Bucket::from_keys(keys);
        }
        self.fill_u32s(payload)?;
        let [seed, used] = self.u64s::<2>()?;
        let used = usize::try_from(used).map_err(|_| LoadError::Invalid)?;
        if !used.is_power_of_two() || !(MIN_BUCKETS..=BUCKETS).contains(&used) {
            return Err(LoadError::Invalid);
        }
        lx.set_hash(seed, used);
        Ok(())
    }

    fn shapes(&mut self, s: &mut ShapeTable) -> Result<(), LoadError> {
        for b in s.buckets_mut().iter_mut() {
            *b = ShapeBucket::from_raw(self.u32s::<SHAPE_SLOTS>()?, self.u16s::<SHAPE_SLOTS>()?);
        }
        s.set_seed(self.u64()?);
        Ok(())
    }

    fn candidates(&mut self, c: &mut CandidateTable) -> Result<(), LoadError> {
        for row in c.all_records_mut() {
            for r in row.iter_mut() {
                let head = self.u8s::<4>()?;
                *r = Candidate {
                    span_len: head[0],
                    op_count: head[1],
                    _pad: [head[2], head[3]],
                    ops: self.u32s::<MAX_OPS>()?,
                };
            }
        }
        let literals = self.rows::<MAX_LITERALS, MAX_LITERAL_LEN>()?;
        let lens = self.u8s::<MAX_LITERALS>()?;
        let count = usize::try_from(self.u64()?).map_err(|_| LoadError::Invalid)?;
        if count > MAX_LITERALS {
            return Err(LoadError::Invalid);
        }
        c.set_literals(literals, lens, count);
        Ok(())
    }

    fn vocab(&mut self, v: &mut Vocab) -> Result<(), LoadError> {
        v.names = self.rows::<{ name::COUNT }, 16>()?;
        v.name_id = self.u16s()?;
        v.months = self.rows()?;
        v.hours = self.rows()?;
        v.centuries = self.rows()?;
        v.zones = self.rows::<MAX_ZONES, 3>()?;
        let [after, before] = self.u32s::<2>()?;
        (v.not_after_number, v.not_before_number) = (after, before);
        Ok(())
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
