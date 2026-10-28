/// The most tokens one [`Normalizer::format`](crate::Normalizer::format) call takes.
pub const MAX_TOKENS: usize = 8;
/// Rows in `Tables::left_folds`.
pub const MAX_LEFT_FOLDS: usize = 2;
pub const MAX_KEY_LEN: usize = 15;
/// Rows in `Tables::idioms`, and words in one.
pub const MAX_IDIOMS: usize = 8;
pub const MAX_IDIOM_LEN: usize = 6;

/// Word-sense bits of a lexicon word, in `Tables::sense`. The line rules that
/// keep a number word as a word read them.
pub mod sense {
    /// "one" to "nine".
    pub const COUNT: u32 = 1;
    pub const ONE: u32 = 1 << 1;
    /// "one" after these is a pronoun ("the old one").
    pub const ONE_AFTER: u32 = 1 << 2;
    /// "one" before these closes a clause ("pick one that works").
    pub const ONE_CLAUSE: u32 = 1 << 3;
    /// "one" before these is a pronoun when no number follows ("one of").
    pub const ONE_BEFORE: u32 = 1 << 4;
    /// A count before "of" and these is a word ("the two of us").
    pub const GROUP: u32 = 1 << 5;
    pub const FRACTION: u32 = 1 << 6;
    /// A word of a `Tables::idioms` row.
    pub const IDIOM: u32 = 1 << 7;
    /// Joins a minute word to an hour ("ten to four").
    pub const RELATIVE: u32 = 1 << 8;
    pub const HOUR: u32 = 1 << 9;
    pub const MINUTE: u32 = 1 << 10;
    /// A relative time after these is a clock reading ("it is ten to four").
    pub const TIME_CUE: u32 = 1 << 11;
    pub const A: u32 = 1 << 12;
    pub const AN: u32 = 1 << 13;
    pub const BY: u32 = 1 << 14;
    pub const DAY: u32 = 1 << 15;
    pub const IN: u32 = 1 << 16;
    pub const PIECE: u32 = 1 << 17;
    pub const MILLION: u32 = 1 << 18;
    pub const OF: u32 = 1 << 19;
    pub const TWENTY: u32 = 1 << 20;
    pub const FIVE: u32 = 1 << 21;
    pub const QUARTER: u32 = 1 << 22;
    pub const AND: u32 = 1 << 23;
    /// "am", "pm" or the "m" of "a m".
    pub const MERIDIEM: u32 = 1 << 24;
    /// A word after which a month name is a verb ("we march").
    pub const SUBJECT: u32 = 1 << 25;
}

/// Word kinds in `Tables::kind_of`: the words the line passes look for. A
/// kind with an `aux` value in `Tables::aux_of` says what it carries.
pub mod kind {
    /// The article before a scale or unit ("a hundred").
    pub const A: u8 = 1;
    /// A lone letter. Never in the table; found from the spelling.
    pub const LETTER: u8 = 2;
    pub const OF: u8 = 3;
    pub const PER: u8 = 4;
    pub const DOT: u8 = 5;
    /// Repeats the digit after it; `aux` is the copies added ("double" 1).
    pub const DOUBLE: u8 = 6;
    pub const HUNDRED: u8 = 7;
    pub const STREET: u8 = 8;
    /// A month name that can be a verb; `aux` 1 also lets an adverb follow.
    pub const MONTH_VERB: u8 = 9;
    /// "1" to "12" written as digits. Never in the table.
    pub const CLOCK_DIGITS: u8 = 10;
    pub const HUNDREDS: u8 = 11;
    pub const FILLER: u8 = 12;
    /// "thousand" and above; `aux` is the power of ten.
    pub const SCALE: u8 = 13;
    /// "noon" or "midnight".
    pub const NOON: u8 = 14;
}

/// Words the engine writes or compares by name, in `Vocab::names`.
pub mod name {
    pub const ONE: usize = 0;
    pub const TWO: usize = 1;
    pub const OH: usize = 2;
    pub const THOUSAND: usize = 3;
    pub const BILLION: usize = 4;
    pub const PERCENT: usize = 5;
    /// The two-word spelling of `PERCENT`; has no ID.
    pub const PER_CENT: usize = 6;
    pub const TWENTY: usize = 7;
    pub const TWELVE: usize = 8;
    pub const HOURS: usize = 9;
    pub const AND: usize = 10;
    pub const CENT: usize = 11;
    pub const FIRST: usize = 12;
    /// The meridiem a spelled "a m" joins to.
    pub const AM: usize = 13;
    pub const DOT: usize = 14;
    pub const COUNT: usize = 15;
}

