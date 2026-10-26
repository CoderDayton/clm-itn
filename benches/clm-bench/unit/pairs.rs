use crate::pairs::*;

fn span(class: &str, spoken: &str, written: &str) -> Span {
    Span {
        class: class.into(),
        spoken: spoken.into(),
        written: written.into(),
    }
}

fn parse_pairs(text: &str, is_number: impl Fn(&str) -> bool) -> (Vec<Span>, usize) {
    let (records, bad) = parse_records(text, is_number);
    let spans = records
        .into_iter()
        .flat_map(|r| r.items.into_iter().map(|(_, s)| s))
        .collect();
    (spans, bad)
}

#[test]
fn number_words_have_a_role_and_month_names_do_not_count() {
    let t = clm_itn::english();
    for w in ["one", "five", "twenty", "first", "hundred"] {
        assert!(number_word(t, w), "{w}");
    }
    for w in ["january", "october", "dog", "the"] {
        assert!(!number_word(t, w), "{w}");
    }
}

#[test]
fn affix_roles_are_not_number_words() {
    use crate::score::WordRole;
    let t = clm_itn::english();
    for w in ["percent", "dollars"] {
        assert_eq!(word_role(t, w), WordRole::Affix, "{w}");
        assert!(!number_word(t, w), "{w}");
    }
    assert_eq!(word_role(t, "five"), WordRole::Number);
    assert_eq!(word_role(t, "january"), WordRole::Plain);
}

#[test]
fn records_keep_the_raw_line_and_each_item_range() {
    let text = "S i saw one dog, at two pm.\nT 5 2 TIME 2 p.m.\n\nS no numbers\n\n";
    let (recs, bad) = parse_records(text, |w| matches!(w, "one" | "two"));
    assert_eq!(bad, 0);
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[0].line, "i saw one dog, at two pm.");
    assert_eq!(
        recs[0].items,
        vec![
            (5..7, span("TIME", "two pm", "2 p.m.")),
            (0..5, span(PLAIN, "i saw one dog at", "i saw one dog at")),
        ]
    );
    assert!(recs[1].items.is_empty());
    let (flat, _) = parse_pairs(text, |w| matches!(w, "one" | "two"));
    assert_eq!(flat.len(), 2);
}

fn out(raw: &str, formatted: &str, range: std::ops::Range<usize>) -> String {
    let r: Vec<&str> = raw.split(' ').collect();
    let o: Vec<&str> = formatted.split_whitespace().collect();
    span_output(&r, &o, range)
}

#[test]
fn span_output_takes_what_replaced_the_span_in_context() {
    let raw = "i saw two dogs at five pm today";
    let fmt = "i saw two dogs at 5 p.m. today";
    assert_eq!(out(raw, fmt, 5..7), "5 p.m.");
    assert_eq!(out(raw, fmt, 2..3), "two");
    assert_eq!(out(raw, fmt, 0..4), "i saw two dogs");
}

#[test]
fn span_output_trims_unchanged_tokens_like_the_spoken_side() {
    let raw = "i saw one dog, — then two";
    let fmt = "i saw one dog, — then 2";
    assert_eq!(out(raw, fmt, 0..6), "i saw one dog then");
}

#[test]
fn span_output_matches_anchors_case_insensitively() {
    assert_eq!(out("on march first we", "on March 1 we", 1..3), "March 1");
}

#[test]
fn span_output_strips_only_the_edge_punctuation_of_the_spoken_tokens() {
    assert_eq!(out("twenty five, then", "25, then", 0..2), "25");
    assert_eq!(out("(five) dogs", "(5) dogs", 0..1), "5");
    assert_eq!(out("at two pm, then", "at 2 p.m., then", 1..3), "2 p.m.");
}

#[test]
fn span_output_returns_a_whole_merged_gap() {
    let raw = "one hundred and five dogs";
    assert_eq!(out(raw, "105 dogs", 0..2), "105");
    assert_eq!(out(raw, "105 dogs", 4..5), "dogs");
}

#[test]
fn parses_spans_with_spaced_written_forms() {
    let text = "S at two o three pm on january fifth, twenty twenty four.\n\
                T 1 4 TIME 2:03 p.m.\n\
                T 6 5 DATE Jan. 5, 2024\n\
                \n\
                S one of them\n\
                \n";
    let (spans, bad) = parse_pairs(text, |_| false);
    assert_eq!(bad, 0);
    assert_eq!(
        spans,
        vec![
            span("TIME", "two o three pm", "2:03 p.m."),
            span("DATE", "january fifth twenty twenty four", "Jan. 5, 2024"),
        ]
    );
}

#[test]
fn trims_punctuation_around_each_spoken_token() {
    let text = "S “twenty-one,” (five) …\nT 0 3 CARDINAL x\n\n";
    let (spans, bad) = parse_pairs(text, |_| false);
    assert_eq!(bad, 0);
    assert_eq!(spans, vec![span("CARDINAL", "twenty-one five", "x")]);
}

#[test]
fn counts_malformed_lines_and_out_of_range_spans() {
    let text = "T 0 1 CARDINAL 1\n\
                S one two\n\
                T 1 2 CARDINAL 2\n\
                T x 1 CARDINAL 2\n\
                T 0 0 CARDINAL 2\n\
                T 0 1 CARDINAL\n\
                T 0 1 CARDINAL 1\n\
                garbage\n\
                \n";
    let (spans, bad) = parse_pairs(text, |_| false);
    assert_eq!(spans, vec![span("CARDINAL", "one", "1")]);
    assert_eq!(bad, 6);
}

