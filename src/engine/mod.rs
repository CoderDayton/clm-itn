//! The formatting engine.

pub mod span;
pub mod window;

use alloc::string::String;

use crate::tables::context::{abstains, select};
use crate::tables::{Tables, WORDS_FIRST_WORDS};
use core::fmt;

use crate::types::{
    CANDIDATE_NONE, CELL_ABSTAINED, CLASS_BOUNDARY, CLASS_UNKNOWN, CandidateId, ClassId,
    DOMAIN_CARDINAL, DOMAIN_COUNT, DOMAIN_IDENTIFIER, DOMAIN_ORDINAL, DOMAIN_PLAIN, DOMAIN_TIME,
    DomainId, FormatError, MAX_TOKENS, ROLE_AMPM, ROLE_DIGIT, ROLE_MER, SHAPE_UNKNOWN, Setting,
    SettingBits, ShapeId, SpanReceipt, TokenInfo, cell_index, cell_shape,
};
use span::{Affixes, Span, segment};

/// Domain, candidate, cell, lead length, and whether the lead stays as spoken.
type Chosen = (DomainId, CandidateId, u16, u8, bool);

/// What [`Internals::format`] wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Formatted {
    /// Bytes written to the output buffer.
    pub len: usize,
    /// Number of spans that were rewritten.
    pub spans: usize,
}

/// Turns spoken-form text into written form.
///
/// A `Normalizer` is a small value that borrows read-only tables. It is `Copy`,
/// `Send` and `Sync`, so build it once and share it. Every [`Setting`] starts
/// on.
///
/// ```
/// let engine = clm_itn::Normalizer::english();
/// assert_eq!(engine.normalize("at five thirty pm"), "at 5:30 pm");
/// ```
#[derive(Clone, Copy)]
pub struct Normalizer<'t> {
    tables: &'t Tables,
    off: SettingBits,
}

struct Off(SettingBits);

impl fmt::Debug for Off {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let off = Setting::ALL.iter().filter(|s| self.0 & s.bit() != 0);
        f.debug_list().entries(off).finish()
    }
}

impl fmt::Debug for Normalizer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Normalizer")
            .field("off", &Off(self.off))
            .finish_non_exhaustive()
    }
}

impl Default for Normalizer<'static> {
    fn default() -> Self {
        Normalizer::english()
    }
}

/// Engine entry points for this repo's tests, bench and tools. Not public API.
#[cfg(feature = "internals")]
pub trait Internals<'t>: Sized {
    /// A normalizer over `tables`, with every [`Setting`] on.
    fn new(tables: &'t Tables) -> Self;
    /// The tables this normalizer reads.
    fn tables(&self) -> &'t Tables;
    /// Rewrites at most `MAX_TOKENS` pre-split tokens into `out` with the
    /// table engine alone, without the line layer's rules.
    ///
    /// # Errors
    ///
    /// [`FormatError::TooManyTokens`] past `MAX_TOKENS` tokens and
    /// [`FormatError::Overflow`] if `out` is too small.
    fn format(&self, tokens: &[&[u8]], out: &mut [u8]) -> Result<Formatted, FormatError>;
    /// [`format`](Self::format), recording which cell rewrote each span.
    ///
    /// # Errors
    ///
    /// As [`format`](Self::format).
    fn format_receipts(
        &self,
        tokens: &[&[u8]],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Result<Formatted, FormatError>;
    /// [`format_receipts`](Self::format_receipts), timing each tier with `now`.
    fn format_profiled(
        &self,
        tokens: &[&[u8]],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
        now: &mut impl FnMut() -> u64,
    ) -> (Result<Formatted, FormatError>, [u64; 4]);
}

#[cfg(feature = "internals")]
impl<'t> Internals<'t> for Normalizer<'t> {
    fn new(tables: &'t Tables) -> Self {
        Normalizer { tables, off: 0 }
    }

    fn tables(&self) -> &'t Tables {
        self.tables
    }

    fn format(&self, tokens: &[&[u8]], out: &mut [u8]) -> Result<Formatted, FormatError> {
        let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
        self.format_receipts(tokens, out, &mut receipts)
    }

    fn format_receipts(
        &self,
        tokens: &[&[u8]],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Result<Formatted, FormatError> {
        if tokens.len() > MAX_TOKENS {
            return Err(FormatError::TooManyTokens);
        }
        let mut infos = [TokenInfo::default(); MAX_TOKENS];
        self.tables.lexicon.lookup_window(tokens, &mut infos);
        self.format_known(tokens, infos, out, receipts)
    }

    fn format_profiled(
        &self,
        tokens: &[&[u8]],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
        now: &mut impl FnMut() -> u64,
    ) -> (Result<Formatted, FormatError>, [u64; 4]) {
        self.profiled(tokens, out, receipts, now)
    }
}

