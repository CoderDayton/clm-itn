use crate::replay::*;
use clm_itn::Setting;

fn parse(v: &[&str]) -> Result<Mode, String> {
    parse_args(v.iter().map(|s| s.to_string()))
}

fn corpus(path: &str) -> CorpusOpts {
    CorpusOpts {
        path: path.into(),
        passes: crate::lines::DEFAULT_PASSES,
        tag: None,
        all_failures: false,
        off: Vec::new(),
    }
}

#[test]
fn defaults_to_the_gate() {
    let want = Mode::Gate {
        n: DEFAULT_N,
        seed: DEFAULT_SEED,
        tiers: false,
    };
    assert_eq!(parse(&[]), Ok(want));
}

#[test]
fn parses_n_seed_and_tiers_in_any_order() {
    let want = Ok(Mode::Gate {
        n: 500,
        seed: 9,
        tiers: true,
    });
    assert_eq!(parse(&["--n", "500", "--seed", "9", "--tiers"]), want);
    assert_eq!(parse(&["--tiers", "--seed", "9", "--n", "500"]), want);
    assert!(parse(&["--tiers=1"]).is_err());
}

#[test]
fn ignores_the_bench_flag_cargo_bench_appends() {
    assert_eq!(parse(&["--bench"]), parse(&[]));
    assert_eq!(
        parse(&["--corpus", "d.tsv", "--bench"]),
        Ok(Mode::Corpus(corpus("d.tsv")))
    );
}

#[test]
fn parses_corpus_options() {
    assert_eq!(
        parse(&["--corpus", "corpora/"]),
        Ok(Mode::Corpus(corpus("corpora/")))
    );
    let want = CorpusOpts {
        passes: 3,
        tag: Some("time!".into()),
        all_failures: true,
        off: vec![Setting::SingleDigits, Setting::Percent],
        ..corpus("d.tsv")
    };
    let args = [
        "--no-single-digits",
        "--passes",
        "3",
        "--corpus",
        "d.tsv",
        "--all-failures",
        "--tag",
        "time!",
        "--no-percent",
    ];
    assert_eq!(parse(&args), Ok(Mode::Corpus(want)));
}

#[test]
fn parses_pairs_and_spell() {
    assert_eq!(
        parse(&["--pairs", "val.txt"]),
        Ok(Mode::Pairs {
            path: "val.txt".into(),
            isolated: false
        })
    );
    assert_eq!(
        parse(&["--isolated", "--pairs", "val.txt"]),
        Ok(Mode::Pairs {
            path: "val.txt".into(),
            isolated: true
        })
    );
    assert_eq!(
        parse(&["--spell", "p.tsv"]),
        Ok(Mode::Spell("p.tsv".into()))
    );
}

#[test]
fn rejects_mixed_modes_and_stray_mode_flags() {
    assert!(parse(&["--corpus", "d.tsv", "--tiers"]).is_err());
    assert!(parse(&["--corpus", "d.tsv", "--n", "5"]).is_err());
    assert!(parse(&["--seed", "2", "--corpus", "d.tsv"]).is_err());
    assert!(parse(&["--corpus", "d.tsv", "--pairs", "v.txt"]).is_err());
    assert!(parse(&["--spell", "p.tsv", "--pairs", "v.txt"]).is_err());
    assert!(parse(&["--passes", "3"]).is_err());
    assert!(parse(&["--tag", "time"]).is_err());
    assert!(parse(&["--all-failures"]).is_err());
    assert!(parse(&["--no-percent"]).is_err());
    assert!(parse(&["--isolated"]).is_err());
    assert!(parse(&["--pairs", "v.txt", "--tag", "x"]).is_err());
}

#[test]
fn rejects_bad_values_and_repeats() {
    assert!(parse(&["--n"]).is_err());
    assert!(parse(&["--n", "x"]).is_err());
    assert!(parse(&["--n", "0"]).is_err());
    assert!(parse(&["--n", "5", "--n", "6"]).is_err());
    assert!(parse(&["--seed", "-1"]).is_err());
    assert!(parse(&["--corpus"]).is_err());
    assert!(parse(&["--corpus", "--tag", "x"]).is_err());
    assert!(parse(&["--corpus", "d.tsv", "--passes", "0"]).is_err());
    assert!(parse(&["--corpus", "d.tsv", "--tag"]).is_err());
    assert!(parse(&["--corpus", "d.tsv", "--tag", "a", "--tag", "b"]).is_err());
    assert!(parse(&["--corpus", "d.tsv", "--no-dates", "--no-dates"]).is_err());
    assert!(parse(&["--corpus", "d.tsv", "--no-such-thing"]).is_err());
    assert!(parse(&["--lines", "d.tsv"]).is_err());
    assert!(parse(&["--bogus"]).is_err());
}

#[test]
fn net_ticks_subtracts_overhead_and_saturates() {
    assert_eq!(net_ticks(100, 150, 30), 20);
    assert_eq!(net_ticks(100, 120, 30), 0);
    assert_eq!(net_ticks(0, u64::MAX, 0), u32::MAX);
}

#[test]
fn percentile_is_nearest_rank() {
    let v: Vec<u32> = (1..=1000).collect();
    assert_eq!(percentile(&v, 0.50), 500);
    assert_eq!(percentile(&v, 0.99), 990);
    assert_eq!(percentile(&v, 0.999), 999);
    assert_eq!(percentile(&v, 1.0), 1000);
    assert_eq!(percentile(&[7], 0.99), 7);
}
