use crate::types::{
    CANDIDATE_NONE, CandidateId, DOMAIN_CARDINAL, DOMAIN_COUNT, DomainId, FormatError,
};

pub const MAX_OPS: usize = 15;
pub const FIELDS: usize = 4;
pub const MAX_LITERALS: usize = 255;
pub const MAX_LITERAL_LEN: usize = 15;
pub const CANDIDATES: usize = 256;

pub type Records = [[Candidate; CANDIDATES]; DOMAIN_COUNT];

#[inline(always)]
const fn row(domain: DomainId) -> usize {
    domain as usize % DOMAIN_COUNT
}

pub mod op {
    pub const FOLD: u8 = 1;
    pub const DIGITS: u8 = 2;
    pub const EMIT: u8 = 3;
    pub const LIT: u8 = 4;
    pub const ORD: u8 = 5;
    pub const RAW: u8 = 6;
    pub const SKIP: u8 = 7;
    pub const LITV: u8 = 8;
    pub const CHECK: u8 = 9;
    /// Like `CHECK`, but a field out of range renders the fallback candidate
    /// passed to `render_or` instead of the raw words ("19:99" is no time).
    pub const FIT: u8 = 10;

    pub const fn encode(opcode: u8, a: u8, b: u8, c: u8) -> u32 {
        u32::from_le_bytes([opcode, a, b, c])
    }

    pub const fn fold(field: u8, n: u8) -> u32 {
        encode(FOLD, field, n, 0)
    }

    pub const fn digits(field: u8, n: u8) -> u32 {
        encode(DIGITS, field, n, 0)
    }

    pub const fn emit(field: u8, width: u8) -> u32 {
        encode(EMIT, field, width, 0)
    }

    /// No thousands commas.
    pub const GROUP_NONE: u8 = 0;
    /// Thousands commas from 1,000 up.
    pub const GROUP: u8 = 1;
    /// Thousands commas, except numbers that read as years (see `year_like`).
    pub const GROUP_NOT_YEAR: u8 = 2;

    pub const fn emit_grouped(field: u8, group: u8) -> u32 {
        encode(EMIT, field, 0, group)
    }

    pub const fn lit(id: u8) -> u32 {
        encode(LIT, id, 0, 0)
    }

    pub const fn ord(field: u8) -> u32 {
        encode(ORD, field, 0, 0)
    }

    pub const fn raw(n: u8) -> u32 {
        encode(RAW, n, 0, 0)
    }

    pub const fn skip(n: u8) -> u32 {
        encode(SKIP, n, 0, 0)
    }

    pub const fn litv(k: u8) -> u32 {
        encode(LITV, k, 0, 0)
    }

    pub const fn check(field: u8, lo: u8, hi: u8) -> u32 {
        encode(CHECK, field, lo, hi)
    }

    pub const fn fit(field: u8, lo: u8, hi: u8) -> u32 {
        encode(FIT, field, lo, hi)
    }
}

#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Candidate {
    pub span_len: u8,
    pub op_count: u8,
    pub _pad: [u8; 2],
    pub ops: [u32; MAX_OPS],
}