impl Normalizer<'static> {
    /// A normalizer over the built-in English tables, which are compiled into
    /// the binary. Building it allocates nothing.
    #[must_use]
    pub fn english() -> Normalizer<'static> {
        Normalizer {
            tables: crate::tables::english::english(),
            off: 0,
        }
    }
}

impl<'t> Normalizer<'t> {
    /// Switches `setting` off.
    ///
    /// ```
    /// use clm_itn::{Normalizer, Setting};
    ///
    /// let engine = Normalizer::english()
    ///     .without(Setting::SingleDigits)
    ///     .without(Setting::Currency);
    /// assert_eq!(engine.normalize("eighty dollars"), "80 dollars");
    /// assert_eq!(engine.normalize("like eight years"), "like eight years");
    /// ```
    #[must_use]
    pub const fn without(mut self, setting: Setting) -> Self {
        self.off |= setting.bit();
        self
    }

    /// Switches `setting` back on.
    #[must_use]
    pub const fn with(mut self, setting: Setting) -> Self {
        self.off &= !setting.bit();
        self
    }

    /// Whether `setting` is on.
    #[must_use]
    pub const fn is_on(&self, setting: Setting) -> bool {
        self.off & setting.bit() == 0
    }

    pub(crate) fn is_digit_word(&self, canonical: u16, role: u8) -> bool {
        role == ROLE_DIGIT && self.value(canonical) < 10
    }

    /// Rewrites a line of spoken-form text and returns it as a new `String`.
    ///
    /// The line can be any length. Numbers are never cut, and punctuation
    /// stays where it was. This allocates; to reuse a buffer, use
    /// [`normalize_into`](Self::normalize_into).
    ///
    /// ```
    /// let engine = clm_itn::Normalizer::english();
    /// assert_eq!(engine.normalize("room four oh two."), "room 402.");
    /// ```
    #[must_use]
    pub fn normalize(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        self.normalize_into(text, &mut out);
        out
    }

    /// Like [`normalize`](Self::normalize), but appends to `out`. Clear `out`
    /// first to reuse one buffer for many lines.
    ///
    /// ```
    /// let engine = clm_itn::Normalizer::english();
    /// let mut out = String::new();
    /// for line in ["fifty percent off", "it costs five euros"] {
    ///     out.clear();
    ///     engine.normalize_into(line, &mut out);
    ///     println!("{out}");
    /// }
    /// ```
    pub fn normalize_into(&self, text: &str, out: &mut String) {
        window::format_line(self, text, out);
    }

    /// Rewrites at most `MAX_TOKENS` tokens, whose lexicon entries are already
    /// in `infos`, into `out`. Only the line layer's rules make this correct
    /// on its own words; see [`window::format_line`].
    pub(crate) fn format_infos(
        &self,
        tokens: &[&[u8]],
        infos: [TokenInfo; MAX_TOKENS],
        out: &mut [u8],
    ) -> Result<Formatted, FormatError> {
        debug_assert!(tokens.len() <= MAX_TOKENS);
        let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
        self.format_known(tokens, infos, out, &mut receipts)
    }

