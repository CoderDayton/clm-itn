use crate::gold::*;
use crate::lines::Case;

fn case(s: &str, w: &str, t: &str) -> Case {
    Case {
        spoken: s.into(),
        written: Some(w.into()),
        tag: t.into(),
    }
}

#[test]
fn tally_scores_exact_digits_and_overformat() {
    let mut t = LineTally::default();
    t.record(
        &case("call me at five", "call me at 5", "time"),
        "call me at 5",
    );
    t.record(
        &case("i paid one thousand", "i paid 1,000", "money"),
        "i paid 1000",
    );
    t.record(&case("one of them", "one of them", "keep"), "1 of them");
    t.record(
        &case("the first time", "the first time", "keep"),
        "the first time",
    );
    t.record(&case("two dogs", "2 dogs", "count!"), "two dogs");
    let a = t.all;
    assert_eq!((a.total, a.exact, a.digits), (5, 2, 3));
    assert_eq!((a.overformat, a.no_digit_gold), (1, 2));
    assert_eq!((t.firm.total, t.firm.exact), (4, 2));
    assert_eq!(t.failures.len(), 3);
    assert_eq!(t.failures[1].input, "one of them");
}

#[test]
fn tally_skips_cases_without_a_written_form() {
    let mut t = LineTally::default();
    let spoken_only = Case {
        spoken: "five pm".into(),
        written: None,
        tag: "time".into(),
    };
    t.record(&spoken_only, "5 pm");
    assert_eq!(t.all.total, 0);
    assert!(t.tags.is_empty() && t.failures.is_empty());
}

#[test]
fn tally_keeps_scores_per_tag_in_first_seen_order() {
    let mut t = LineTally::default();
    t.record(&case("one of them", "one of them", "keep"), "1 of them");
    t.record(&case("five pm", "5 p.m.", "time"), "5 p.m.");
    t.record(&case("the one", "the one", "keep"), "the one");
    let names: Vec<&str> = t.tags.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["keep", "time"]);
    let keep = t.tags[0].1;
    assert_eq!((keep.total, keep.exact, keep.overformat), (2, 1, 1));
}

#[test]
fn a_tag_filter_matches_the_base_tag_and_its_debatable_form() {
    assert!(tag_matches("time", "time"));
    assert!(tag_matches("time!", "time"));
    assert!(tag_matches("time!", "time!"));
    assert!(!tag_matches("time", "time!"));
    assert!(!tag_matches("timer", "time"));
    assert!(!tag_matches("trap_time", "time"));
}

#[test]
fn all_failures_lifts_the_cap() {
    let mut t = LineTally {
        all_failures: true,
        ..LineTally::default()
    };
    for i in 0..15 {
        t.record(&case(&i.to_string(), "want", "a"), "got");
    }
    assert_eq!(t.failures.len(), 15);
}

#[test]
fn trims_output_and_keeps_at_most_ten_failures() {
    let mut t = LineTally::default();
    t.record(&case("x", "5", "a"), " 5 ");
    assert_eq!(t.all.exact, 1);
    for i in 0..15 {
        t.record(&case(&i.to_string(), "want", "a"), "got");
    }
    assert_eq!(t.failures.len(), MAX_LINE_FAILURES);
    assert_eq!(t.failures[9].input, "9");
}

#[test]
fn scores_the_dictation_gold_set() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/dictation.tsv");
    let files = crate::lines::load(path).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].malformed, 0);
    let cases: Vec<&Case> = files[0].cases.iter().collect();
    assert!(cases.iter().all(|c| c.written.is_some()));
    let t = score(&clm_itn::Normalizer::english(), &cases, false);
    assert_eq!(t.all.total, cases.len());
    assert!(t.all.exact > t.all.total / 2, "{} exact", t.all.exact);
}