impl Candidate {
    pub const fn new(span_len: u8, ops: &[u32]) -> Candidate {
        let mut c = Candidate {
            span_len,
            op_count: if ops.len() > MAX_OPS {
                u8::MAX
            } else {
                ops.len() as u8
            },
            _pad: [0; 2],
            ops: [0; MAX_OPS],
        };
        let mut i = 0;
        while i < ops.len() && i < MAX_OPS {
            c.ops[i] = ops[i];
            i += 1;
        }
        c
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateError {
    NoneCandidate,
    TooManyOps,
    UnknownOpcode { op: usize },
    BadField { op: usize },
    UnknownLiteral { op: usize },
    LengthMismatch { span_len: u8, consumed: u32 },
    LiteralTooLong,
    LiteralPoolFull,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CandidateTable {
    records: Records,
    literals: [[u8; MAX_LITERAL_LEN]; MAX_LITERALS],
    literal_lens: [u8; MAX_LITERALS],
    literal_count: usize,
}

impl CandidateTable {
    pub const fn new() -> CandidateTable {
        CandidateTable {
            records: [[Candidate {
                span_len: 0,
                op_count: 0,
                _pad: [0; 2],
                ops: [0; MAX_OPS],
            }; CANDIDATES]; DOMAIN_COUNT],
            literals: [[0; MAX_LITERAL_LEN]; MAX_LITERALS],
            literal_lens: [0; MAX_LITERALS],
            literal_count: 0,
        }
    }

    pub const fn from_raw(
        records: Records,
        literals: [[u8; MAX_LITERAL_LEN]; MAX_LITERALS],
        literal_lens: [u8; MAX_LITERALS],
        literal_count: usize,
    ) -> CandidateTable {
        CandidateTable {
            records,
            literals,
            literal_lens,
            literal_count,
        }
    }

    pub fn records(&self, domain: DomainId) -> &[Candidate; CANDIDATES] {
        &self.records[row(domain)]
    }

    pub fn all_records(&self) -> &Records {
        &self.records
    }

    pub fn literals(&self) -> &[[u8; MAX_LITERAL_LEN]; MAX_LITERALS] {
        &self.literals
    }

    pub fn literal_lens(&self) -> &[u8; MAX_LITERALS] {
        &self.literal_lens
    }

    pub fn literal_count(&self) -> usize {
        self.literal_count
    }

    pub fn set(&mut self, domain: DomainId, id: CandidateId, candidate: Candidate) {
        self.records[row(domain)][id as usize] = candidate;
    }

    #[inline(always)]
    pub fn span_len(&self, domain: DomainId, id: CandidateId) -> u8 {
        self.records[row(domain)][id as usize].span_len
    }

    pub fn add_literal(&mut self, text: &[u8]) -> Result<u8, CandidateError> {
        if text.len() > MAX_LITERAL_LEN {
            return Err(CandidateError::LiteralTooLong);
        }
        if self.literal_count == MAX_LITERALS {
            return Err(CandidateError::LiteralPoolFull);
        }
        let id = self.literal_count;
        self.literals[id][..text.len()].copy_from_slice(text);
        self.literal_lens[id] = text.len() as u8;
        self.literal_count += 1;
        Ok(id as u8)
    }

    pub fn validate(&self, domain: DomainId, id: CandidateId) -> Result<(), CandidateError> {
        if id == 0 {
            return Err(CandidateError::NoneCandidate);
        }
        let c = &self.records[row(domain)][id as usize];
        let count = c.op_count as usize;
        if count > MAX_OPS {
            return Err(CandidateError::TooManyOps);
        }
        let mut consumed = 0u32;
        for (i, &word) in c.ops[..count].iter().enumerate() {
            let [opcode, a, b, group] = word.to_le_bytes();
            let field_ok = (a as usize) < FIELDS;
            match opcode {
                op::FOLD | op::DIGITS if field_ok => consumed += u32::from(b),
                op::EMIT if field_ok && group <= op::GROUP_NOT_YEAR => {}
                op::ORD | op::CHECK | op::FIT if field_ok => {}
                op::FOLD | op::DIGITS | op::EMIT | op::ORD | op::CHECK | op::FIT => {
                    return Err(CandidateError::BadField { op: i });
                }
                op::LIT if (a as usize) < self.literal_count => {}
                op::LIT => return Err(CandidateError::UnknownLiteral { op: i }),
                op::RAW | op::SKIP => consumed += u32::from(a),
                op::LITV if a < c.span_len => {}
                op::LITV => return Err(CandidateError::BadField { op: i }),
                _ => return Err(CandidateError::UnknownOpcode { op: i }),
            }
        }
        if consumed != u32::from(c.span_len) {
            return Err(CandidateError::LengthMismatch {
                span_len: c.span_len,
                consumed,
            });
        }
        Ok(())
    }

    pub fn render(
        &self,
        domain: DomainId,
        id: CandidateId,
        values: &[u32],
        raw: &[&[u8]],
        out: &mut [u8],
        pos: &mut usize,
    ) -> Result<(), FormatError> {
        self.render_or(domain, (id, CANDIDATE_NONE, false), values, raw, out, pos)
    }

    /// Renders candidate `id`; when a `FIT` fails, renders the cardinal
    /// candidate `fallback` if it reads the same words. Else a `soft` cue keeps
    /// the raw words ("it was one ninety nine"), and a firm one renders `id`
    /// anyway ("at nine sixty nine pm" stays a time).
    #[inline(always)]
    pub fn render_or(
        &self,
        domain: DomainId,
        (id, fallback, soft): (CandidateId, CandidateId, bool),
        values: &[u32],
        raw: &[&[u8]],
        out: &mut [u8],
        pos: &mut usize,
    ) -> Result<(), FormatError> {
        if self.run(domain, id, values, raw, out, pos, true)? {
            return Ok(());
        }
        let same = self.span_len(DOMAIN_CARDINAL, fallback) == self.span_len(domain, id);
        let start = *pos;
        if fallback != CANDIDATE_NONE
            && same
            && self.run(DOMAIN_CARDINAL, fallback, values, raw, out, pos, true)?
        {
            // A plain reading that is only the spoken words is no reading.
            if !is_spoken(&out[start..*pos], raw) {
                return Ok(());
            }
            *pos = start;
        }
        if soft {
            return write_joined(out, pos, raw);
        }
        self.run(domain, id, values, raw, out, pos, false)
            .map(|_| ())
    }

    /// Runs one candidate. `Ok(false)`: a `FIT` failed under `strict` and
    /// nothing was written.
    #[allow(clippy::too_many_arguments)] // private; a struct would only rename them
    #[inline(always)]
    fn run(
        &self,
        domain: DomainId,
        id: CandidateId,
        values: &[u32],
        raw: &[&[u8]],
        out: &mut [u8],
        pos: &mut usize,
        strict: bool,
    ) -> Result<bool, FormatError> {
        let c = &self.records[row(domain)][id as usize];
        let count = (c.op_count as usize).min(MAX_OPS);
        let mut fields = [0u64; FIELDS];
        let mut ok = true;
        let mut fits = true;
        let mut w = *pos;
        let mut t = 0usize;
        for &word in &c.ops[..count] {
            let [opcode, a, b, group] = word.to_le_bytes();
            let f = a as usize % FIELDS;
            match opcode {
                op::FOLD => {
                    let (v, fits) = fold(take(values, &mut t, b));
                    fields[f] = v;
                    ok &= fits;
                }
                op::CHECK => {
                    let [_, _, _, hi] = word.to_le_bytes();
                    ok &= (u64::from(b)..=u64::from(hi)).contains(&fields[f]);
                }
                op::FIT => {
                    let [_, _, _, hi] = word.to_le_bytes();
                    fits &= (u64::from(b)..=u64::from(hi)).contains(&fields[f]);
                }
                op::DIGITS => {
                    for &v in take(values, &mut t, b) {
                        fields[f] = fields[f]
                            .wrapping_mul(pow10_digits(v))
                            .wrapping_add(u64::from(v));
                    }
                }
                op::EMIT => {
                    let v = fields[f];
                    let year = group == op::GROUP_NOT_YEAR && year_like(v);
                    if group != op::GROUP_NONE && v >= 1000 && !year {
                        write_grouped(out, &mut w, v)?;
                    } else {
                        write_decimal(out, &mut w, v, b as usize)?;
                    }
                }
                op::LIT => self.write_literal(a as usize, out, &mut w)?,
                op::ORD => write(out, &mut w, ordinal_suffix(fields[f]))?,
                op::RAW => write_joined(out, &mut w, take(raw, &mut t, a))?,
                op::SKIP => t = t.saturating_add(a as usize),
                op::LITV => {
                    let id = values.get(a as usize).map_or(usize::MAX, |&v| v as usize);
                    self.write_literal(id, out, &mut w)?;
                }
                _ => {}
            }
        }
        if strict && !fits {
            return Ok(false);
        }
        if !ok {
            w = *pos;
            write_joined(out, &mut w, raw)?;
        }
        *pos = w;
        Ok(true)
    }

    #[inline(always)]
    pub fn write_literal(
        &self,
        id: usize,
        out: &mut [u8],
        pos: &mut usize,
    ) -> Result<(), FormatError> {
        if id < self.literal_count {
            let len = self.literal_lens[id] as usize;
            write(out, pos, &self.literals[id][..len])?;
        }
        Ok(())
    }
}

#[inline(always)]
fn take<'a, T>(items: &'a [T], t: &mut usize, n: u8) -> &'a [T] {
    let start = (*t).min(items.len());
    *t = t.saturating_add(n as usize);
    &items[start..(*t).min(items.len())]
}

#[inline(always)]
fn fold(values: &[u32]) -> (u64, bool) {
    let (mut total, mut cur, mut ok) = (0u64, 0u64, true);
    for &v in values {
        let p = pow10_digits(v);
        let v = u64::from(v);
        if v == 100 {
            cur = cur.max(1).wrapping_mul(100);
        } else if v >= 1000 {
            let low = total % v;
            let group = if low > 0 { low + cur } else { cur.max(1) };
            total = (total - low).wrapping_add(group.wrapping_mul(v));
            cur = 0;
        } else {
            ok &= cur % p == 0;
            cur = cur.wrapping_add(v);
        }
    }
    (total.wrapping_add(cur), ok)
}

#[inline(always)]
fn pow10_digits(v: u32) -> u64 {
    let mut p = 10u64;
    let mut x = v / 10;
    while x > 0 {
        p *= 10;
        x /= 10;
    }
    p
}

#[inline(always)]
fn ordinal_suffix(v: u64) -> &'static [u8] {
    match (v % 100, v % 10) {
        (11..=13, _) => b"th",
        (_, 1) => b"st",
        (_, 2) => b"nd",
        (_, 3) => b"rd",
        _ => b"th",
    }
}

