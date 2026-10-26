use crate::score::*;

#[test]
fn digit_runs_ignore_letters_punctuation_and_thousands_commas() {
    assert_eq!(digit_runs("January 8, 1815"), ["8", "1815"]);
    assert_eq!(digit_runs("jan. 8th 1815"), ["8", "1815"]);
    assert_eq!(digit_runs("$1,250,000.50"), ["1250000", "50"]);
    assert_eq!(digit_runs("10-23-26"), ["10", "23", "26"]);
    assert!(digit_runs("mister smith").is_empty());
    assert_eq!(digit_runs("a, 1"), ["1"]);
}

#[test]
fn digits_match_compares_digit_run_sequences() {
    assert!(digits_match("jan. 8th 1815", "January 8, 1815"));
    assert!(digits_match("1000", "1,000"));
    assert!(digits_match("the 5th of april 1771", "the 5th April 1771"));
    assert!(digits_match("sixties", "sixties"));
    assert!(!digits_match("sixties", "60s"));
    assert!(!digits_match("8 1815", "1815 8"));
    assert!(!digits_match("1 dog", "one dog"));
    assert!(!digits_match("12.7", "127"));
}

fn number_or_unit(w: &str) -> WordRole {
    const AFFIX: [&str; 11] = [
        "a", "m", "p", "mi", "per", "hour", "degree", "am", "pm", "g", "t",
    ];
    const NUMBER: [&str; 12] = [
        "nine", "trillion", "quarter", "to", "million", "and", "half", "november", "seven",
        "sixty", "l", "v",
    ];
    if AFFIX.contains(&w) {
        WordRole::Affix
    } else if NUMBER.contains(&w) {
        WordRole::Number
    } else {
        WordRole::Plain
    }
}

#[test]
fn partial_is_digits_plus_a_leftover_input_number_word() {
    let p = |got, want| is_partial(got, want, number_or_unit);
    assert!(p("1140 a m", "11:40 a.m."));
    assert!(p("quarter to 12", "11:45"));
    assert!(p("Nine Trillion 789", "9789000000000"));
    assert!(p("2 and a half", "2 1/2"));
    assert!(p("five to 6 mi", "5-6 mi"));
}

#[test]
fn partial_ignores_affix_words_when_the_digits_are_correct() {
    let p = |got, want| is_partial(got, want, number_or_unit);
    assert!(!p("60 mi per hour", "60 mph"));
    assert!(!p("60 mi, per hour.", "60 mph"));
    assert!(!p("60 degree", "60°"));
    assert!(!p("7 p m", "7 p.m."));
    assert!(p("61 mi per hour", "60 mph"));
}

#[test]
fn partial_excludes_raw_output_clean_output_and_words_the_gold_keeps() {
    let p = |got, want| is_partial(got, want, number_or_unit);
    assert!(!p("nine trillion", "9000000000000"));
    assert!(!p("11:40 a.m.", "11:40 a.m."));
    assert!(!p("5.2 million", "5.2 million"));
    assert!(!p("5.2 Million", "5.2 million"));
    assert!(!p("1 dog", "one dog"));
    assert!(!p("12:00 noon", "12:00 noon"));
}

#[test]
fn partial_ignores_month_names_and_scales_right_after_digits() {
    let p = |got, want| is_partial(got, want, number_or_unit);
    assert!(!p("November 3", "Nov. 3"));
    assert!(!p("10 million", "10,000,000"));
    assert!(!p("$5.2 Million.", "$5,200,000"));
    assert!(p("66 degree-sixty seven degree", "66°-67°"));
    assert!(p("nine million 5", "9000005"));
}

#[test]
fn partial_compares_words_without_inner_punctuation() {
    let p = |got, want| is_partial(got, want, number_or_unit);
    assert!(!p("12:01 am", "12:01 a.m."));
    assert!(!p("5 pm", "5:00 P.M."));
    assert!(p("1140 a m", "11:40 a.m."));
    assert!(p("66 degree-sixty", "66°-60°"));
}

#[test]
fn partial_skips_spelled_letters_except_a_m_and_p_m() {
    let p = |got, want| is_partial(got, want, number_or_unit);
    assert!(!p("l 24", "L24"));
    assert!(!p("1449 v", "1449V"));
    assert!(!p("8:00 g m t", "08:00 gmt"));
    assert!(p("1140 a m", "11:40 a.m."));
    assert!(p("1140 p m", "11:40 p.m."));
}

#[test]
fn time_value_parses_hour_minute_and_meridiem() {
    use Meridiem::{Am, Pm};
    assert_eq!(time_value("08:06"), Some((8, 6, None)));
    assert_eq!(time_value("8:06"), Some((8, 6, None)));
    assert_eq!(time_value("2 p.m."), Some((2, 0, Some(Pm))));
    assert_eq!(time_value("02:00 p.m."), Some((2, 0, Some(Pm))));
    assert_eq!(time_value("2:00 PM"), Some((2, 0, Some(Pm))));
    assert_eq!(time_value("7 a m e s t"), Some((7, 0, Some(Am))));
    assert_eq!(time_value("10.30 A.M."), Some((10, 30, Some(Am))));
    assert_eq!(time_value("11:59:59 p.m."), Some((11, 59, Some(Pm))));
    assert_eq!(time_value("5 o'clock"), Some((5, 0, None)));
    assert_eq!(time_value("5 oclock"), Some((5, 0, None)));
    assert_eq!(
        time_value("set alarm at 10:50 p.m."),
        Some((10, 50, Some(Pm)))
    );
    assert_eq!(time_value("two thirty"), None);
    assert_eq!(time_value("5"), None);
    assert_eq!(time_value("25:00"), None);
    assert_eq!(time_value("1140 a m"), None);
}

#[test]
fn time_match_normalizes_padding_and_24_hour_clock() {
    assert!(time_match("8:06", "08:06"));
    assert!(time_match("2 p.m.", "02:00 p.m."));
    assert!(time_match("2:00 PM", "14:00"));
    assert!(time_match("14:00 pm", "2 p.m."));
    assert!(time_match("5 o'clock", "5:00"));
    assert!(time_match("8:00 g m t", "08:00 gmt"));
    assert!(time_match("12 am", "00:00"));
    assert!(!time_match("2:00", "02:00 p.m."));
    assert!(!time_match("2:00 a.m.", "02:00 p.m."));
    assert!(!time_match("two thirty", "02:30"));
    assert!(!time_match("x", "y"));
}

#[test]
fn value_match_uses_time_parsing_only_for_time_classes() {
    assert!(value_match("time", "2 p.m.", "02:00 p.m."));
    assert!(value_match("time_cased", "2 P.M.", "02:00 P.M."));
    assert!(value_match("TIME", "8:06", "08:06"));
    assert!(!value_match("cardinal", "2 p.m.", "02:00 p.m."));
    assert!(value_match("cardinal", "1,000", "1000"));
}

#[test]
fn accuracy_is_percent_and_zero_total_is_zero() {
    assert_eq!(accuracy(1, 4), 25.0);
    assert_eq!(accuracy(0, 0), 0.0);
}