pub type ClassId = u8;
pub const CLASS_BOUNDARY: ClassId = 0;
pub const CLASS_UNKNOWN: ClassId = 1;

pub type CanonicalId = u16;

pub type Role = u8;
pub const ROLE_NONE: Role = 0;
pub const ROLE_DIGIT: Role = 1;
pub const ROLE_TEEN: Role = 2;
pub const ROLE_TENS: Role = 3;
pub const ROLE_HUNDRED: Role = 4;
pub const ROLE_SCALE: Role = 5;
pub const ROLE_POINT: Role = 6;
pub const ROLE_AND: Role = 7;
pub const ROLE_OH: Role = 8;
pub const ROLE_ORDINAL: Role = 9;
pub const ROLE_MSCALE: Role = 10;
pub const ROLE_CURRENCY: Role = 11;
pub const ROLE_AMPM: Role = 12;
pub const ROLE_UNIT: Role = 13;
pub const ROLE_MINUS: Role = 14;
pub const ROLE_MER: Role = 15;

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

/// Why [`Normalizer::format`](crate::Normalizer::format) failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormatError {
    /// The output buffer is too small.
    Overflow,
    /// The call got more than [`MAX_TOKENS`] tokens.
    TooManyTokens,
}

impl core::fmt::Display for FormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FormatError::Overflow => f.write_str("output buffer is too small"),
            FormatError::TooManyTokens => {
                write!(f, "more than {MAX_TOKENS} tokens in one call")
            }
        }
    }
}

impl core::error::Error for FormatError {}

/// A rewrite beyond writing numbers as digits. Every setting is on by default
/// except [`SingleDigits`](Setting::SingleDigits). Switch one off with
/// [`Normalizer::without`](crate::Normalizer::without) and on with
/// [`Normalizer::with`](crate::Normalizer::with).
///
/// New settings may be added, so a `match` on `Setting` needs a `_` arm.
///
/// ```
/// use clm_itn::{Normalizer, Setting};
///
/// let plain = Normalizer::english().without(Setting::Percent);
/// assert_eq!(plain.normalize("fifty percent off"), "50 percent off");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Setting {
    /// A lone 0–9 becomes a digit: "like 8 years". Off, the default: "like eight years".
    SingleDigits,
    /// "percent" becomes `%`: "50%". Off: "50 percent".
    Percent,
    /// "dollars" and "cents" become `$`: "$5", "$4.50". Off: "5 dollars".
    Currency,
    /// A month date gets a capital and a comma before the year:
    /// "July 4th, 1990". Off: "july 4th 1990".
    Dates,
    /// "seven a m e s t" → "7 am EST". Off: "7 a m e s t".
    Letters,
    /// "twelve noon" → "noon", "twelve midnight" → "midnight". Off: "12 noon".
    Noon,
    /// "oclock" is written "o'clock". Off: "5 oclock".
    Oclock,
    /// "fahrenheit" → "Fahrenheit", "centigrade" → "Celsius". Off: as spoken.
    Temperature,
}

impl Setting {
    /// Every setting, in declaration order.
    pub const ALL: &'static [Setting] = &[
        Setting::SingleDigits,
        Setting::Percent,
        Setting::Currency,
        Setting::Dates,
        Setting::Letters,
        Setting::Noon,
        Setting::Oclock,
        Setting::Temperature,
    ];

    /// The kebab-case name, as the `clm-itn --no-<name>` flags use it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Setting::SingleDigits => "single-digits",
            Setting::Percent => "percent",
            Setting::Currency => "currency",
            Setting::Dates => "dates",
            Setting::Letters => "letters",
            Setting::Noon => "noon",
            Setting::Oclock => "oclock",
            Setting::Temperature => "temperature",
        }
    }

    /// The setting with this [`name`](Self::name).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Setting> {
        Setting::ALL.iter().copied().find(|s| s.name() == name)
    }

    pub(crate) const fn bit(self) -> SettingBits {
        1 << self as u16
    }
}

/// One bit per [`Setting`], at `1 << setting as u16`.
pub(crate) type SettingBits = u16;

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
}