    #[inline(always)]
    fn format_known(
        &self,
        tokens: &[&[u8]],
        mut infos: [TokenInfo; MAX_TOKENS],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Result<Formatted, FormatError> {
        let n = tokens.len();
        let t = self.tables;
        let classes = self.classes(&mut infos, n);
        let mut spans = [Span::default(); MAX_TOKENS];
        let known = |k| t.shapes.lookup(k) != SHAPE_UNKNOWN;
        let span_count = segment(&classes[..n], &t.role_of, self.affixes(), known, &mut spans);

        let mut pos = 0usize;
        let mut rendered = 0usize;
        let mut next_span = 0usize;
        let mut i = 0usize;
        while i < n {
            if i > 0 {
                put(out, &mut pos, b" ")?;
            }
            if next_span < span_count && spans[next_span].start as usize == i {
                let span = spans[next_span];
                next_span += 1;
                let len = span.len as usize;
                if let Some(pick) = self.choose(&span, &infos) {
                    let at = i..i + len;
                    let soft = self.soft(span.left);
                    self.render_span(pick, soft, &infos[at.clone()], &tokens[at], out, &mut pos)?;
                    let cell = pick.2;
                    receipts[rendered] = SpanReceipt {
                        cell,
                        start: span.start,
                        len: span.len,
                    };
                    rendered += 1;
                    i += len;
                    continue;
                }
            }
            put(out, &mut pos, tokens[i])?;
            i += 1;
        }
        Ok(Formatted {
            len: pos,
            spans: rendered,
        })
    }

    #[cfg(feature = "internals")]
    fn profiled(
        &self,
        tokens: &[&[u8]],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
        now: &mut impl FnMut() -> u64,
    ) -> (Result<Formatted, FormatError>, [u64; 4]) {
        let mut d = [0u64; 4];
        let n = tokens.len();
        if n > MAX_TOKENS {
            return (Err(FormatError::TooManyTokens), d);
        }
        let t = self.tables;
        let t0 = now();
        let mut infos = [TokenInfo::default(); MAX_TOKENS];
        t.lexicon.lookup_window(tokens, &mut infos);
        let t1 = now();
        d[0] = t1.wrapping_sub(t0);

        let classes = self.classes(&mut infos, n);
        let mut spans = [Span::default(); MAX_TOKENS];
        let known = |k| t.shapes.lookup(k) != SHAPE_UNKNOWN;
        let span_count = segment(&classes[..n], &t.role_of, self.affixes(), known, &mut spans);
        let mut routes = [None; MAX_TOKENS];
        for (r, span) in routes.iter_mut().zip(&spans[..span_count]) {
            *r = self.route(span, &infos);
        }
        let t2 = now();
        d[1] = t2.wrapping_sub(t1);

        let mut chosen = [None; MAX_TOKENS];
        for (c, r) in chosen.iter_mut().zip(&routes[..span_count]) {
            *c = r.and_then(|(shape, domain, abstain, len, lead)| {
                self.pick(shape, domain, abstain, len, lead)
            });
        }
        let t3 = now();
        d[2] = t3.wrapping_sub(t2);

        let result =
            self.render_spans(tokens, &infos, &spans[..span_count], &chosen, out, receipts);
        if result.is_ok() {
            d[3] = now().wrapping_sub(t3);
        }
        (result, d)
    }

    /// Token classes, with classes of switched-off settings read as unknown.
    #[inline(always)]
    fn classes(&self, infos: &mut [TokenInfo; MAX_TOKENS], n: usize) -> [u8; MAX_TOKENS] {
        let mut classes = [0u8; MAX_TOKENS];
        for (c, info) in classes.iter_mut().zip(&mut infos[..n]) {
            let hide = self.tables.class_settings[info.class as usize] & self.off != 0;
            info.class = select(hide, info.class, CLASS_UNKNOWN);
            *c = info.class;
        }
        classes
    }

    #[cfg(feature = "internals")]
    #[inline(always)]
    fn render_spans(
        &self,
        tokens: &[&[u8]],
        infos: &[TokenInfo; MAX_TOKENS],
        spans: &[Span],
        chosen: &[Option<Chosen>; MAX_TOKENS],
        out: &mut [u8],
        receipts: &mut [SpanReceipt; MAX_TOKENS],
    ) -> Result<Formatted, FormatError> {
        let n = tokens.len();
        let mut pos = 0usize;
        let mut rendered = 0usize;
        let mut next_span = 0usize;
        let mut i = 0usize;
        while i < n {
            if i > 0 {
                put(out, &mut pos, b" ")?;
            }
            if next_span < spans.len() && spans[next_span].start as usize == i {
                let span = spans[next_span];
                let pick = chosen[next_span];
                next_span += 1;
                let len = span.len as usize;
                if let Some(pick) = pick {
                    let at = i..i + len;
                    let soft = self.soft(span.left);
                    self.render_span(pick, soft, &infos[at.clone()], &tokens[at], out, &mut pos)?;
                    let cell = pick.2;
                    receipts[rendered] = SpanReceipt {
                        cell,
                        start: span.start,
                        len: span.len,
                    };
                    rendered += 1;
                    i += len;
                    continue;
                }
            }
            put(out, &mut pos, tokens[i])?;
            i += 1;
        }
        Ok(Formatted {
            len: pos,
            spans: rendered,
        })
    }

    #[inline(always)]
    fn render_span(
        &self,
        (domain, cand, cell, lead, spoken_lead): Chosen,
        soft: bool,
        infos: &[TokenInfo],
        tokens: &[&[u8]],
        out: &mut [u8],
        pos: &mut usize,
    ) -> Result<(), FormatError> {
        let c = &self.tables.candidates;
        // "pm" or "o'clock" in the span is a firm cue of its own.
        let soft = soft
            && !infos.iter().any(|i| {
                let role = self.tables.role_of[i.class as usize] & 0x0F;
                role == ROLE_AMPM || role == ROLE_MER
            });
        let fallback = if cell == CELL_ABSTAINED || domain == DOMAIN_CARDINAL {
            CANDIDATE_NONE
        } else {
            self.tables.cardinal_default[cell_shape(cell) as usize]
        };
        let lead = lead as usize;
        let mut values = [0u32; MAX_TOKENS];
        for (v, info) in values.iter_mut().zip(infos) {
            *v = self.value(info.canonical);
        }
        if lead > 0 && spoken_lead {
            put(out, pos, tokens[0])?;
            put(out, pos, b" ")?;
        } else if lead > 0 {
            c.write_literal(values[0] as usize, out, pos)?;
        }
        c.render_or(
            domain,
            (cand, fallback, soft),
            &values[lead..infos.len()],
            &tokens[lead..],
            out,
            pos,
        )?;
        let last = infos.last().map_or(0, |i| i.canonical as usize);
        let plural = (self.tables.plural[(last >> 6) % WORDS_FIRST_WORDS] >> (last & 63)) & 1;
        put(out, pos, &b"s"[..plural as usize])
    }

    /// Whether `left` is a fold's class, a weak cue ("it was").
    #[inline(always)]
    fn soft(&self, left: ClassId) -> bool {
        left != 0 && self.tables.left_folds.iter().any(|f| f[2] == left)
    }

    #[inline(always)]
    fn choose(&self, span: &Span, infos: &[TokenInfo; MAX_TOKENS]) -> Option<Chosen> {
        let (shape, domain, abstain, len, lead) = self.route(span, infos)?;
        self.pick(shape, domain, abstain, len, lead)
    }

    #[inline(always)]
    fn affixes(&self) -> Affixes {
        Affixes {
            suffix: self.tables.suffix_roles,
            prefix: self.tables.prefix_roles,
            starts: self.tables.starts.0,
            per_stop: self.tables.per_stop.0,
            per_units: self.tables.per_units.0,
            pair_only: self.tables.pair_only.0,
            connectors: self.tables.connector_roles,
            binds: self.tables.bind_roles,
            lead_skip: self.tables.lead_skip_roles,
            folds: self.tables.left_folds,
        }
    }

    #[inline(always)]
    fn route(
        &self,
        span: &Span,
        infos: &[TokenInfo; MAX_TOKENS],
    ) -> Option<(ShapeId, DomainId, bool, u8, u8)> {
        let t = self.tables;
        let shape = t.shapes.lookup(span.key);
        if shape == SHAPE_UNKNOWN || t.shape_len[shape as usize] != span.core {
            return None;
        }
        let (domain, confidence) = t.context.lookup(span.left, span.right);
        let abstain = abstains(confidence);
        let r = span.right as usize;
        let lone_after = (t.lone_after.0[(r >> 6) & 3] >> (r & 63)) & 1 == 1;
        let info = infos[span.start as usize % MAX_TOKENS];
        let c = info.canonical as usize;
        let marked = (t.words_first[(c >> 6) % WORDS_FIRST_WORDS] >> (c & 63)) & 1 == 1;
        let role = t.role_of[info.class as usize] & 0x0F;
        let any_domain = (t.words_first_roles >> role) & 1 == 1;
        let base = abstain
            | (domain == DOMAIN_CARDINAL)
            | (domain == DOMAIN_ORDINAL)
            | ((domain == DOMAIN_TIME) & (any_domain | lone_after))
            | ((domain == DOMAIN_IDENTIFIER) & any_domain);
        let inside = (span.left != CLASS_BOUNDARY) | (span.right != CLASS_BOUNDARY);
        let before_month = (t.starts.0[(r >> 6) & 3] >> (r & 63)) & 1 == 1;
        let digit_word = self.is_on(Setting::SingleDigits)
            & (span.len == 1)
            & self.is_digit_word(info.canonical, role);
        let words =
            marked & (span.len == 1) & base & inside & !span.dropped & !before_month & !digit_word;
        Some((
            shape,
            select(words, domain, DOMAIN_PLAIN),
            (abstain & !words) | digit_word,
            span.len,
            span.lead,
        ))
    }

    #[inline(always)]
    fn pick(
        &self,
        shape: ShapeId,
        domain: DomainId,
        abstain: bool,
        len: u8,
        lead: u8,
    ) -> Option<Chosen> {
        let t = self.tables;
        let cell = cell_index(domain, shape);
        let default = t.cardinal_default[shape as usize];
        let second = t.domain_settings[domain as usize % DOMAIN_COUNT] & self.off != 0;
        let ranked = (t.initial_cells[cell as usize] >> (8 * u32::from(second))) as CandidateId;
        let fallback = abstain | (ranked == CANDIDATE_NONE);
        let cand = select(fallback, ranked, default);
        let d = select(fallback, domain, DOMAIN_CARDINAL);
        let have = t.candidates.span_len(d, cand);
        let lead = if have == len {
            0
        } else if have == len - lead && lead > 0 {
            lead
        } else {
            return None;
        };
        let cell = if abstain { CELL_ABSTAINED } else { cell };
        Some((d, cand, cell, lead, second))
    }

    #[inline(always)]
    fn value(&self, canonical: u16) -> u32 {
        self.tables
            .value_of
            .get(canonical as usize)
            .copied()
            .unwrap_or(0)
    }
}

#[inline(always)]
fn put(out: &mut [u8], pos: &mut usize, bytes: &[u8]) -> Result<(), FormatError> {
    let end = pos
        .checked_add(bytes.len())
        .filter(|&e| e <= out.len())
        .ok_or(FormatError::Overflow)?;
    out[*pos..end].copy_from_slice(bytes);
    *pos = end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::candidate::{Candidate, op};
    use crate::tables::cell::pack;
    use crate::tables::context::{ContextLut, pack_entry};
    use crate::tables::lexicon::Lexicon;
    use crate::tables::shape::ShapeTable;
    use crate::tables::{Aligned, MAX_CANONICAL, Tables};
    use crate::types::*;

    const DIGIT: ClassId = 2;
    const TENS: ClassId = 3;
    const PM: ClassId = 4;
    const AT: ClassId = 5;

    const SHAPE_DT: ShapeId = 1;
    const SHAPE_D: ShapeId = 2;

    const C_FOLD2: CandidateId = 1;
    const C_TIME: CandidateId = 2;
    const C_FOLD1: CandidateId = 3;
    const C_RAW1: CandidateId = 4;

    fn tables() -> Box<Tables> {
        let lexicon = Lexicon::build(&[
            (b"five", 5, DIGIT),
            (b"six", 6, DIGIT),
            (b"thirty", 30, TENS),
            (b"pm", 31, PM),
            (b"at", 32, AT),
        ])
        .expect("lexicon");
        let mut role_of = [0; 256];
        role_of[DIGIT as usize] = 1;
        role_of[TENS as usize] = 3;
        let mut value_of = [0u32; MAX_CANONICAL];
        for (i, v) in value_of.iter_mut().enumerate().take(33) {
            *v = i as u32;
        }
        let shapes = ShapeTable::build(&[(0x31, SHAPE_DT), (0x1, SHAPE_D)]).expect("shapes");
        let mut shape_len = [0; crate::types::SHAPE_COUNT];
        shape_len[SHAPE_DT as usize] = 2;
        shape_len[SHAPE_D as usize] = 1;
        let mut context = ContextLut::new();
        for left in 0..=255u8 {
            context.set(left, PM, pack_entry(DOMAIN_TIME, 15));
        }
        context.set(AT, CLASS_BOUNDARY, pack_entry(DOMAIN_CARDINAL, 8));
        let mut t = Tables::boxed_empty();
        let candidates = &mut t.candidates;
        let colon = candidates.add_literal(b":").expect("literal");
        let time = [
            op::fold(0, 1),
            op::fold(1, 1),
            op::emit(0, 1),
            op::lit(colon),
            op::emit(1, 2),
        ];
        for d in 0..crate::types::DOMAIN_COUNT as DomainId {
            let fold2 = Candidate::new(2, &[op::fold(0, 2), op::emit(0, 0)]);
            candidates.set(d, C_FOLD2, fold2);
            candidates.set(d, C_TIME, Candidate::new(2, &time));
            let fold1 = Candidate::new(1, &[op::fold(0, 1), op::emit(0, 0)]);
            candidates.set(d, C_FOLD1, fold1);
            candidates.set(d, C_RAW1, Candidate::new(1, &[op::raw(1)]));
        }
        let mut cardinal_default = [CANDIDATE_NONE; crate::types::SHAPE_COUNT];
        cardinal_default[SHAPE_DT as usize] = C_FOLD2;
        cardinal_default[SHAPE_D as usize] = C_FOLD1;
        let mut initial_cells = [0u64; CELL_COUNT];
        for (cell, ranks) in [
            (cell_index(DOMAIN_TIME, SHAPE_DT), [C_TIME, C_FOLD2, 0, 0]),
            (
                cell_index(DOMAIN_CARDINAL, SHAPE_DT),
                [C_FOLD2, C_TIME, 0, 0],
            ),
            (
                cell_index(DOMAIN_CARDINAL, SHAPE_D),
                [C_FOLD1, C_RAW1, 0, 0],
            ),
        ] {
            initial_cells[cell as usize] = pack(ranks, [0; 4]);
        }
        t.lexicon = lexicon;
        t.role_of = Aligned(role_of);
        t.value_of = Aligned(value_of);
        t.shapes = shapes;
        t.shape_len = Aligned(shape_len);
        t.context = context;
        t.cardinal_default = Aligned(cardinal_default);
        t.initial_cells = Aligned(initial_cells);
        t
    }

    fn fmt(engine: &Normalizer, text: &str) -> (String, Vec<SpanReceipt>) {
        let tokens: Vec<&[u8]> = text.split(' ').map(str::as_bytes).collect();
        let mut out = [0u8; 128];
        let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
        let done = engine
            .format_receipts(&tokens, &mut out, &mut receipts)
            .expect("format");
        (
            String::from_utf8(out[..done.len].to_vec()).expect("utf8"),
            receipts[..done.spans].to_vec(),
        )
    }

    #[test]
    fn time_context_formats_span_and_keeps_context_word() {
        let t = tables();
        let engine = Normalizer::new(&t);
        let (text, receipts) = fmt(&engine, "five thirty PM");
        assert_eq!(text, "5:30 PM");
        assert_eq!(
            receipts,
            vec![SpanReceipt {
                cell: cell_index(DOMAIN_TIME, SHAPE_DT),
                start: 0,
                len: 2
            }]
        );
    }

    #[test]
    fn low_confidence_uses_cardinal_default_and_abstained_receipt() {
        let t = tables();
        let engine = Normalizer::new(&t);
        let (text, receipts) = fmt(&engine, "hello five thirty");
        assert_eq!(text, "hello five thirty");
        assert_eq!(receipts[0].cell, CELL_ABSTAINED);
        assert_eq!((receipts[0].start, receipts[0].len), (1, 2));
    }

    #[test]
    fn unknown_words_pass_through_unchanged() {
        let t = tables();
        let engine = Normalizer::new(&t);
        let (text, receipts) = fmt(&engine, "Hello World");
        assert_eq!(text, "Hello World");
        assert!(receipts.is_empty());
    }

    #[test]
    fn unknown_shape_passes_through() {
        let t = tables();
        let engine = Normalizer::new(&t);
        assert_eq!(fmt(&engine, "thirty five").0, "thirty five");
    }

    #[test]
    fn confident_cardinal_reads_cell() {
        let t = tables();
        let engine = Normalizer::new(&t).without(Setting::SingleDigits);
        let (text, receipts) = fmt(&engine, "at five");
        assert_eq!(text, "at 5");
        assert_eq!(receipts[0].cell, cell_index(DOMAIN_CARDINAL, SHAPE_D));
    }

    #[test]
    fn small_buffer_overflows() {
        let t = tables();
        let engine = Normalizer::new(&t);
        let tokens: [&[u8]; 3] = [b"five", b"thirty", b"pm"];
        let mut out = [0u8; 4];
        assert_eq!(
            engine.format(&tokens, &mut out).err(),
            Some(FormatError::Overflow)
        );
    }

    #[test]
    fn more_than_max_tokens_is_rejected() {
        let t = tables();
        let engine = Normalizer::new(&t);
        let tokens: [&[u8]; 9] = [b"at"; 9];
        let mut out = [0u8; 64];
        assert_eq!(
            engine.format(&tokens, &mut out).err(),
            Some(FormatError::TooManyTokens)
        );
    }

    #[test]
    fn candidate_of_the_wrong_length_leaves_the_span_raw() {
        let mut t = tables();
        let wrong = pack([C_FOLD2, 0, 0, 0], [0; 4]);
        t.initial_cells[cell_index(DOMAIN_CARDINAL, SHAPE_D) as usize] = wrong;
        t.cardinal_default[SHAPE_D as usize] = C_FOLD2;
        let engine = Normalizer::new(&t);
        assert_eq!(fmt(&engine, "five").0, "five");
        assert_eq!(fmt(&engine, "at five").0, "at five");
    }

    #[test]
    fn affix_split_reads_money_and_percent_on_plain_shapes() {
        let engine = Normalizer::new(crate::tables::english());
        for (text, want) in [
            ("costs two thousand five dollars", "costs $2,005"),
            ("twenty three thousand dollars", "$23,000"),
            ("five point two percent", "5.2%"),
            ("five percent", "5%"),
            ("fifty cents", "50 cents"),
        ] {
            assert_eq!(fmt(&engine, text).0, want, "{text}");
        }
    }

    #[test]
    fn litv_reads_units_currencies_and_keeps_the_meridiem() {
        let engine = Normalizer::new(crate::tables::english());
        for (text, want) in [
            ("two hundred meters", "200 meters"),
            ("eighteen point five kilometers", "18.5 kilometers"),
            ("five euros", "5 euros"),
            ("five dollars", "$5"),
            ("fifty cents", "50 cents"),
            ("five percent", "5%"),
            ("at five thirty pm", "at 5:30 pm"),
            ("call at five am", "call at 5 am"),
        ] {
            assert_eq!(fmt(&engine, text).0, want, "{text}");
        }
    }

    #[test]
    fn two_token_units_stay_words() {
        let engine = Normalizer::new(crate::tables::english());
        for (text, want) in [
            ("two square meters", "2 square meters"),
            ("five cubic meters", "5 cubic meters"),
            ("eighteen square kilometers", "18 square kilometers"),
            ("two kilo watt", "2 kilo watt"),
            ("seven point five peta bytes", "7.5 peta bytes"),
            ("one fifty c c", "150 c c"),
            ("five square", "five square"),
            ("five kilo", "five kilo"),
            ("five kilometers", "5 kilometers"),
            ("five percent", "5%"),
            ("five euros", "5 euros"),
        ] {
            assert_eq!(fmt(&engine, text).0, want, "{text}");
        }
    }

    #[test]
    fn minus_stays_a_word() {
        let engine = Normalizer::new(crate::tables::english());
        for (text, want) in [
            ("minus sixty six kilograms", "minus 66 kilograms"),
            (
                "minus two thousand twelve kilo liters",
                "minus 2,012 kilo liters",
            ),
            ("minus five dollars", "minus $5"),
            ("minus five percent", "minus 5%"),
            ("minus two square meters", "minus 2 square meters"),
            ("minus sixty", "minus 60"),
            ("minus kilograms", "minus kilograms"),
        ] {
            assert_eq!(fmt(&engine, text).0, want, "{text}");
        }
    }

    #[test]
    fn composite_money_writes_dollars_and_cents() {
        let engine = Normalizer::new(crate::tables::english());
        for (text, want) in [
            ("three dollars twenty", "$3.20"),
            ("three dollars ninety nine", "$3.99"),
            ("four dollars and fifty cents", "$4.50"),
            ("forty eight dollars and ninety six cents", "$48.96"),
            ("seven dollars fifty cents", "$7.50"),
        ] {
            assert_eq!(fmt(&engine, text).0, want, "{text}");
        }
    }

    type Profiled = (
        Result<(String, Vec<SpanReceipt>), FormatError>,
        [u64; 4],
        u64,
    );

    fn profiled(engine: &Normalizer, tokens: &[&[u8]], out_len: usize) -> Profiled {
        let mut out = [0u8; 128];
        let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
        let ticks = [0u64, 10, 30, 60, 100];
        let mut calls = 0u64;
        let mut now = || {
            calls += 1;
            ticks[(calls - 1) as usize % ticks.len()]
        };
        let (r, d) = engine.format_profiled(tokens, &mut out[..out_len], &mut receipts, &mut now);
        let r = r.map(|done| {
            (
                String::from_utf8(out[..done.len].to_vec()).expect("utf8"),
                receipts[..done.spans].to_vec(),
            )
        });
        (r, d, calls)
    }

    #[test]
    fn profiled_output_matches_format() {
        let t = tables();
        let engine = Normalizer::new(&t);
        for text in [
            "five thirty PM",
            "hello five thirty",
            "Hello World",
            "thirty five",
            "at five",
            "at five six thirty pm",
        ] {
            let tokens: Vec<&[u8]> = text.split(' ').map(str::as_bytes).collect();
            let (r, _, _) = profiled(&engine, &tokens, 128);
            assert_eq!(r, Ok(fmt(&engine, text)), "{text}");
        }
    }

    #[test]
    fn profiled_reports_four_stage_deltas_from_five_clock_reads() {
        let t = tables();
        let engine = Normalizer::new(&t);
        let tokens: [&[u8]; 3] = [b"five", b"thirty", b"pm"];
        let (r, d, calls) = profiled(&engine, &tokens, 128);
        assert!(r.is_ok());
        assert_eq!((d, calls), ([10, 20, 30, 40], 5));
    }

    #[test]
    fn profiled_overflow_keeps_measured_stages_and_zero_render() {
        let t = tables();
        let engine = Normalizer::new(&t);
        let tokens: [&[u8]; 3] = [b"five", b"thirty", b"pm"];
        let (r, d, calls) = profiled(&engine, &tokens, 4);
        assert_eq!(r, Err(FormatError::Overflow));
        assert_eq!((d, calls), ([10, 20, 30, 0], 4));
    }

    #[test]
    fn profiled_too_many_tokens_reads_no_clock() {
        let t = tables();
        let engine = Normalizer::new(&t);
        let tokens: [&[u8]; 9] = [b"at"; 9];
        let (r, d, calls) = profiled(&engine, &tokens, 128);
        assert_eq!(r, Err(FormatError::TooManyTokens));
        assert_eq!((d, calls), ([0; 4], 0));
    }
}
