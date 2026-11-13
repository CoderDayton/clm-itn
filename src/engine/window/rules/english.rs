use super::Rules;

pub struct English;

impl Rules for English {
    const DECIMAL: char = '.';
    const GROUPED: bool = true;
    const WHOLE_HUNDREDS: bool = false;
    const CENTS_AFTER_CURRENCY: bool = false;
    const CARDINAL_DAYS: bool = false;
    const PERCENT_WORD_IS_NUMBER: bool = false;
}
