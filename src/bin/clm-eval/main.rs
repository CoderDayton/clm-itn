mod eval;
mod lines;
mod pairs;
mod spell;

use std::process::ExitCode;

use clm_itn::engine::window::format_line;
use clm_itn::types::ROLE_NONE;
use clm_itn::{Engine, Style};

use eval::{WordRole, accuracy, is_partial};

fn word_role(t: &clm_itn::Tables, w: &str) -> WordRole {
    let c = t.lexicon.lookup(w.as_bytes()).class as usize;
    let start = (t.starts[c / 64] >> (c % 64)) & 1 == 1;
    let role = t.role_of[c];
    let affix = role < 16 && ((t.suffix_roles | t.prefix_roles) >> role) & 1 == 1;
    match (role != ROLE_NONE && !start, affix) {
        (false, _) => WordRole::Plain,
        (true, true) => WordRole::Affix,
        (true, false) => WordRole::Number,
    }
}

fn number_word(t: &clm_itn::Tables, w: &str) -> bool {
    word_role(t, w) == WordRole::Number
}

const USAGE: &str = "usage: clm-eval --pairs <pairgen split file> | --lines <spoken TAB written [TAB tag] file> [--tag <t>] [--all-failures] [--single-digits] | --spell <id TAB raw TAB final file>";