#[inline(always)]
fn reserve(out: &[u8], w: usize, len: usize) -> Result<usize, FormatError> {
    w.checked_add(len)
        .filter(|&end| end <= out.len())
        .ok_or(FormatError::Overflow)
}

#[inline(always)]
fn write(out: &mut [u8], w: &mut usize, bytes: &[u8]) -> Result<(), FormatError> {
    let end = reserve(out, *w, bytes.len())?;
    out[*w..end].copy_from_slice(bytes);
    *w = end;
    Ok(())
}

#[inline(always)]
fn write_joined(out: &mut [u8], w: &mut usize, texts: &[&[u8]]) -> Result<(), FormatError> {
    for (i, text) in texts.iter().enumerate() {
        if i > 0 {
            write(out, w, b" ")?;
        }
        write(out, w, text)?;
    }
    Ok(())
}

/// Whether `text` is `words` joined by single spaces, as `write_joined` writes them.
fn is_spoken(text: &[u8], words: &[&[u8]]) -> bool {
    let mut rest = text;
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            let Some(r) = rest.strip_prefix(b" ") else {
                return false;
            };
            rest = r;
        }
        let Some(r) = rest.strip_prefix(*word) else {
            return false;
        };
        rest = r;
    }
    rest.is_empty()
}

#[inline(always)]
fn write_decimal(
    out: &mut [u8],
    w: &mut usize,
    mut v: u64,
    width: usize,
) -> Result<(), FormatError> {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let digits = &buf[i..];
    let zeros = width.saturating_sub(digits.len());
    let end = reserve(out, *w, zeros + digits.len())?;
    out[*w..*w + zeros].fill(b'0');
    out[*w + zeros..end].copy_from_slice(digits);
    *w = end;
    Ok(())
}

