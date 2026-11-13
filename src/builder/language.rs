//! What a language supplies to the builder: its word classes, the spoken
//! examples the shapes are ranked from, and the words the line passes read.
//! The builder and the engine name no word; `english::LANGUAGE` is the only
//! definition so far.

use crate::types::{ClassId, DomainId, MAX_LEFT_FOLDS, Role, Setting, name};

pub const SRC_CARDINAL: u8 = 1;
pub const SRC_TIME: u8 = 2;
pub const SRC_IDENTIFIER: u8 = 4;
pub const SRC_YEAR: u8 = 8;
pub const SRC_DATE: u8 = 16;

/// Words of one class: each spoken word and the number it stands for.
pub struct ClassDef {
    pub class: ClassId,
    pub role: Role,
    pub words: &'static [(&'static str, u32)],
}

/// Words of one class that print as text: each spoken word and what it prints.
pub struct LiteralClassDef {
    pub class: ClassId,
    pub role: Role,
    pub words: &'static [(&'static str, &'static str)],
}

/// One spoken form with the weight it counts for in ranking shapes.
pub struct Example {
    pub words: Vec<&'static str>,
    pub src: u8,
    pub weight: f64,
    pub kind: &'static str,
}

/// Words that share a `types::kind`.
pub struct WordKind {
    pub kind: u8,
    /// What the kind carries; see `types::kind`.
    pub aux: u8,
    pub words: &'static [&'static str],
}

/// The classes the context rules and affix routing name.
#[derive(Clone, Copy)]
pub struct Classes {
    pub dollar: ClassId,
    pub cent: ClassId,
    pub percent: ClassId,
    pub unit: ClassId,
    pub power: ClassId,
    pub prefix: ClassId,
    pub degree: ClassId,
    /// Starts a date: "march" the month.
    pub month: ClassId,
    /// A month word that is also a verb: "may".
    pub may: ClassId,
    pub dot: ClassId,
    /// Left context of a time ("at").
    pub time_left: ClassId,
    /// Right context of a time ("pm", "o'clock").
    pub time_right: &'static [ClassId],
    /// Left context of an identifier ("room", "flight").
    pub identifier_left: &'static [ClassId],
}

/// The text the candidate programs write.
#[derive(Clone, Copy)]
pub struct Literals {
    /// After a bare hour (" o'clock").
    pub oclock: &'static str,
    /// Before the day in a date whose month is "may".
    pub may: &'static str,
}

#[derive(Clone, Copy)]
pub struct NumberStyle {
    pub decimal: &'static str,
    pub grouped: bool,
    pub pairs: bool,
    pub fraction_number: bool,
    pub currency_after: bool,
}

/// Words and spellings the line passes read or write.
#[derive(Clone, Copy)]
pub struct VocabSpec {
    /// `types::name` slots.
    pub names: [&'static str; name::COUNT],
    pub months: [&'static str; 12],
    /// The words for "1" to "12".
    pub hours: [&'static str; 12],
    pub centuries: [&'static str; 9],
    pub zones: &'static [&'static str],
    /// Letters that never join to the number before them.
    pub not_after_number: &'static str,
    /// Letters that never join to the number after them.
    pub not_before_number: &'static str,
}

#[derive(Clone, Copy)]
pub struct Language {
    pub name: &'static str,
    pub classes: &'static [ClassDef],
    pub literal_classes: &'static [LiteralClassDef],
    pub class_ids: Classes,
    pub kinds: &'static [WordKind],
    pub vocab: VocabSpec,
    pub literals: Literals,
    pub numbers: NumberStyle,
    /// Word-sense bits and the words that carry them.
    pub senses: &'static [(u32, &'static [&'static str])],
    pub idioms: &'static [&'static [&'static str]],
    pub plural: &'static [&'static str],
    pub words_first: &'static [&'static str],
    pub suffix_roles: u16,
    pub prefix_roles: u16,
    pub bind_roles: u16,
    pub lead_skip_roles: u16,
    pub left_folds: [[ClassId; 3]; MAX_LEFT_FOLDS],
    pub aliases: &'static [(ClassId, ClassId)],
    /// Classes a span can start with that no number word starts.
    pub start_classes: &'static [ClassId],
    pub per: ClassId,
    pub per_units: &'static [ClassId],
    pub pair_only: &'static [ClassId],
    /// See `Tables::lone_after`.
    pub lone_after: &'static [ClassId],
    pub setting_classes: &'static [(Setting, &'static [ClassId])],
    pub setting_domains: &'static [(Setting, &'static [DomainId])],
    /// The domains of a date whose month is any month and whose month is "may".
    pub month_domain: DomainId,
    pub may_domain: DomainId,
    /// The affix domain a unit, currency or prefix word belongs to.
    pub affix_domain: fn(&str) -> Option<DomainId>,
    pub examples: fn() -> Vec<Example>,
    pub kind_mass: fn(&str) -> f64,
}

impl Language {
    /// The role of each word of the classes that print as text and the
    /// classes that stand for numbers.
    pub fn word_roles(&self) -> impl Iterator<Item = (&'static str, Role)> {
        let numbers = self
            .classes
            .iter()
            .flat_map(|d| d.words.iter().map(move |w| (w.0, d.role)));
        let texts = self
            .literal_classes
            .iter()
            .flat_map(|d| d.words.iter().map(move |w| (w.0, d.role)));
        numbers.chain(texts)
    }

    /// The highest class ID a word class uses.
    pub fn last_class(&self) -> ClassId {
        let numbers = self.classes.iter().map(|d| d.class);
        let texts = self.literal_classes.iter().map(|d| d.class);
        numbers.chain(texts).max().unwrap_or(1)
    }
}
