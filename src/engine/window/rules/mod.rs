mod english;
mod spanish;

use super::Word;
use crate::tables::Tables;
use alloc::vec::Vec;

pub(super) use english::English;
pub(super) use spanish::Spanish;

pub(super) trait Rules {
    const DECIMAL: char;
    const GROUPED: bool;
    const WHOLE_HUNDREDS: bool;
    const CENTS_AFTER_CURRENCY: bool;
    const CARDINAL_DAYS: bool;
    const PERCENT_WORD_IS_NUMBER: bool;

    fn hold(_t: &Tables, _words: &[Word], _i: usize) -> Option<(usize, usize)> {
        None
    }

    fn meridiem(_first: &str, _second: &str) -> Option<(&'static str, usize)> {
        None
    }

    fn rewrite<'a>(_t: &'a Tables, _line: &'a str, words: Vec<Word<'a>>) -> Vec<Word<'a>> {
        words
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    English,
    Spanish,
}