/// "two thousand eight" is a year more often than a count.
#[inline(always)]
fn year_like(v: u64) -> bool {
    (2000..=2099).contains(&v)
}

/// `v` with a comma between each group of three digits.
#[inline(always)]
fn write_grouped(out: &mut [u8], w: &mut usize, mut v: u64) -> Result<(), FormatError> {
    let mut buf = [0u8; 26];
    let mut i = buf.len();
    let mut n = 0;
    loop {
        if n > 0 && n % 3 == 0 {
            i -= 1;
            buf[i] = b',';
        }
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
        if v == 0 {
            break;
        }
    }
    write(out, w, &buf[i..])
}

impl Default for CandidateTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: DomainId = 3;
    const TIME: CandidateId = 1;
    const CARDINAL: CandidateId = 2;
    const DECIMAL: CandidateId = 3;
    const ORDINAL: CandidateId = 4;
    const VERBATIM: CandidateId = 5;
    const TIME3: CandidateId = 6;

    fn table() -> CandidateTable {
        let mut t = CandidateTable::new();
        let colon = t.add_literal(b":").expect("colon");
        let dot = t.add_literal(b".").expect("dot");
        t.set(
            D,
            TIME,
            Candidate::new(
                2,
                &[
                    op::fold(0, 1),
                    op::emit(0, 1),
                    op::lit(colon),
                    op::fold(1, 1),
                    op::emit(1, 2),
                ],
            ),
        );
        t.set(
            D,
            TIME3,
            Candidate::new(
                3,
                &[
                    op::fold(0, 1),
                    op::emit(0, 1),
                    op::lit(colon),
                    op::fold(1, 2),
                    op::emit(1, 2),
                ],
            ),
        );
        t.set(
            D,
            CARDINAL,
            Candidate::new(4, &[op::fold(0, 4), op::emit(0, 1)]),
        );
        t.set(
            D,
            DECIMAL,
            Candidate::new(
                3,
                &[
                    op::fold(0, 1),
                    op::emit(0, 1),
                    op::skip(1),
                    op::lit(dot),
                    op::digits(1, 1),
                    op::emit(1, 1),
                ],
            ),
        );
        t.set(
            D,
            ORDINAL,
            Candidate::new(2, &[op::fold(0, 2), op::emit(0, 1), op::ord(0)]),
        );
        t.set(D, VERBATIM, Candidate::new(2, &[op::raw(2)]));
        t
    }

    fn render(t: &CandidateTable, id: CandidateId, values: &[u32]) -> String {
        let raw: Vec<&[u8]> = values.iter().map(|_| b"w".as_slice()).collect();
        let mut out = [0u8; 64];
        let mut pos = 0;
        t.render(D, id, values, &raw, &mut out, &mut pos)
            .expect("render");
        String::from_utf8(out[..pos].to_vec()).expect("utf8")
    }

    #[test]
    fn each_domain_has_its_own_ids() {
        let mut t = table();
        t.set(D + 1, TIME, Candidate::new(1, &[op::raw(1)]));
        assert_eq!(render(&t, TIME, &[5, 30]), "5:30");
        assert_eq!((t.span_len(D, TIME), t.span_len(D + 1, TIME)), (2, 1));
        assert_eq!(t.span_len(D + 2, TIME), 0);
        assert_eq!(t.validate(D + 1, TIME), Ok(()));
    }

    #[test]
    fn from_raw_round_trips() {
        let t = table();
        let copy = CandidateTable::from_raw(
            *t.all_records(),
            *t.literals(),
            *t.literal_lens(),
            t.literal_count(),
        );
        assert!(copy == t);
        assert_eq!(render(&copy, TIME, &[5, 30]), "5:30");
    }

    #[test]
    fn op_word_packs_opcode_then_operands() {
        assert_eq!(op::encode(op::EMIT, 1, 2, 3), 0x0302_0103);
        assert_eq!(op::fold(2, 4), 0x0004_0201);
        assert_eq!(op::skip(1), 0x0000_0107);
    }

    #[test]
    fn candidate_record_is_one_cache_line() {
        assert_eq!(core::mem::size_of::<Candidate>(), 64);
        assert_eq!(core::mem::align_of::<Candidate>(), 64);
    }

    #[test]
    fn candidate_new_sets_count_and_ops() {
        let c = Candidate::new(2, &[op::raw(2)]);
        assert_eq!(
            (c.span_len, c.op_count, c.ops[0], c.ops[1]),
            (2, 1, op::raw(2), 0)
        );
    }

    #[test]
    fn time_five_thirty() {
        assert_eq!(render(&table(), TIME, &[5, 30]), "5:30");
    }

    #[test]
    fn time_six_forty() {
        assert_eq!(render(&table(), TIME, &[6, 40]), "6:40");
    }

    #[test]
    fn time_five_oh_five() {
        assert_eq!(render(&table(), TIME3, &[5, 0, 5]), "5:05");
    }

    #[test]
    fn cardinal_one_hundred_twenty_three() {
        assert_eq!(render(&table(), CARDINAL, &[1, 100, 20, 3]), "123");
    }

    #[test]
    fn cardinal_two_thousand_five() {
        let mut t = table();
        t.set(
            D,
            CARDINAL,
            Candidate::new(3, &[op::fold(0, 3), op::emit(0, 1)]),
        );
        assert_eq!(render(&t, CARDINAL, &[2, 1000, 5]), "2005");
    }

    #[test]
    fn cardinal_bare_hundred_and_scale_after_hundreds() {
        let mut t = table();
        t.set(
            D,
            CARDINAL,
            Candidate::new(1, &[op::fold(0, 1), op::emit(0, 1)]),
        );
        assert_eq!(render(&t, CARDINAL, &[100]), "100");
        t.set(
            D,
            CARDINAL,
            Candidate::new(4, &[op::fold(0, 4), op::emit(0, 1)]),
        );
        assert_eq!(render(&t, CARDINAL, &[3, 100, 1000, 7]), "300007");
    }

    #[test]
    fn decimal_five_point_two() {
        assert_eq!(render(&table(), DECIMAL, &[5, 0, 2]), "5.2");
    }

    #[test]
    fn digits_concatenates_values_including_zero() {
        let mut t = table();
        t.set(
            D,
            CARDINAL,
            Candidate::new(4, &[op::digits(0, 4), op::emit(0, 1)]),
        );
        assert_eq!(render(&t, CARDINAL, &[1, 0, 23, 4]), "10234");
    }

    #[test]
    fn emit_zero_pads_to_width() {
        let mut t = table();
        t.set(
            D,
            CARDINAL,
            Candidate::new(1, &[op::fold(0, 1), op::emit(0, 4)]),
        );
        assert_eq!(render(&t, CARDINAL, &[7]), "0007");
        assert_eq!(render(&t, CARDINAL, &[12345]), "12345");
    }

    #[test]
    fn grouped_emit_adds_thousands_commas() {
        let mut t = CandidateTable::new();
        for (id, group) in [(1, op::GROUP), (2, op::GROUP_NOT_YEAR)] {
            let ops = [op::fold(0, 1), op::emit_grouped(0, group)];
            t.set(D, id, Candidate::new(1, &ops));
            assert_eq!(t.validate(D, id), Ok(()));
        }
        assert_eq!(render(&t, 1, &[999]), "999");
        assert_eq!(render(&t, 1, &[1000]), "1,000");
        assert_eq!(render(&t, 1, &[2005]), "2,005");
        assert_eq!(render(&t, 1, &[1_234_567_890]), "1,234,567,890");
        assert_eq!(render(&t, 2, &[2005]), "2005");
        assert_eq!(render(&t, 2, &[2000]), "2000");
        assert_eq!(render(&t, 2, &[1500]), "1,500");
        assert_eq!(render(&t, 2, &[1950]), "1,950");
        assert_eq!(render(&t, 2, &[2100]), "2,100");
        assert_eq!(render(&t, 2, &[1000]), "1,000");
        t.set(
            D,
            3,
            Candidate::new(1, &[op::fold(0, 1), op::emit_grouped(0, 3)]),
        );
        assert_eq!(t.validate(D, 3), Err(CandidateError::BadField { op: 1 }));
    }

    #[test]
    fn ordinal_twenty_one() {
        assert_eq!(render(&table(), ORDINAL, &[20, 1]), "21st");
    }

    #[test]
    fn ordinal_suffixes() {
        let t = table();
        let cases = [
            (0, 1, "1st"),
            (0, 2, "2nd"),
            (0, 3, "3rd"),
            (0, 4, "4th"),
            (0, 11, "11th"),
            (0, 12, "12th"),
            (0, 13, "13th"),
            (20, 2, "22nd"),
            (90, 3, "93rd"),
        ];
        for (a, b, want) in cases {
            assert_eq!(render(&t, ORDINAL, &[a, b]), want);
        }
        let mut t = table();
        t.set(
            D,
            ORDINAL,
            Candidate::new(3, &[op::fold(0, 3), op::emit(0, 1), op::ord(0)]),
        );
        assert_eq!(render(&t, ORDINAL, &[1, 100, 11]), "111th");
        assert_eq!(render(&t, ORDINAL, &[1, 100, 12]), "112th");
    }

    #[test]
    fn raw_writes_texts_joined_by_single_spaces() {
        let t = table();
        let mut out = [0u8; 32];
        let mut pos = 0;
        t.render(
            D,
            VERBATIM,
            &[5, 30],
            &[b"Five", b"thirty"],
            &mut out,
            &mut pos,
        )
        .expect("render");
        assert_eq!(&out[..pos], b"Five thirty");
    }

    #[test]
    fn render_appends_at_pos() {
        let t = table();
        let mut out = *b"at ______";
        let mut pos = 3;
        t.render(D, TIME, &[5, 30], &[b"five", b"thirty"], &mut out, &mut pos)
            .expect("render");
        assert_eq!(pos, 7);
        assert_eq!(&out, b"at 5:30__");
    }

    #[test]
    fn litv_writes_the_literal_named_by_a_token_value_and_reads_nothing() {
        let mut t = CandidateTable::new();
        let dollar = t.add_literal(b"$").expect("lit");
        let meters = t.add_literal(b" m").expect("lit");
        let money = Candidate::new(
            2,
            &[op::litv(1), op::fold(0, 1), op::emit(0, 0), op::skip(1)],
        );
        let measure = Candidate::new(
            2,
            &[op::fold(0, 1), op::emit(0, 0), op::litv(1), op::skip(1)],
        );
        t.set(D, 1, money);
        t.set(D, 2, measure);
        assert_eq!(t.validate(D, 1), Ok(()));
        assert_eq!(t.validate(D, 2), Ok(()));
        assert_eq!(render(&t, 1, &[5, u32::from(dollar)]), "$5");
        assert_eq!(render(&t, 2, &[200, u32::from(meters)]), "200 m");
        assert_eq!(render(&t, 2, &[7, 99]), "7");
    }

    #[test]
    fn fold_that_breaks_place_value_renders_the_span_raw() {
        let mut t = CandidateTable::new();
        t.set(D, 1, Candidate::new(2, &[op::fold(0, 2), op::emit(0, 0)]));
        t.set(D, 2, Candidate::new(3, &[op::fold(0, 3), op::emit(0, 0)]));
        assert_eq!(render(&t, 1, &[20, 5]), "25");
        assert_eq!(render(&t, 1, &[50, 50]), "w w");
        assert_eq!(render(&t, 1, &[1, 5]), "w w");
        assert_eq!(render(&t, 1, &[20, 15]), "w w");
        assert_eq!(render(&t, 2, &[1, 100, 5]), "105");
        assert_eq!(render(&t, 2, &[60, 1, 100]), "6100");
        assert_eq!(render(&t, 2, &[5, 1000, 20]), "5020");
        assert_eq!(render(&t, 2, &[1, 100, 0]), "100");
    }

    #[test]
    fn check_renders_the_span_raw_when_a_field_is_out_of_range() {
        let mut t = CandidateTable::new();
        let ops = [
            op::fold(0, 1),
            op::fold(1, 1),
            op::check(0, 10, 29),
            op::emit(0, 0),
            op::emit(1, 2),
        ];
        t.set(D, 1, Candidate::new(2, &ops));
        assert_eq!(t.validate(D, 1), Ok(()));
        assert_eq!(render(&t, 1, &[19, 5]), "1905");
        assert_eq!(render(&t, 1, &[20, 26]), "2026");
        assert_eq!(render(&t, 1, &[50, 15]), "w w");
        t.set(D, 2, Candidate::new(1, &[op::check(4, 0, 1), op::raw(1)]));
        assert_eq!(t.validate(D, 2), Err(CandidateError::BadField { op: 0 }));
    }

    #[test]
    fn fit_out_of_range_renders_the_fallback_else_the_candidate_anyway() {
        let mut t = CandidateTable::new();
        let time = [
            op::fold(0, 1),
            op::fold(1, 1),
            op::fit(1, 0, 59),
            op::emit(0, 0),
            op::emit(1, 3),
        ];
        let plain = [
            op::fold(0, 1),
            op::fold(1, 1),
            op::emit(0, 0),
            op::emit(1, 2),
        ];
        t.set(D, 1, Candidate::new(2, &time));
        t.set(DOMAIN_CARDINAL, 2, Candidate::new(2, &plain));
        t.set(
            DOMAIN_CARDINAL,
            3,
            Candidate::new(1, &[op::fold(0, 1), op::emit(0, 0)]),
        );
        assert_eq!(t.validate(D, 1), Ok(()));
        let go = |fallback, values: &[u32]| {
            let mut out = [0u8; 16];
            let mut pos = 0;
            t.render_or(
                D,
                (1, fallback, false),
                values,
                &[b"w", b"w"],
                &mut out,
                &mut pos,
            )
            .expect("render");
            String::from_utf8(out[..pos].to_vec()).expect("utf8")
        };
        assert_eq!(go(2, &[5, 30]), "5030");
        assert_eq!(go(2, &[19, 99]), "1999");
        assert_eq!(go(CANDIDATE_NONE, &[19, 99]), "19099");
        assert_eq!(go(3, &[19, 99]), "19099");
    }

    #[test]
    fn validate_rejects_litv_past_the_span() {
        let mut t = CandidateTable::new();
        t.set(D, 1, Candidate::new(1, &[op::litv(1), op::raw(1)]));
        assert_eq!(t.validate(D, 1), Err(CandidateError::BadField { op: 0 }));
    }

    #[test]
    fn validate_accepts_well_formed_candidates() {
        let t = table();
        for id in [TIME, TIME3, CARDINAL, DECIMAL, ORDINAL, VERBATIM] {
            assert_eq!(t.validate(D, id), Ok(()), "id {id}");
        }
    }

    #[test]
    fn validate_rejects_none_id() {
        assert_eq!(table().validate(D, 0), Err(CandidateError::NoneCandidate));
    }

    #[test]
    fn validate_rejects_wrong_length_sum() {
        let mut t = table();
        t.set(D, 9, Candidate::new(3, &[op::fold(0, 1), op::skip(1)]));
        assert_eq!(
            t.validate(D, 9),
            Err(CandidateError::LengthMismatch {
                span_len: 3,
                consumed: 2
            })
        );
    }

    #[test]
    fn validate_rejects_bad_field() {
        let mut t = table();
        t.set(D, 9, Candidate::new(1, &[op::fold(4, 1), op::emit(0, 1)]));
        assert_eq!(t.validate(D, 9), Err(CandidateError::BadField { op: 0 }));
        t.set(D, 9, Candidate::new(1, &[op::fold(0, 1), op::emit(7, 1)]));
        assert_eq!(t.validate(D, 9), Err(CandidateError::BadField { op: 1 }));
    }

    #[test]
    fn validate_rejects_unknown_literal() {
        let mut t = table();
        t.set(D, 9, Candidate::new(1, &[op::raw(1), op::lit(2)]));
        assert_eq!(
            t.validate(D, 9),
            Err(CandidateError::UnknownLiteral { op: 1 })
        );
    }

    #[test]
    fn validate_rejects_unknown_opcode_and_op_count() {
        let mut t = table();
        t.set(
            D,
            9,
            Candidate::new(1, &[op::raw(1), op::encode(11, 0, 0, 0)]),
        );
        assert_eq!(
            t.validate(D, 9),
            Err(CandidateError::UnknownOpcode { op: 1 })
        );
        let mut c = Candidate::new(1, &[op::raw(1)]);
        c.op_count = 16;
        t.set(D, 9, c);
        assert_eq!(t.validate(D, 9), Err(CandidateError::TooManyOps));
    }

    #[test]
    fn literal_pool_limits() {
        let mut t = CandidateTable::new();
        assert_eq!(
            t.add_literal(&[b'x'; 16]),
            Err(CandidateError::LiteralTooLong)
        );
        for i in 0..MAX_LITERALS {
            assert_eq!(t.add_literal(&[b'x'; 15]), Ok(i as u8));
        }
        assert_eq!(t.add_literal(b"y"), Err(CandidateError::LiteralPoolFull));
    }

    #[test]
    fn overflow_on_two_byte_buffer_writes_nothing_past_end() {
        let t = table();
        let mut buf = [0xAAu8; 8];
        let mut pos = 0;
        let result = t.render(
            D,
            TIME,
            &[5, 30],
            &[b"five", b"thirty"],
            &mut buf[..2],
            &mut pos,
        );
        assert_eq!(result, Err(FormatError::Overflow));
        assert_eq!(pos, 0);
        assert!(buf[2..].iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn overflow_when_pos_is_already_at_end() {
        let t = table();
        let mut out = [0u8; 4];
        let mut pos = 4;
        assert_eq!(
            t.render(D, VERBATIM, &[0, 0], &[b"a", b"b"], &mut out, &mut pos),
            Err(FormatError::Overflow)
        );
        assert_eq!(pos, 4);
    }

    #[test]
    fn exact_fit_succeeds() {
        let t = table();
        let mut out = [0u8; 4];
        let mut pos = 0;
        t.render(D, TIME, &[5, 30], &[b"five", b"thirty"], &mut out, &mut pos)
            .expect("fits");
        assert_eq!(&out, b"5:30");
    }
}