fn main() -> ExitCode {
    match pairs::parse_args(std::env::args().skip(1)) {
        Ok(pairs::Mode::Pairs { path, isolated }) => run_pairs(&path, isolated),
        Ok(pairs::Mode::Lines(opts)) => run_lines(&opts),
        Ok(pairs::Mode::Spell(path)) => spell::run(&path),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

fn run_lines(opts: &pairs::LinesOpts) -> ExitCode {
    let path = opts.path.as_str();
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (cases, malformed) = lines::parse_lines(&text);
    let style = if opts.single_digits {
        Style::SingleDigits
    } else {
        Style::Ap
    };
    let engine = Engine::english().with_style(style);
    let mut tally = lines::LineTally {
        all_failures: opts.all_failures,
        ..lines::LineTally::default()
    };
    let mut got = String::new();
    let wanted = |c: &&lines::LineCase| {
        opts.tag
            .as_deref()
            .is_none_or(|t| lines::tag_matches(&c.tag, t))
    };
    for c in cases.iter().filter(wanted) {
        got.clear();
        format_line(&engine, &c.spoken, &mut got);
        tally.record(c, &got);
    }
    let t = &tally;
    println!("lines file: {path}");
    if let Some(t) = &opts.tag {
        println!("tag filter: {t}");
    }
    println!(
        "{:<22} {:>5} {:>8} {:>8} {:>16}",
        "tag", "lines", "exact", "digits", "over-format"
    );
    let row = |label: &str, s: &lines::Score| {
        println!(
            "{label:<22} {:>5} {:>7.1}% {:>7.1}% {:>5.1}% ({}/{})",
            s.total,
            accuracy(s.exact, s.total),
            accuracy(s.digits, s.total),
            accuracy(s.overformat, s.no_digit_gold),
            s.overformat,
            s.no_digit_gold
        );
    };
    let mut tags: Vec<&(String, lines::Score)> = t.tags.iter().collect();
    tags.sort_by(|a, b| a.0.cmp(&b.0));
    for (tag, s) in tags {
        row(tag, s);
    }
    println!("{}", "-".repeat(66));
    row("all lines", &t.all);
    row("firm (no \"!\" tags)", &t.firm);
    println!("over-format: output has digits although the gold has none (rate over those lines)");
    println!("{DIGITS_LEGEND}");
    println!("malformed lines skipped: {malformed}");
    if !t.failures.is_empty() {
        let scope = if opts.all_failures { "all" } else { "first" };
        println!("\nfailures ({scope} {}):", t.failures.len());
        for f in &t.failures {
            println!(
                "  {:?}\n    got  {:?}\n    want {:?}",
                f.input, f.got, f.want
            );
        }
    }
    ExitCode::SUCCESS
}

fn run_pairs(path: &str, isolated: bool) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let tables = clm_itn::english();
    let (records, malformed) = pairs::parse_records(&text, |w| number_word(tables, w));
    let engine = Engine::new(tables);
    let mut tally = pairs::PairTally::default();
    let mut sentence = String::new();
    let mut got = String::new();
    for r in &records {
        sentence.clear();
        if !isolated {
            format_line(&engine, &r.line, &mut sentence);
        }
        let raw: Vec<&str> = r.line.split(' ').collect();
        let out: Vec<&str> = sentence.split_whitespace().collect();
        for (range, s) in &r.items {
            got.clear();
            if isolated {
                format_line(&engine, &s.spoken, &mut got);
            } else {
                got.push_str(&pairs::span_output(&raw, &out, range.clone()));
            }
            tally.record(s, &got);
            if is_partial(&got, &s.written, |w| word_role(tables, w)) {
                tally.record_partial(s, &got);
            }
        }
    }
    let mode = if isolated {
        "isolated (each span formatted alone)"
    } else {
        "in context (whole sentence formatted, spans aligned)"
    };
    println!("scoring: {mode}");
    report_pairs(path, &tally, malformed);
    ExitCode::SUCCESS
}

const DIGITS_LEGEND: &str = "digits: same sequence of digit runs (thousands commas removed); letters, case and punctuation ignored; time classes: same parsed hour, minute and am/pm (24-hour aware)";
const PARTIAL_LEGEND: &str = "partial: output has digits and keeps an input number/unit word the gold lacks; lower is better";

fn report_pairs(path: &str, tally: &pairs::PairTally, malformed: usize) {
    let mut classes: Vec<&pairs::ClassScore> = tally.classes.iter().collect();
    classes.sort_by(|a, b| b.total.cmp(&a.total).then(a.class.cmp(&b.class)));
    println!("pairs file: {path}");
    println!(
        "{:<12} {:>6} {:>8} {:>8} {:>8} {:>12}",
        "class", "spans", "exact", "relaxed", "digits", "partial"
    );
    for c in &classes {
        let scope = if pairs::OUT_OF_SCOPE.contains(&c.class.as_str()) {
            "  (out of scope)"
        } else {
            ""
        };
        println!(
            "{:<12} {:>6} {:>7.1}% {:>7.1}% {:>7.1}% {:>5} {:>5.1}%{scope}",
            c.class,
            c.total,
            accuracy(c.exact, c.total),
            accuracy(c.relaxed, c.total),
            accuracy(c.digits, c.total),
            c.partial,
            accuracy(c.partial, c.total)
        );
    }
    println!("{}", "-".repeat(60));
    for (label, scope) in [
        ("in scope", pairs::Total::InScope),
        ("all classes", pairs::Total::All),
        ("balanced", pairs::Total::Balanced),
    ] {
        let (t, e, r, d) = tally.totals(scope);
        let p = tally.partials(scope);
        println!(
            "{label:<12} {t:>6} {:>7.1}% {:>7.1}% {:>7.1}% {p:>5} {:>5.1}%",
            accuracy(e, t),
            accuracy(r, t),
            accuracy(d, t),
            accuracy(p, t)
        );
    }
    println!("out of scope: {}", pairs::OUT_OF_SCOPE.join(", "));
    println!(
        "{}: non-span runs with a number word, correct when kept as words; only in balanced",
        pairs::PLAIN
    );
    println!("balanced = in scope + {}", pairs::PLAIN);
    println!("relaxed ignores case, whitespace and commas");
    println!("{DIGITS_LEGEND}");
    println!("{PARTIAL_LEGEND}");
    println!("malformed lines skipped: {malformed}");
    for c in &classes {
        if c.partial_examples.is_empty() {
            continue;
        }
        println!(
            "\n{} partial (first {}):",
            c.class,
            c.partial_examples.len()
        );
        for m in &c.partial_examples {
            println!(
                "  {:?}\n    got  {:?}\n    want {:?}",
                m.spoken, m.got, m.want
            );
        }
    }
    for c in &classes {
        if c.misses.is_empty() {
            continue;
        }
        println!("\n{} misses (first {}):", c.class, c.misses.len());
        for m in &c.misses {
            println!(
                "  {:?}\n    got  {:?}\n    want {:?}",
                m.spoken, m.got, m.want
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WordRole, number_word, word_role};

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
        let t = clm_itn::english();
        for w in ["percent", "dollars", "minus"] {
            assert_eq!(word_role(t, w), WordRole::Affix, "{w}");
            assert!(!number_word(t, w), "{w}");
        }
        assert_eq!(word_role(t, "five"), WordRole::Number);
        assert_eq!(word_role(t, "january"), WordRole::Plain);
    }
}
