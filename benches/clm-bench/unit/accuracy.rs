use crate::accuracy::*;
use crate::lines::{CorpusFile, tsv_cases};
use crate::replay::{Mode, parse_args};
use crate::score::value_runs;

#[test]
fn value_runs_ignore_style_but_not_numbers() {
    assert_eq!(value_runs("€2,20"), value_runs("2,20 €"));
    assert_eq!(value_runs("1.33"), value_runs("1,33"));
    assert_eq!(value_runs("200 m"), value_runs("200 meter"));
    assert_eq!(value_runs("1,000,001"), ["1000001"]);
    assert_eq!(value_runs("11:45 Uhr"), ["11", "45"]);
    assert_ne!(value_runs("12.7"), value_runs("127 7"));
    assert_ne!(value_runs("2 y 63"), value_runs("263"));
}

#[test]
fn specs_name_a_language_digits_and_a_path() {
    let s = parse_spec("en+digits:tests/data").unwrap();
    assert_eq!(
        (s.lang.as_str(), s.digits, s.path.as_str()),
        ("en", true, "tests/data")
    );
    let s = parse_spec("pt:/tmp/nemo_pt").unwrap();
    assert_eq!((s.lang.as_str(), s.digits), ("pt", false));
    assert!(parse_spec("xx:path").is_err());
    assert!(parse_spec("es").is_err());
    assert!(parse_spec("es:").is_err());
}

#[test]
fn accuracy_is_its_own_mode() {
    let args = |a: &[&str]| parse_args(a.iter().map(|s| s.to_string()));
    match args(&["--accuracy", "en:a", "--accuracy", "es:b"]) {
        Ok(Mode::Accuracy(specs)) => assert_eq!(specs.len(), 2),
        other => panic!("{other:?}"),
    }
    assert!(args(&["--accuracy", "en:a", "--corpus", "b"]).is_err());
}

#[test]
fn tilde_lines_read_as_spoken_and_written() {
    let (cases, bad) = tsv_cases("dos dólares~$2\nuno\tuno\nsin~\n");
    assert_eq!(bad, 1);
    assert_eq!(cases[0].spoken, "dos dólares");
    assert_eq!(cases[0].written.as_deref(), Some("$2"));
    assert_eq!(cases[1].written.as_deref(), Some("uno"));
}

#[test]
fn tally_counts_exact_and_value_matches() {
    let (cases, _) = tsv_cases("veinte~20\ndoce euros~12 €\ndos~2\n");
    let file = CorpusFile {
        name: "t".into(),
        cases,
        malformed: 0,
    };
    let mut failures = Vec::new();
    let t = tally(&clm_itn::Normalizer::spanish(), &file, &mut failures);
    assert_eq!((t.lines, t.exact, t.values), (3, 1, 2));
    assert_eq!(failures.len(), 1);
}