#[test]
fn relaxed_ignores_case_whitespace_and_commas() {
    assert_eq!(relaxed(" $1,250 Million "), relaxed("$1250 million"));
    assert_eq!(relaxed("10:30 A.M."), "10:30a.m.");
    assert_ne!(relaxed("10.30"), relaxed("10:30"));
}

#[test]
fn tally_scores_exact_and_relaxed_per_class() {
    let mut t = PairTally::default();
    t.record(&span("CARDINAL", "one thousand", "1,000"), "1,000");
    t.record(&span("CARDINAL", "one thousand", "1,000"), "1000");
    t.record(&span("CARDINAL", "two", "2"), "two");
    t.record(&span("TIME", "two pm", "2 p.m."), " 2 p.m. ");
    let c = &t.classes[0];
    assert_eq!(c.class, "CARDINAL");
    assert_eq!((c.total, c.exact, c.relaxed), (3, 1, 2));
    assert_eq!(
        c.misses,
        vec![Miss {
            spoken: "two".into(),
            got: "two".into(),
            want: "2".into()
        }]
    );
    assert_eq!((t.classes[1].exact, t.classes[1].relaxed), (1, 1));
    assert_eq!((t.classes[0].digits, t.classes[1].digits), (2, 1));
}

#[test]
fn in_scope_totals_skip_whitelist_and_electronic() {
    let mut t = PairTally::default();
    t.record(&span("CARDINAL", "one", "1"), "1");
    t.record(&span("WHITELIST", "mister", "Mr."), "mister");
    t.record(&span("ELECTRONIC", "a dot b", "a.b"), "a dot b");
    t.record(&span("DATE", "may", "May"), "may");
    assert_eq!(t.totals(Total::All), (4, 1, 2, 4));
    assert_eq!(t.totals(Total::InScope), (2, 1, 2, 2));
}

fn number(w: &str) -> bool {
    matches!(w, "one" | "two" | "five")
}

#[test]
fn plain_runs_are_maximal_non_span_runs_with_a_number_word() {
    let text = "S i saw one dog then at two pm we left\n\
                T 6 2 TIME 2 p.m.\n\
                \n";
    let (spans, bad) = parse_pairs(text, number);
    assert_eq!(bad, 0);
    assert_eq!(
        spans,
        vec![
            span("TIME", "two pm", "2 p.m."),
            span(PLAIN, "i saw one dog then at", "i saw one dog then at"),
        ]
    );
}

#[test]
fn plain_runs_cover_zero_span_records_and_skip_punctuation_only_tokens() {
    let text = "S “one,” — of them\n\nS no numbers here\n\nS five";
    let (spans, bad) = parse_pairs(text, number);
    assert_eq!(bad, 0);
    assert_eq!(
        spans,
        vec![
            span(PLAIN, "one of them", "one of them"),
            span(PLAIN, "five", "five"),
        ]
    );
}

#[test]
fn plain_runs_split_around_every_span() {
    let text = "S one two five one\nT 1 1 CARDINAL 2\nT 2 1 CARDINAL 5\n\n";
    let (spans, _) = parse_pairs(text, number);
    let plain: Vec<_> = spans.iter().filter(|s| s.class == PLAIN).collect();
    assert_eq!(
        plain,
        [&span(PLAIN, "one", "one"), &span(PLAIN, "one", "one")]
    );
}

#[test]
fn balanced_adds_plain_to_in_scope_and_other_totals_ignore_it() {
    let mut t = PairTally::default();
    t.record(&span("CARDINAL", "one", "1"), "1");
    t.record(&span("WHITELIST", "mister", "Mr."), "mister");
    t.record(&span(PLAIN, "one dog", "one dog"), "1 dog");
    t.record(&span(PLAIN, "two of us", "two of us"), "two of us");
    assert_eq!(t.totals(Total::InScope), (1, 1, 1, 1));
    assert_eq!(t.totals(Total::All), (2, 1, 1, 2));
    assert_eq!(t.totals(Total::Balanced), (3, 2, 2, 2));
}

#[test]
fn partials_count_per_class_keep_three_examples_and_follow_scope() {
    let mut t = PairTally::default();
    let time = span("TIME", "eleven forty a m", "11:40 a.m.");
    for _ in 0..4 {
        t.record(&time, "1140 a m");
        t.record_partial(&time, "1140 a m");
    }
    let el = span("ELECTRONIC", "a dot one", "a.1");
    t.record(&el, "a dot 1");
    t.record_partial(&el, "a dot 1");
    assert_eq!(t.classes[0].partial, 4);
    assert_eq!(t.classes[0].partial_examples.len(), MAX_MISSES);
    assert_eq!(t.classes[0].partial_examples[0].got, "1140 a m");
    assert_eq!(t.partials(Total::InScope), 4);
    assert_eq!(t.partials(Total::All), 5);
}

#[test]
fn keeps_at_most_three_misses_per_class() {
    let mut t = PairTally::default();
    for i in 0..6 {
        t.record(&span("DATE", &i.to_string(), "want"), "got");
    }
    assert_eq!(t.classes[0].misses.len(), MAX_MISSES);
    assert_eq!(t.classes[0].misses[2].spoken, "2");
}
