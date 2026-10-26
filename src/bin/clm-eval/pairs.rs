pub const OUT_OF_SCOPE: [&str; 2] = ["WHITELIST", "ELECTRONIC"];
pub const MAX_MISSES: usize = 3;
pub const PLAIN: &str = "PLAIN";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Total {
    InScope,
    All,
    Balanced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub class: String,
    pub spoken: String,
    pub written: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Miss {
    pub spoken: String,
    pub got: String,
    pub want: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassScore {
    pub class: String,
    pub total: usize,
    pub exact: usize,
    pub relaxed: usize,
    pub digits: usize,
    pub partial: usize,
    pub partial_examples: Vec<Miss>,
    pub misses: Vec<Miss>,
}

#[derive(Debug, Default)]
pub struct PairTally {
    pub classes: Vec<ClassScore>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Pairs { path: String, isolated: bool },
    Lines(LinesOpts),
    Spell(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinesOpts {
    pub path: String,
    pub tag: Option<String>,
    pub all_failures: bool,
    pub single_digits: bool,
}

fn lines_opts(rest: &[String]) -> Result<LinesOpts, String> {
    let usage = || crate::USAGE.to_string();
    let (path, mut rest) = match rest {
        [p, rest @ ..] if !p.starts_with("--") => (p, rest),
        _ => return Err(usage()),
    };
    let mut opts = LinesOpts {
        path: path.clone(),
        ..LinesOpts::default()
    };
    while let [flag, tail @ ..] = rest {
        rest = match (flag.as_str(), tail) {
            ("--all-failures", t) if !opts.all_failures => {
                opts.all_failures = true;
                t
            }
            ("--single-digits", t) if !opts.single_digits => {
                opts.single_digits = true;
                t
            }
            ("--tag", [tag, t @ ..]) if opts.tag.is_none() && !tag.starts_with("--") => {
                opts.tag = Some(tag.clone());
                t
            }
            _ => return Err(usage()),
        };
    }
    Ok(opts)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub line: String,
    pub items: Vec<(std::ops::Range<usize>, Span)>,
}

pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Mode, String> {
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [flag, path] if flag == "--pairs" => Ok(Mode::Pairs {
            path: path.clone(),
            isolated: false,
        }),
        [flag, path, iso] if flag == "--pairs" && iso == "--isolated" => Ok(Mode::Pairs {
            path: path.clone(),
            isolated: true,
        }),
        [flag, rest @ ..] if flag == "--lines" => lines_opts(rest).map(Mode::Lines),
        [flag, path] if flag == "--spell" => Ok(Mode::Spell(path.clone())),
        _ => Err(crate::USAGE.to_string()),
    }
}

fn spoken_tokens(line: &str) -> Vec<&str> {
    line.split(' ')
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .collect()
}

fn parse_span(rest: &str, tokens: &[&str]) -> Option<(Span, std::ops::Range<usize>)> {
    let mut f = rest.splitn(4, ' ');
    let start: usize = f.next()?.parse().ok()?;
    let len: usize = f.next()?.parse().ok()?;
    let class = f.next()?;
    let written = f.next().filter(|w| !w.is_empty())?;
    if len == 0 || class.is_empty() || start.checked_add(len)? > tokens.len() {
        return None;
    }
    let words: Vec<&str> = tokens[start..start + len]
        .iter()
        .copied()
        .filter(|t| !t.is_empty())
        .collect();
    let span = Span {
        class: class.to_string(),
        spoken: words.join(" "),
        written: written.to_string(),
    };
    Some((span, start..start + len))
}

fn push_plain_runs(
    tokens: &[&str],
    covered: &[bool],
    is_number: &impl Fn(&str) -> bool,
    out: &mut Vec<(std::ops::Range<usize>, Span)>,
) {
    let mut i = 0;
    while i < tokens.len() {
        if covered[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < tokens.len() && !covered[i] {
            i += 1;
        }
        let words: Vec<&str> = tokens[start..i]
            .iter()
            .copied()
            .filter(|t| !t.is_empty())
            .collect();
        if words.iter().any(|w| is_number(w)) {
            let text = words.join(" ");
            let span = Span {
                class: PLAIN.to_string(),
                spoken: text.clone(),
                written: text,
            };
            out.push((start..i, span));
        }
    }
}

#[cfg(test)]
pub fn parse_pairs(text: &str, is_number: impl Fn(&str) -> bool) -> (Vec<Span>, usize) {
    let (records, bad) = parse_records(text, is_number);
    let spans = records
        .into_iter()
        .flat_map(|r| r.items.into_iter().map(|(_, s)| s))
        .collect();
    (spans, bad)
}

struct Open<'a> {
    line: &'a str,
    toks: Vec<&'a str>,
    covered: Vec<bool>,
    items: Vec<(std::ops::Range<usize>, Span)>,
}

fn close(open: Open<'_>, is_number: &impl Fn(&str) -> bool) -> Record {
    let mut items = open.items;
    push_plain_runs(&open.toks, &open.covered, is_number, &mut items);
    Record {
        line: open.line.to_string(),
        items,
    }
}

pub fn parse_records(text: &str, is_number: impl Fn(&str) -> bool) -> (Vec<Record>, usize) {
    let mut records = Vec::new();
    let mut bad = 0;
    let mut open: Option<Open> = None;
    for line in text.lines() {
        if line.is_empty() {
            if let Some(o) = open.take() {
                records.push(close(o, &is_number));
            }
        } else if let Some(s) = line.strip_prefix("S ") {
            if let Some(o) = open.take() {
                records.push(close(o, &is_number));
            }
            let toks = spoken_tokens(s);
            open = Some(Open {
                line: s,
                covered: vec![false; toks.len()],
                toks,
                items: Vec::new(),
            });
        } else if let (Some(rest), Some(o)) = (line.strip_prefix("T "), open.as_mut()) {
            match parse_span(rest, &o.toks) {
                Some((s, range)) => {
                    o.covered[range.clone()].fill(true);
                    o.items.push((range, s));
                }
                None => bad += 1,
            }
        } else {
            bad += 1;
        }
    }
    if let Some(o) = open {
        records.push(close(o, &is_number));
    }
    (records, bad)
}

fn anchors(raw: &[&str], out: &[&str]) -> Vec<Option<usize>> {
    let eq = |a: &str, b: &str| a.to_lowercase() == b.to_lowercase();
    let (n, m) = (raw.len(), out.len());
    let mut lcs = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if eq(raw[i], out[j]) {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut map = vec![None; n];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if eq(raw[i], out[j]) && lcs[i][j] == lcs[i + 1][j + 1] + 1 {
            map[i] = Some(j);
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    map
}

fn gap_output(map: &[Option<usize>], out_len: usize, i: usize) -> (usize, std::ops::Range<usize>) {
    let mut s = i;
    while s > 0 && map[s - 1].is_none() {
        s -= 1;
    }
    let mut e = i;
    while e < map.len() && map[e].is_none() {
        e += 1;
    }
    let lo = s.checked_sub(1).and_then(|k| map[k]).map_or(0, |j| j + 1);
    let hi = map.get(e).copied().flatten().unwrap_or(out_len);
    (e, lo..hi.max(lo))
}

pub fn span_output(raw: &[&str], out: &[&str], range: std::ops::Range<usize>) -> String {
    let map = anchors(raw, out);
    let mut pieces: Vec<&str> = Vec::new();
    let mut i = range.start;
    while i < range.end.min(raw.len()) {
        match map[i] {
            Some(j) => {
                let t = out[j].trim_matches(|c: char| !c.is_alphanumeric());
                if !t.is_empty() {
                    pieces.push(t);
                }
                i += 1;
            }
            None => {
                let (end, gap) = gap_output(&map, out.len(), i);
                pieces.extend_from_slice(&out[gap]);
                i = end;
            }
        }
    }
    let joined = pieces.join(" ");
    let edge = |c: char| !c.is_alphanumeric();
    let first = raw.get(range.start).copied().unwrap_or("");
    let last = raw.get(range.end.saturating_sub(1)).copied().unwrap_or("");
    let lead = &first[..first.len() - first.trim_start_matches(edge).len()];
    let trail = &last[last.trim_end_matches(edge).len()..];
    let s = joined.strip_prefix(lead).unwrap_or(&joined);
    s.strip_suffix(trail).unwrap_or(s).to_string()
}

pub fn relaxed(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .flat_map(char::to_lowercase)
        .collect()
}

impl PairTally {
    pub fn record(&mut self, span: &Span, got: &str) {
        let idx = match self.classes.iter().position(|c| c.class == span.class) {
            Some(i) => i,
            None => {
                self.classes.push(ClassScore {
                    class: span.class.clone(),
                    ..ClassScore::default()
                });
                self.classes.len() - 1
            }
        };
        let c = &mut self.classes[idx];
        c.total += 1;
        c.exact += usize::from(got.trim() == span.written);
        let ok = relaxed(got) == relaxed(&span.written);
        c.relaxed += usize::from(ok);
        c.digits += usize::from(crate::eval::value_match(&span.class, got, &span.written));
        if !ok && c.misses.len() < MAX_MISSES {
            c.misses.push(Miss {
                spoken: span.spoken.clone(),
                got: got.trim().to_string(),
                want: span.written.clone(),
            });
        }
    }

    pub fn record_partial(&mut self, span: &Span, got: &str) {
        let Some(c) = self.classes.iter_mut().find(|c| c.class == span.class) else {
            return;
        };
        c.partial += 1;
        if c.partial_examples.len() < MAX_MISSES {
            c.partial_examples.push(Miss {
                spoken: span.spoken.clone(),
                got: got.trim().to_string(),
                want: span.written.clone(),
            });
        }
    }

    fn in_total(c: &ClassScore, scope: Total) -> bool {
        let out = OUT_OF_SCOPE.contains(&c.class.as_str());
        let plain = c.class == PLAIN;
        match scope {
            Total::InScope => !out && !plain,
            Total::All => !plain,
            Total::Balanced => !out,
        }
    }

    pub fn partials(&self, scope: Total) -> usize {
        self.classes
            .iter()
            .filter(|c| Self::in_total(c, scope))
            .map(|c| c.partial)
            .sum()
    }

    pub fn totals(&self, scope: Total) -> (usize, usize, usize, usize) {
        self.classes
            .iter()
            .filter(|c| Self::in_total(c, scope))
            .fold((0, 0, 0, 0), |(t, e, r, d), c| {
                (t + c.total, e + c.exact, r + c.relaxed, d + c.digits)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(class: &str, spoken: &str, written: &str) -> Span {
        Span {
            class: class.into(),
            spoken: spoken.into(),
            written: written.into(),
        }
    }

    fn args(a: &[&str]) -> Result<Mode, String> {
        parse_args(a.iter().map(|s| s.to_string()))
    }

    #[test]
    fn args_select_a_pairs_file_and_reject_a_bare_path() {
        assert!(args(&["data"]).is_err());
        assert_eq!(
            args(&["--pairs", "val.txt"]),
            Ok(Mode::Pairs {
                path: "val.txt".into(),
                isolated: false
            })
        );
        assert_eq!(
            args(&["--pairs", "val.txt", "--isolated"]),
            Ok(Mode::Pairs {
                path: "val.txt".into(),
                isolated: true
            })
        );
        assert!(args(&[]).is_err());
        assert!(args(&["--pairs"]).is_err());
        assert!(args(&["a", "b"]).is_err());
        assert!(args(&["--pairs", "x", "y"]).is_err());
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
    fn args_select_a_lines_file() {
        let opts = |tag: Option<&str>, all| {
            Ok(Mode::Lines(LinesOpts {
                path: "d.tsv".into(),
                tag: tag.map(Into::into),
                all_failures: all,
                single_digits: false,
            }))
        };
        assert_eq!(
            args(&["--lines", "d.tsv", "--single-digits"]),
            Ok(Mode::Lines(LinesOpts {
                path: "d.tsv".into(),
                tag: None,
                all_failures: false,
                single_digits: true,
            }))
        );
        assert!(args(&["--lines", "d.tsv", "--single-digits", "--single-digits"]).is_err());
        assert_eq!(args(&["--lines", "d.tsv"]), opts(None, false));
        assert_eq!(
            args(&["--lines", "d.tsv", "--tag", "time"]),
            opts(Some("time"), false)
        );
        assert_eq!(
            args(&["--lines", "d.tsv", "--all-failures"]),
            opts(None, true)
        );
        assert_eq!(
            args(&["--lines", "d.tsv", "--all-failures", "--tag", "unit!"]),
            opts(Some("unit!"), true)
        );
        assert!(args(&["--lines"]).is_err());
        assert!(args(&["--lines", "--tag", "x"]).is_err());
        assert!(args(&["--lines", "d.tsv", "--tag"]).is_err());
        assert!(args(&["--lines", "d.tsv", "--bogus"]).is_err());
        assert!(args(&["--lines", "d.tsv", "--tag", "a", "--tag", "b"]).is_err());
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
}
